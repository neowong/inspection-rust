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
    let total = ips.len();

    tracing::info!("存活扫描开始: subnet={}, hosts={}, timeout={}ms", subnet, total, timeout_ms);
    let start = std::time::Instant::now();

    let sem = Arc::new(tokio::sync::Semaphore::new(80));
    let mut handles = Vec::with_capacity(total);

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

    let alive = results.iter().filter(|r| r.alive).count();
    let latency = start.elapsed().as_millis();
    tracing::info!("存活扫描完成: subnet={}, total={}, alive={}, latency={}ms", subnet, total, alive, latency);

    Ok(results)
}

#[tauri::command]
pub async fn scan_ports(
    app: tauri::AppHandle,
    ip: String,
    ports: String,
    timeout_ms: u64,
) -> Result<Vec<services::port_scanner::PortScanResult>, String> {
    tracing::info!("TCP 端口扫描开始: ip={}, ports={}, timeout={}ms", ip, ports, timeout_ms);
    let start = std::time::Instant::now();
    let results = services::port_scanner::scan_ports_with_callback(&ip, &ports, timeout_ms, move |result| {
        let _ = app.emit("port-scan-result", &result);
    }).await;
    let latency = start.elapsed().as_millis();
    match &results {
        Ok(r) => tracing::info!("TCP 端口扫描完成: ip={}, ports={}, results={}, latency={}ms", ip, ports, r.len(), latency),
        Err(e) => tracing::warn!("TCP 端口扫描失败: ip={}, ports={}, latency={}ms, error={}", ip, ports, latency, e),
    }
    results
}

#[tauri::command]
pub async fn scan_udp_ports(
    app: tauri::AppHandle,
    ip: String,
    ports: String,
    timeout_ms: u64,
) -> Result<Vec<services::port_scanner::UdpPortResult>, String> {
    tracing::info!("UDP 端口扫描开始: ip={}, ports={}, timeout={}ms", ip, ports, timeout_ms);
    let start = std::time::Instant::now();
    let results = services::port_scanner::scan_udp_ports_with_callback(&ip, &ports, timeout_ms, move |result| {
        let _ = app.emit("udp-scan-result", &result);
    }).await;
    let latency = start.elapsed().as_millis();
    match &results {
        Ok(r) => tracing::info!("UDP 端口扫描完成: ip={}, ports={}, results={}, latency={}ms", ip, ports, r.len(), latency),
        Err(e) => tracing::warn!("UDP 端口扫描失败: ip={}, ports={}, latency={}ms, error={}", ip, ports, latency, e),
    }
    results
}

#[tauri::command]
pub async fn check_web_urls(
    urls: Vec<String>,
    timeout_secs: u64,
) -> Result<Vec<services::web_checker::WebCheckResult>, String> {
    tracing::info!("WEB 检测开始: urls={}, timeout={}s", urls.len(), timeout_secs);
    let start = std::time::Instant::now();
    let result = services::web_checker::check_urls(&urls, timeout_secs).await;
    let latency = start.elapsed().as_millis();
    tracing::info!("WEB 检测完成: urls={}, results={}, latency={}ms", urls.len(), result.len(), latency);
    Ok(result)
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

/// 获取应用版本号（编译时从 Cargo.toml 读取，前后端统一）
#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// 获取操作系统信息（类型和版本），用于问题反馈等场景展示
#[tauri::command]
pub fn get_os_info() -> serde_json::Value {
    let info = os_info::get();
    serde_json::json!({
        "os": info.os_type().to_string(),
        "os_version": info.version().to_string(),
    })
}

/// 检查离线 IP 归属地库是否已加载
#[tauri::command]
pub fn has_ip_db(state: tauri::State<'_, crate::AppState>) -> bool {
    state.ip_db.read().is_some()
}

/// 检查 GitHub Releases 是否有新版本
/// 返回 (最新版本号, 下载地址, 发布说明)，无更新时返回 None
#[tauri::command]
pub async fn check_update(
    current_version: String,
) -> Result<Option<serde_json::Value>, String> {
    let url = "https://api.github.com/repos/neowong/inspection-rust/releases/latest";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("ai-inspection-update-check")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let resp = client.get(url).send().await
        .map_err(|e| format!("检查更新失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("检查更新失败，HTTP {}", resp.status()));
    }

    let release: serde_json::Value = resp.json().await
        .map_err(|e| format!("解析更新信息失败: {}", e))?;

    let latest_tag = release.get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches("internal-")
        .trim_start_matches('v')
        .to_string();

    let current = current_version.trim_start_matches('v');

    // 简单版本比较：按 . 分割逐段比较
    let latest_parts: Vec<u32> = latest_tag.split('.').filter_map(|s| s.parse().ok()).collect();
    let current_parts: Vec<u32> = current.split('.').filter_map(|s| s.parse().ok()).collect();

    let has_update = latest_parts > current_parts;

    if has_update {
        let html_url = release.get("html_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let body = release.get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok(Some(serde_json::json!({
            "version": latest_tag,
            "url": html_url,
            "body": body,
        })))
    } else {
        Ok(None)
    }
}

