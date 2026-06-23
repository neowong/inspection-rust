use crate::services;
use serde_json;
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

// ============================================================
// 路由跟踪 (Traceroute)
// ============================================================

#[derive(serde::Serialize)]
pub struct TraceHop {
    /// 跳数（从1开始）
    pub hop: u32,
    /// 节点 IP，None 表示该跳超时无响应
    pub ip: Option<String>,
    /// 归属地（格式化后，如"中国 浙江省杭州市 电信"），空串表示无记录
    pub region: String,
    /// 延迟（毫秒），None 表示超时
    pub rtt_ms: Option<f64>,
}

/// 检查离线 IP 归属地库是否已加载
#[tauri::command]
pub fn has_ip_db(state: tauri::State<'_, crate::AppState>) -> bool {
    state.ip_db.read().is_some()
}

/// 静默下载 ip2region_v4.xdb 到二进制同目录，完成后自动加载到内存
/// 前端通过 listen("ip-db-download-progress") 监听进度 {percent, downloaded, total}
#[tauri::command]
pub async fn download_ip_db(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<String, String> {
    let url = "https://github.com/lionsoul2014/ip2region/raw/master/data/ip2region_v4.xdb";
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .ok_or("无法获取程序目录")?;
    let dest = exe_dir.join("ip2region_v4.xdb");

    tracing::info!("[ip-db] 开始下载 {} → {}", url, dest.display());

    // 流式下载
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let resp = client.get(url).send().await.map_err(|e| {
        tracing::error!("[ip-db] 请求失败: {}", e);
        format!("下载请求失败: {}", e)
    })?;

    if !resp.status().is_success() {
        return Err(format!("下载失败，HTTP {}", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    // 写入临时文件，成功后 rename（避免中断留下损坏文件）
    let tmp_path = dest.with_extension("xdb.tmp");
    let mut file = std::fs::File::create(&tmp_path)
        .map_err(|e| format!("创建临时文件失败: {}", e))?;

    let mut stream = resp.bytes_stream();
    use futures::StreamExt;
    use std::io::Write;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| {
            // 下载中断，清理临时文件
            let _ = std::fs::remove_file(&tmp_path);
            format!("下载中断: {}", e)
        })?;
        file.write_all(&chunk).map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            format!("写入文件失败: {}", e)
        })?;
        downloaded += chunk.len() as u64;

        // 发进度事件（每 256KB 或完成时）
        if total > 0 && (downloaded % 262144 < chunk.len() as u64 || downloaded == total) {
            let percent = (downloaded * 100 / total) as u32;
            let _ = app.emit("ip-db-download-progress", serde_json::json!({
                "percent": percent,
                "downloaded": downloaded,
                "total": total,
            }));
        }
    }

    file.flush().map_err(|e| format!("刷新文件失败: {}", e))?;
    drop(file);

    // rename 到最终路径
    std::fs::rename(&tmp_path, &dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        format!("重命名文件失败: {}", e)
    })?;

    tracing::info!("[ip-db] 下载完成: {} ({} 字节)", dest.display(), downloaded);

    // 加载到内存
    match crate::services::ip_location::load_xdb(&dest) {
        Ok(data) => {
            *state.ip_db.write() = Some(Arc::new(data));
            tracing::info!("[ip-db] 已加载到内存");
            Ok("下载完成，归属地功能已启用".to_string())
        }
        Err(e) => {
            tracing::warn!("[ip-db] 下载成功但加载失败: {}", e);
            Err(format!("文件已下载但加载失败: {}", e))
        }
    }
}

