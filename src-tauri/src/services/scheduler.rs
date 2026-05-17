/// 定时任务调度器 — 设备状态定期检测
///
/// 对应 Python: backend/app/services/scheduler.py
use std::net::TcpStream;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

/// 设备状态检测配置
pub struct SchedulerConfig {
    pub interval_minutes: u64,
    pub enabled: bool,
    pub timeout_seconds: f64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            interval_minutes: 1,
            enabled: true,
            timeout_seconds: 2.0,
        }
    }
}

/// 检测设备在线状态（Ping + TCP 端口探测）
fn ping_device(ip: &str, port: u16) -> String {
    let ping_ok = Command::new("ping")
        .arg("-c").arg("1").arg("-W").arg("2").arg(ip)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    let addr = format!("{}:{}", ip, port);
    let tcp_ok = TcpStream::connect_timeout(
        &addr.parse().unwrap_or(std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
            std::net::Ipv4Addr::new(0, 0, 0, 0), 0,
        ))),
        Duration::from_secs_f64(3.0),
    ).is_ok();

    if ping_ok || tcp_ok { "online".into() } else { "offline".into() }
}

/// 执行一次设备状态检测
pub fn check_all_devices_status(db_path: &str) -> Result<(), String> {
    let db = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;

    // 排除离线巡检模式的设备
    let mut stmt = db.prepare(
        "SELECT id, name, ip, ssh_port, status FROM devices WHERE inspection_mode != 'offline'"
    ).map_err(|e| e.to_string())?;

    let devices: Vec<(i64, String, String, i64, String)> = stmt.query_map([], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    if devices.is_empty() { return Ok(()); }

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let mut changed_count = 0i64;

    for (id, name, ip, port, old_status) in &devices {
        let new_status = ping_device(ip, *port as u16);

        if *old_status != new_status {
            db.execute(
                "INSERT INTO device_status_logs (device_id, old_status, new_status, checked_at) VALUES (?1,?2,?3,?4)",
                rusqlite::params![id, old_status, new_status, now],
            ).ok();
            db.execute("UPDATE devices SET status=?1, last_checked_at=?2 WHERE id=?3",
                rusqlite::params![new_status, now, id]).ok();
            changed_count += 1;
            info!("设备状态变更: {} ({}) {} -> {}", name, ip, old_status, new_status);
        } else {
            db.execute("UPDATE devices SET last_checked_at=?1 WHERE id=?2",
                rusqlite::params![now, id]).ok();
        }
    }

    if changed_count > 0 {
        info!("设备状态检测完成: {} 台, {} 台状态变化", devices.len(), changed_count);
    }

    Ok(())
}

/// 启动设备状态检测调度循环
/// 在 Tauri 应用中通过 tokio::spawn 在后台运行
pub async fn start_device_status_checker(db_path: String, config: SchedulerConfig) {
    if !config.enabled {
        info!("设备状态检测已禁用");
        return;
    }

    info!("启动设备状态检测（间隔: {} 分钟）", config.interval_minutes);

    loop {
        let db_path = db_path.clone();
        tokio::task::spawn_blocking(move || {
            check_all_devices_status(&db_path).ok();
        }).await.ok();

        tokio::time::sleep(Duration::from_secs(config.interval_minutes * 60)).await;
    }
}
