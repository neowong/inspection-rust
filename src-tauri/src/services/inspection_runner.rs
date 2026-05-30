use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use ssh2::Session;

pub struct SSHSessionSource {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

/// 网络设备常用的旧版 SSH 算法配置
/// 确保能连接运行老旧固件的 H3C/华为/思科/锐捷设备
mod legacy_algorithms {
    /// 密钥交换算法（按优先级排列）
    /// 使用 + 前缀追加到默认列表，确保兼容老旧设备
    pub const KEX: &str = "+diffie-hellman-group1-sha1,diffie-hellman-group14-sha1,diffie-hellman-group-exchange-sha1";

    /// 加密算法
    /// 使用 + 前缀追加旧加密算法
    pub const CIPHERS: &str = "+aes128-cbc,aes192-cbc,aes256-cbc,3des-cbc";

    /// 消息认证码（MAC）算法
    /// 使用 + 前缀追加旧 MAC 算法
    pub const MACS: &str = "+hmac-sha1,hmac-sha1-96,hmac-md5";

    /// 主机密钥算法
    /// 使用 + 前缀追加旧主机密钥算法（ssh-dss 在 OpenSSH 10.x 已移除）
    pub const HOST_KEY: &str = "+ssh-rsa";

}

/// Execute commands on a network device via SSH using libssh2.
/// Returns a HashMap mapping each command to its output text.
pub fn run_commands(
    source: &SSHSessionSource,
    vendor: &str,
    commands: &[String],
    on_progress: Option<Arc<std::sync::Mutex<String>>>,
) -> Result<HashMap<String, String>, String> {
    tracing::info!(
        "SSH 开始: {}@{}:{}, 厂商={}, 命令数={}",
        source.username, source.host, source.port, vendor, commands.len()
    );

    run_commands_libssh2(source, vendor, commands, on_progress)
}

/// netmiko-style: connect to device, open ONE persistent shell channel,
/// detect the device prompt, then execute all commands through it.
/// Uses prompt detection instead of fixed-timeout heuristics to determine
/// when command output is complete.
fn run_commands_libssh2(
    source: &SSHSessionSource,
    vendor: &str,
    commands: &[String],
    on_progress: Option<Arc<std::sync::Mutex<String>>>,
) -> Result<HashMap<String, String>, String> {
    // 1. TCP connect + SSH handshake + authenticate
    let session = connect_session(source)?;

    // 2. Open interactive shell, find prompt, disable paging
    let (prompt, mut channel) = open_shell_session(&session, vendor, source.password.as_str(), &source.host)?;

    // 3. Execute each command through the persistent shell
    let host = &source.host;
    let mut results = HashMap::new();
    let mut consecutive_timeouts = 0u32;
    for (i, cmd) in commands.iter().enumerate() {
        if let Some(ref progress) = on_progress {
            if let Ok(mut guard) = progress.lock() {
                *guard = format!("[{}/{}] {}", i + 1, commands.len(), cmd);
            }
        }

        match send_command(&mut channel, cmd, &prompt, source.password.as_str(), host, vendor) {
            Ok(output) => {
                consecutive_timeouts = 0;
                if let Some(error_msg) = check_unrecognized_command(&output) {
                    tracing::info!("[{}] 跳过不支持的命令 '{}': {}", host, cmd, error_msg);
                    results.insert(format!("{} [不支持]", cmd), error_msg);
                } else {
                    results.insert(cmd.clone(), output);
                }
            }
            Err(e) => {
                let is_timeout = e.contains("超时");
                tracing::warn!("[{}] 命令 '{}' 执行失败: {}", host, cmd, e);
                results.insert(format!("{} [失败]", cmd), e);
                if is_timeout {
                    consecutive_timeouts += 1;
                    if consecutive_timeouts >= 2 {
                        tracing::error!("[{}] 连续 {} 次超时，放弃剩余 {} 条命令", host, consecutive_timeouts, commands.len() - i - 1);
                        for skipped in &commands[i + 1..] {
                            results.insert(format!("{} [跳过]", skipped), "设备无响应，已跳过".to_string());
                        }
                        break;
                    }
                }
            }
        }
    }

    // 4. Cleanup — send exit and close
    let _ = writeln!(channel, "exit");
    session.set_blocking(true);
    let _ = channel.wait_close();

    Ok(results)
}

// ---------------------------------------------------------------------------
// Session management
// ---------------------------------------------------------------------------

/// Establish TCP connection, SSH handshake, and password authentication.
fn connect_session(source: &SSHSessionSource) -> Result<Session, String> {
    let addr = format!("{}:{}", source.host, source.port)
        .to_socket_addrs()
        .map_err(|e| format!("地址解析失败: {}", e))?
        .next()
        .ok_or_else(|| "无法解析主机地址".to_string())?;

    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("TCP连接失败(10s超时): {}", e))?;

