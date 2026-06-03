use serde::Serialize;
use std::net::Ipv4Addr;
use std::sync::Arc;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize)]
pub struct LiveHostResult {
    pub ip: String,
    pub alive: bool,
    pub response_time_ms: Option<f64>,
}

fn time_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"time[=<](\d+\.?\d*)\s*ms").unwrap())
}

#[cfg(target_os = "windows")]
fn build_ping_cmd(ip: &str, timeout_secs: u64) -> std::process::Command {
    let mut cmd = std::process::Command::new("ping");
    cmd.args(["-n", "1", "-w", &(timeout_secs * 1000).to_string(), ip]);
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

    let alive = output.status.success();
    let stdout = String::from_utf8_lossy(&output.stdout);

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

fn parse_cidr(subnet: &str) -> Result<Vec<Ipv4Addr>, String> {
    let parts: Vec<&str> = subnet.split('/').collect();
    if parts.len() != 2 {
        return Err("无效的CIDR格式，示例: 192.168.1.0/24".into());
    }
    let base = parts[0].trim().parse::<Ipv4Addr>()
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
        ips.push(Ipv4Addr::from(i));
    }
    Ok(ips)
}

pub async fn scan_subnet(subnet: &str, timeout_ms: u64) -> Result<Vec<LiveHostResult>, String> {
    let ips = parse_cidr(subnet)?;
    let timeout_secs = (timeout_ms as f64 / 1000.0).ceil() as u64;
    let max_concurrent = 80usize;
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
    let mut handles = Vec::with_capacity(ips.len());

    for ip in ips {
        let sem = semaphore.clone();
        let ip_str = ip.to_string();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await;
            let ip = ip_str;
            tokio::task::spawn_blocking(move || ping_one(&ip, timeout_secs)).await.unwrap()
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for h in handles {
        results.push(h.await.unwrap());
    }
    results.sort_by_key(|r| {
        let mut parts = r.ip.split('.').map(|s| s.parse::<u32>().unwrap_or(0));
        (parts.next().unwrap_or(0) << 24)
            | (parts.next().unwrap_or(0) << 16)
            | (parts.next().unwrap_or(0) << 8)
            | parts.next().unwrap_or(0)
    });
    Ok(results)
}