/// 提交问题反馈到统计服务端（静默，失败忽略）
#[tauri::command]
pub async fn submit_feedback(
    feedback_type: String,
    title: String,
    content: String,
    contact: Option<String>,
    version: String,
) -> Result<(), String> {
    let device_id = {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_default();
        let mac = get_mac_address().unwrap_or_default();
        let raw = format!("{}:{}", hostname, mac);
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(raw.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    let os_info = os_info::get();
    let os_type = os_info.os_type().to_string();
    let os_version = os_info.version().to_string();

    let payload = serde_json::json!({
        "device_id": &device_id,
        "feedback_type": &feedback_type,
        "title": &title,
        "content": &content,
        "contact": contact.unwrap_or_default(),
        "version": &version,
        "os": os_type,
        "os_version": os_version,
    });

    let api_url = "https://neowong.eu.org/stats/api/feedback";
    tracing::info!("[feedback] 提交反馈: type={}, title={}", feedback_type, title);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let resp = client.post(api_url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("提交反馈失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("提交失败，服务器返回 {}", resp.status()));
    }

    tracing::info!("[feedback] 反馈提交成功");
    Ok(())
}

/// 获取本机 MAC 地址
fn get_mac_address() -> Option<String> {
    // 读取 /sys/class/net/*/address (Linux) 或通过网络接口获取
    #[cfg(target_os = "linux")]
    {
        std::fs::read_dir("/sys/class/net").ok()?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name != "lo" && !name.starts_with("docker") && !name.starts_with("br-")
            })
            .filter_map(|entry| {
                let path = entry.path().join("address");
                std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
            })
            .next()
    }
    #[cfg(target_os = "windows")]
    {
        // Windows 通过 ipconfig /all 获取
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        std::process::Command::new("ipconfig")
            .args(["/all"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()
            .and_then(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.lines()
                    .find(|line| line.contains("Physical Address") || line.contains("物理地址"))
                    .and_then(|line| {
                        line.split(':').last().map(|s| s.trim().replace('-', ":"))
                    })
            })
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        None
    }
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

        // 大小上限：ip2region.xdb 正常约 11MB，给 30MB 余量，超出视为异常中止
        const MAX_IPDB_SIZE: u64 = 30 * 1024 * 1024;
        if downloaded > MAX_IPDB_SIZE {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(format!("下载文件超过最大限制 ({}MB)，已中止", MAX_IPDB_SIZE / 1024 / 1024));
        }

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

/// 路由跟踪：调用系统 traceroute/tracert，逐跳实时 emit 事件
///
/// 前端通过 listen("trace-hop") 接收每跳结果，listen("trace-done") 知道完成。
/// - Windows: `tracert -d -h <max_hops> -w <timeout> <target>`
/// - Linux:   `traceroute -n -m <max_hops> -w <secs> -q 1 <target>`
#[tauri::command]
pub async fn trace_route(
    app: tauri::AppHandle,
    target: String,
    max_hops: u32,
    timeout_ms: u64,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    let target = target.trim().to_string();
    if target.is_empty() {
        return Err("请输入目标 IP 或域名".to_string());
    }
    // 校验 target 为合法 IP 或域名（防止注入命令行标志）
    if target.starts_with('-') ||
       !target.chars().all(|c| c.is_alphanumeric() || c == '.' || c == ':' || c == '-' || c == '_') {
        return Err("目标地址格式无效".to_string());
    }
    let max_hops = if max_hops == 0 { 30 } else { max_hops };
    let timeout_ms = if timeout_ms == 0 { 1000 } else { timeout_ms };

    // 复制 ip_db 到 spawn_blocking 闭包
    let ip_db: Option<Arc<Vec<u8>>> = state.ip_db.read().clone();
    let app_clone = app.clone();

    tokio::task::spawn_blocking(move || {
        run_traceroute_stream(&app_clone, &ip_db, &target, max_hops, timeout_ms)
    })
    .await
    .map_err(|e| format!("跟踪任务失败: {}", e))?
}

/// 流式执行 traceroute：逐行读 stdout，每解析一跳立即 emit 事件给前端
fn run_traceroute_stream(
    app: &tauri::AppHandle,
    ip_db: &Option<Arc<Vec<u8>>>,
    target: &str,
    max_hops: u32,
    timeout_ms: u64,
) -> Result<(), String> {
    use std::io::{BufRead, BufReader};
    use std::process::Command;

    let (program, args) = if cfg!(target_os = "windows") {
        ("tracert", vec![
            "-d".to_string(),
            "-h".to_string(), max_hops.to_string(),
            "-w".to_string(), timeout_ms.to_string(),
            target.to_string(),
        ])
    } else {
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

    let mut child = cmd.spawn().map_err(|e| {
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

    // 正则预编译
    let hop_re = regex::Regex::new(r"^\s*(\d+)\s").unwrap();
    let ip_re = regex::Regex::new(r"(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})").unwrap();
    let ms_re = regex::Regex::new(r"(\d+(?:\.\d+)?)\s*ms").unwrap();

    // 逐行读 stdout，实时解析
    let stdout = child.stdout.take().expect("stdout was piped");
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        tracing::trace!("[trace_route] {}", line);

        // 尝试解析跳数
        let Some(hop_cap) = hop_re.captures(&line) else { continue };
        let hop: u32 = match hop_cap[1].parse() { Ok(n) => n, Err(_) => continue };

        let ip = ip_re.captures(&line).map(|c| c[1].to_string());
        let rtt = ms_re.captures(&line).and_then(|c| c[1].parse::<f64>().ok());

        // 查归属地
        let region = match (ip_db, &ip) {
            (Some(db), Some(addr)) => {
                crate::services::ip_location::lookup(db, addr)
                    .map(|raw| crate::services::ip_location::format_region(&raw, Some(addr.as_str())))
                    .unwrap_or_default()
            }
            _ => {
                ip.as_ref()
                    .filter(|addr| crate::services::ip_location::is_private_ip(addr))
                    .map(|_| "局域网".to_string())
                    .unwrap_or_default()
            }
        };

        // 立即 emit 给前端
        let _ = app.emit("trace-hop", serde_json::json!({
            "hop": hop,
            "ip": ip,
            "region": region,
            "rtt_ms": rtt,
        }));
    }

    // 等待进程结束
    let status = child.wait().map_err(|e| format!("等待进程结束失败: {}", e))?;
    if !status.success() {
        // 退出码 2 通常是 DNS 解析失败，读 stderr 获取具体错误
        let stderr_output = child.stderr.take()
            .and_then(|s| {
                let mut buf = String::new();
                std::io::BufReader::new(s).read_line(&mut buf).ok().map(|_| buf)
            })
            .unwrap_or_default()
            .trim()
            .to_string();
        let msg = if stderr_output.is_empty() {
            format!("路由跟踪失败（退出码 {}）", status.code().unwrap_or(-1))
        } else {
            format!("路由跟踪失败: {}", stderr_output)
        };
        tracing::warn!("[trace_route] {}", msg);
        let _ = app.emit("trace-done", serde_json::json!({ "success": false }));
        return Err(msg);
    }

    // 通知前端完成
    let _ = app.emit("trace-done", serde_json::json!({ "success": true }));
    Ok(())
}

// ============================================================
// TFTP Server
// ============================================================

use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;
use tokio::net::UdpSocket;
use tokio::fs;

static TFTP_RUNNING: AtomicBool = AtomicBool::new(false);

/// Linux 下绑定低端口 (<1024) 需要授权，尝试用 pkexec setcap
#[cfg(target_os = "linux")]
async fn bind_privileged_port(port: u16) -> Result<UdpSocket, String> {
    let addr = format!("0.0.0.0:{}", port);
    match UdpSocket::bind(&addr).await {
        Ok(s) => return Ok(s),
        Err(_) => {} // EACCES, fall through to pkexec
    }

    // Linux: 用 pkexec 启动当前二进制子进程 bind 端口，通过 Unix socket 传回 fd
    use std::os::unix::io::{FromRawFd, AsRawFd};
    use std::os::unix::net::UnixStream;
    let (parent, child) = UnixStream::pair().map_err(|e| format!("创建 socket pair 失败: {}", e))?;

    let exe = std::env::current_exe().map_err(|e| format!("无法获取程序路径: {}", e))?;
    let child_fd = child.as_raw_fd();
    let port_str = port.to_string();

    let mut cmd = std::process::Command::new("pkexec");
    cmd.arg(&exe)
        .arg("--bind-privileged-port")
        .arg(&port_str)
        .arg(child_fd.to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    // 保持 child fd 在子进程中可用
    unsafe {
        use std::os::unix::process::CommandExt;
        cmd.pre_exec(move || {
            // 不清除 fd
            Ok(())
        });
    }

    let mut child_proc = cmd.spawn().map_err(|e| format!("启动授权进程失败: {}", e))?;

    // 关闭子进程端的 fd
    drop(child);
    let parent_fd = parent.as_raw_fd();

    // spawn_blocking: recvmsg 是阻塞调用，不能阻塞 tokio 事件循环
    let sock_fd = tokio::task::spawn_blocking(move || -> Result<i32, String> {
        let mut buf = [0u8; 1];
        let mut cmsg_buf = [0u8; 32];
        let mut iov = libc::iovec { iov_base: buf.as_mut_ptr() as *mut _, iov_len: 1 };
        let mut hdr: libc::msghdr = unsafe { std::mem::zeroed() };
        hdr.msg_iov = &mut iov;
        hdr.msg_iovlen = 1;
        hdr.msg_control = cmsg_buf.as_mut_ptr() as *mut _;
        hdr.msg_controllen = cmsg_buf.len();

        let n = unsafe { libc::recvmsg(parent_fd, &mut hdr, 0) };
        if n < 0 {
            return Err("接收授权端口失败".into());
        }
        let cmsg = unsafe { libc::CMSG_FIRSTHDR(&hdr) };
        if cmsg.is_null() || unsafe { (*cmsg).cmsg_type != libc::SCM_RIGHTS } {
            return Err("未收到端口文件描述符".into());
        }
        Ok(unsafe { *(libc::CMSG_DATA(cmsg) as *const libc::c_int) })
    }).await.map_err(|e| format!("内部错误: {}", e))??;

    let _ = child_proc.wait();

    let std_sock = unsafe { std::net::UdpSocket::from_raw_fd(sock_fd) };
    std_sock.set_nonblocking(true).ok();
    let socket = UdpSocket::from_std(std_sock).map_err(|e| format!("转换 socket 失败: {}", e))?;
    Ok(socket)
}

#[cfg(not(target_os = "linux"))]
async fn bind_privileged_port(port: u16) -> Result<UdpSocket, String> {
    let addr = format!("0.0.0.0:{}", port);
    UdpSocket::bind(&addr).await.map_err(|e| format!("端口 {} 绑定失败: {}", port, e))
}

#[tauri::command]
pub async fn start_tftp_server(
    app: tauri::AppHandle,
    file_path: String,
    port: Option<u16>,
) -> Result<(), String> {
    if TFTP_RUNNING.swap(true, Ordering::SeqCst) {
        return Err("TFTP 服务已在运行中".into());
    }

    let port = port.unwrap_or(69);
    let base_dir = std::path::PathBuf::from(&file_path);
    if !base_dir.is_dir() {
        TFTP_RUNNING.store(false, Ordering::SeqCst);
        return Err("选择的路径不是有效目录".into());
    }

    let socket = bind_privileged_port(port).await.map_err(|e| {
        TFTP_RUNNING.store(false, Ordering::SeqCst);
        e
    })?;

    tracing::info!("[tftp] 启动服务, 目录: {}, 端口: {}", file_path, port);
    let _ = app.emit("tftp-log", serde_json::json!({
        "msg": format!("TFTP 服务已启动，根目录: {}，端口: {}", file_path, port),
        "type": "info"
    }));

    tokio::spawn(async move {
        let mut buf = vec![0u8; 516];
        let block_size: u16 = 512;
        // 跟踪每个客户端的传输状态: (file_name, file_data, current_block)
        let mut clients: HashMap<String, (String, Vec<u8>, u16)> = HashMap::new();

        /// 发送 TFTP 错误包
        async fn send_error(socket: &UdpSocket, dst: std::net::SocketAddr, code: u16, msg: &str) {
            let mut err_pkt = vec![0u8; 5 + msg.len() + 1];
            err_pkt[0] = 0; err_pkt[1] = 5; // ERROR opcode
            err_pkt[2] = (code >> 8) as u8; err_pkt[3] = code as u8;
            err_pkt[4..4 + msg.len()].copy_from_slice(msg.as_bytes());
            let _ = socket.send_to(&err_pkt, dst).await;
        }

        loop {
            if !TFTP_RUNNING.load(Ordering::SeqCst) {
                let _ = app.emit("tftp-log", serde_json::json!({ "msg": "TFTP 服务已停止", "type": "info" }));
                break;
            }

            let (n, src) = match tokio::time::timeout(
                std::time::Duration::from_secs(1), socket.recv_from(&mut buf)
            ).await {
                Ok(Ok(v)) => v,
                _ => continue,
            };

            let opcode = u16::from_be_bytes([buf[0], buf[1]]);
            let client_key = src.to_string();

            match opcode {
                1 => { // RRQ - 读请求
                    let end = buf[2..n].iter().position(|&b| b == 0).unwrap_or(n - 2);
                    let req_name = String::from_utf8_lossy(&buf[2..2 + end]);
                    // 防止路径穿越：只取文件名部分
                    let safe_name = std::path::Path::new(req_name.as_ref())
                        .file_name().map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| req_name.to_string());
                    let full_path = base_dir.join(&safe_name);

                    let _ = app.emit("tftp-log", serde_json::json!({
                        "msg": format!("{} → RRQ 请求下载: {}", src.ip(), safe_name),
                        "type": "info"
                    }));

                    match fs::read(&full_path).await {
                        Ok(data) => {
                            let file_size = data.len() as u64;
                            let block_num = 1u16;
                            let chunk_end = std::cmp::min(block_size as usize, data.len());
                            let chunk = data[..chunk_end].to_vec();
                            clients.insert(client_key.clone(), (safe_name, data, block_num));

                            let mut pkt = vec![0u8; 4 + chunk.len()];
                            pkt[0] = 0; pkt[1] = 3;
                            pkt[2] = (block_num >> 8) as u8;
                            pkt[3] = block_num as u8;
                            pkt[4..].copy_from_slice(&chunk);
                            let _ = socket.send_to(&pkt, src).await;
                            let _ = app.emit("tftp-progress", serde_json::json!({
                                "ip": src.ip().to_string(),
                                "bytes": chunk.len() as u64,
                                "total": file_size,
                                "done": false
                            }));
                        }
                        Err(_) => {
                            let _ = app.emit("tftp-log", serde_json::json!({
                                "msg": format!("{} → 文件未找到: {}", src.ip(), safe_name),
                                "type": "error"
                            }));
                            send_error(&socket, src, 1, "File not found").await;
                        }
                    }
                }
                4 => { // ACK
                    let block = u16::from_be_bytes([buf[2], buf[3]]);
                    let client_info = clients.get(&client_key).cloned();
                    if let Some((fname, file_data, current)) = client_info {
                        if block == current {
                            let file_size = file_data.len() as u64;
                            let next_block = block.wrapping_add(1);
                            let offset = (block as usize) * block_size as usize;

                            if offset >= file_data.len() {
                                clients.remove(&client_key);
                                let _ = app.emit("tftp-log", serde_json::json!({
                                    "msg": format!("{} → 下载完成 ✓ {} ({:.1} KB)", src.ip(), fname, file_size as f64 / 1024.0),
                                    "type": "success"
                                }));
                                let _ = app.emit("tftp-progress", serde_json::json!({
                                    "ip": src.ip().to_string(), "bytes": file_size, "total": file_size, "done": true
                                }));
                            } else {
                                let chunk_end = std::cmp::min(offset + block_size as usize, file_data.len());
                                let chunk = file_data[offset..chunk_end].to_vec();
                                let mut pkt = vec![0u8; 4 + chunk.len()];
                                pkt[0] = 0; pkt[1] = 3;
                                pkt[2] = (next_block >> 8) as u8;
                                pkt[3] = next_block as u8;
                                pkt[4..].copy_from_slice(&chunk);
                                if let Err(e) = socket.send_to(&pkt, src).await {
                                    let _ = app.emit("tftp-log", serde_json::json!({ "msg": format!("发送失败: {}", e), "type": "error" }));
                                    clients.remove(&client_key);
                                } else {
                                    clients.insert(client_key.clone(), (fname.clone(), file_data.clone(), next_block));
                                    let bytes_sent = offset as u64 + chunk.len() as u64;
                                    let _ = app.emit("tftp-progress", serde_json::json!({
                                        "ip": src.ip().to_string(), "bytes": bytes_sent.min(file_size), "total": file_size, "done": false
                                    }));
                                }
                            }
                        }
                    }
                }
                2 => { // WRQ - 写请求（设备上传文件）
                    let end = buf[2..n].iter().position(|&b| b == 0).unwrap_or(n - 2);
                    let req_name = String::from_utf8_lossy(&buf[2..2 + end]);
                    let safe_name = std::path::Path::new(req_name.as_ref())
                        .file_name().map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| req_name.to_string());

                    let _ = app.emit("tftp-log", serde_json::json!({
                        "msg": format!("{} → WRQ 上传请求: {}", src.ip(), req_name),
                        "type": "info"
                    }));

                    let save_path = base_dir.join(&*safe_name);

                    let save_path_clone = save_path.clone();
                    let app_clone = app.clone();
                    let src_clone = src;

                    // 发送 ACK 0 表示准备好接收
                    let ack = [0u8, 4, 0, 0];
                    let _ = socket.send_to(&ack, src).await;

                    // 接收文件
                    tokio::spawn(async move {
                        let recv_socket = match UdpSocket::bind("0.0.0.0:0").await {
                            Ok(s) => s,
                            Err(e) => {
                                let _ = app_clone.emit("tftp-log", serde_json::json!({ "msg": format!("创建接收socket失败: {}", e), "type": "error" }));
                                return;
                            }
                        };
                        let _ = recv_socket.connect(src_clone).await;

                        let mut file_buf: Vec<u8> = Vec::new();
                        let mut rcv_buf = vec![0u8; 516];
                        let mut expected_block = 1u16;

                        loop {
                            match tokio::time::timeout(
                                std::time::Duration::from_secs(5),
                                recv_socket.recv(&mut rcv_buf)
                            ).await {
                                Ok(Ok(n)) => {
                                    if n < 4 { break; }
                                    let recv_block = u16::from_be_bytes([rcv_buf[2], rcv_buf[3]]);
                                    if recv_block == expected_block {
                                        file_buf.extend_from_slice(&rcv_buf[4..n]);
                                        expected_block = expected_block.wrapping_add(1);
                                        // ACK
                                        let ack = [0u8, 4, rcv_buf[2], rcv_buf[3]];
                                        let _ = recv_socket.send(&ack).await;
                                        let _ = app_clone.emit("tftp-progress", serde_json::json!({
                                            "ip": src_clone.ip().to_string(),
                                            "bytes": file_buf.len() as u64,
                                            "total": 0,
                                            "done": false
                                        }));
                                        // 最后一个数据包 < 512 表示传输结束
                                        if n < 516 {
                                            break;
                                        }
                                    }
                                }
                                _ => break,
                            }
                        }

                        match fs::write(&save_path, &file_buf).await {
                            Ok(_) => {
                                let _ = app_clone.emit("tftp-log", serde_json::json!({
                                    "msg": format!("{} → 上传完成 ✓ 保存至: {} ({} B)", src_clone.ip(), save_path_clone.display(), file_buf.len()),
                                    "type": "success"
                                }));
                                let _ = app_clone.emit("tftp-progress", serde_json::json!({
                                    "ip": src_clone.ip().to_string(),
                                    "bytes": file_buf.len() as u64,
                                    "total": file_buf.len() as u64,
                                    "done": true
                                }));
                            }
                            Err(e) => {
                                let _ = app_clone.emit("tftp-log", serde_json::json!({ "msg": format!("保存文件失败: {}", e), "type": "error" }));
                            }
                        }
                    });
                }
                _ => {}
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub fn stop_tftp_server() -> Result<(), String> {
    if !TFTP_RUNNING.swap(false, Ordering::SeqCst) {
        return Err("TFTP 服务未在运行".into());
    }
    Ok(())
}

// ============================================================
// Syslog 接收器
// ============================================================

static SYSLOG_RUNNING: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub async fn start_syslog_server(
    app: tauri::AppHandle,
    port: Option<u16>,
) -> Result<(), String> {
    if SYSLOG_RUNNING.swap(true, Ordering::SeqCst) {
        return Err("Syslog 服务已在运行中".into());
    }

    let port = port.unwrap_or(514);

    let socket = bind_privileged_port(port).await.map_err(|e| {
        SYSLOG_RUNNING.store(false, Ordering::SeqCst);
        e
    })?;

    tracing::info!("[syslog] 启动监听, 端口: {}", port);
    let _ = app.emit("syslog-log", serde_json::json!({
        "msg": format!("Syslog 服务已启动，监听端口 {}", port),
        "type": "info"
    }));

    tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        loop {
            if !SYSLOG_RUNNING.load(Ordering::SeqCst) {
                let _ = app.emit("syslog-log", serde_json::json!({ "msg": "Syslog 服务已停止", "type": "info" }));
                break;
            }

            let (n, src) = match tokio::time::timeout(
                std::time::Duration::from_secs(1), socket.recv_from(&mut buf)
            ).await {
                Ok(Ok(v)) => v,
                _ => continue,
            };

            let raw = String::from_utf8_lossy(&buf[..n]);
            let ts = chrono::Local::now().format("%H:%M:%S").to_string();

            let _ = app.emit("syslog-msg", serde_json::json!({
                "time": ts,
                "ip": src.ip().to_string(),
                "msg": raw.trim().to_string(),
            }));
        }
    });

    Ok(())
}

#[tauri::command]
pub fn stop_syslog_server() -> Result<(), String> {
    if !SYSLOG_RUNNING.swap(false, Ordering::SeqCst) {
        return Err("Syslog 服务未在运行".into());
    }
    Ok(())
}