    let mut session = Session::new()
        .map_err(|e| format!("创建SSH会话失败: {}", e))?;

    session.set_banner("SSH-2.0-OpenSSH_8.0").ok();

    let _ = session.method_pref(ssh2::MethodType::Kex, legacy_algorithms::KEX);
    let _ = session.method_pref(ssh2::MethodType::CryptCs, legacy_algorithms::CIPHERS);
    let _ = session.method_pref(ssh2::MethodType::CryptSc, legacy_algorithms::CIPHERS);
    let _ = session.method_pref(ssh2::MethodType::MacCs, legacy_algorithms::MACS);
    let _ = session.method_pref(ssh2::MethodType::MacSc, legacy_algorithms::MACS);
    let _ = session.method_pref(ssh2::MethodType::HostKey, legacy_algorithms::HOST_KEY);

    session.set_tcp_stream(tcp);

    session
        .handshake()
        .map_err(|e| format!("SSH握手失败(可能需要启用旧算法): {}", e))?;

    session
        .userauth_password(&source.username, &source.password)
        .map_err(|e| format!("SSH密码认证失败: {}", e))?;

    if !session.authenticated() {
        return Err("SSH认证未通过".to_string());
    }

    tracing::info!("SSH 认证成功: {}@{}", source.username, source.host);
    Ok(session)
}

/// Open an interactive shell channel, detect the device prompt,
/// and run session preparation (disable paging).
/// Returns the detected prompt string and the open channel.
fn open_shell_session<'s>(
    session: &'s Session,
    vendor: &str,
    password: &str,
    host: &str,
) -> Result<(String, ssh2::Channel), String> {
    let mut channel = session
        .channel_session()
        .map_err(|e| format!("创建SSH通道失败: {}", e))?;

    channel
        .request_pty("xterm", None, None)
        .map_err(|e| format!("请求PTY失败: {}", e))?;

    channel
        .shell()
        .map_err(|e| format!("启动Shell失败: {}", e))?;

    // Switch to non-blocking for prompt detection
    session.set_blocking(false);

    let (prompt, full_prompt) = find_prompt(&mut channel)?;
    tracing::info!("检测到设备提示符: {:?}", full_prompt.trim());

    // Disable paging — must succeed; without it, subsequent commands
    // will hang at "---- More ----" pagination prompts.
    if let Some(disable_cmd) = get_disable_paging_cmd(vendor) {
        send_command(&mut channel, disable_cmd, &prompt, password, host, vendor)
            .map_err(|e| format!("分页禁用命令 '{}' 失败: {}", disable_cmd, e))?;
        tracing::info!("[{}] 已发送分页禁用命令: {}", host, disable_cmd);
    }

    Ok((prompt, channel))
}

// ---------------------------------------------------------------------------
// Prompt detection (netmiko-style read_until_prompt)
// ---------------------------------------------------------------------------

