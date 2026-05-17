/// SSH 巡检执行器 — 异步执行 SSH 命令并记录结果
///
/// 对应 Python: backend/app/services/inspection_runner.py
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::io::Read;
use std::net::TcpStream;
use std::time::Duration;
use chrono::Utc;
use tracing::{info, warn, error};

/// 厂商 → netmiko device_type 映射
fn vendor_to_device_type(vendor: &str) -> &str {
    match vendor.to_lowercase().as_str() {
        "huawei" => "huawei",
        "cisco" | "思科" => "cisco_ios",
        "h3c" | "华三" => "hp_comware",
        "ruijie" | "锐捷" => "ruijie_os",
        "sharp" => "sharp",
        "linux" | "ubuntu" | "centos" | "debian" | "redhat" | "openeuler" => "linux",
        _ => "linux",
    }
}

/// 网络设备厂商（需要用分页禁用等网络设备特性）
fn is_network_vendor(vendor: &str) -> bool {
    matches!(vendor.to_lowercase().as_str(),
        "huawei" | "cisco" | "思科" | "h3c" | "华三" | "ruijie" | "锐捷" | "sharp")
}

/// 服务器厂商
fn is_server_vendor(vendor: &str) -> bool {
    matches!(vendor.to_lowercase().as_str(),
        "ubuntu" | "centos" | "rhel" | "debian" | "linux" | "openeuler" | "redhat" | "fedora" | "suse")
}

/// 异步 SSH 执行（Linux/系统设备）
fn ssh_run_async(src: &SSHSessionSource, commands: &[String]) -> HashMap<String, String> {
    let mut outputs = HashMap::new();

    let tcp = match TcpStream::connect_timeout(
        &format!("{}:{}", src.host, src.port).parse().unwrap(),
        Duration::from_secs(10),
    ) {
        Ok(tcp) => tcp,
        Err(e) => {
            warn!("[SSH] {} 连接超时（10s）: {}", src.host, e);
            for cmd in commands {
                outputs.insert(cmd.clone(), "[ERROR] 设备连接超时（10s），请检查设备是否可达".into());
            }
            return outputs;
        }
    };

    let mut session = match ssh2::Session::new() {
        Ok(s) => s,
        Err(e) => {
            warn!("[SSH] {} 创建 session 失败: {}", src.host, e);
            for cmd in commands {
                outputs.insert(cmd.clone(), format!("[ERROR] SSH session 创建失败: {}", e));
            }
            return outputs;
        }
    };

    session.set_tcp_stream(tcp);
    session.set_timeout(10000);

    if let Err(e) = session.handshake() {
        warn!("[SSH] {} 握手失败: {}", src.host, e);
        for cmd in commands {
            outputs.insert(cmd.clone(), format!("[ERROR] SSH 握手失败: {}", e));
        }
        return outputs;
    }

    if let Err(e) = session.userauth_password(&src.username, &src.password) {
        warn!("[SSH] {} 认证失败: {}", src.host, e);
        for cmd in commands {
            outputs.insert(cmd.clone(), format!("[ERROR] 设备认证失败，请检查用户名/密码"));
        }
        return outputs;
    }

    for cmd in commands {
        let mut channel = match session.channel_session() {
            Ok(c) => c,
            Err(e) => {
                outputs.insert(cmd.clone(), format!("[ERROR] 创建通道失败: {}", e));
                continue;
            }
        };

        match channel.exec(cmd) {
            Ok(()) => {
                let mut output = String::new();
                channel.read_to_string(&mut output).ok();
                channel.wait_close().ok();
                let exit = channel.exit_status().unwrap_or(-1);
                let result = if output.trim().is_empty() {
                    if exit == 0 { "(命令执行成功，无输出)".into() } else { format!("[ERROR] 退出码: {}", exit) }
                } else { output };
                outputs.insert(cmd.clone(), result);
            }
            Err(e) => {
                warn!("[SSH] {} 命令执行失败: {} → {}", src.host, cmd, e);
                outputs.insert(cmd.clone(), format!("[ERROR] 命令执行失败: {}", e));
            }
        }
    }

    outputs
}

