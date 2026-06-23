use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use tracing::{info, warn};

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .connect_timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
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
fn format_command_outputs(command_outputs: &HashMap<String, String>) -> String {
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
        parts.push(format!("【命令】{}\n【输出】\n{}", cmd, truncated));
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
) -> Result<serde_json::Value, String> {
    let base_url = if base_url.is_empty() {
        "https://api.openai.com"
    } else {
        base_url.trim_end_matches('/')
    };

    let url = format!("{}/v1/chat/completions", base_url);
    let formatted_input = format_command_outputs(command_outputs);

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

    // ── 响应日志（debug 级别记录完整响应）──
    tracing::debug!(
        "AI 响应详情: model={}, status={}, latency={}ms, response_len={}\n--- RESPONSE (前 5000 字) ---\n{}",
        model, status, latency, response_text.len(),
        response_text.chars().take(5000).collect::<String>()
    );

    if !status.is_success() {
        warn!(
            "AI 请求失败: model={}, status={}, latency={}ms, body_len={}",
            model, status, latency, response_text.len()
        );
        return Err(format!("OpenAI API 错误 ({}): {}", status, response_text));
    }

    let parsed: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| {
            warn!(
                "AI 响应 JSON 解析失败: model={}, latency={}ms, error={}, 前 300 字: {}",
                model, latency, e,
                response_text.chars().take(300).collect::<String>()
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

    let content = parsed["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| {
            warn!("AI 响应格式异常: 缺少 choices[0].message.content, response_len={}", response_text.len());
            "OpenAI 响应格式异常: 未找到分析结果".to_string()
        })?;

    let finish_reason = parsed["choices"][0]["finish_reason"].as_str().unwrap_or("");

    // The content itself should be JSON
    let analysis: serde_json::Value = serde_json::from_str(content).map_err(|e| {
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
