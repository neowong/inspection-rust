use serde::Serialize;
use std::collections::HashMap;
use std::sync::OnceLock;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Default)]
pub struct PortScanResult {
    pub port: u16,
    pub open: bool,
    pub service: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UdpPortResult {
    pub port: u16,
    pub open: bool,         // true = got response data
    pub filtered: bool,     // true = no response (could be open or filtered)
    pub service: String,
    pub detail: String,     // "响应数据" | "端口关闭(ICMP)" | "无响应(开放或被过滤)"
    pub error: Option<String>,
}

// ============================================================================
// Service names
// ============================================================================

fn common_ports() -> &'static HashMap<u16, &'static str> {
    static MAP: OnceLock<HashMap<u16, &str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            (21, "FTP"),
            (22, "SSH"),
            (23, "Telnet"),
            (25, "SMTP"),
            (53, "DNS"),
            (80, "HTTP"),
            (110, "POP3"),
            (111, "RPC"),
            (135, "MSRPC"),
            (139, "NetBIOS"),
            (143, "IMAP"),
            (161, "SNMP"),
            (443, "HTTPS"),
            (445, "SMB"),
            (993, "IMAPS"),
            (995, "POP3S"),
            (1433, "MSSQL"),
            (1521, "Oracle"),
            (3306, "MySQL"),
            (3389, "RDP"),
            (5432, "PostgreSQL"),
            (6379, "Redis"),
            (8000, "HTTP-Alt"),
            (8080, "HTTP-Alt"),
            (8443, "HTTPS-Alt"),
            (9090, "Web-Alt"),
            (27017, "MongoDB"),
            (5000, "HTTP-Alt"),
        ])
    })
}

fn service_name(port: u16) -> String {
    common_ports()
        .get(&port)
        .map(|s| s.to_string())
        .unwrap_or_default()
}

// ============================================================================
// TCP Scanner
// ============================================================================

fn scan_one(ip: &str, port: u16, timeout: std::time::Duration) -> PortScanResult {
    let addr = format!("{}:{}", ip, port);
    let sock_addr = match addr.parse::<std::net::SocketAddr>() {
        Ok(a) => a,
        Err(_) => return PortScanResult { port, open: false, service: service_name(port), error: None },
    };
    let open = std::net::TcpStream::connect_timeout(&sock_addr, timeout).is_ok();
    PortScanResult {
        port,
        open,
        service: service_name(port),
        error: None,
    }
}

fn parse_ports(input: &str) -> Result<Vec<u16>, String> {
    let mut ports = Vec::new();
    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part.contains('-') {
            let range: Vec<&str> = part.split('-').collect();
            if range.len() != 2 {
                return Err(format!("无效的端口范围: {}", part));
            }
            let start: u16 = range[0].trim().parse()
                .map_err(|_| format!("无效的端口号: {}", range[0]))?;
            let end: u16 = range[1].trim().parse()
                .map_err(|_| format!("无效的端口号: {}", range[1]))?;
            if start > end {
                return Err(format!("端口范围起始大于结束: {}-{}", start, end));
            }
            if end - start > 5000 {
                return Err("端口范围过大，最多5000个端口".into());
            }
            for p in start..=end {
                ports.push(p);
            }
        } else {
            let p: u16 = part.parse()
                .map_err(|_| format!("无效的端口号: {}", part))?;
            ports.push(p);
        }
    }
    if ports.is_empty() {
        return Err("请输入要扫描的端口".into());
    }
    ports.sort();
    ports.dedup();
    Ok(ports)
}

pub async fn scan_ports(ip: &str, ports_input: &str, timeout_ms: u64) -> Result<Vec<PortScanResult>, String> {
    scan_ports_with_callback(ip, ports_input, timeout_ms, |_| {}).await
}