/// 同步 SSH 执行（网络设备 — netmiko 模式）
fn ssh_run_netmiko_style(src: &SSHSessionSource, vendor: &str, commands: &[String]) -> HashMap<String, String> {
    let mut outputs = HashMap::new();

    let tcp = match TcpStream::connect_timeout(
        &format!("{}:{}", src.host, src.port).parse().unwrap(),
        Duration::from_secs(10),
    ) {
        Ok(tcp) => tcp,
        Err(e) => {
            warn!("[SSH] {} 连接超时: {}", src.host, e);
            for cmd in commands { outputs.insert(cmd.clone(), format!("[ERROR] 设备连接超时: {}", e)); }
            return outputs;
        }
    };

    let mut session = match ssh2::Session::new() {
        Ok(s) => s,
        Err(e) => {
            for cmd in commands { outputs.insert(cmd.clone(), format!("[ERROR] SSH session 创建失败: {}", e)); }
            return outputs;
        }
    };

    session.set_tcp_stream(tcp);
    session.set_timeout(10000);

    if let Err(e) = session.handshake() {
        for cmd in commands { outputs.insert(cmd.clone(), format!("[ERROR] SSH 握手失败: {}", e)); }
        return outputs;
    }

    if let Err(e) = session.userauth_password(&src.username, &src.password) {
        let msg = format!("[ERROR] 设备认证失败，请检查用户名/密码");
        for cmd in commands { outputs.insert(cmd.clone(), msg.clone()); }
        return outputs;
    }

    // 禁用分页（网络设备常见需求）
    for disable_cmd in &["screen-length 0 temporary", "terminal length 0"] {
        if let Ok(mut ch) = session.channel_session() {
            ch.exec(disable_cmd).ok();
            let mut buf = String::new();
            ch.read_to_string(&mut buf).ok();
            ch.wait_close().ok();
        }
    }

    for cmd in commands {
        let mut channel = match session.channel_session() {
            Ok(c) => c,
            Err(e) => {
                outputs.insert(cmd.clone(), format!("[ERROR] 创建通道失败: {}", e));
                continue;
            }
        };

        match channel.exec(cmd) {
            Ok(()) => {
                let mut output = String::new();
                channel.read_to_string(&mut output).ok();
                channel.wait_close().ok();
                let result = if output.trim().is_empty() {
                    // Retry once for empty output
                    let mut ch2 = match session.channel_session() {
                        Ok(c) => c,
                        Err(_) => {
                            outputs.insert(cmd.clone(), "(无输出)".into());
                            continue;
                        }
                    };
                    ch2.exec(cmd).ok();
                    let mut out2 = String::new();
                    ch2.read_to_string(&mut out2).ok();
                    ch2.wait_close().ok();
                    if out2.trim().is_empty() { "(无输出)".into() } else { out2 }
                } else { output };
                outputs.insert(cmd.clone(), result);
            }
            Err(e) => {
                warn!("[SSH] {} 命令执行失败: {} → {}", src.host, cmd, e);
                // Retry once
                if let Ok(mut ch2) = session.channel_session() {
                    ch2.exec(cmd).ok();
                    let mut out2 = String::new();
                    ch2.read_to_string(&mut out2).ok();
                    ch2.wait_close().ok();
                    outputs.insert(cmd.clone(), if out2.trim().is_empty() { format!("[ERROR] 命令执行失败: {}", e) } else { out2 });
                } else {
                    outputs.insert(cmd.clone(), format!("[ERROR] 命令执行失败: {}", e));
                }
            }
        }
    }

    outputs
}

