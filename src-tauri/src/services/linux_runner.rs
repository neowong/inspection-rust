//! Linux exec channel 执行器（多连接并行）
//!
//! 通过开 N 条独立 SSH 连接并行执行命令，每条连接内部串行：
//!   - libssh2 单 session 上多 channel 实际串行（C 库有全局锁），所以"单连接多 channel 并发"是假并行
//!   - 工业惯例（netmiko、parallel-ssh）都是"多连接"实现真并行
//!
//! 需要 root 权限的命令通过 `sudo -S` + stdin 密码方式提权。

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ssh2::Session;

use crate::services::inspection_runner::SSHSessionSource;

/// 每条命令执行的超时时间
const CMD_TIMEOUT: Duration = Duration::from_secs(30);

/// 单个 worker 内连续超时阈值，超过则该 worker 放弃剩余分片
const MAX_CONSECUTIVE_TIMEOUTS: usize = 2;

/// 并行连接数（每个连接内部串行）。
/// 实测 4 个连接对绝大多数 Linux 服务器是平衡点：
///   - 太少（1）失去并行效果
///   - 太多（>8）会被 sshd MaxStartups / fail2ban 限流，反而触发"connection refused"
const PARALLEL_CONNECTIONS: usize = 4;

/// 通过多 SSH 连接并行执行一组 Linux 命令
pub fn run_commands_exec(
    source: &SSHSessionSource,
    commands: &[String],
    needs_root_map: &HashMap<String, bool>,
    cancel: Option<Arc<AtomicBool>>,
    on_progress: Option<Arc<parking_lot::Mutex<String>>>,
) -> Result<indexmap::IndexMap<String, String>, String> {
    let total = commands.len();
    tracing::info!(
        "Linux SSH exec 开始: {}@{}:{}, 命令数={}, 并行连接数={}",
        source.username,
        source.host,
        source.port,
        total,
        PARALLEL_CONNECTIONS,
    );

    if total == 0 {
        return Ok(indexmap::IndexMap::new());
    }

    // 0. TCP 预检：单次快速探测（避免 N 个 worker 同时超时等待）
    {
        let addr = format!("{}:{}", source.host, source.port)
            .to_socket_addrs()
            .map_err(|e| format!("地址解析失败: {}", e))?
            .next()
            .ok_or_else(|| "无法解析主机地址".to_string())?;
        TcpStream::connect_timeout(&addr, Duration::from_secs(3))
            .map_err(|e| format!("TCP 端口不可达 ({}): {}", source.port, e))?;
    }

    // 1. 把命令列表轮询分片：cmd[i] 分给 worker (i % N)
    //    这样每个 worker 拿到的命令 size/类型分布均匀，避免某个 worker 全是慢命令
    let n_workers = PARALLEL_CONNECTIONS.min(total);
    let mut shards: Vec<Vec<usize>> = (0..n_workers).map(|_| Vec::new()).collect();
    for (i, _) in commands.iter().enumerate() {
        shards[i % n_workers].push(i);
    }

    // 2. 共享结果存储：Vec<Option<String>> 按原索引下标回填，保证顺序
    let results: Arc<parking_lot::Mutex<Vec<Option<String>>>> =
        Arc::new(parking_lot::Mutex::new(vec![None; total]));
    // 完成计数（用于进度反馈）
    let done = Arc::new(AtomicUsize::new(0));

    // 3. spawn N 个线程，每个线程一条独立 SSH 连接
    let errors: Arc<parking_lot::Mutex<Vec<String>>> = Arc::new(parking_lot::Mutex::new(Vec::new()));

    std::thread::scope(|scope| {
        for (worker_id, shard) in shards.into_iter().enumerate() {
            if shard.is_empty() {
                continue;
            }
            let source = source.clone();
            let commands = commands.to_vec();
            let needs_root_map = needs_root_map.clone();
            let cancel = cancel.clone();
            let on_progress = on_progress.clone();
            let results = Arc::clone(&results);
            let done = Arc::clone(&done);
            let errors = Arc::clone(&errors);

            scope.spawn(move || {
                // 每个 worker 独立建立 SSH 连接
                let session = match crate::services::inspection_runner::connect_session(&source) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("worker #{} SSH 连接失败: {}", worker_id, e);
                        let mut errs = errors.lock();
                        errs.push(format!("worker {}: {}", worker_id, e));
                        // 给该分片所有命令打上连接失败标记，保证 results 不留 None
                        let mut r = results.lock();
                        for &idx in &shard {
                            r[idx] = Some(format!("[SSH 连接失败: {}]", e));
                            done.fetch_add(1, Ordering::Relaxed);
                        }
                        return;
                    }
                };

                let mut consecutive_timeouts = 0usize;
                for (shard_pos, &idx) in shard.iter().enumerate() {
                    // 取消检查
                    if let Some(ref flag) = cancel {
                        if flag.load(Ordering::Relaxed) {
                            let remaining_count = shard.len() - shard_pos;
                            tracing::info!(
                                "worker #{} 取消，剩余 {} 条未执行",
                                worker_id,
                                remaining_count
                            );
                            // 把该 worker 剩余位置标记为"已取消"
                            let mut r = results.lock();
                            for &remaining in shard.iter().skip(shard_pos) {
                                if r[remaining].is_none() {
                                    r[remaining] = Some("[已取消]".to_string());
                                    done.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            return;
                        }
                    }

                    let cmd = &commands[idx];
                    let needs_root = needs_root_map.get(cmd).copied().unwrap_or(false);

                    // 进度反馈：当前正在执行的命令名（多 worker 时只反映最后一个写入的）
                    if let Some(ref progress) = on_progress {
                        let mut p = progress.lock();
                        *p = format!(
                            "{} ({}/{})",
                            cmd,
                            done.load(Ordering::Relaxed) + 1,
                            total
                        );
                    }

                    let result = exec_single_command(&session, cmd, needs_root, &source.password);
                    let value = match result {
                        Ok(output) => {
                            consecutive_timeouts = 0;
                            output
                        }
                        Err(e) => {
                            if e.contains("超时") {
                                consecutive_timeouts += 1;
                                let timeout_msg = format!("[命令执行超时: {}]", cmd);
                                if consecutive_timeouts >= MAX_CONSECUTIVE_TIMEOUTS {
                                    tracing::warn!(
                                        "worker #{} 连续 {} 次超时，跳过该 worker 剩余命令",
                                        worker_id,
                                        consecutive_timeouts
                                    );
                                    // 当前命令仍记录超时
                                    {
                                        let mut r = results.lock();
                                        r[idx] = Some(timeout_msg);
                                        done.fetch_add(1, Ordering::Relaxed);
                                    }
                                    // 标记 worker 剩余位置为"前序超时已跳过"
                                    let mut r = results.lock();
                                    for &remaining in shard.iter().skip(shard_pos + 1) {
                                        r[remaining] =
                                            Some("[因前序命令超时已跳过]".to_string());
                                        done.fetch_add(1, Ordering::Relaxed);
                                    }
                                    return;
                                }
                                timeout_msg
                            } else {
                                format!("[执行错误: {}]", e)
                            }
                        }
                    };
                    {
                        let mut r = results.lock();
                        r[idx] = Some(value);
                    }
                    done.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });

    // 4. 按原顺序组装 IndexMap
    let final_results = results.lock();
    let mut outputs = indexmap::IndexMap::with_capacity(total);
    for (i, cmd) in commands.iter().enumerate() {
        let value = final_results[i].clone().unwrap_or_else(|| "[未执行]".to_string());
        outputs.insert(cmd.clone(), value);
    }

    let errs = errors.lock();
    tracing::info!(
        "Linux exec 完成: {}/{} 条命令, 连接错误={}",
        done.load(Ordering::Relaxed),
        total,
        errs.len(),
    );

    // 全部 worker 都连接失败时返回错误，否则尽量保留部分结果
    if !errs.is_empty() && errs.len() >= PARALLEL_CONNECTIONS.min(total) {
        return Err(format!("所有 SSH 连接均失败: {}", errs.join("; ")));
    }
    Ok(outputs)
}

/// 执行单条命令（占用一条 exec channel）
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
