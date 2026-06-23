use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ZabbixAgentResult {
    pub reachable: bool,
    pub version: Option<String>,
    pub hostname: Option<String>,
    pub ping_ok: bool,
    pub active_mode_note: String,
    pub response_time_ms: u64,
    pub error: Option<String>,
}

fn build_zabbix_frame(json: &str) -> Vec<u8> {
    let header = b"ZBXD\x01";
    let payload = json.as_bytes();
    let len = payload.len() as u64;
    let mut frame = Vec::with_capacity(5 + 8 + payload.len());
    frame.extend_from_slice(header);
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(payload);
    frame
}

fn parse_zabbix_response(data: &[u8]) -> Option<String> {
    if data.len() < 13 { return None; }
    if &data[..5] != b"ZBXD\x01" { return None; }
    let payload_len = u64::from_le_bytes(data[5..13].try_into().ok()?) as usize;
    if data.len() < 13 + payload_len { return None; }
    let json_str = std::str::from_utf8(&data[13..13 + payload_len]).ok()?;
    Some(json_str.to_string())
}

fn hex_preview(data: &[u8], max: usize) -> String {
    data.iter().take(max)
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

fn read_all_with_timeout(stream: &mut std::net::TcpStream, timeout: std::time::Duration) -> Result<Vec<u8>, String> {
    stream.set_read_timeout(Some(timeout)).ok();
    let mut buf = vec![0u8; 65536];
    let mut total = 0;

    // Read header (13 bytes)
    while total < 13 {
        match std::io::Read::read(stream, &mut buf[total..13]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                break;
            }
            Err(e) => return Err(format!("读取响应失败: {}", e)),
        }
    }

    if total < 13 {
        if total == 0 {
            return Err("无响应数据（服务未返回任何数据，可能不是Zabbix agent或启用了TLS加密）".into());
        }
        return Err(format!("响应头不完整 (仅收到 {} 字节): {}", total, hex_preview(&buf[..total], total)));
    }

    // Verify header
    if &buf[..4] != b"ZBXD" {
        return Err(format!("非Zabbix协议 (头部: {})", hex_preview(&buf[..total.min(32)], total.min(32))));
    }

    let payload_len = u64::from_le_bytes(buf[5..13].try_into().unwrap()) as usize;
    let expected_total = 13 + payload_len;

    // Read remaining payload bytes
    while total < expected_total {
        match std::io::Read::read(stream, &mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                break;
            }
            Err(e) => return Err(format!("读取响应负载失败: {}", e)),
        }
    }

    Ok(buf[..total].to_vec())
}

fn send_zabbix_request(ip: &str, port: u16, request_json: &str, timeout: std::time::Duration) -> Result<String, String> {
    let addr = format!("{}:{}", ip, port);
    let sock_addr: std::net::SocketAddr = addr.parse()
        .map_err(|e| format!("无效的地址: {}", e))?;
    let mut stream = std::net::TcpStream::connect_timeout(&sock_addr, timeout)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::TimedOut || e.kind() == std::io::ErrorKind::WouldBlock {
                "连接超时".into()
            } else {
                format!("连接失败: {}", e)
            }
        })?;

    stream.set_write_timeout(Some(timeout)).ok();

    let frame = build_zabbix_frame(request_json);
    std::io::Write::write_all(&mut stream, &frame)
        .map_err(|e| format!("发送失败: {}", e))?;

    let data = read_all_with_timeout(&mut stream, timeout)?;
    parse_zabbix_response(&data)
        .ok_or_else(|| {
            format!("响应格式无效 (收到 {} 字节): {}", data.len(), hex_preview(&data, data.len().min(64)))
        })
}