/// SSH 连接参数
pub struct SSHSessionSource {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

/// 数据库默认端口
fn default_db_port(db_type: &str) -> u16 {
    match db_type {
        "mysql" => 3306,
        "postgresql" => 5432,
        "oracle" => 1521,
        _ => 3306,
    }
}

/// 包装 SQL 为 SSH shell 命令
fn wrap_sql_for_ssh(db_type: &str, db_user: &str, db_password: &str, db_os_user: Option<&str>, sql: &str) -> String {
    let escaped = sql.replace('\'', "'\"'\"'");
    match db_type {
        "mysql" => format!("mysql -u {} -p'{}' -e '{}' 2>&1", db_user, db_password, escaped),
        "postgresql" => {
            let prefix = if let Some(os_user) = db_os_user { format!("sudo -u {} ", os_user) } else { String::new() };
            format!("{}PGPASSWORD='{}' psql -U {} -c '{}' 2>&1", prefix, db_password, db_user, escaped)
        }
        "oracle" => {
            let prefix = if let Some(os_user) = db_os_user { format!("su - {} -c ", os_user) } else { String::new() };
            format!("{}\"echo \\\"{}\\\" | sqlplus -S {}/{}\" 2>&1", prefix, escaped, db_user, db_password)
        }
        _ => sql.to_string(),
    }
}

/// TCP 直连数据库执行 SQL
fn db_tcp_exec(db_type: &str, host: &str, port: u16, username: &str, password: &str, sqls: &[String]) -> HashMap<String, String> {
    let mut outputs = HashMap::new();
    for sql in sqls {
        outputs.insert(sql.clone(), format!("[ERROR] {} 直连未实现（需安装对应驱动）", db_type));
    }
    outputs
}

/// 获取配置备份命令（固化，不依赖用户模板）
fn get_config_cmd(vendor: &str) -> Option<String> {
    match vendor.to_lowercase().as_str() {
        "h3c" | "华三" | "huawei" | "华为" | "hp_comware" => Some("display current-configuration | include sysname".into()),
        "cisco" | "思科" | "ruijie" | "锐捷" => Some("show running-config | include hostname".into()),
        _ => None,
    }
}

/// 获取设备信息命令（固化，提取型号和序列号）
fn get_info_cmd(vendor: &str) -> Option<String> {
    match vendor.to_lowercase().as_str() {
        "h3c" | "华三" | "huawei" | "华为" | "hp_comware" => Some("display device manuinfo".into()),
        "cisco" | "思科" | "ruijie" | "锐捷" => Some("show inventory".into()),
        _ => None,
    }
}

/// 获取服务器元信息命令
fn get_meta_cmds(vendor: &str) -> Vec<String> {
    if is_server_vendor(vendor) {
        vec![
            "hostname".into(), "cat /etc/os-release".into(), "uname -a".into(),
            "lscpu".into(), "free -h".into(),
        ]
    } else { vec![] }
}

/// 检查设备可达性
fn check_device_reachable(host: &str, port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("{}:{}", host, port).parse().unwrap(),
        Duration::from_secs(3),
    ).is_ok()
}

pub struct InspectionRunner {
    db_path: String,
}

impl InspectionRunner {
    pub fn new(db_path: String) -> Self {
        Self { db_path }
    }

    /// 执行整个批次
    pub fn run_batch(&self, batch_id: i64, skip_device_ids: Option<&[i64]>) -> Result<(), String> {
        info!("[巡检执行器] 开始运行批次 {}", batch_id);
        let db = rusqlite::Connection::open(&self.db_path).map_err(|e| e.to_string())?;

        // Update batch status
        db.execute("UPDATE inspection_batches SET status='running', started_at=datetime('now') WHERE id=?1",
            rusqlite::params![batch_id]).map_err(|e| e.to_string())?;

        // Get device IDs from batch
        let device_ids_str: String = db.query_row(
            "SELECT device_ids FROM inspection_batches WHERE id=?1", rusqlite::params![batch_id], |r| r.get(0)
        ).map_err(|e| e.to_string())?;

        let mut device_ids: Vec<i64> = serde_json::from_str(&device_ids_str).unwrap_or_default();
        if let Some(skip) = skip_device_ids {
            let skip_set: std::collections::HashSet<i64> = skip.iter().copied().collect();
            device_ids.retain(|id| !skip_set.contains(id));
        }

        let online: Vec<i64> = device_ids.iter().filter(|id| {
            let ip: String = db.query_row("SELECT ip FROM devices WHERE id=?1", rusqlite::params![id], |r| r.get(0)).unwrap_or_default();
            let port: i64 = db.query_row("SELECT ssh_port FROM devices WHERE id=?1", rusqlite::params![id], |r| r.get(0)).unwrap_or(22);
            check_device_reachable(&ip, port as u16)
        }).copied().collect();

        info!("[巡检执行器] 批次 {} 设备数: {} 在线, {} 跳过", batch_id, online.len(), device_ids.len() - online.len());

        for device_id in &online {
            self.inspect_device(&db, batch_id, *device_id);
        }

        // Finalize batch
        let record_count: i64 = db.query_row("SELECT COUNT(*) FROM inspection_records WHERE batch_id=?1",
            rusqlite::params![batch_id], |r| r.get(0)).unwrap_or(0);
        let failed_count: i64 = db.query_row("SELECT COUNT(*) FROM inspection_records WHERE batch_id=?1 AND status='failed'",
            rusqlite::params![batch_id], |r| r.get(0)).unwrap_or(0);
        let status = if failed_count > 0 { "failed" } else { "completed" };

        db.execute("UPDATE inspection_batches SET status=?1, completed_at=datetime('now') WHERE id=?2",
            rusqlite::params![status, batch_id]).map_err(|e| e.to_string())?;
        info!("[巡检执行器] 批次 {} 完成: {} ({} 条记录, {} 失败)", batch_id, status, record_count, failed_count);
        Ok(())
    }

