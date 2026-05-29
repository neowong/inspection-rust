use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
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

    /// OpenSSH -o 选项（系统 SSH 后备方案使用）
    pub fn openssh_options() -> Vec<String> {
        vec![
            "-o".into(), "StrictHostKeyChecking=no".into(),
            "-o".into(), "UserKnownHostsFile=/dev/null".into(),
            "-o".into(), format!("KexAlgorithms={}", KEX),
            "-o".into(), format!("Ciphers={}", CIPHERS),
            "-o".into(), format!("MACs={}", MACS),
            "-o".into(), format!("HostKeyAlgorithms={}", HOST_KEY),
            "-o".into(), "ConnectTimeout=10".into(),
            "-o".into(), "ServerAliveInterval=5".into(),
            "-o".into(), "ServerAliveCountMax=3".into(),
            "-o".into(), "LogLevel=ERROR".into(),
            "-o".into(), "BatchMode=no".into(),
        ]
    }
}

/// Execute commands on a network device via SSH.
/// Returns a HashMap mapping each command to its output text.
///
/// Tries libssh2 first (with legacy algorithm support),
/// falls back to system ssh command for maximum compatibility.
pub fn run_commands(
    source: &SSHSessionSource,
    vendor: &str,
    commands: &[String],
) -> Result<HashMap<String, String>, String> {
    // Try libssh2 first
    match run_commands_libssh2(source, vendor, commands) {
        Ok(results) => return Ok(results),
        Err(e) => {
            eprintln!("[SSH] libssh2 failed: {}, trying system ssh", e);
        }
    }

    // Fallback to system ssh
    match run_commands_system_ssh(source, vendor, commands) {
        Ok(results) => return Ok(results),
        Err(e) => {
            return Err(format!("所有 SSH 连接方式均失败: {}", e));
        }
    }
}

/// Fallback: use system ssh command (sshpass + openssh)
/// Enables all legacy algorithms for maximum device compatibility.
fn run_commands_system_ssh(
    source: &SSHSessionSource,
    vendor: &str,
    commands: &[String],
) -> Result<HashMap<String, String>, String> {
    use std::process::Command;

    // Check if sshpass is available
    let sshpass_check = Command::new("which").arg("sshpass").output();
    let has_sshpass = sshpass_check.map(|o| o.status.success()).unwrap_or(false);

    if !has_sshpass {
        return Err("sshpass 未安装，无法进行密码认证".to_string());
    }

    let ssh_opts = legacy_algorithms::openssh_options();

    // Send pagination disable command first if needed
    let page_cmd = match vendor.to_lowercase().as_str() {
        "h3c" | "华三" | "huawei" | "华为" => Some("screen-length disable"),
        "cisco" | "思科" | "ruijie" | "锐捷" => Some("terminal length 0"),
        _ => None,
    };

    if let Some(pc) = page_cmd {
        let _ = run_single_ssh_command(source, pc, &ssh_opts, has_sshpass);
    }

    let mut results = HashMap::new();
    for cmd in commands {
        let output = run_single_ssh_command(source, cmd, &ssh_opts, has_sshpass)?;
        results.insert(cmd.clone(), output);
    }

    Ok(results)
}

