use crate::services;

#[tauri::command]
pub async fn scan_live_hosts(
    subnet: String,
    timeout_ms: u64,
) -> Result<Vec<services::live_scanner::LiveHostResult>, String> {
    services::live_scanner::scan_subnet(&subnet, timeout_ms).await
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