    /// 单设备巡检
    fn inspect_device(&self, db: &rusqlite::Connection, batch_id: i64, device_id: i64) {
        let device_info: Option<(String, String, String, String, Option<String>, i64, Option<String>, Option<i64>, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>)> = db.query_row(
            "SELECT name, ip, vendor, inspection_mode, ssh_username, ssh_port, ssh_password_encrypted, template_id, db_type, db_port, db_username, db_password_encrypted, db_os_user FROM devices WHERE id=?1",
            rusqlite::params![device_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?, r.get(12)?)),
        ).ok();

        let Some((name, ip, vendor, inspection_mode, ssh_username, ssh_port, ssh_pw_enc, template_id, db_type, db_port, db_username, db_pw_enc, db_os_user)) = device_info else {
            warn!("[设备巡检] 设备 {} 不存在，跳过", device_id);
            return;
        };

        let tag = format!("{} ({})", name, ip);

        // Skip offline/web mode devices
        if inspection_mode == "offline" || inspection_mode == "web" {
            let note = if inspection_mode == "web" { "WEB设备，请截图上传达结果" } else { "离线设备，请上传执行结果" };
            let upload_source = if inspection_mode == "web" { "web" } else { "offline" };
            db.execute(
                "INSERT OR REPLACE INTO inspection_records (batch_id, device_id, status, upload_source, command_outputs, completed_at) VALUES (?1,?2,'completed',?3,?4,datetime('now'))",
                rusqlite::params![batch_id, device_id, upload_source, serde_json::json!({"_note": note}).to_string()],
            ).ok();
            info!("[设备巡检] {} [{}模式] 跳过SSH", tag, inspection_mode);
            return;
        }

        // Create record
        db.execute(
            "INSERT OR REPLACE INTO inspection_records (batch_id, device_id, status, started_at) VALUES (?1,?2,'running',datetime('now'))",
            rusqlite::params![batch_id, device_id],
        ).ok();

        let record_id = db.last_insert_rowid();

        // Get commands from template
        let all_cmds = self.get_commands(db, template_id);
        if all_cmds.is_empty() {
            db.execute("UPDATE inspection_records SET status='completed', completed_at=datetime('now') WHERE id=?1", rusqlite::params![record_id]).ok();
            return;
        }

        let (ssh_cmds, db_cmds): (Vec<String>, Vec<String>) = {
            let mut s = Vec::new();
            let mut d = Vec::new();
            for (cmd, ctype) in all_cmds {
                if ctype == "ssh" { s.push(cmd); } else { d.push(cmd); }
            }
            (s, d)
        };

        info!("[设备巡检] {} 命令: OS {} 条, DB {} 条", tag, ssh_cmds.len(), db_cmds.len());

        // Decrypt password
        let password = ssh_pw_enc
            .and_then(|enc| crate::services::crypto::CryptoService::decrypt(&enc).ok())
            .unwrap_or_default();

        let mut outputs: HashMap<String, String> = HashMap::new();

        // ── 1. OS 命令：SSH 通道 ──
        if !ssh_cmds.is_empty() {
            let mut builtins = Vec::new();
            if let Some(cmd) = get_config_cmd(&vendor) { builtins.push(cmd); }
            if let Some(cmd) = get_info_cmd(&vendor) { builtins.push(cmd); }
            builtins.extend(get_meta_cmds(&vendor));

            // DB devices get extra OS meta commands
            if db_type.is_some() {
                builtins.extend(vec![
                    "hostname".into(), "cat /etc/os-release".into(), "uname -a".into(),
                    "lscpu".into(), "free -h".into(), "df -h".into(),
                    "uptime".into(), "whoami".into(),
                ]);
            }

            let all_ssh: Vec<String> = {
                let mut cmds = ssh_cmds.clone();
                for bc in builtins {
                    if !cmds.contains(&bc) { cmds.push(bc); }
                }
                cmds
            };

            let src = SSHSessionSource {
                host: ip.clone(),
                port: ssh_port as u16,
                username: ssh_username.clone().unwrap_or_else(|| "admin".into()),
                password: password.clone(),
            };

            if is_network_vendor(&vendor) {
                info!("[设备巡检] {} 使用 netmiko 模式执行 OS 命令", tag);
                outputs.extend(ssh_run_netmiko_style(&src, &vendor, &all_ssh));
            } else {
                info!("[设备巡检] {} 使用 asyncssh 模式执行 OS 命令", tag);
                outputs.extend(ssh_run_async(&src, &all_ssh));
            }
        }