/// 扫描端口，每完成一个端口立即调用回调函数（用于实时推送结果）
pub async fn scan_ports_with_callback<F>(ip: &str, ports_input: &str, timeout_ms: u64, callback: F) -> Result<Vec<PortScanResult>, String>
where
    F: Fn(&PortScanResult) + Send + Sync + 'static,
{
    if ip.trim().is_empty() || ip.trim().parse::<std::net::IpAddr>().is_err() {
        return Err("请输入有效的 IP 地址".into());
    }
    let ports = parse_ports(ports_input)?;
    let timeout = std::time::Duration::from_millis(timeout_ms);
    let max_concurrent = 500usize;
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrent));
    let callback = std::sync::Arc::new(callback);
    let mut handles: Vec<(u16, tokio::task::JoinHandle<PortScanResult>)> = Vec::with_capacity(ports.len());

    let ip = ip.to_string();
    for &port in &ports {
        let ip = ip.clone();
        let sem = semaphore.clone();
        let cb = callback.clone();
        let h = tokio::spawn(async move {
            let _permit = sem.acquire().await;
            let result = match tokio::task::spawn_blocking(move || scan_one(&ip, port, timeout)).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("端口扫描任务 panic (port={}): {}", port, e);
                    PortScanResult { port, open: false, service: service_name(port), error: Some(e.to_string()) }
                }
            };
            cb(&result);
            result
        });
        handles.push((port, h));
    }

    let mut results = Vec::with_capacity(handles.len());
    for (port, h) in handles {
        results.push(match h.await {
            Ok(r) => r,
            Err(e) => { tracing::warn!("扫描任务异常退出 (port={}): {}", port, e); PortScanResult { port, open: false, service: service_name(port), error: Some(e.to_string()) } }
        });
    }
    results.sort_by_key(|r| r.port);
    Ok(results)
}

// ============================================================================
// UDP Scanner
// ============================================================================

fn udp_probe_payload(port: u16) -> Vec<u8> {
    match port {
        53 => {
            // DNS query for "." (root), type A, class IN
            vec![
                0x00, 0x00, // transaction ID
                0x01, 0x00, // flags: standard query
                0x00, 0x01, // questions: 1
                0x00, 0x00, // answers: 0
                0x00, 0x00, // authority: 0
                0x00, 0x00, // additional: 0
                0x00,       // name: root (empty label = root)
                0x00, 0x01, // type A
                0x00, 0x01, // class IN
            ]
        }
        161 => {
            // SNMP v1 GET for sysDescr (1.3.6.1.2.1.1.1.0), community "public"
            // Pre-encoded: 30 26 02 01 00 04 06 70 75 62 6c 69 63 a0 19 02 ...
            vec![
                0x30, 0x26, 0x02, 0x01, 0x00, 0x04, 0x06, 0x70, 0x75, 0x62,
                0x6c, 0x69, 0x63, 0xa0, 0x19, 0x02, 0x01, 0x01, 0x02, 0x01,
                0x00, 0x02, 0x01, 0x00, 0x30, 0x0e, 0x30, 0x0c, 0x06, 0x08,
                0x2b, 0x06, 0x01, 0x02, 0x01, 0x01, 0x01, 0x00, 0x05, 0x00,
            ]
        }
        123 => {
            // NTP version 3 client request (48 bytes)
            let mut pkt = vec![0u8; 48];
            pkt[0] = 0x1b; // LI=0, VN=3, Mode=3 (client)
            pkt
        }
        _ => {
            vec![0x00] // single null byte probe
        }
    }
}

fn udp_service_name(port: u16) -> String {
    match port {
        53 => "DNS".into(),
        67 => "DHCP".into(),
        68 => "DHCP".into(),
        69 => "TFTP".into(),
        123 => "NTP".into(),
        161 => "SNMP".into(),
        162 => "SNMP Trap".into(),
        389 => "LDAP".into(),
        514 => "Syslog".into(),
        623 => "IPMI".into(),
        1194 => "OpenVPN".into(),
        1812 => "RADIUS".into(),
        1813 => "RADIUS".into(),
        1900 => "SSDP".into(),
        2049 => "NFS".into(),
        5353 => "mDNS".into(),
        5683 => "CoAP".into(),
        _ => String::new(),
    }
}

