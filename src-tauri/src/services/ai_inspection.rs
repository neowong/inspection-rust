use std::collections::HashMap;
use std::sync::OnceLock;

use tracing::{info, warn};

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(reqwest::Client::new)
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
            {"role": "user", "content": formatted_input}
        ],
        "temperature": 0.3,
        "max_tokens": 4096
    });

    info!(
        "Sending request to OpenAI API (model: {}, commands: {})",
        model,
        command_outputs.len()
    );

    let client = get_client();
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("OpenAI API 请求失败: {}", e))?;

    let status = response.status();
    let response_text = response
        .text()
        .await
        .map_err(|e| format!("读取 OpenAI 响应失败: {}", e))?;

    if !status.is_success() {
        warn!("OpenAI API returned error status {}: {}", status, response_text);
        return Err(format!("OpenAI API 错误 ({}): {}", status, response_text));
    }

    let parsed: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("解析 OpenAI 响应 JSON 失败: {}", e))?;

    let content = parsed["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| {
            warn!("OpenAI 响应缺少 choices[0].message.content: {}", response_text);
            "OpenAI 响应格式异常: 未找到分析结果".to_string()
        })?;

    // The content itself should be JSON
    let analysis: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| format!("解析 AI 分析结果 JSON 失败: {} — 原始内容: {}", e, content))?;

    info!("Successfully analyzed {} commands with OpenAI", command_outputs.len());
    Ok(analysis)
}

/// Analyze command outputs using the Anthropic messages API.
///
/// * `api_key` - Anthropic API key
/// * `model` - Model name (e.g., "claude-sonnet-4-20250514", "claude-3-haiku-20240307")
/// * `base_url` - API base URL; defaults to "https://api.anthropic.com" if empty
/// * `command_outputs` - Map of command name to its text output
pub async fn analyze_with_anthropic(
    api_key: &str,
    model: &str,
    base_url: &str,
    command_outputs: &HashMap<String, String>,
) -> Result<serde_json::Value, String> {
    let base_url = if base_url.is_empty() {
        "https://api.anthropic.com"
    } else {
        base_url.trim_end_matches('/')
    };

    let url = format!("{}/v1/messages", base_url);
    let formatted_input = format_command_outputs(command_outputs);

    let body = serde_json::json!({
        "model": model,
        "system": SYSTEM_PROMPT,
        "messages": [
            {"role": "user", "content": formatted_input}
        ],
        "max_tokens": 4096
    });

    info!(
        "Sending request to Anthropic API (model: {}, commands: {})",
        model,
        command_outputs.len()
    );

    let client = get_client();
    let response = client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2025-01-25")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Anthropic API 请求失败: {}", e))?;

    let status = response.status();
    let response_text = response
        .text()
        .await
        .map_err(|e| format!("读取 Anthropic 响应失败: {}", e))?;

    if !status.is_success() {
        warn!("Anthropic API returned error status {}: {}", status, response_text);
        return Err(format!("Anthropic API 错误 ({}): {}", status, response_text));
    }

    let parsed: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("解析 Anthropic 响应 JSON 失败: {}", e))?;

    // Anthropic response structure: content[0].text
    let content = parsed["content"][0]["text"]
        .as_str()
        .ok_or_else(|| {
            warn!("Anthropic 响应缺少 content[0].text: {}", response_text);
            "Anthropic 响应格式异常: 未找到分析结果".to_string()
        })?;

    // The content itself should be JSON
    let analysis: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| format!("解析 AI 分析结果 JSON 失败: {} — 原始内容: {}", e, content))?;

    info!("Successfully analyzed {} commands with Anthropic", command_outputs.len());
    Ok(analysis)
}
