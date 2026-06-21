use serde::Serialize;
use regex::Regex;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

#[derive(Debug, Clone, Serialize)]
pub struct LiveHostResult {
    pub ip: String,
    pub alive: bool,
    pub response_time_ms: Option<f64>,
}

/// TCP probe ports as ICMP fallback. Windows 135 (RPC) is nearly always
/// listening + reachable on LAN; 445 (SMB) is common on domain/office networks.
const TCP_FALLBACK_PORTS: &[u16] = &[135, 445];

// ---- ICMP Ping -------------------------------------------------------------

fn time_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"time[=<](\d+\.?\d*)\s*ms").unwrap())
}

#[cfg(target_os = "windows")]
fn build_ping_cmd(ip: &str, timeout_secs: u64) -> std::process::Command {
    let mut cmd = std::process::Command::new("ping");
    cmd.args(["-n", "1", "-w", &(timeout_secs * 1000).to_string(), ip]);
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(target_os = "windows"))]
fn build_ping_cmd(ip: &str, timeout_secs: u64) -> std::process::Command {
    let mut cmd = std::process::Command::new("ping");
    cmd.args(["-c", "1", "-W", &timeout_secs.to_string(), ip]);
    cmd
}

fn ping_one(ip: &str, timeout_secs: u64) -> LiveHostResult {
    let output = match build_ping_cmd(ip, timeout_secs)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
    {
        Ok(o) => o,
        Err(_e) => {
            return LiveHostResult {
                ip: ip.to_string(),
                alive: false,
                response_time_ms: None,
            };
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    let alive = stdout.contains("TTL=")
        || stdout.contains("ttl=")
        || stdout.contains("time=")
        || stdout.contains("time<");

    let response_time_ms = if alive {
        time_regex()
            .captures(&stdout)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<f64>().ok())
    } else {
        None
    };

    LiveHostResult {
        ip: ip.to_string(),
        alive,
        response_time_ms,
    }
}

// ---- TCP fallback (internal, no UI) ----------------------------------------
// When ping fails (ICMP blocked), try a quick TCP connect to common Windows
// ports. 135 (RPC) is nearly always listening on any Windows machine.
// Connection refused (RST) means the host is alive; timeout means dead.

async fn tcp_fallback(ip: &str, per_port_ms: u64) -> bool {
    let semaphore = Arc::new(tokio::sync::Semaphore::new(10));
    let mut handles = Vec::with_capacity(TCP_FALLBACK_PORTS.len());

    for &port in TCP_FALLBACK_PORTS {
        let sem = semaphore.clone();
        let addr = match format!("{}:{}", ip, port).parse::<SocketAddr>() {
            Ok(a) => a,
            Err(_) => continue,
        };
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await;
            timeout(Duration::from_millis(per_port_ms), TcpStream::connect(addr))
                .await
                .ok()
                .and_then(|r| r.ok())
                .is_some()
        }));
    }

    for h in handles {
        if h.await.unwrap_or(false) {
            return true;
        }
    }
    false
}

pub async fn check_alive(ip: &str, timeout_secs: u64) -> LiveHostResult {
    // 1. ICMP ping
    let r = tokio::task::spawn_blocking({
        let ip = ip.to_string();
        move || ping_one(&ip, timeout_secs)
    })
    .await
    .unwrap_or(LiveHostResult {
        ip: ip.to_string(),
        alive: false,
        response_time_ms: None,
    });
    if r.alive {
        return r;
    }

    // 2. TCP fallback (ICMP likely blocked)
    let tcp_alive = tcp_fallback(ip, timeout_secs * 1000).await;
    LiveHostResult {
        ip: ip.to_string(),
        alive: tcp_alive,
        response_time_ms: None,
    }
}

// ---- CIDR parser -----------------------------------------------------------

pub fn parse_cidr(subnet: &str) -> Result<Vec<std::net::Ipv4Addr>, String> {
    let parts: Vec<&str> = subnet.split('/').collect();
    if parts.len() != 2 {
        return Err("无效的CIDR格式，示例: 192.168.1.0/24".into());
    }
    let base = parts[0].trim().parse::<std::net::Ipv4Addr>()
        .map_err(|_| format!("无效的IP地址: {}", parts[0]))?;
    let prefix: u8 = parts[1].trim().parse()
        .map_err(|_| format!("无效的CIDR前缀: {}", parts[1]))?;
    if prefix > 32 {
        return Err("CIDR前缀必须在0-32之间".into());
    }
    if prefix < 16 {
        return Err(format!("子网太大 (/{}), 请使用 /16 或更小的前缀", prefix));
    }

    let base_u32 = u32::from(base);
    let mask = if prefix == 0 { 0 } else { (!0u32).checked_shl(32 - prefix as u32).unwrap_or(0) };
    let network = base_u32 & mask;
    let range_end = if prefix >= 31 {
        network.wrapping_add(1u32 << (32 - prefix as u32))
    } else {
        network.wrapping_add((1u32 << (32 - prefix as u32)) - 1)
    };
    let start = if prefix >= 31 { network } else { network.wrapping_add(1) };

    let mut ips = Vec::new();
    for i in start..range_end {
        ips.push(std::net::Ipv4Addr::from(i));
    }
    Ok(ips)
}

// ---- Public API ------------------------------------------------------------
// scan_subnet removed: 实时推送版在 commands/tools.rs::scan_live_hosts 中实现
// 底层函数 parse_cidr / check_alive 仍为 pub，供其他模块复用