/// Read initial shell output until the device prompt is detected,
/// then continue draining until the channel is silent to ensure
/// all login banners and MOTD are fully consumed.
/// Timeout: 15 seconds.
fn find_prompt(channel: &mut ssh2::Channel) -> Result<(String, String), String> {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(15);
    let mut buffer = String::new();
    let mut buf = [0u8; 4096];
    let mut prompt: Option<String> = None;
    let mut full_prompt = String::new();
    let mut silent_rounds = 0u32;
    const SILENT_THRESHOLD: u32 = 5; // 5 consecutive WouldBlocks (~500ms of silence)

    loop {
        if start.elapsed() > timeout {
            return Err("等待设备提示符超时（15秒）".to_string());
        }

        match channel.read(&mut buf) {
            Ok(0) => {
                if prompt.is_some() {
                    silent_rounds += 1;
                    if silent_rounds >= SILENT_THRESHOLD {
                        return Ok((prompt.unwrap(), full_prompt.clone()));
                    }
                } else if let Some(p) = extract_prompt(&buffer) {
                    prompt = Some(p);
                    full_prompt = last_non_empty_line(&buffer).unwrap_or_default();
                    silent_rounds = 0;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Ok(n) => {
                silent_rounds = 0;
                buffer.push_str(&String::from_utf8_lossy(&buf[..n]));
                if let Some(p) = extract_prompt(&buffer) {
                    prompt = Some(p);
                    full_prompt = last_non_empty_line(&buffer).unwrap_or_default();
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if prompt.is_some() {
                    silent_rounds += 1;
                    if silent_rounds >= SILENT_THRESHOLD {
                        return Ok((prompt.unwrap(), full_prompt.clone()));
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                if prompt.is_some() {
                    silent_rounds += 1;
                    if silent_rounds >= SILENT_THRESHOLD {
                        return Ok((prompt.unwrap(), full_prompt.clone()));
                    }
                } else if let Some(p) = extract_prompt(&buffer) {
                    prompt = Some(p);
                    full_prompt = last_non_empty_line(&buffer).unwrap_or_default();
                    silent_rounds = 0;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(format!("读取提示符失败: {}", e)),
        }
    }
}

/// Get the last non-empty line from the buffer, stripped of ANSI and trimmed.
fn last_non_empty_line(buffer: &str) -> Option<String> {
    buffer
        .lines()
        .filter(|l| !l.trim().is_empty())
        .last()
        .map(|l| strip_ansi(l).trim().to_string())
}

/// Extract the base prompt from buffer.
/// Strips leading control characters and trailing terminator characters.
fn extract_prompt(buffer: &str) -> Option<String> {
    let last = buffer
        .lines()
        .filter(|l| !l.trim().is_empty())
        .last()?;

    let trimmed = strip_ansi(last).trim().to_string();
    let cleaned: String = trimmed.chars().skip_while(|c| c.is_control()).collect();

    // Strip known terminator characters
    let base = cleaned
        .strip_suffix('>')
        .or_else(|| cleaned.strip_suffix('#'))
        .or_else(|| cleaned.strip_suffix(']'))
        .or_else(|| cleaned.strip_suffix('$'));

    base.map(|s| s.to_string())
}

/// Check whether the last non-empty line of output matches the expected
/// prompt for this vendor. Uses vendor-specific patterns.
fn line_looks_like_prompt(line: &str, vendor: &str) -> bool {
    let line = line.trim();
    if line.is_empty() { return false; }
    match vendor.to_lowercase().as_str() {
        // H3C/Huawei: <hostname> or [hostname]
        "h3c" | "华三" | "huawei" | "华为" => {
            (line.starts_with('<') && line.ends_with('>'))
                || (line.starts_with('[') && line.ends_with(']'))
        }
        // Cisco/Ruijie: hostname# or hostname>
        "cisco" | "思科" | "ruijie" | "锐捷" => {
            line.ends_with('#') || line.ends_with('>')
        }
        // Fallback: any common terminator
        _ => line.ends_with('>') || line.ends_with('#') || line.ends_with(']') || line.ends_with('$'),
    }
}

/// Check whether the accumulated output ends with a line that looks like
/// a device prompt for the given vendor.
fn output_contains_prompt(output: &str, base_prompt: &str, vendor: &str) -> bool {
    if base_prompt.is_empty() && vendor.is_empty() {
        return false;
    }
    let cleaned = strip_ansi(output);
    cleaned
        .lines()
        .filter(|l| !l.trim().is_empty())
        .last()
        .map_or(false, |last| {
            let lt = last.trim();
            // Exact match via contains (handles prompt changes gracefully)
            if !base_prompt.is_empty() && lt.contains(base_prompt) {
                return true;
            }
            // Vendor-specific pattern fallback
            line_looks_like_prompt(lt, vendor)
        })
}

/// Read channel output until the device prompt appears at the end of the
/// accumulated output, signalling that the command has finished.
/// Handles password prompts (e.g., enable mode).
fn read_until_prompt(
    channel: &mut ssh2::Channel,
    prompt: &str,
    password: &str,
    host: &str,
    cmd: &str,
    vendor: &str,
) -> Result<String, String> {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(15);
    let mut output = String::new();
    let mut buf = [0u8; 4096];
    let mut password_sent = false;

    loop {
        if start.elapsed() > timeout {
            let cleaned = strip_ansi(&output);
            let last_line = cleaned.lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("(none)");
            let tail: String = output.chars().rev().take(120).collect::<String>().chars().rev().collect();
            let msg = format!(
                "[{}] 命令 '{}' 超时（15秒），已收到 {} 字节, base_prompt='{}', 最后一行='{}', 尾部: {}",
                host, cmd, output.len(), prompt, last_line.trim(), tail.trim()
            );
            tracing::warn!("{}", msg);
            return Err(msg);
        }

        match channel.read(&mut buf) {
            Ok(0) => {
                if channel.eof() {
                    return Ok(clean_output(&output, ""));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok(n) => {
                let text = String::from_utf8_lossy(&buf[..n]);
                output.push_str(&text);

                // Handle enable/super user password prompts
                if !password_sent
                    && text.contains("assword:")
                    && !password.is_empty()
                {
                    let _ = channel.write_all(password.as_bytes());
                    let _ = channel.write_all(b"\n");
                    password_sent = true;
                }

                if output_contains_prompt(&output, prompt, vendor) {
                    return Ok(clean_output(&output, prompt));
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                if output_contains_prompt(&output, prompt, vendor) {
                    return Ok(clean_output(&output, prompt));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(format!("读取命令输出失败: {}", e)),
        }
    }
}

// ---------------------------------------------------------------------------
// Command execution (netmiko-style send_command)
// ---------------------------------------------------------------------------

/// Vendor-specific paging disable command.
fn get_disable_paging_cmd(vendor: &str) -> Option<&str> {
    match vendor.to_lowercase().as_str() {
        "h3c" | "华三" | "huawei" | "华为" => Some("screen-length disable"),
        "cisco" | "思科" | "ruijie" | "锐捷" => Some("terminal length 0"),
        _ => None,
    }
}

/// Drain stale data from the channel until it goes silent for a short period.
fn clear_channel_buffer(channel: &mut ssh2::Channel) {
    let start = std::time::Instant::now();
    let max_drain = Duration::from_secs(2);
    let mut buf = [0u8; 4096];
    let mut silent_rounds = 0u32;
    const QUIET_THRESHOLD: u32 = 3; // 3 × 50ms = 150ms of silence

    loop {
        if start.elapsed() > max_drain {
            break;
        }
        match channel.read(&mut buf) {
            Ok(0) => {
                silent_rounds += 1;
                if silent_rounds >= QUIET_THRESHOLD { break; }
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok(_) => {
                silent_rounds = 0;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                silent_rounds += 1;
                if silent_rounds >= QUIET_THRESHOLD { break; }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => break,
        }
    }
}

/// Send a single command through the shell channel and return the cleaned output.
/// Implements netmiko's send_command flow: clear buffer → write command →
/// read until prompt → clean output.
fn send_command(
    channel: &mut ssh2::Channel,
    cmd: &str,
    prompt: &str,
    password: &str,
    host: &str,
    vendor: &str,
) -> Result<String, String> {
    clear_channel_buffer(channel);

    tracing::info!("[{}] 执行命令: {}", host, cmd);

    writeln!(channel, "{}", cmd)
        .map_err(|e| format!("[{}] 发送命令失败 '{}': {}", host, cmd, e))?;

    read_until_prompt(channel, prompt, password, host, cmd, vendor)
}

// ---------------------------------------------------------------------------
// Output cleaning
// ---------------------------------------------------------------------------

/// Remove ANSI escape sequences from a string.
fn strip_ansi(raw: &str) -> String {
    let mut cleaned = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.next() == Some('[') {
                while let Some(nc) = chars.next() {
                    if nc.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            cleaned.push(c);
        }
    }
    cleaned
}

/// Clean command output: strip ANSI codes, the echoed command line,
/// and the trailing device prompt.
/// `base_prompt` is the prompt without its terminator (netmiko-style).
fn clean_output(raw: &str, base_prompt: &str) -> String {
    let cleaned = strip_ansi(raw);
    let lines: Vec<&str> = cleaned.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    // Skip the first line if it looks like the echoed command
    // (e.g., "<H3C>display version", "Router#show version")
    let start = if !base_prompt.is_empty()
        && lines.len() > 1
        && lines[0].trim().contains(base_prompt.trim())
    {
        1
    } else {
        0
    };

    // Strip trailing prompt lines (containing base_prompt) and blank lines
    let mut end = lines.len();
    let bp = base_prompt.trim();
    while end > start {
        let last = lines[end - 1].trim();
        if last.is_empty() || last.contains(bp) {
            end -= 1;
        } else {
            break;
        }
    }

    if start >= end {
        return String::new();
    }

    lines[start..end].join("\n").trim().to_string()
}

/// Check if command output indicates the command is not recognized by the device.
/// Returns Some(error_message) if unrecognized, None otherwise.
fn check_unrecognized_command(output: &str) -> Option<String> {
    let output_lower = output.to_lowercase();

    // Common patterns for unrecognized commands across vendors
    let patterns = [
        "unrecognized command",
        "invalid input detected",
        "unknown command",
        "command not found",
        "% invalid",
        "% unrecognized",
        "syntax error",
        "incomplete command",
    ];

    for pattern in patterns {
        if output_lower.contains(pattern) {
            // Extract the error line for better reporting
            let error_line = output.lines()
                .find(|line| line.to_lowercase().contains(pattern))
                .unwrap_or(output);
            return Some(format!("设备不支持此命令: {}", error_line.trim()));
        }
    }

    None
}

/// Returns true if the given vendor string corresponds to a known network device vendor.
pub fn is_network_vendor(vendor: &str) -> bool {
    matches!(
        vendor.to_lowercase().as_str(),
        "huawei" | "cisco" | "思科" | "h3c" | "华三" | "ruijie" | "锐捷" | "其它"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi() {
        let raw = "\x1b[1;32mHello\x1b[0m World";
        assert_eq!(strip_ansi(raw), "Hello World");
    }

    #[test]
    fn test_clean_output() {
        // Normal: echo + output + prompt (base_prompt without terminator)
        let raw = "<H3C>display version\nH3C Comware V7.1.070\n<H3C>";
        let result = clean_output(raw, "<H3C");
        assert!(result.contains("H3C Comware V7.1.070"));
        assert!(!result.contains("<H3C"));
    }

    #[test]
    fn test_clean_output_empty() {
        assert_eq!(clean_output("", ""), "");
        assert_eq!(clean_output("<H3C>", "<H3C"), "");
    }

    #[test]
    fn test_extract_prompt() {
        // extract_prompt now returns the base_prompt without terminator
        assert_eq!(extract_prompt("<H3C>"), Some("<H3C".to_string()));
        assert_eq!(extract_prompt("Router#"), Some("Router".to_string()));
        assert_eq!(extract_prompt("[Core-Switch]"), Some("[Core-Switch".to_string()));
        assert_eq!(extract_prompt("user$"), Some("user".to_string()));
        assert_eq!(extract_prompt("no prompt here"), None);
        assert_eq!(extract_prompt(""), None);
    }

    #[test]
    fn test_output_contains_prompt() {
        // base_prompt contains match
        assert!(output_contains_prompt("screen-length disable\n<aHope_WLAN_AC>", "<aHope_WLAN_AC", "H3C"));
        assert!(output_contains_prompt("Router#show version\nstuff\nRouter#", "Router", "Cisco"));
        // Vendor pattern fallback
        assert!(output_contains_prompt("some output\n<aHope>", "", "H3C"));
        assert!(output_contains_prompt("some output\nRouter#", "", "Cisco"));
        assert!(output_contains_prompt("some output\n[Core]", "", "H3C"));
        // Not matching
        assert!(!output_contains_prompt("some output without prompt", "Router", "Cisco"));
        assert!(!output_contains_prompt("stuff", "", ""));
    }

    #[test]
    fn test_line_looks_like_prompt() {
        assert!(line_looks_like_prompt("<aHope_WLAN_AC>", "H3C"));
        assert!(line_looks_like_prompt("[Core-Switch]", "H3C"));
        assert!(line_looks_like_prompt("Router#", "Cisco"));
        assert!(line_looks_like_prompt("Router>", "Cisco"));
        assert!(!line_looks_like_prompt("some output", "H3C"));
        assert!(!line_looks_like_prompt("", "H3C"));
    }

    #[test]
    fn test_check_unrecognized_command() {
        // H3C style error
        let output = "<aHope_WLAN_AC>display ntp-status\n                       ^\n % Unrecognized command found at '^' position.";
        let result = check_unrecognized_command(output);
        assert!(result.is_some());
        assert!(result.unwrap().contains("设备不支持此命令"));

        // Cisco style error
        let output = "Router#show ntp\n% Invalid input detected at '^' marker.";
        let result = check_unrecognized_command(output);
        assert!(result.is_some());

        // Normal command output
        let output = "<H3C>display version\nH3C Comware V7.1.070\n<H3C>";
        let result = check_unrecognized_command(output);
        assert!(result.is_none());

        // Huawei style error
        let output = "<Huawei>display abc\nError: Unrecognized command";
        let result = check_unrecognized_command(output);
        assert!(result.is_some());
    }

    #[test]
    fn test_clean_output_with_prompt() {
        // Multi-line Cisco output with prompt
        let raw = "Router>show version\nCisco IOS Software\nVersion 15.2(4)M\nRouter>";
        let result = clean_output(raw, "Router");
        assert!(result.contains("Cisco IOS Software"));
        assert!(result.contains("Version 15.2"));
        assert!(!result.contains("Router"));
    }
}
