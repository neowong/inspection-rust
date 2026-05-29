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

/// Execute commands on a network device via SSH.
/// Returns a HashMap mapping each command to its output text.
pub fn run_commands(
    source: &SSHSessionSource,
    vendor: &str,
    commands: &[String],
) -> Result<HashMap<String, String>, String> {
    // Try libssh2 first, fallback to system ssh
    match run_commands_libssh2(source, vendor, commands) {
        Ok(results) => Ok(results),
        Err(e) => {
            eprintln!("libssh2 failed: {}, trying system ssh", e);
            run_commands_system_ssh(source, commands)
        }
    }
}

/// Fallback: use system ssh command
fn run_commands_system_ssh(
    source: &SSHSessionSource,
    commands: &[String],
) -> Result<HashMap<String, String>, String> {
    use std::process::{Command, Stdio};

    let mut results = HashMap::new();

    for cmd in commands {
        let child = Command::new("sshpass")
            .args(&["-p", &source.password])
            .arg("ssh")
            .args(&[
                "-o", "StrictHostKeyChecking=no",
                "-o", "HostKeyAlgorithms=+ssh-rsa",
                "-o", "PubkeyAcceptedKeyTypes=+ssh-rsa",
                "-o", "ConnectTimeout=10",
                "-p", &source.port.to_string(),
                &format!("{}@{}", source.username, source.host),
                cmd,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
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
        results.insert(cmd.clone(), stdout);
    }

    Ok(results)
}

/// Original libssh2 implementation
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

    // 2. SSH handshake with password authentication
    let mut session = Session::new()
        .map_err(|e| format!("创建SSH会话失败: {}", e))?;
    session.set_tcp_stream(tcp);

    // Set banner for better compatibility with network devices
    session.set_banner("SSH-2.0-OpenSSH_8.0").ok();

    session
        .handshake()
        .map_err(|e| format!("SSH握手失败: {}", e))?;
    session
        .userauth_password(&source.username, &source.password)
        .map_err(|e| format!("SSH密码认证失败: {}", e))?;

    if !session.authenticated() {
        return Err("SSH认证未通过".to_string());
    }

    // Set session timeout early - some devices need this before channel creation
    session.set_timeout(30_000);

    // Small delay after authentication for devices that need time
    std::thread::sleep(std::time::Duration::from_millis(500));

    // 4. Send pagination disable command based on vendor
    let page_cmd = match vendor.to_lowercase().as_str() {
        "h3c" | "华三" | "huawei" | "华为" => "screen-length disable",
        "cisco" | "思科" | "ruijie" | "锐捷" => "terminal length 0",
        _ => "",
    };

    if !page_cmd.is_empty() {
        // Ignore pagination command failure - some devices don't support it
        let _ = exec_command_on_session(&session, page_cmd, source.password.as_str());
    }

    // 5-6. Execute each command, collect output
    let mut results = HashMap::new();
    for cmd in commands {
        let output = exec_command_on_session(&session, cmd, source.password.as_str())?;
        results.insert(cmd.clone(), output);
    }

    // 9. Drop session (closes channels and connection)
    drop(session);

    Ok(results)
}

/// Execute a single command on an established SSH session.
/// 30 second per-command timeout, reads up to 4096 bytes.
fn exec_command_on_session(session: &Session, cmd: &str, password: &str) -> Result<String, String> {
    // 7. 30 second per-command timeout
    session.set_timeout(30_000);

    // 3. Create a channel for this command
    let mut channel = session
        .channel_session()
        .map_err(|e| format!("创建SSH通道失败: {}", e))?;

    // Request PTY for network devices that require it
    let _ = channel.request_pty("xterm", None, None);

    // Try shell mode instead of exec for better compatibility
    channel
        .shell()
        .map_err(|e| format!("启动Shell失败: {}", e))?;

    // Send the command
    writeln!(channel, "{}", cmd)
        .map_err(|e| format!("发送命令失败 '{}': {}", cmd, e))?;

    // Send exit to close the shell after command completes
    writeln!(channel, "exit")
        .map_err(|e| format!("发送exit失败: {}", e))?;

    // 5. Read stdout up to 4096 bytes
    let mut output = String::new();
    let mut buf = [0u8; 1024];
    let mut total = 0usize;
    let mut password_sent = false;

    loop {
        match channel.read(&mut buf) {
            Ok(0) => break, // EOF
            Ok(n) => {
                if total + n > 4096 {
                    let remaining = 4096 - total;
                    let text = std::str::from_utf8(&buf[..remaining])
                        .map_err(|_| "输出编码错误(非UTF-8)".to_string())?;
                    output.push_str(text);
                    break;
                }
                total += n;
                let text = std::str::from_utf8(&buf[..n])
                    .map_err(|_| "输出编码错误(非UTF-8)".to_string())?;
                output.push_str(text);

                // Handle sudo/super user enable password prompts (only once per command)
                if !password_sent && text.contains("assword:") && !password.is_empty() {
                    let _ = channel.write_all(password.as_bytes());
                    let _ = channel.write_all(b"\n");
                    password_sent = true;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // Per-command timeout (30s) reached, return what we have
                break;
            }
            Err(e) => {
                return Err(format!("读取命令输出失败 '{}': {}", cmd, e));
            }
        }

        if channel.eof() {
            break;
        }
    }

    // Wait for command completion and close the channel
    let _ = channel.wait_close();

    Ok(output.trim().to_string())
}

/// Returns true if the given vendor string corresponds to a known network device vendor.
pub fn is_network_vendor(vendor: &str) -> bool {
    matches!(
        vendor.to_lowercase().as_str(),
        "huawei" | "cisco" | "思科" | "h3c" | "华三" | "ruijie" | "锐捷"
    )
}