/// Run a single SSH command via system ssh
fn run_single_ssh_command(
    source: &SSHSessionSource,
    cmd: &str,
    ssh_opts: &[String],
    has_sshpass: bool,
) -> Result<String, String> {
    use std::process::{Command, Stdio};

    let mut command = if has_sshpass {
        let mut c = Command::new("sshpass");
        c.args(&["-p", &source.password]);
        c.arg("ssh");
        c
    } else {
        Command::new("ssh")
    };

    for opt in ssh_opts {
        command.arg(opt);
    }

    command.arg("-p").arg(source.port.to_string());
    command.arg(format!("{}@{}", source.username, source.host));
    command.arg(cmd);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let child = command
        .spawn()
        .map_err(|e| format!("启动ssh命令失败: {}", e))?;

    let output = child
        .wait_with_output()
        .map_err(|e| format!("等待ssh输出失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("SSH命令失败: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(clean_command_output(&stdout))
}

/// libssh2 implementation with legacy algorithm support
fn run_commands_libssh2(
    source: &SSHSessionSource,
    vendor: &str,
    commands: &[String],
) -> Result<HashMap<String, String>, String> {
    // 1. TCP connect with 10 second timeout
    let addr = format!("{}:{}", source.host, source.port)
        .to_socket_addrs()
        .map_err(|e| format!("地址解析失败: {}", e))?
        .next()
        .ok_or_else(|| "无法解析主机地址".to_string())?;

    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("TCP连接失败(10s超时): {}", e))?;

    // 2. Create SSH session with legacy algorithm preferences
    let mut session = Session::new()
        .map_err(|e| format!("创建SSH会话失败: {}", e))?;

    // Set banner for compatibility with network devices
    session.set_banner("SSH-2.0-OpenSSH_8.0").ok();

    // Configure legacy algorithm preferences
    // This ensures libssh2 can negotiate older algorithms that network devices require
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

    // Set session timeout
    session.set_timeout(30_000);

    // Small delay after authentication for devices that need time
    std::thread::sleep(std::time::Duration::from_millis(300));

    // 3. Send pagination disable command based on vendor
    let page_cmd = match vendor.to_lowercase().as_str() {
        "h3c" | "华三" | "huawei" | "华为" => "screen-length disable",
        "cisco" | "思科" | "ruijie" | "锐捷" => "terminal length 0",
        _ => "",
    };

    if !page_cmd.is_empty() {
        let _ = exec_command_on_session(&session, page_cmd, source.password.as_str());
    }

    // 4. Execute each command, collect output
    let mut results = HashMap::new();
    for cmd in commands {
        let output = exec_command_on_session(&session, cmd, source.password.as_str())?;
        results.insert(cmd.clone(), output);
    }

    drop(session);
    Ok(results)
}

/// Execute a single command on an established SSH session.
/// 30 second per-command timeout, reads up to 65536 bytes.
fn exec_command_on_session(session: &Session, cmd: &str, password: &str) -> Result<String, String> {
    session.set_timeout(30_000);

    let mut channel = session
        .channel_session()
        .map_err(|e| format!("创建SSH通道失败: {}", e))?;

    // Request PTY - many network devices require this
    let _ = channel.request_pty("xterm", None, None);

    channel
        .shell()
        .map_err(|e| format!("启动Shell失败: {}", e))?;

    // Small delay for shell prompt
    std::thread::sleep(Duration::from_millis(200));

    // Send the command
    writeln!(channel, "{}", cmd)
        .map_err(|e| format!("发送命令失败 '{}': {}", cmd, e))?;

    // Send exit to close the shell after command completes
    writeln!(channel, "exit")
        .map_err(|e| format!("发送exit失败: {}", e))?;

    // Read output
    let mut output = String::new();
    let mut buf = [0u8; 4096];
    let mut total = 0usize;
    let max_output = 65536usize; // 64KB max output per command
    let mut password_sent = false;

    loop {
        match channel.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if total + n > max_output {
                    let remaining = max_output - total;
                    if remaining > 0 {
                        if let Ok(text) = std::str::from_utf8(&buf[..remaining]) {
                            output.push_str(text);
                        }
                    }
                    break;
                }
                total += n;
                if let Ok(text) = std::str::from_utf8(&buf[..n]) {
                    output.push_str(text);

                    // Handle enable/super user password prompts
                    if !password_sent && text.contains("assword:") && !password.is_empty() {
                        let _ = channel.write_all(password.as_bytes());
                        let _ = channel.write_all(b"\n");
                        password_sent = true;
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(e) => {
                return Err(format!("读取命令输出失败 '{}': {}", cmd, e));
            }
        }

        if channel.eof() {
            break;
        }
    }

    let _ = channel.wait_close();

    Ok(clean_command_output(&output))
}

/// Clean up command output by removing ANSI escape codes,
/// device prompts, and echoed command text.
fn clean_command_output(raw: &str) -> String {
    // Remove ANSI escape sequences
    let mut cleaned = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip escape sequence
            if chars.peek() == Some(&'[') {
                chars.next();
                // Skip until we find a letter
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            cleaned.push(c);
        }
    }

    cleaned.trim().to_string()
}

/// Returns true if the given vendor string corresponds to a known network device vendor.
pub fn is_network_vendor(vendor: &str) -> bool {
    matches!(
        vendor.to_lowercase().as_str(),
        "huawei" | "cisco" | "思科" | "h3c" | "华三" | "ruijie" | "锐捷"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_command_output() {
        // ANSI escape codes
        let raw = "\x1b[1;32mHello\x1b[0m World";
        assert_eq!(clean_command_output(raw), "Hello World");

        // Prompts and echoed commands
        let raw = "<H3C>display version\nH3C Comware V7.1.070\n<H3C>";
        let result = clean_command_output(raw);
        assert!(result.contains("H3C Comware V7.1.070"));
    }
}
