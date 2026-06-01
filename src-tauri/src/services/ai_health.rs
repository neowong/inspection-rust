use std::sync::OnceLock;
use tracing::{info, warn};

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to create reqwest client for AI health check")
    })
}

/// Result of an AI health check.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AiHealthResult {
    pub reachable: bool,
    pub status: String,        // "ok" | "degraded" | "error"
    pub error_type: Option<String>,  // "network" | "auth" | "rate_limit" | "billing" | "server_error" | "unknown"
    pub message: String,
    pub provider: String,
    pub model: String,
}

/// Classify a reqwest error into a user-readable message and error type.
fn classify_reqwest_error(e: &reqwest::Error) -> (String, String) {
    if e.is_timeout() {
        ("network".to_string(), "API 请求超时 (15s)，请检查网络连接或 API 地址".to_string())
    } else if e.is_connect() {
        ("network".to_string(), format!("无法连接到 API 服务器，请检查: 1) 网络是否可达 2) API 地址是否正确 3) 是否需要配置代理 ({})", e))
    } else if e.is_request() {
        // Could be DNS, TLS, or other request-level errors
        let msg = format!("API 请求失败: {}", e);
        if msg.contains("dns") || msg.contains("resolve") || msg.contains("Name or service not known") {
            ("network".to_string(), format!("API 域名解析失败，请检查 DNS 配置 ({})", e))
        } else if msg.contains("tls") || msg.contains("certificate") || msg.contains("SSL") {
            ("network".to_string(), format!("TLS/SSL 连接失败，请检查证书配置或使用 HTTP 地址 ({})", e))
        } else {
            ("network".to_string(), format!("API 网络请求失败: {}", e))
        }
    } else {
        ("unknown".to_string(), format!("API 请求异常: {}", e))
    }
}

/// Classify an HTTP error response into user-readable message.
/// Tries to parse the response body for API-specific error details.
fn classify_http_error(status: u16, body: &str, provider: &str) -> (String, String) {
    // Try to extract error message from response body
    let api_error = extract_api_error_message(body, provider);

    match status {
        401 => (
            "auth".to_string(),
            format!("API Key 无效或已过期。请检查 AI 配置中的 API Key 是否正确{}", api_error),
        ),
        403 => (
            "auth".to_string(),
            format!("API 访问被拒绝 (403)。可能原因: 1) API Key 无权限 2) 账户被禁用 3) 未授权的地区/IP{}", api_error),
        ),
        402 => (
            "billing".to_string(),
            format!("账户余额不足，请充值。API 返回 402 Payment Required{}", api_error),
        ),
        429 => (
            "rate_limit".to_string(),
            format!("API 请求频率超限 (Rate Limit)，请稍后重试。如果频繁出现，建议: 1) 降低并发数 2) 升级 API 套餐{}", api_error),
        ),
        500..=599 => (
            "server_error".to_string(),
            format!("API 服务器内部错误 (HTTP {})，请稍后重试或检查 API 服务状态{}", status, api_error),
        ),
        _ => (
            "unknown".to_string(),
            format!("API 返回错误状态码 {}: {} {}", status, body.chars().take(200).collect::<String>(), api_error),
        ),
    }
}

/// Try to extract a meaningful error message from API response body.
fn extract_api_error_message(body: &str, _provider: &str) -> String {
    if body.is_empty() {
        return String::new();
    }

    // Try parsing as JSON and extracting known error fields
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
        // OpenAI/DeepSeek format: { "error": { "message": "...", "code": "..." } }
        if let Some(msg) = val["error"]["message"].as_str() {
            return format!(" — 详情: {}", msg);
        }
        // Anthropic format: { "error": { "message": "..." } }
        if let Some(msg) = val["error"].as_str() {
            return format!(" — 详情: {}", msg);
        }
        // Simple format: { "message": "..." }
        if let Some(msg) = val["message"].as_str() {
            return format!(" — 详情: {}", msg);
        }
    }

    // Fallback: include the raw body (truncated)
    let truncated: String = body.chars().take(150).collect();
    if truncated.len() < body.len() {
        format!(" — 响应: {}...", truncated)
    } else {
        format!(" — 响应: {}", truncated)
    }
}

/// Check AI API health by making a minimal chat request.
pub async fn check_ai_health(
    provider: &str,
    api_key: &str,
    model: &str,
    base_url: &str,
) -> AiHealthResult {
    let (url, body) = match provider {
        "openai" | "deepseek" => {
            let base = if base_url.is_empty() {
                if provider == "deepseek" {
                    "https://api.deepseek.com"
                } else {
                    "https://api.openai.com"
                }
            } else {
                base_url.trim_end_matches('/')
            };
            let url = format!("{}/v1/chat/completions", base);
            let body = serde_json::json!({
                "model": model,
                "messages": [{"role": "user", "content": "hi"}],
                "max_tokens": 1,
                "temperature": 0.0,
            });
            (url, body)
        }
        "anthropic" => {
            let base = if base_url.is_empty() {
                "https://api.anthropic.com"
            } else {
                base_url.trim_end_matches('/')
            };
            let url = format!("{}/v1/messages", base);
            let body = serde_json::json!({
                "model": model,
                "messages": [{"role": "user", "content": "hi"}],
                "max_tokens": 1,
            });
            (url, body)
        }
        _ => {
            return AiHealthResult {
                reachable: false,
                status: "error".to_string(),
                error_type: Some("unknown".to_string()),
                message: format!("不支持的 AI 提供商: {}", provider),
                provider: provider.to_string(),
                model: model.to_string(),
            };
        }
    };

    info!("AI 健康检查: {} (provider: {}, model: {})", url, provider, model);

    let client = get_client();
    let request = client.post(&url).json(&body);

    let request = match provider {
        "openai" | "deepseek" => request.header("Authorization", format!("Bearer {}", api_key)),
        "anthropic" => request
            .header("x-api-key", api_key)
            .header("anthropic-version", "2025-01-25"),
        _ => unreachable!(),
    };

    match request.send().await {
        Ok(response) => {
            let status = response.status();
            // Read body regardless for error extraction
            let body = response.text().await.unwrap_or_default();

            if status.is_success() {
                info!("AI 健康检查通过 (provider: {})", provider);
                AiHealthResult {
                    reachable: true,
                    status: "ok".to_string(),
                    error_type: None,
                    message: format!("{} API 连接正常", provider),
                    provider: provider.to_string(),
                    model: model.to_string(),
                }
            } else {
                let (error_type, message) = classify_http_error(status.as_u16(), &body, provider);
                warn!("AI 健康检查失败 (provider: {}, status: {}): {}", provider, status, message);
                AiHealthResult {
                    reachable: false,
                    status: "error".to_string(),
                    error_type: Some(error_type),
                    message,
                    provider: provider.to_string(),
                    model: model.to_string(),
                }
            }
        }
        Err(e) => {
            let (error_type, message) = classify_reqwest_error(&e);
            warn!("AI 健康检查网络错误 (provider: {}): {}", provider, message);
            AiHealthResult {
                reachable: false,
                status: "error".to_string(),
                error_type: Some(error_type),
                message,
                provider: provider.to_string(),
                model: model.to_string(),
            }
        }
    }
}