/// 路由跟踪：调用系统 traceroute/tracert，解析每跳并查归属地
///
/// - Windows: `tracert -d -h <max_hops> -w <timeout> <target>`
/// - Linux:   `traceroute -n -m <max_hops> -w <secs> -q 1 <target>`（需已安装）
#[tauri::command]
pub async fn trace_route(
    target: String,
    max_hops: u32,
    timeout_ms: u64,
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<TraceHop>, String> {
    let target = target.trim().to_string();
    if target.is_empty() {
        return Err("请输入目标 IP 或域名".to_string());
    }
    let max_hops = if max_hops == 0 { 30 } else { max_hops };
    let timeout_ms = if timeout_ms == 0 { 1000 } else { timeout_ms };

    let target_clone = target.clone();
    let hops_result = tokio::task::spawn_blocking(move || {
        run_traceroute(&target_clone, max_hops, timeout_ms)
    })
    .await
    .map_err(|e| format!("跟踪任务失败: {}", e))?;

    let (hops, err) = hops_result?;

    // 查归属地
    let ip_db = state.ip_db.read().clone();
    let mut result: Vec<TraceHop> = Vec::new();
    for (hop, ip, rtt) in hops {
        let region = match (&ip_db, &ip) {
            (Some(db), Some(ip)) => {
                crate::services::ip_location::lookup(db, ip)
                    .map(|raw| crate::services::ip_location::format_region(&raw))
                    .unwrap_or_default()
            }
            _ => String::new(),
        };
        result.push(TraceHop { hop, ip, region, rtt_ms: rtt });
    }

    // 若 traceroute 整体失败（如未安装），但已解析部分跳，附加错误信息
    if let Some(e) = err {
        if result.is_empty() {
            return Err(e);
        }
        tracing::warn!("[trace_route] 部分失败: {}", e);
    }
    Ok(result)
}

/// 解析出的一跳：跳数 / IP（None=超时）/ 延迟ms（None=超时）
type ParsedHop = (u32, Option<String>, Option<f64>);

/// 执行 traceroute 并解析输出，返回 (每跳, 错误信息)
fn run_traceroute(
    target: &str,
    max_hops: u32,
    timeout_ms: u64,
) -> Result<(Vec<ParsedHop>, Option<String>), String> {
    use std::process::Command;

    let (program, args) = if cfg!(target_os = "windows") {
        ("tracert", vec![
            "-d".to_string(),
            "-h".to_string(), max_hops.to_string(),
            "-w".to_string(), timeout_ms.to_string(),
            target.to_string(),
        ])
    } else {
        // Linux: traceroute -n -m -w(秒) -q 1
        let secs = timeout_ms.div_ceil(1000);
        ("traceroute", vec![
            "-n".to_string(),
            "-m".to_string(), max_hops.to_string(),
            "-w".to_string(), secs.to_string(),
            "-q".to_string(), "1".to_string(),
            target.to_string(),
        ])
    };

    let mut cmd = Command::new(program);
    cmd.args(&args);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            if cfg!(target_os = "windows") {
                "未找到 tracert 命令，请检查系统".to_string()
            } else {
                "未找到 traceroute 命令，请先安装：sudo apt install traceroute".to_string()
            }
        } else {
            format!("执行 {} 失败: {}", program, e)
        }
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    tracing::debug!("[trace_route] {} stdout:\n{}", program, stdout);

    let hops = parse_traceroute_output(&stdout);

    // 非零退出码且无解析结果 → 报错
    let err = if !output.status.success() && hops.is_empty() {
        Some(if stderr.trim().is_empty() {
            format!("{} 执行失败", program)
        } else {
            stderr.trim().to_string()
        })
    } else {
        None
    };

    Ok((hops, err))
}

/// 解析 traceroute/tracert 输出，提取每跳 (hop, ip, rtt_ms)
fn parse_traceroute_output(stdout: &str) -> Vec<ParsedHop> {
    use regex::Regex;
    let hop_re = Regex::new(r"^\s*(\d+)\s").unwrap();
    let ip_re = Regex::new(r"(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})").unwrap();
    let ms_re = Regex::new(r"(\d+(?:\.\d+)?)\s*ms").unwrap();

    let mut hops = Vec::new();
    for line in stdout.lines() {
        // 跳过表头（Windows"通过最多...跟踪"/Linux"traceroute to..."）
        let Some(hop_cap) = hop_re.captures(line) else { continue };
        let hop: u32 = match hop_cap[1].parse() { Ok(n) => n, Err(_) => continue };

        // 提取第一个 IPv4（跳过 *）
        let ip = ip_re.captures(line).map(|c| c[1].to_string());

        // 提取第一个延迟（ms）
        let rtt = ms_re.captures(line).and_then(|c| c[1].parse::<f64>().ok());

        hops.push((hop, ip, rtt));
    }
    hops
}
