/// SSH 巡检执行器 — 网络设备 SSH 命令执行
use std::collections::HashMap;
use std::io::Read;
use std::net::TcpStream;
use std::time::Duration;
use tracing::{info, warn};

/// 网络设备厂商
fn is_network_vendor(vendor: &str) -> bool {
    matches!(vendor.to_lowercase().as_str(),
        "huawei" | "cisco" | "思科" | "h3c" | "华三" | "ruijie" | "锐捷" | "sharp")
}

/// 获取配置备份命令
fn get_config_cmd(vendor: &str) -> Option<String> {
    match vendor.to_lowercase().as_str() {
        "h3c" | "华三" | "huawei" | "华为" | "hp_comware" => Some("display current-configuration | include sysname".into()),
        "cisco" | "思科" | "ruijie" | "锐捷" => Some("show running-config | include hostname".into()),
        _ => None,
    }
}

/// 获取设备信息命令
fn get_info_cmd(vendor: &str) -> Option<String> {
    match vendor.to_lowercase().as_str() {
        "h3c" | "华三" | "huawei" | "华为" | "hp_comware" => Some("display device manuinfo".into()),
        "cisco" | "思科" | "ruijie" | "锐捷" => Some("show inventory".into()),
        _ => None,
    }
}

/// 网络设备 SSH 执行（带分页禁用 + 重试）
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

    // 禁用分页
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
                    let mut ch2 = match session.channel_session() {
                        Ok(c) => c,
                        Err(_) => { outputs.insert(cmd.clone(), "(无输出)".into()); continue; }
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

pub struct SSHSessionSource {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

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

    pub fn run_batch(&self, batch_id: i64, skip_device_ids: Option<&[i64]>) -> Result<(), String> {
        info!("[巡检执行器] 开始运行批次 {}", batch_id);
        let db = rusqlite::Connection::open(&self.db_path).map_err(|e| e.to_string())?;

        db.execute("UPDATE inspection_batches SET status='running', started_at=datetime('now') WHERE id=?1",
            rusqlite::params![batch_id]).map_err(|e| e.to_string())?;

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

        info!("[巡检执行器] 批次 {} 设备数: {} 在线", batch_id, online.len());

        for device_id in &online {
            self.inspect_device(&db, batch_id, *device_id);
        }

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

    fn inspect_device(&self, db: &rusqlite::Connection, batch_id: i64, device_id: i64) {
        let device_info: Option<(String, String, String, Option<String>, i64, Option<String>, Option<i64>)> = db.query_row(
            "SELECT name, ip, vendor, ssh_username, ssh_port, ssh_password_encrypted, template_id FROM devices WHERE id=?1",
            rusqlite::params![device_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        ).ok();

        let Some((name, ip, vendor, ssh_username, ssh_port, ssh_pw_enc, template_id)) = device_info else {
            warn!("[设备巡检] 设备 {} 不存在，跳过", device_id);
            return;
        };

        let tag = format!("{} ({})", name, ip);

        db.execute(
            "INSERT OR REPLACE INTO inspection_records (batch_id, device_id, status, started_at) VALUES (?1,?2,'running',datetime('now'))",
            rusqlite::params![batch_id, device_id],
        ).ok();

        let record_id = db.last_insert_rowid();

        let commands = self.get_commands(db, template_id);
        if commands.is_empty() {
            db.execute("UPDATE inspection_records SET status='completed', completed_at=datetime('now') WHERE id=?1", rusqlite::params![record_id]).ok();
            return;
        }

        info!("[设备巡检] {} 命令: {} 条", tag, commands.len());

        let password = ssh_pw_enc
            .and_then(|enc| crate::services::crypto::CryptoService::decrypt(&enc).ok())
            .unwrap_or_default();

        let src = SSHSessionSource {
            host: ip.clone(),
            port: ssh_port as u16,
            username: ssh_username.clone().unwrap_or_else(|| "admin".into()),
            password: password.clone(),
        };

        // Add builtin commands
        let mut all_cmds = commands.clone();
        if let Some(cmd) = get_config_cmd(&vendor) { if !all_cmds.contains(&cmd) { all_cmds.push(cmd); } }
        if let Some(cmd) = get_info_cmd(&vendor) { if !all_cmds.contains(&cmd) { all_cmds.push(cmd); } }

        let outputs = ssh_run_netmiko_style(&src, &vendor, &all_cmds);

        let outputs_json = serde_json::to_string(&outputs).unwrap_or_else(|_| "{}".into());

        let all_errors = !commands.is_empty() && commands.iter().all(|cmd| {
            outputs.get(cmd).map(|o| o.starts_with("[ERROR]")).unwrap_or(false)
        });
        let status = if all_errors { "failed" } else { "completed" };
        let error_msg = if all_errors { Some("所有命令均执行失败".to_string()) } else { None };

        db.execute(
            "UPDATE inspection_records SET status=?1, command_outputs=?2, error_message=?3, completed_at=datetime('now') WHERE id=?4",
            rusqlite::params![status, outputs_json, error_msg, record_id],
        ).ok();

        info!("[设备巡检] {} 完成: {} ({} 条结果)", tag, status, outputs.len());
    }

    fn get_commands(&self, db: &rusqlite::Connection, template_id: Option<i64>) -> Vec<String> {
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
                .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
        }

        let mut cmds = Vec::new();
        for cid in cmd_ids {
            if let Ok(cmd_text) = db.query_row(
                "SELECT command FROM command_pool WHERE id=?1",
                rusqlite::params![cid],
                |r| r.get::<_, String>(0),
            ) {
                cmds.push(cmd_text);
            }
        }
        cmds
    }
}
