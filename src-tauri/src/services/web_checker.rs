use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct WebCheckResult {
    pub url: String,
    pub final_url: String,
    pub status_code: Option<u16>,
    pub response_time_ms: u64,
    pub error: Option<String>,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
}

fn is_ip_like(s: &str) -> bool {
    // Strip port if present
    let host = s.split(':').next().unwrap_or(s);
    // IPv4: four dot-separated numbers
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok()) {
        return true;
    }
    // IPv6: starts with [
    if host.starts_with('[') && host.contains(']') {
        return true;
    }
    false
}

fn normalize_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if is_ip_like(trimmed) {
        // IP addresses default to HTTP (SSL certs are domain-bound)
        format!("http://{}", trimmed)
    } else {
        format!("https://{}", trimmed)
    }
}

async fn check_one(client: &reqwest::Client, raw_url: &str) -> WebCheckResult {
    let url = normalize_url(raw_url);
    let start = std::time::Instant::now();

    match client.get(&url).send().await {
        Ok(resp) => {
            let elapsed = start.elapsed().as_millis() as u64;
            let final_url = resp.url().to_string();
            let status = resp.status().as_u16();
            let content_type = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let content_length = resp
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());
            WebCheckResult {
                url: raw_url.to_string(),
                final_url,
                status_code: Some(status),
                response_time_ms: elapsed,
                error: None,
                content_type,
                content_length,
            }
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            let error_msg = if e.is_timeout() {
                "请求超时".to_string()
            } else if e.is_connect() {
                "连接失败".to_string()
            } else if e.is_redirect() {
                "重定向过多".to_string()
            } else {
                format!("{}", e)
            };
            WebCheckResult {
                url: raw_url.to_string(),
                final_url: url,
                status_code: None,
                response_time_ms: elapsed,
                error: Some(error_msg),
                content_type: None,
                content_length: None,
            }
        }
    }
}

pub async fn check_urls(raw_urls: &[String], timeout_secs: u64) -> Vec<WebCheckResult> {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .danger_accept_invalid_certs(false)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            return raw_urls
                .iter()
                .map(|u| WebCheckResult {
                    url: u.clone(),
                    final_url: u.clone(),
                    status_code: None,
                    response_time_ms: 0,
                    error: Some("无法创建 HTTP 客户端".into()),
                    content_type: None,
                    content_length: None,
                })
                .collect();
        }
    };

    let mut tasks = Vec::with_capacity(raw_urls.len());
    for raw_url in raw_urls {
        let c = client.clone();
        let u = raw_url.clone();
        tasks.push(tokio::spawn(async move { check_one(&c, &u).await }));
    }

    let mut results = Vec::with_capacity(tasks.len());
    for (i, t) in tasks.into_iter().enumerate() {
        results.push(match t.await {
            Ok(r) => r,
            Err(e) => {
                let url = raw_urls.get(i).cloned().unwrap_or_default();
                tracing::warn!("URL 检测任务 panic (url={}): {}", url, e);
                WebCheckResult {
                    url,
                    error: Some(format!("检测任务异常: {}", e)),
                    ..Default::default()
                }
            }
        });
    }
    results
}