pub fn check_zabbix_agent(ip: &str, port: u16, timeout_ms: u64) -> ZabbixAgentResult {
    let timeout = std::time::Duration::from_millis(timeout_ms);
    let start = std::time::Instant::now();
    tracing::info!("Zabbix Agent 检测开始: ip={}, port={}, timeout={}ms", ip, port, timeout_ms);

    if ip.trim().is_empty() || ip.trim().parse::<std::net::IpAddr>().is_err() {
        return ZabbixAgentResult {
            reachable: false, version: None, hostname: None, ping_ok: false,
            active_mode_note: String::new(),
            response_time_ms: start.elapsed().as_millis() as u64,
            error: Some("请输入有效的 IP 地址".into()),
        };
    }

    // Step 1: TCP connect check
    let addr = format!("{}:{}", ip, port);
    let sock_addr = match addr.parse::<std::net::SocketAddr>() {
        Ok(a) => a,
        Err(_) => {
            return ZabbixAgentResult {
                reachable: false, version: None, hostname: None, ping_ok: false,
                active_mode_note: String::new(),
                response_time_ms: start.elapsed().as_millis() as u64,
                error: Some(format!("无效的IP地址: {}", ip)),
            };
        }
    };
    let reachable = std::net::TcpStream::connect_timeout(&sock_addr, timeout).is_ok();

    if !reachable {
        return ZabbixAgentResult {
            reachable: false,
            version: None,
            hostname: None,
            ping_ok: false,
            active_mode_note: "被动模式端口 10050 不可达，主动模式依赖 agent 主动出站连接，无法从外部检测".into(),
            response_time_ms: start.elapsed().as_millis() as u64,
            error: Some("TCP 端口不可达".into()),
        };
    }

    // Step 2: Try agent.ping
    let ping_json = r#"{"request":"agent.ping"}"#;
    let ping_result = send_zabbix_request(ip, port, ping_json, timeout);

    let elapsed = start.elapsed().as_millis() as u64;

    if let Err(ref e) = ping_result {
        let is_not_zabbix = e.contains("非Zabbix协议");
        return ZabbixAgentResult {
            reachable: true,
            version: None,
            hostname: None,
            ping_ok: false,
            active_mode_note: if is_not_zabbix {
                "端口可达但未返回 Zabbix 协议数据，可能不是 Zabbix agent".into()
            } else {
                "主动模式：agent 主动连接 server:10051，无法从外部被动检测".into()
            },
            response_time_ms: elapsed,
            error: Some(e.clone()),
        };
    }

    let ping_ok = ping_result
        .as_ref()
        .map(|r| r.contains(r#""response":"success""#))
        .unwrap_or(false);

    // Step 3: Get version
    let version = send_zabbix_request(ip, port, r#"{"request":"agent.version"}"#, timeout)
        .ok()
        .and_then(|r| {
            serde_json::from_str::<serde_json::Value>(&r).ok()
                .and_then(|v| v.get("version").cloned())
                .and_then(|v| v.as_str().map(String::from))
        });

    // Step 4: Get hostname
    let hostname = send_zabbix_request(ip, port, r#"{"request":"agent.hostname"}"#, timeout)
        .ok()
        .and_then(|r| {
            serde_json::from_str::<serde_json::Value>(&r).ok()
                .and_then(|v| v.get("hostname").cloned())
                .and_then(|v| v.as_str().map(String::from))
        });

    let result = ZabbixAgentResult {
        reachable: true,
        version,
        hostname,
        ping_ok,
        active_mode_note: if ping_ok {
            "主动模式：agent 主动连接 Zabbix server 的 10051 端口上报数据，需在 server 端检测 agent 注册状态".into()
        } else {
            "主动模式：agent 主动出站连接 server:10051，无法从外部被动检测".into()
        },
        response_time_ms: elapsed,
        error: None,
    };
    tracing::info!(
        "Zabbix Agent 检测完成: ip={}, port={}, reachable={}, ping_ok={}, version={:?}, latency={}ms",
        ip, port, result.reachable, result.ping_ok, result.version, elapsed
    );
    result
}
