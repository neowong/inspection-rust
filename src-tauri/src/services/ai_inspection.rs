use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use tracing::{info, warn};

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// 对响应文本中的 API key 形态打码（sk-... / Bearer ...），
/// 防止错误体或 debug 日志泄露密钥。仅做模式替换，不影响正常错误信息。
fn redact_secrets(s: &str) -> String {
    let mut out = s.to_string();
    // sk- 开头的 token（OpenAI/DeepSeek 等）
    let mut redacted = String::new();
    let bytes: Vec<char> = out.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        // 匹配 "sk-" 后接非空白字符序列
        if i + 3 <= bytes.len() && bytes[i] == 's' && bytes[i + 1] == 'k' && bytes[i + 2] == '-' {
            redacted.push_str("sk-***");
            i += 3;
            while i < bytes.len() && !bytes[i].is_whitespace() && !matches!(bytes[i], '"' | '\'' | ',' | '}' | ']') {
                i += 1;
            }
            continue;
        }
        redacted.push(bytes[i]);
        i += 1;
    }
    out = redacted;
    // Bearer <token>
    if out.find("Bearer ").is_some() {
        out = out.replace("Bearer ", "Bearer ***");
    }
    out
}

pub fn get_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        // AI API 无需重定向；禁用后避免 307/308 把 Authorization 头带往非预期端点。
        reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .connect_timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("reqwest client 构建失败：请检查系统 TLS 配置（可能需要安装 ca-certificates 或 pkg-config openssl）")
    })
}

/// 统一构建 AI API 的 chat/completions 端点 URL。
/// base_url 即为完整的 endpoint 前缀（不含 /chat/completions），用户按官方文档填：
/// - OpenAI:  https://api.openai.com/v1
/// - DeepSeek: https://api.deepseek.com          （无 /v1）
/// - Qwen:    https://dashscope.aliyuncs.com/compatible-mode/v1
/// - 空值默认 OpenAI
///
/// 兼容旧配置：DeepSeek 的 base_url 若存了 /v1 后缀会自动去除（旧代码曾强制加 /v1）。
pub fn build_chat_url(base_url: &str) -> String {
    let base = if base_url.is_empty() {
        "https://api.openai.com/v1".to_string()
    } else {
        let trimmed = base_url.trim_end_matches('/').to_string();
        // DeepSeek API 不含 /v1，旧配置可能误带 /v1 后缀，自动去除
        if trimmed.contains("deepseek.com") {
            trimmed.strip_suffix("/v1").unwrap_or(&trimmed).to_string()
        } else {
            trimmed
        }
    };
    format!("{}/chat/completions", base)
}

pub const SYSTEM_PROMPT: &str = r#"你是一位专业的 IT 运维巡检工程师，负责分析设备巡检命令输出，判断设备运行状态是否正常。

对于每台设备，你会收到一组命令及其输出。你的任务是：
1. 对收到的**每一条命令**逐条进行评判，不允许跳过任何命令
2. 每条命令给出状态判定（ok=正常 / info=注意 / warning=警告 / critical=严重）
3. 给出简要的整体总结

重要：items 中的 command 字段必须与输入中的【命令】完全一致，包括空格和大小写，不得修改、省略或重新格式化。

请用 JSON 格式返回分析结果：
{
  "summary": "整体状态总结，一句话",
  "overall": "ok/info/warning/critical",
  "items": [
    {
      "command": "命令名称（必须与输入的【命令】完全一致）",
      "title": "巡检项目简短概括，≤6个字",
      "status": "ok/info/warning/critical",
      "finding": "判断结论（不超过12字，正常写'正常'）",
      "suggestion": "建议或改进措施（正常时为空，异常时不超过15字）"
    }
  ]
}

当分析 Linux 服务器巡检数据时，参考以下阈值：
- CPU 使用率 > 80% → warning, > 95% → critical
- 内存使用率 > 85% → warning, > 95% → critical
- 磁盘使用率 > 80% → warning, > 90% → critical
- load average > CPU 核心数 → warning
- failed services > 0 → warning
- 关键端口未监听（如 22/80/443） → info
- /var/log 中有 error 级别日志 → warning"#;

/// Format command outputs into a readable text block for the LLM.
/// expectations: optional map of command → expected result description
fn format_command_outputs(
    command_outputs: &HashMap<String, String>,
    expectations: &HashMap<String, String>,
) -> String {
    let mut parts = Vec::new();
    // Sort keys for deterministic ordering
    let mut keys: Vec<&String> = command_outputs.keys().collect();
    keys.sort();
    for cmd in keys {
        let output = &command_outputs[cmd];
        // Truncate output to 2000 chars to avoid overly large prompts
        let truncated = if output.len() > 2000 {
            let end = output.char_indices().nth(2000).map(|(i, _)| i).unwrap_or(output.len());
            format!("{}...\n[输出已截断，共 {} 字节]", &output[..end], output.len())
        } else {
            output.clone()
        };
        let expectation_line = expectations
            .get(cmd.as_str())
            .filter(|e| !e.is_empty())
            .map(|e| format!("\n【期望】{}", e))
            .unwrap_or_default();
        parts.push(format!("【命令】{}{}\n【输出】\n{}", cmd, expectation_line, truncated));
    }
    parts.join("\n\n---\n\n")
}

