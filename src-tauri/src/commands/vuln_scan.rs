use crate::services::{banner_grabber, cve_checker, cve_db, port_scanner};
use serde::Serialize;
use tauri::State;
use crate::AppState;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct VulnScanResult {
    pub ip: String,
    pub os_info: String,
    pub total_ports: usize,
    pub total_cves: usize,
    pub max_cvss: f64,
    pub overall: String,
    pub summary: String,
    pub banners: Vec<serde_json::Value>,
    pub cve_details: Vec<CveDetail>,
    pub cve_api_ok: bool,
    pub scan_phase: String,
    pub nuclei_enabled: bool,
    pub nuclei_findings: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct CveDetail {
    pub product: String,
    pub version: String,
    pub total_cves: usize,
    pub max_cvss: f64,
    pub cves: Vec<cve_checker::CveItem>,
}

// ============================================================================
// 端口列表
// ============================================================================

/// 快速扫描端口（Top 88，覆盖最常用的服务）
const QUICK_PORTS: [u16; 88] = [
    21,22,23,25,53,80,110,111,135,139,143,161,162,389,443,445,465,500,514,587,
    636,993,995,1080,1433,1521,2049,2082,2083,2181,2375,2376,3128,3306,3389,
    3690,4369,4444,4848,5000,5001,5222,5432,5555,5601,5666,5672,5900,5901,
    5984,6000,6001,6379,6443,6666,7001,7002,7077,8000,8001,8009,8010,8040,
    8069,8080,8081,8086,8088,8089,8090,8443,8530,8531,8649,8686,8787,8880,
    8888,8889,8983,9000,9001,9002,9042,9060,9090,9100,9200,
];

/// 扫描 1-65535 全量端口
fn full_port_range() -> Vec<u16> {
    (1..=65535).collect()
}

// ============================================================================
// OS 识别
// ============================================================================

/// 从端口和服务信息推测操作系统
fn guess_os(banners: &[serde_json::Value]) -> String {
    let mut has_ssh = false;
    let mut has_rdp = false;
    let mut has_smb = false;
    let mut has_http = false;
    let mut os_hint = String::new();

    for b in banners {
        let banner = b["banner"].as_str().unwrap_or("").to_lowercase();
        let port = b["port"].as_u64().unwrap_or(0);

        match port {
            22 => {
                has_ssh = true;
                if banner.contains("ubuntu") { os_hint = "Ubuntu Linux".to_string(); }
                else if banner.contains("debian") { os_hint = "Debian Linux".to_string(); }
                else if banner.contains("centos") { os_hint = "CentOS Linux".to_string(); }
                else if banner.contains("rhel") || banner.contains("red hat") { os_hint = "RHEL Linux".to_string(); }
                else if banner.contains("freebsd") { os_hint = "FreeBSD".to_string(); }
                else if banner.contains("darwin") || banner.contains("apple") { os_hint = "macOS".to_string(); }
            }
            3389 => has_rdp = true,
            139 | 445 => has_smb = true,
            80 | 443 | 8080 | 8443 => has_http = true,
            _ => {}
        }
    }

    if !os_hint.is_empty() { return os_hint; }
    if has_smb || has_rdp {
        if has_ssh { "Linux（含 Samba 服务）".to_string() }
        else { "Windows Server".to_string() }
    } else if has_ssh && has_http {
        "Linux（通用服务器）".to_string()
    } else if has_ssh {
        "Linux / Unix".to_string()
    } else {
        "未知".to_string()
    }
}

// ============================================================================
// Helpers
// ============================================================================

async fn query_cves(products: &[(String, String)]) -> (Vec<CveDetail>, bool) {
    let mut results = Vec::new();
    let has_local = cve_db::has_local_data();
    let mut api_ok = has_local;
    let mut seen = std::collections::HashSet::new();
    for (product, version) in products {
        if product.is_empty() || !seen.insert(product.clone()) {
            continue;
        }
        let ver = version.clone();

        // 1. 优先本地文本搜索（快）
        let mut cves = if has_local {
            cve_db::query_cve_by_product_local(product).unwrap_or_default()
        } else { Vec::new() };

        // 2. 文本搜索没结果 → 试 CVE 在线服务
        if cves.is_empty() {
            if api_ok {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
            // 先试自建 CVE 服务（局域网，快）
            cves = match cve_checker::query_cve_from_server(product, "http://192.168.9.72:18080").await {
                Ok(c) => { api_ok = true; c }
                Err(_) => {
                    // CVE 服务不可用 → 试 TridentStack 兜底
                    match cve_checker::query_cve_by_product(product).await {
                        Ok(c) => { api_ok = true; c }
                        Err(_) => Vec::new(),
                    }
                }
            };
        }

        let max_cvss = cves.iter().map(|c| c.cvss_score).fold(0.0, f64::max);
        if !cves.is_empty() {
            results.push(CveDetail {
                product: product.clone(),
                version: ver,
                total_cves: cves.len(),
                max_cvss,
                cves,
            });
        }
    }
    (results, api_ok)
}

fn classify_overall(total_cves: usize, max_cvss: f64) -> String {
    if total_cves == 0 { "ok".to_string() }
    else if max_cvss >= 9.0 { "critical".to_string() }
    else if max_cvss >= 7.0 { "warning".to_string() }
    else if max_cvss >= 4.0 { "info".to_string() }
    else { "ok".to_string() }
}

/// 通用扫描：端口扫描 + Banner + OS + CVE
async fn scan_common(ip: &str, ports: &[u16]) -> Result<VulnScanResult, String> {
    let port_list = ports.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",");
    let scan_results = port_scanner::scan_ports(ip, &port_list, 2500)
        .await.map_err(|e| format!("端口扫描失败: {}", e))?;

    let open_ports: Vec<_> = scan_results.iter().filter(|r| r.open).collect();

    if open_ports.is_empty() {
        return Ok(VulnScanResult {
            ip: ip.to_string(), os_info: "未知".to_string(),
            total_ports: 0, total_cves: 0, max_cvss: 0.0, overall: "info".to_string(),
            summary: "未发现开放端口，或设备不在线".to_string(),
            banners: vec![], cve_details: vec![], cve_api_ok: false, scan_phase: "done".to_string(),
            nuclei_enabled: false, nuclei_findings: vec![],
        });
    }

    let mut products: Vec<(String, String)> = Vec::new(); // (product, version)
    let mut banners: Vec<serde_json::Value> = Vec::new();

    for r in &open_ports {
        if let Some(b) = banner_grabber::grab_banner(ip, r.port) {
            let pname = b.product.to_lowercase();
            if !pname.is_empty() {
                let key = (pname.clone(), b.version.clone());
                if !products.contains(&key) {
                    products.push(key);
                }
            }
            banners.push(serde_json::json!({
                "port": b.port, "service": b.service,
                "product": b.product, "version": b.version, "banner": b.banner,
            }));
        } else {
            banners.push(serde_json::json!({
                "port": r.port, "service": r.service,
                "product": "", "version": "", "banner": "",
            }));
        }
    }

    let os_info = guess_os(&banners);
    let (cve_details, cve_api_ok) = query_cves(&products).await;
    let total_cves: usize = cve_details.iter().map(|r| r.total_cves).sum();
    let max_cvss = cve_details.iter().map(|r| r.max_cvss).fold(0.0, f64::max);
    let overall = classify_overall(total_cves, max_cvss);

    let nuclei_enabled = crate::services::nuclei_runner::is_nuclei_ready();
    let mut nuclei_findings: Vec<serde_json::Value> = Vec::new();
    if nuclei_enabled {
        tracing::info!("nuclei 已就绪，开始对 {} 个开放端口进行漏洞验证", open_ports.len());
        for r in &open_ports {
            tracing::info!("nuclei 验证: {}:{}/{}", ip, r.port, r.service);
            match crate::services::nuclei_runner::scan_target(ip, r.port, &r.service) {
                Ok(nf) => {
                    tracing::info!("nuclei {}:{} 发现 {} 个结果", ip, r.port, nf.len());
                    nuclei_findings.extend(nf);
                }
                Err(e) => tracing::warn!("nuclei {}:{} 失败: {}", ip, r.port, e),
            }
        }
        tracing::info!("nuclei 扫描完成，共发现 {} 个漏洞", nuclei_findings.len());
    } else {
        tracing::info!("nuclei 未安装，跳过漏洞验证（仅做版本匹配）");
    }

    Ok(VulnScanResult {
        ip: ip.to_string(), os_info,
        total_ports: open_ports.len(), total_cves, max_cvss, overall,
        summary: format!("发现 {} 个开放端口，检测到 {} 个已知 CVE 漏洞", open_ports.len(), total_cves),
        banners, cve_details, cve_api_ok, scan_phase: "done".to_string(),
        nuclei_enabled, nuclei_findings,
    })
}

// ============================================================================
// Commands
// ============================================================================

#[tauri::command]
pub async fn scan_ip_vulns(ip: String, full_scan: bool, custom_ports: String) -> Result<VulnScanResult, String> {
    if ip.parse::<std::net::IpAddr>().is_err() {
        return Err("请输入有效的 IP 地址".to_string());
    }

    let ports: Vec<u16> = if !custom_ports.trim().is_empty() {
        // 自定义端口：支持逗号、空格、横杠分隔的端口和范围
        let mut list = Vec::new();
        for part in custom_ports.split(|c: char| c == ',' || c == ' ' || c == '\t' || c == ';') {
            let part = part.trim();
            if part.is_empty() { continue; }
            if let Some((start, end)) = part.split_once('-') {
                let s: u16 = start.trim().parse().map_err(|_| format!("无效端口范围: {}", part))?;
                let e: u16 = end.trim().parse().map_err(|_| format!("无效端口范围: {}", part))?;
                if s > e {
                    return Err(format!("无效端口范围: {} (1-65535)", part));
                }
                for p in s..=e { list.push(p); }
            } else {
                let p: u16 = part.parse().map_err(|_| format!("无效端口: {}", part))?;
                list.push(p);
            }
        }
        if list.is_empty() { return Err("未输入有效端口".to_string()); }
        list.sort();
        list.dedup();
        list
    } else if full_scan {
        full_port_range()
    } else {
        QUICK_PORTS[..].to_vec()
    };

    scan_common(&ip, &ports).await
}

/// 下载 CVE 离线数据库
#[tauri::command]
pub async fn download_cve_db() -> Result<String, String> {
    let progress = std::sync::Arc::new(std::sync::Mutex::new(|done: usize, err: usize| {
        tracing::info!("CVE 数据库下载进度: {} 条已处理, {} 条错误", done, err);
    }));
    cve_db::download_cve_db(move |done, err| {
        let p = progress.lock().unwrap();
        p(done, err);
    })
    .await?;
    let info = cve_db::db_info().map_err(|e| e.to_string())?;
    Ok(format!(
        "CVE 数据库下载完成，共 {} 条记录",
        info["count"].as_i64().unwrap_or(0)
    ))
}

/// 获取 CVE 数据库状态
#[tauri::command]
pub fn get_cve_db_info() -> Result<serde_json::Value, String> {
    cve_db::db_info()
}

/// 检查是否有本地 CVE 数据
#[tauri::command]
pub fn has_cve_local_db() -> bool {
    cve_db::has_local_data()
}

/// 获取 nuclei 安装状态
#[tauri::command]
pub fn get_nuclei_status() -> serde_json::Value {
    crate::services::nuclei_runner::get_nuclei_info()
}

/// 下载 nuclei 二进制和模板
#[tauri::command]
pub async fn download_nuclei() -> Result<(), String> {
    let progress = std::sync::Arc::new(std::sync::Mutex::new(|_: u64, _: u64| {}));
    crate::services::nuclei_runner::download_nuclei(progress).await
}

/// 对目标运行 nuclei 漏洞验证
#[tauri::command]
pub fn run_nuclei_scan(target: String, port: u16, service: String) -> Result<Vec<serde_json::Value>, String> {
    crate::services::nuclei_runner::scan_target(&target, port, &service)
}

/// 诊断：测试 CVE API 是否正常
#[tauri::command]
pub async fn test_cve_api() -> Result<serde_json::Value, String> {
    let start = std::time::Instant::now();
    match cve_checker::query_cve_by_product("nginx").await {
        Ok(cves) => {
            Ok(serde_json::json!({
                "ok": true, "product": "nginx",
                "cve_count": cves.len(),
                "elapsed_ms": start.elapsed().as_millis(),
                "cves": cves.iter().map(|c| serde_json::json!({
                    "cve_id": c.cve_id, "cvss": c.cvss_score,
                })).collect::<Vec<_>>(),
            }))
        }
        Err(e) => {
            Ok(serde_json::json!({
                "ok": false, "error": e,
                "elapsed_ms": start.elapsed().as_millis(),
            }))
        }
    }
}

#[tauri::command]
pub fn estimate_scan_time(full_scan: bool) -> u64 {
    if full_scan { 120 } else { 10 }
}

#[tauri::command]
pub async fn scan_device_vulns(
    state: State<'_, AppState>,
    device_id: i64,
    full_scan: bool,
) -> Result<serde_json::Value, String> {
    let ip = {
        let conn = state.db.lock();
        let sql = "SELECT ip FROM devices WHERE id = ?1";
        conn.query_row(sql, rusqlite::params![device_id], |row| row.get::<_, String>(0))
            .map_err(|_| "设备不存在".to_string())
    }?;

    let result = if full_scan {
        scan_common(&ip, &full_port_range()).await?
    } else {
        scan_common(&ip, &QUICK_PORTS).await?
    };

    Ok(serde_json::json!({
        "device_id": device_id, "ip": ip,
        "os_info": result.os_info,
        "total_ports": result.total_ports, "total_cves": result.total_cves,
        "max_cvss": result.max_cvss, "overall": result.overall,
        "summary": result.summary,
        "banners": result.banners, "cve_details": result.cve_details,
        "cve_api_ok": result.cve_api_ok,
    }))
}