        // ── 2. DB 命令：TCP 直连 → SSH 兜底 ──
        if !db_cmds.is_empty() {
            if let Some(ref db_type_str) = db_type {
                let db_password = db_pw_enc
                    .and_then(|enc| crate::services::crypto::CryptoService::decrypt(&enc).ok())
                    .unwrap_or_default();
                let db_port_val: u16 = db_port.map(|p| p as u16).unwrap_or_else(|| default_db_port(db_type_str));
                let db_user = db_username.unwrap_or_default();

                // Try TCP first, fallback to SSH
                let tcp_outputs = db_tcp_exec(db_type_str, &ip, db_port_val, &db_user, &db_password, &db_cmds);
                for (sql, result) in tcp_outputs {
                    if result.starts_with("[ERROR]") && result.contains("直连未实现") {
                        // SSH fallback: wrap SQL
                        let ssh_cmd = wrap_sql_for_ssh(db_type_str, &db_user, &db_password, db_os_user.as_deref(), &sql);
                        let src = SSHSessionSource {
                            host: ip.clone(), port: ssh_port as u16,
                            username: ssh_username.clone().unwrap_or_else(|| "admin".into()),
                            password: password.clone(),
                        };
                        let ssh_result = ssh_run_async(&src, &[ssh_cmd]);
                        outputs.insert(sql, ssh_result.values().next().cloned().unwrap_or_else(|| format!("[ERROR] SSH 回退执行失败")));
                    } else {
                        outputs.insert(sql, result);
                    }
                }
            }
        }

        let outputs_json = serde_json::to_string(&outputs).unwrap_or_else(|_| "{}".into());

        let user_cmds: Vec<&String> = ssh_cmds.iter().chain(db_cmds.iter()).collect();
        let all_errors = !user_cmds.is_empty() && user_cmds.iter().all(|cmd| {
            outputs.get(*cmd).map(|o| o.starts_with("[ERROR]")).unwrap_or(false)
        });
        let status = if all_errors { "failed" } else { "completed" };
        let error_msg = if all_errors {
            Some("所有命令均执行失败".to_string())
        } else { None };

        db.execute(
            "UPDATE inspection_records SET status=?1, command_outputs=?2, error_message=?3, completed_at=datetime('now') WHERE id=?4",
            rusqlite::params![status, outputs_json, error_msg, record_id],
        ).ok();

        info!("[设备巡检] {} 完成: {} ({} 条结果)", tag, status, outputs.len());
    }

    /// 从设备模板获取命令列表及类型 [(command, command_type)]
    fn get_commands(&self, db: &rusqlite::Connection, template_id: Option<i64>) -> Vec<(String, String)> {
        let tid = match template_id { Some(id) => id, None => return vec![] };

        let config: Option<String> = db.query_row(
            "SELECT config FROM inspection_templates WHERE id=?1", rusqlite::params![tid], |r| r.get(0)
        ).ok().flatten();

        let config = match config { Some(c) => c, None => return vec![] };

        let cfg: serde_json::Value = match serde_json::from_str(&config) { Ok(v) => v, Err(_) => return vec![] };

        let cmd_ids: Vec<i64> = cfg.get("command_ids").and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_i64()).collect())
            .unwrap_or_default();

        if cmd_ids.is_empty() {
            return cfg.get("commands").and_then(|v| v.as_array())
                .map(|a| a.iter()
                    .filter_map(|v| v.as_str().map(|s| (s.to_string(), "ssh".to_string())))
                    .collect())
                .unwrap_or_default();
        }

        // Batch query commands
        let mut cmds = Vec::new();
        for cid in cmd_ids {
            if let Ok((cmd_text, cmd_type)) = db.query_row(
                "SELECT command, command_type FROM command_pool WHERE id=?1",
                rusqlite::params![cid],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            ) {
                cmds.push((cmd_text, cmd_type));
            }
        }
        cmds
    }
}