fn scan_udp_one(ip: &str, port: u16, timeout: std::time::Duration) -> UdpPortResult {
    let service = udp_service_name(port);
    let addr = format!("{}:{}", ip, port);

    let socket = match std::net::UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => {
            return UdpPortResult {
                port, open: false, filtered: true, service,
                detail: "无法创建套接字".into(),
                error: None,
            }
        }
    };

    // connect() is essential: the kernel only delivers ICMP Port Unreachable
    // errors to connected UDP sockets, translating them to ECONNREFUSED on recv()
    if socket.connect(&addr).is_err() {
        return UdpPortResult {
            port, open: false, filtered: true, service,
            detail: "连接失败".into(),
            error: None,
        };
    }
    socket.set_read_timeout(Some(timeout)).ok();

    let probe = udp_probe_payload(port);

    // Send the probe (use send() on connected socket, not send_to())
    if socket.send(&probe).is_err() {
        return UdpPortResult {
            port, open: false, filtered: true, service,
            detail: "发送失败".into(),
            error: None,
        };
    }

    // Try to receive a response (or ICMP error via ECONNREFUSED)
    let mut buf = [0u8; 2048];
    match socket.recv(&mut buf) {
        Ok(len) => {
            // Got data → port is definitely open
            let detail = if port == 53 && len >= 12 {
                format!("DNS 响应 ({} 字节)", len)
            } else if port == 161 && len >= 2 {
                format!("SNMP 响应 ({} 字节)", len)
            } else if port == 123 && len >= 48 {
                "NTP 响应".into()
            } else {
                format!("响应数据 ({} 字节)", len)
            };
            UdpPortResult { port, open: true, filtered: false, service, detail, error: None }
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            // ICMP Port Unreachable delivered via connected socket → definitely closed
            UdpPortResult { port, open: false, filtered: false, service, detail: "端口关闭 (ICMP)".into(), error: None }
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
            // No response, no ICMP error → port is open (service didn't respond to probe)
            // or very rarely filtered by a firewall that silently drops
            UdpPortResult { port, open: false, filtered: true, service, detail: "开放 (服务未响应探针)".into(), error: None }
        }
        Err(e) => {
            UdpPortResult { port, open: false, filtered: true, service, detail: format!("错误: {}", e), error: None }
        }
    }
}

pub async fn scan_udp_ports(ip: &str, ports_input: &str, timeout_ms: u64) -> Result<Vec<UdpPortResult>, String> {
    scan_udp_ports_with_callback(ip, ports_input, timeout_ms, |_| {}).await
}

/// 扫描 UDP 端口，每完成一个端口立即调用回调函数（用于实时推送结果）
pub async fn scan_udp_ports_with_callback<F>(ip: &str, ports_input: &str, timeout_ms: u64, callback: F) -> Result<Vec<UdpPortResult>, String>
where
    F: Fn(&UdpPortResult) + Send + Sync + 'static,
{
    if ip.trim().is_empty() || ip.trim().parse::<std::net::IpAddr>().is_err() {
        return Err("请输入有效的 IP 地址".into());
    }
    let ports = parse_ports(ports_input)?;
    let timeout = std::time::Duration::from_millis(timeout_ms);
    let max_concurrent = 300usize;
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrent));
    let callback = std::sync::Arc::new(callback);
    let mut handles: Vec<(u16, tokio::task::JoinHandle<UdpPortResult>)> = Vec::with_capacity(ports.len());

    let ip = ip.to_string();
    for &port in &ports {
        let ip = ip.clone();
        let sem = semaphore.clone();
        let cb = callback.clone();
        let h = tokio::spawn(async move {
            let _permit = sem.acquire().await;
            let result = match tokio::task::spawn_blocking(move || scan_udp_one(&ip, port, timeout)).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("UDP 扫描任务 panic (port={}): {}", port, e);
                    UdpPortResult { port, open: false, filtered: true, service: udp_service_name(port), detail: String::new(), error: Some(e.to_string()) }
                }
            };
            cb(&result);
            result
        });
        handles.push((port, h));
    }

    let mut results = Vec::with_capacity(handles.len());
    for (port, h) in handles {
        results.push(match h.await {
            Ok(r) => r,
            Err(e) => { tracing::warn!("UDP 扫描任务异常退出 (port={}): {}", port, e); UdpPortResult { port, open: false, filtered: true, service: udp_service_name(port), detail: String::new(), error: Some(e.to_string()) } }
        });
    }
    results.sort_by_key(|r| r.port);
    Ok(results)
}
