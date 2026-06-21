use crate::services;
use std::str::FromStr;
use std::sync::Arc;
use tauri::Emitter;

#[tauri::command]
pub async fn scan_live_hosts(
    app_handle: tauri::AppHandle,
    subnet: String,
    timeout_ms: u64,
) -> Result<Vec<services::live_scanner::LiveHostResult>, String> {
    let ips = services::live_scanner::parse_cidr(&subnet)?;
    let timeout_secs = (timeout_ms as f64 / 1000.0).ceil() as u64;
    let sem = Arc::new(tokio::sync::Semaphore::new(80));
    let mut handles = Vec::with_capacity(ips.len());

    for ip in ips {
        let sem = sem.clone();
        let ip_str = ip.to_string();
        let app = app_handle.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await;
            let result = services::live_scanner::check_alive(&ip_str, timeout_secs).await;
            // 每扫完一个 IP 立即推事件给前端
            let _ = app.emit("live-scan-result", &result);
            result
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for h in handles {
        match h.await {
            Ok(r) => results.push(r),
            Err(e) => tracing::warn!("存活扫描任务 panic: {}", e),
        }
    }
    results.sort_by_key(|r| {
        let parts: Vec<u32> = r.ip.split('.').filter_map(|s| s.parse().ok()).collect();
        (parts.first().copied().unwrap_or(0) << 24)
            | (parts.get(1).copied().unwrap_or(0) << 16)
            | (parts.get(2).copied().unwrap_or(0) << 8)
            | parts.get(3).copied().unwrap_or(0)
    });
    Ok(results)
}

#[tauri::command]
pub async fn scan_ports(
    ip: String,
    ports: String,
    timeout_ms: u64,
) -> Result<Vec<services::port_scanner::PortScanResult>, String> {
    services::port_scanner::scan_ports(&ip, &ports, timeout_ms).await
}

#[tauri::command]
pub async fn scan_udp_ports(
    ip: String,
    ports: String,
    timeout_ms: u64,
) -> Result<Vec<services::port_scanner::UdpPortResult>, String> {
    services::port_scanner::scan_udp_ports(&ip, &ports, timeout_ms).await
}

#[tauri::command]
pub async fn check_web_urls(
    urls: Vec<String>,
    timeout_secs: u64,
) -> Result<Vec<services::web_checker::WebCheckResult>, String> {
    Ok(services::web_checker::check_urls(&urls, timeout_secs).await)
}

#[tauri::command]
pub async fn snmp_get(
    ip: String,
    community: String,
    oid: String,
    timeout_secs: u64,
) -> Result<services::snmp_checker::SnmpResult, String> {
    services::snmp_checker::snmp_v2c_get(&ip, &community, &oid, timeout_secs).await
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn snmp_v3_get(
    ip: String,
    username: String,
    auth_protocol: String,
    auth_password: String,
    priv_protocol: String,
    priv_password: String,
    oid: String,
    timeout_secs: u64,
) -> Result<services::snmp_checker::SnmpResult, String> {
    let auth = services::snmp_checker::AuthProtocol::from_str(&auth_protocol)?;
    let priv_p = services::snmp_checker::PrivProtocol::from_str(&priv_protocol)?;
    services::snmp_checker::snmp_v3_get(
        &ip, &username, auth, &auth_password, priv_p, &priv_password, &oid, timeout_secs,
    ).await
}

#[tauri::command]
pub async fn check_zabbix_agent(
    ip: String,
    port: u16,
    timeout_ms: u64,
) -> Result<services::zabbix_checker::ZabbixAgentResult, String> {
    let ip_clone = ip;
    tokio::task::spawn_blocking(move || {
        Ok(services::zabbix_checker::check_zabbix_agent(&ip_clone, port, timeout_ms))
    })
    .await
    .map_err(|e| format!("任务失败: {}", e))?
}
