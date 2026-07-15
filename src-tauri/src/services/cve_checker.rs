use serde::{Deserialize, Serialize};
use std::time::Duration;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CveItem {
    pub cve_id: String,
    pub summary: String,
    pub cvss_score: f64,
    pub severity: String,
    pub fix_version: Option<String>,
    pub exploit_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VersionCheckResult {
    pub product: String,
    pub version: String,
    pub cves: Vec<CveItem>,
    pub max_cvss: f64,
    pub total_cves: usize,
}

/// 通过自建 CVE 服务查询（NVD 关键词搜索 + SQLite 缓存）
pub async fn query_cve_from_server(product: &str, server_url: &str) -> Result<Vec<CveItem>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .no_proxy()  // 绕过系统代理，直连 CVE 服务
        .build()
        .map_err(|e| format!("HTTP 客户端: {}", e))?;

    let url = format!("{}/api/v1/cve?product={}&version=*", server_url, product);
    tracing::info!("CVE 服务查询: {}", url);

    let resp = client.get(&url).send().await
        .map_err(|e| format!("CVE 服务请求失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("CVE 服务错误: {}", resp.status()));
    }

    let text = resp.text().await.map_err(|e| format!("读响应失败: {}", e))?;
    let api_resp: ServerResponse = serde_json::from_str(&text)
        .map_err(|e| format!("JSON 解析失败: {}", e))?;

    let cves: Vec<CveItem> = api_resp.cves.into_iter().map(|c| {
        let cvss = c.cvss;
        CveItem {
            cve_id: c.cve_id,
            summary: c.summary,
            cvss_score: cvss,
            severity: if cvss >= 9.0 { "critical" } else if cvss >= 7.0 { "high" } else if cvss >= 4.0 { "medium" } else { "low" }.to_string(),
            fix_version: None,
            exploit_available: false,
        }
    }).collect();

    Ok(cves)
}

#[derive(Debug, serde::Deserialize)]
struct ServerResponse {
    cves: Vec<ServerCve>,
}

#[derive(Debug, serde::Deserialize)]
struct ServerCve {
    cve_id: String,
    summary: String,
    cvss: f64,
}

/// 通过 TridentStack API 搜索某个产品的关联 CVE（文本搜索，不取详情，速度块）。
pub async fn query_cve_by_product(product: &str) -> Result<Vec<CveItem>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP 客户端: {}", e))?;

    let url = format!("https://tridentstack.com/api/v1/cve?q={}&limit=10", urlencoding(product));
    tracing::info!("CVE 搜索: {}", url);

    let resp = client
        .get(&url)
        .header("User-Agent", "HopeInspection/1.0")
        .send()
        .await
        .map_err(|e| format!("CVE 请求失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("CVE API 错误: {}", resp.status()));
    }

    let text = resp.text().await.map_err(|e| format!("读响应失败: {}", e))?;
    let api_resp: TridentSearchResp = serde_json::from_str(&text)
        .map_err(|e| format!("JSON 解析失败: {}", e))?;

    let mut cves: Vec<CveItem> = Vec::new();
    for r in api_resp.results {
        let cvss = r.cvss_score;
        let severity = if cvss >= 9.0 { "critical" } else if cvss >= 7.0 { "high" } else if cvss >= 4.0 { "medium" } else { "low" };
        cves.push(CveItem {
            cve_id: r.cve_id,
            summary: String::new(),
            cvss_score: cvss,
            severity: severity.to_string(),
            fix_version: None,
            exploit_available: r.kev,
        });
    }

    cves.sort_by(|a, b| b.cvss_score.partial_cmp(&a.cvss_score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(cves)
}

/// 通过 CIRCL CPE API 精准匹配产品版本（兜底用）
pub async fn query_cve_by_cpe(product: &str, version: &str) -> Result<Vec<CveItem>, String> {
    let product_lower = product.to_lowercase();
    let vendor = match product_lower.as_str() {
        "nginx" => "nginx", "redis" => "redis", "openssh" => "openbsd",
        "mysql" => "oracle", "postgresql" => "postgresql", "openssl" => "openssl",
        "python" => "python", "git" => "git", "curl" => "curl",
        p => p,
    };
    let ver_clean: String = version.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    if ver_clean.is_empty() { return Err("版本号无效".to_string()); }

    let cpe = format!("cpe:2.3:a:{}:{}:{}", vendor, product_lower, ver_clean);
    let url = format!("https://cve.circl.lu/api/cvefor/{}", cpe);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP 客户端: {}", e))?;

    let resp = client.get(&url).header("User-Agent", "HopeInspection/1.0")
        .send().await.map_err(|e| format!("CIRCL 请求失败: {}", e))?;
    if !resp.status().is_success() { return Err(format!("CIRCL 错误: {}", resp.status())); }

    let text = resp.text().await.map_err(|e| format!("读响应失败: {}", e))?;
    let results: Vec<serde_json::Value> = serde_json::from_str(&text)
        .map_err(|_| "CIRCL 响应非 JSON".to_string())?;

    let mut cves: Vec<CveItem> = results.into_iter()
        .filter_map(|v| {
            let cve_id = v.get("id")?.as_str()?.to_string();
            let cvss = v.get("cvss").and_then(|c| c.as_f64()).unwrap_or(0.0);
            Some(CveItem {
                cve_id, summary: String::new(), cvss_score: cvss,
                severity: if cvss >= 9.0 { "critical" } else if cvss >= 7.0 { "high" } else if cvss >= 4.0 { "medium" } else { "low" }.to_string(),
                fix_version: None, exploit_available: false,
            })
        })
        .collect();

    cves.sort_by(|a, b| b.cvss_score.partial_cmp(&a.cvss_score).unwrap_or(std::cmp::Ordering::Equal));
    cves.truncate(20);
    Ok(cves)
}

// ============================================================================
// Helpers
// ============================================================================

fn urlencoding(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            ' ' => result.push_str("%20"),
            '#' => result.push_str("%23"),
            '&' => result.push_str("%26"),
            '+' => result.push_str("%2B"),
            '/' => result.push_str("%2F"),
            '?' => result.push_str("%3F"),
            '=' => result.push_str("%3D"),
            _ => result.push(ch),
        }
    }
    result
}

// ============================================================================
// API response types
// ============================================================================

#[derive(Debug, Deserialize)]
struct TridentSearchResp {
    results: Vec<TridentSearchItem>,
}

#[derive(Debug, Deserialize)]
struct TridentSearchItem {
    #[serde(rename = "cveId")]
    cve_id: String,
    #[serde(rename = "cvss", default)]
    cvss_score: f64,
    #[serde(rename = "isKev", default)]
    kev: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_cve_server_nginx() {
        let cves = query_cve_from_server("nginx", "http://192.168.9.72:18080").await.unwrap();
        assert!(!cves.is_empty(), "Should return CVEs for nginx");
        println!("CVE server test PASSED: {} CVEs for nginx", cves.len());
        for c in cves.iter().take(3) {
            println!("  {} CVSS:{}", c.cve_id, c.cvss_score);
        }
    }
}