/// Analyze command outputs using the OpenAI chat completions API.
///
/// * `api_key` - OpenAI API key
/// * `model` - Model name (e.g., "gpt-4o", "gpt-4o-mini")
/// * `base_url` - API base URL; defaults to "https://api.openai.com" if empty
/// * `command_outputs` - Map of command name to its text output
pub async fn analyze_with_openai(
    api_key: &str,
    model: &str,
    base_url: &str,
    command_outputs: &HashMap<String, String>,
    expectations: &HashMap<String, String>,
) -> Result<serde_json::Value, String> {
    let url = build_chat_url(base_url);
    let formatted_input = format_command_outputs(command_outputs, expectations);

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": &formatted_input}
        ],
        "temperature": 0.3,
        // 47 条命令的 JSON 输出约 8-12k tokens；4096 不够会被截断成无效 JSON
        "max_tokens": 16384
    });

    let cmd_count = command_outputs.len();

    // ── 请求前日志 ──
    info!(
        "AI 请求开始: model={}, base_url={}, commands={}",
        model, base_url, cmd_count
    );
    // debug 级别仅记录长度，不记录原始命令输出（可能含密码等敏感信息）
    tracing::debug!(
        "AI 请求详情: url={}, system_prompt_len={}, user_prompt_len={}",
        url,
        SYSTEM_PROMPT.len(),
        formatted_input.len()
    );

    let client = get_client();
    let start = std::time::Instant::now();

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            let latency = start.elapsed().as_millis();
            warn!("AI 请求失败: model={}, latency={}ms, error={}", model, latency, e);
            format!("OpenAI API 请求失败: {}", e)
        })?;

    let latency = start.elapsed().as_millis();
    let status = response.status();
    let response_text = response
        .text()
        .await
        .map_err(|e| {
            warn!("AI 响应读取失败: model={}, latency={}ms, error={}", model, latency, e);
            format!("读取 OpenAI 响应失败: {}", e)
        })?;

    // ── 响应日志（debug 级别记录完整响应，对密钥打码）──
    let safe_response = redact_secrets(&response_text);
    tracing::debug!(
        "AI 响应详情: model={}, status={}, latency={}ms, response_len={}\n--- RESPONSE (前 5000 字) ---\n{}",
        model, status, latency, response_text.len(),
        safe_response.chars().take(5000).collect::<String>()
    );

    if !status.is_success() {
        warn!(
            "AI 请求失败: model={}, status={}, latency={}ms, body_len={}",
            model, status, latency, response_text.len()
        );
        // 错误体可能被代理回显请求头（含 API key），截断并对 sk-/Bearer 打码后回传
        return Err(format!("OpenAI API 错误 ({}): {}", status, redact_secrets(&response_text)));
    }

    let parsed: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| {
            let preview = redact_secrets(&response_text).chars().take(300).collect::<String>();
            warn!(
                "AI 响应 JSON 解析失败: model={}, latency={}ms, error={}, 前 300 字: {}",
                model, latency, e, preview
            );
            format!("解析 OpenAI 响应 JSON 失败: {}", e)
        })?;

    // ── Token 用量日志 ──
    let usage = parsed.get("usage");
    let prompt_tokens = usage.and_then(|u| u.get("prompt_tokens")).and_then(|v| v.as_i64()).unwrap_or(0);
    let completion_tokens = usage.and_then(|u| u.get("completion_tokens")).and_then(|v| v.as_i64()).unwrap_or(0);
    let total_tokens = usage.and_then(|u| u.get("total_tokens")).and_then(|v| v.as_i64()).unwrap_or(0);

    info!(
        "AI 请求完成: model={}, latency={}ms, prompt_tokens={}, completion_tokens={}, total_tokens={}, commands={}",
        model, latency, prompt_tokens, completion_tokens, total_tokens, cmd_count
    );

    // 兼容 DeepSeek 等厂商：content 可能为 null 或空字符串，
    // 实际内容在 reasoning_content（thinking 模型）等字段
    let msg = &parsed["choices"][0]["message"];
    let raw_content = msg["content"].as_str().unwrap_or("").trim();
    let content = if raw_content.is_empty() {
        msg["reasoning_content"].as_str().unwrap_or("").trim()
    } else {
        raw_content
    };
    if content.is_empty() {
        warn!("AI 响应内容为空: model={}, message={}", model, msg);
        return Err("AI 响应内容为空，请检查模型名称是否正确".to_string());
    }

    let finish_reason = parsed["choices"][0]["finish_reason"].as_str().unwrap_or("");

    // 去除可能的 markdown 代码块包裹（```json ... ```）
    let cleaned = content
        .trim()
        .strip_prefix("```json")
        .or_else(|| content.trim().strip_prefix("```"))
        .map(|s| s.strip_suffix("```").unwrap_or(s))
        .unwrap_or(content)
        .trim();

    // The content itself should be JSON
    let analysis: serde_json::Value = serde_json::from_str(cleaned).map_err(|e| {
        if finish_reason == "length" {
            warn!(
                "AI 输出被 max_tokens 截断: model={}, finish_reason=length, content_len={}",
                model, content.len()
            );
            "AI 输出被截断（命令数过多导致 max_tokens 不足）。请减少巡检命令数或在系统设置中切换到上下文更长的模型。".to_string()
        } else {
            warn!(
                "AI 分析结果 JSON 解析失败: model={}, error={}, 前 500 字: {}",
                model, e, content.chars().take(500).collect::<String>()
            );
            format!("解析 AI 分析结果 JSON 失败: {} — 原始内容前 500 字: {}",
                e, content.chars().take(500).collect::<String>())
        }
    })?;

    info!(
        "AI 分析完成: model={}, latency={}ms, total_tokens={}, items={}",
        model, latency, total_tokens,
        analysis.get("items").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0)
    );

    // ── 分析结果摘要日志 ──
    tracing::debug!(
        "AI 分析结果: model={}, summary={:?}, overall={:?}",
        model,
        analysis.get("summary").and_then(|v| v.as_str()),
        analysis.get("overall").and_then(|v| v.as_str()),
    );

    Ok(analysis)
}

// Anthropic provider removed — 国内环境不适用，所有厂商均走 OpenAI 兼容 API。
