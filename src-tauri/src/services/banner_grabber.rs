use serde::Serialize;
use std::time::Duration;
use std::io::{Read, Write};
use std::net::{TcpStream, Shutdown};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct BannerResult {
    pub port: u16,
    pub service: String,
    pub product: String,
    pub version: String,
    pub banner: String,
}

/// 常见端口对应的默认探测方式
fn probe_for_port(port: u16) -> &'static [u8] {
    match port {
        22 => b"SSH-2.0-OpenSSH_Check\r\n",
        80 => b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n",
        443 => b"",  // TLS 需要特殊处理，跳过
        21 => b"",
        23 => b"",
        25 => b"EHLO check\r\n",
        110 => b"",
        143 => b"",
        3306 => b"",
        5432 => b"",
        6379 => b"PING\r\n",
        27017 => b"",
        8080 => b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n",
        8443 => b"",
        9090 => b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n",
        _ => b"",
    }
}

/// 尝试获取端口 banner 并识别产品版本。
/// 超时 5 秒，读取最多 2048 字节。
pub fn grab_banner(ip: &str, port: u16) -> Option<BannerResult> {
    let addr = format!("{}:{}", ip, port);
    let timeout = Duration::from_secs(5);

    let mut stream = TcpStream::connect_timeout(&addr.parse().ok()?, timeout).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;

    // 发送探测
    let probe = probe_for_port(port);
    if !probe.is_empty() {
        stream.write_all(probe).ok()?;
        stream.flush().ok()?;
        // HTTP 需要多等一会让响应回来，尤其网络延迟时
        let wait = if port == 80 || port == 8080 || port == 443 || port == 8443 { 2000 } else { 800 };
        std::thread::sleep(Duration::from_millis(wait));
    }

    // 读取返回（尝试多次读取以获取完整响应）
    let mut buf = vec![0u8; 4096];
    let mut total_read = 0usize;
    for _ in 0..3 {
        match stream.read(&mut buf[total_read..]) {
            Ok(0) => break,
            Ok(n) => { total_read += n; if total_read >= buf.len() { break; } }
            Err(_) => break,
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if total_read == 0 {
        return None;
    }
    buf.truncate(total_read);

    // 关闭连接
    stream.shutdown(Shutdown::Both).ok();

    let raw = String::from_utf8_lossy(&buf).to_string();
    let banner = raw.chars().filter(|c| c.is_ascii_graphic() || *c == ' ' || *c == '\n' || *c == '\r').take(500).collect::<String>()
        .lines().next().unwrap_or("").trim().to_string();

    if banner.is_empty() {
        return None;
    }

    // 从 banner 识别产品名和版本
    let (product, version) = detect_product_version(port, &banner);

    let service = match port {
        22 => "SSH",
        21 => "FTP",
        23 => "Telnet",
        25 => "SMTP",
        80 | 8080 | 9090 => "HTTP",
        443 | 8443 => "HTTPS",
        3306 => "MySQL",
        5432 => "PostgreSQL",
        6379 => "Redis",
        27017 => "MongoDB",
        110 => "POP3",
        143 => "IMAP",
        3389 => "RDP",
        5900 => "VNC",
        _ => "Unknown",
    }.to_string();

    Some(BannerResult {
        port,
        service,
        product,
        version: version.unwrap_or_default(),
        banner,
    })
}

/// 从 banner 文本中提取产品名和版本号
fn detect_product_version(port: u16, banner: &str) -> (String, Option<String>) {
    let lower = banner.to_lowercase();

    match port {
        22 => {
            // SSH-2.0-OpenSSH_8.0p1 Ubuntu
            if let Some(rest) = lower.strip_prefix("ssh-") {
                let parts: Vec<&str> = rest.split('-').collect();
                if parts.len() >= 2 {
                    let product = if parts[1].contains("openssh") { "OpenSSH".to_string() } else { parts[1].to_string() };
                    // Extract version: "8.0p1" from "2.0-openssh_8.0p1"
                    let ver = parts.get(1).and_then(|s| {
                        s.split(|c: char| !c.is_alphanumeric() && c != '.' && c != 'p')
                            .find(|p| p.contains('.') || p.starts_with(|c: char| c.is_ascii_digit()))
                    }).map(|s| s.to_string());
                    return (product, ver);
                }
            }
            ("SSH".to_string(), None)
        }
        80 | 8080 | 9090 => {
            // HTTP Server header
            for line in banner.lines() {
                let l = line.to_lowercase();
                if l.starts_with("server:") {
                    let val = l.trim_start_matches("server:").trim();
                    // Extract product and version
                    let parts: Vec<&str> = val.split('/').collect();
                    let product = parts.first().map(|s| s.to_string()).unwrap_or("HTTP".to_string());
                    // Capitalize first letter
                    let product = capitalize(&product);
                    let version = parts.get(1).map(|s| {
                        s.split_whitespace().next().unwrap_or("").to_string()
                    }).filter(|s| !s.is_empty());
                    return (product, version);
                }
            }
            ("HTTP".to_string(), None)
        }
        3306 => {
            // MySQL: often sends version in greeting
            // "\x4a\x00\x00\x00\x0a\x38\x2e\x30\x2e\x33\x36\x00" = "8.0.36"
            if let Some(ver) = find_version_pattern(banner) {
                return ("MySQL".to_string(), Some(ver));
            }
            ("MySQL".to_string(), None)
        }
        6379 => {
            // Redis: "+PONG" or "-ERR"
            if banner.contains("+PONG") || banner.contains("+OK") || banner.contains("-ERR") || banner.contains("-NOAUTH") {
                return ("Redis".to_string(), None);
            }
            ("Redis".to_string(), None)
        }
        _ => {
            // 尝试通用版本号匹配
            let ver = find_version_pattern(banner);
            (capitalize(&banner.split_whitespace().next().unwrap_or("Unknown").to_string()), ver)
        }
    }
}

fn find_version_pattern(text: &str) -> Option<String> {
    // 匹配常见版本号模式: X.Y or X.Y.Z or X.YpZ or X.Y.ZpW
    for word in text.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '-' && c != 'p');
        if clean.len() >= 3 && clean.contains('.') {
            let starts_with_num = clean.starts_with(|c: char| c.is_ascii_digit());
            let has_two_dots = clean.matches('.').count() >= 2;
            if starts_with_num && (has_two_dots || clean.len() <= 8) {
                return Some(clean.to_string());
            }
        }
    }
    None
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    }
}
