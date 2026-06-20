//! Linux exec channel 执行器
//!
//! 每条命令通过独立的 SSH exec channel 执行，无需提示符检测。
//! 需要 root 权限的命令通过 sudo -S + stdin 密码方式提权。

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ssh2::Session;

use crate::services::inspection_runner::SSHSessionSource;

/// 每条命令执行的超时时间
const CMD_TIMEOUT: Duration = Duration::from_secs(30);

/// 连续超时次数阈值，超过则跳过剩余命令
const MAX_CONSECUTIVE_TIMEOUTS: usize = 2;

/// 通过 exec channel 执行一组 Linux 命令
pub fn run_commands_exec(
    source: &SSHSessionSource,
    commands: &[String],
    needs_root_map: &HashMap<String, bool>,
    cancel: Option<Arc<AtomicBool>>,
    on_progress: Option<Arc<std::sync::Mutex<String>>>,
) -> Result<indexmap::IndexMap<String, String>, String> {
    tracing::info!(
        "Linux SSH exec 开始: {}@{}:{}, 命令数={}",
        source.username, source.host, source.port, commands.len()
    );

    // 1. 建立 SSH 连接（复用现有连接逻辑）
    let session = crate::services::inspection_runner::connect_session(source)?;

    let mut outputs = indexmap::IndexMap::new();
    let mut consecutive_timeouts = 0usize;

    // 2. 逐条命令执行
    for cmd in commands {
        // 取消检查
        if let Some(ref flag) = cancel {
            if flag.load(Ordering::Relaxed) {
                tracing::info!("Linux exec 取消，已执行 {}/{} 条命令", outputs.len(), commands.len());
                break;
            }
        }

        // 更新进度
        if let Some(ref progress) = on_progress {
            if let Ok(mut p) = progress.lock() {
                *p = cmd.clone();
            }
        }

        let needs_root = needs_root_map.get(cmd).copied().unwrap_or(false);

        match exec_single_command(&session, cmd, needs_root, &source.password) {
            Ok(output) => {
                consecutive_timeouts = 0;
                outputs.insert(cmd.clone(), output);
            }
            Err(e) => {
                if e.contains("超时") {
                    consecutive_timeouts += 1;
                    outputs.insert(cmd.clone(), format!("[命令执行超时: {}]", cmd));
                    if consecutive_timeouts >= MAX_CONSECUTIVE_TIMEOUTS {
                        tracing::warn!(
                            "Linux exec 连续 {} 次超时，跳过剩余命令",
                            consecutive_timeouts
                        );
                        for remaining_cmd in commands.iter().skip(outputs.len()) {
                            outputs.insert(
                                remaining_cmd.clone(),
                                "[因前序命令超时已跳过]".to_string(),
                            );
                        }
                        break;
                    }
                } else {
                    outputs.insert(cmd.clone(), format!("[执行错误: {}]", e));
                }
            }
        }
    }

    tracing::info!(
        "Linux exec 完成: {}/{} 条命令成功",
        outputs.len(),
        commands.len()
    );
    Ok(outputs)
}

/// 执行单条命令
fn exec_single_command(
    session: &Session,
    cmd: &str,
    needs_root: bool,
    password: &str,
) -> Result<String, String> {
    let mut channel = session.channel_session().map_err(|e| format!("打开 channel 失败: {}", e))?;

    if needs_root {
        // sudo -S 通过 stdin 写入密码
        let escaped = cmd.replace('\'', "'\\''");
        let sudo_cmd = format!("sudo -S sh -c '{}'", escaped);
        channel.exec(&sudo_cmd).map_err(|e| format!("exec 失败: {}", e))?;
        let mut stdin_stream = channel.stream(0);
        writeln!(stdin_stream, "{}", password).map_err(|e| format!("写入 sudo 密码失败: {}", e))?;
        stdin_stream.flush().ok();
        std::thread::sleep(Duration::from_millis(200));
    } else {
        channel.exec(cmd).map_err(|e| format!("exec 失败: {}", e))?;
    }

    // 读取输出直到 EOF
    let mut output = String::new();
    let mut buf = [0u8; 4096];
    let start = Instant::now();

    loop {
        if start.elapsed() > CMD_TIMEOUT {
            let _ = channel.close();
            return Err("超时".to_string());
        }

        match channel.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                output.push_str(&String::from_utf8_lossy(&buf[..n]));
            }
            Err(e) => {
                let kind = e.kind();
                if kind == std::io::ErrorKind::WouldBlock || kind == std::io::ErrorKind::TimedOut {
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
                let _ = channel.close();
                return Err(format!("读取输出失败: {}", e));
            }
        }
    }

    // 读取 stderr
    let mut stderr = String::new();
    let mut stderr_buf = [0u8; 2048];
    loop {
        match channel.stderr().read(&mut stderr_buf) {
            Ok(0) => break,
            Ok(n) => {
                stderr.push_str(&String::from_utf8_lossy(&stderr_buf[..n]));
            }
            Err(_) => break,
        }
    }

    let _ = channel.close();
    let output = clean_exec_output(&output, &stderr);
    Ok(output)
}

/// 清理 exec channel 输出
fn clean_exec_output(stdout: &str, stderr: &str) -> String {
    let mut result = String::new();

    for line in stdout.lines() {
        if line.contains("[sudo]") && line.contains("password") {
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }

    if result.trim().is_empty() && !stderr.is_empty() {
        for line in stderr.lines() {
            if line.contains("[sudo]") && line.contains("password") {
                continue;
            }
            result.push_str(line);
            result.push('\n');
        }
    }

    result.trim_end().to_string()
}
