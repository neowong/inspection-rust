use std::process::Command;
use tauri::State;
use crate::AppState;
use crate::db::models::{Device, DeviceCreate, DeviceUpdate, DeviceStatusLog};
use crate::services::crypto::CryptoService;

fn device_from_row(row: &rusqlite::Row) -> rusqlite::Result<Device> {
    Ok(Device {
        id: row.get(0)?, group_name: row.get(1)?, name: row.get(2)?, ip: row.get(3)?,
        device_type: row.get(4)?, vendor: row.get(5)?, model: row.get(6)?,
        inspection_mode: row.get(7)?, ssh_username: row.get(8)?, ssh_password_encrypted: row.get(9)?,
        ssh_port: row.get(10)?, web_url: row.get(11)?, web_port: row.get(12)?,
        template_id: row.get(13)?, db_type: row.get(14)?, db_port: row.get(15)?,
        db_username: row.get(16)?, db_password_encrypted: row.get(17)?, db_os_user: row.get(18)?,
        status: row.get(19)?, last_checked_at: row.get(20)?, created_at: row.get(21)?, updated_at: row.get(22)?,
    })
}

fn validate_ip(ip: &str) -> Result<(), String> {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 { return Err("无效的 IP 地址格式".into()); }
    for p in parts {
        let n: i32 = p.parse().map_err(|_| "IP 包含非数字段".to_string())?;
        if n > 255 { return Err("IP 地址各段必须在 0-255 之间".into()); }
    }
    Ok(())
}

fn check_unique(db: &rusqlite::Connection, name: &str, ip: &str, exclude_id: Option<i64>) -> Result<(), String> {
    let exists = if let Some(eid) = exclude_id {
        db.query_row("SELECT COUNT(*)>0 FROM devices WHERE (name=?1 OR ip=?2) AND id!=?3", rusqlite::params![name, ip, eid], |r| r.get(0))
    } else {
        db.query_row("SELECT COUNT(*)>0 FROM devices WHERE name=?1 OR ip=?2", rusqlite::params![name, ip], |r| r.get(0))
    }.unwrap_or(false);
    if exists { Err("已存在同名或同 IP 的设备".into()) } else { Ok(()) }
}

#[tauri::command]
pub fn list_devices(group: Option<String>, vendor: Option<String>, status: Option<String>, state: State<AppState>) -> Result<Vec<Device>, String> {
    let db = state.db.lock();
    let mut sql = String::from("SELECT * FROM devices WHERE 1=1");
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(ref v) = vendor { sql.push_str(" AND vendor = ?"); params.push(Box::new(v.clone())); }
    if let Some(ref s) = status { sql.push_str(" AND status = ?"); params.push(Box::new(s.clone())); }
    if let Some(ref g) = group { sql.push_str(" AND group_name = ?"); params.push(Box::new(g.clone())); }
    sql.push_str(" ORDER BY created_at DESC");

    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows: Vec<Device> = stmt.query_map(param_refs.as_slice(), device_from_row)
        .map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn get_device(device_id: i64, state: State<AppState>) -> Result<Device, String> {
    let db = state.db.lock();
    db.query_row("SELECT * FROM devices WHERE id = ?1", rusqlite::params![device_id], device_from_row)
        .map_err(|_| "设备不存在".into())
}

#[tauri::command]
pub fn create_device(data: DeviceCreate, state: State<AppState>) -> Result<Device, String> {
    validate_ip(&data.ip)?;
    let db = state.db.lock();
    check_unique(&db, &data.name, &data.ip, None)?;

    let encrypted_pw = data.ssh_password.as_ref().and_then(|p| CryptoService::encrypt(p).ok());
    let db_encrypted_pw = data.db_password.as_ref().and_then(|p| CryptoService::encrypt(p).ok());

    db.execute(
        "INSERT INTO devices (group_name,name,ip,device_type,vendor,model,inspection_mode,ssh_username,ssh_password_encrypted,ssh_port,template_id,db_type,db_port,db_username,db_password_encrypted,db_os_user,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,datetime('now'),datetime('now'))",
        rusqlite::params![data.group.as_deref().unwrap_or("network"), data.name, data.ip, data.device_type, data.vendor, data.model, data.inspection_mode.as_deref().unwrap_or("ssh"), data.ssh_username, encrypted_pw, data.ssh_port.unwrap_or(22), data.template_id, data.db_type, data.db_port, data.db_username, db_encrypted_pw, data.db_os_user],
    ).map_err(|e| format!("添加设备失败: {}", e))?;
    let id = db.last_insert_rowid();
    db.query_row("SELECT * FROM devices WHERE id = ?1", rusqlite::params![id], device_from_row).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_device(device_id: i64, data: DeviceUpdate, state: State<AppState>) -> Result<Device, String> {
    let db = state.db.lock();
    let device = db.query_row("SELECT * FROM devices WHERE id = ?1", rusqlite::params![device_id], device_from_row)
        .map_err(|_| "设备不存在".to_string())?;
    let name = data.name.as_deref().unwrap_or(&device.name);
    let ip = data.ip.as_deref().unwrap_or(&device.ip);
    if data.name.is_some() || data.ip.is_some() { check_unique(&db, name, ip, Some(device_id))?; }

    let encrypted_pw = data.ssh_password.as_ref().and_then(|p| CryptoService::encrypt(p).ok());
    let db_encrypted_pw = data.db_password.as_ref().and_then(|p| CryptoService::encrypt(p).ok());

    db.execute(
        "UPDATE devices SET group_name=?1,name=?2,ip=?3,device_type=?4,vendor=?5,model=?6,inspection_mode=?7,ssh_username=?8,ssh_password_encrypted=COALESCE(?9,ssh_password_encrypted),ssh_port=?10,template_id=?11,db_type=?12,db_port=?13,db_username=?14,db_password_encrypted=COALESCE(?15,db_password_encrypted),db_os_user=?16,updated_at=datetime('now') WHERE id=?17",
        rusqlite::params![data.group.as_deref().unwrap_or(&device.group_name), name, ip, data.device_type.as_deref().unwrap_or(&device.device_type), data.vendor.as_deref().unwrap_or(&device.vendor), data.model.as_deref().or(device.model.as_deref()), data.inspection_mode.as_deref().unwrap_or(&device.inspection_mode), data.ssh_username.as_deref().or(device.ssh_username.as_deref()), encrypted_pw, data.ssh_port.unwrap_or(device.ssh_port), data.template_id.or(device.template_id), data.db_type.as_deref().or(device.db_type.as_deref()), data.db_port.or(device.db_port), data.db_username.as_deref().or(device.db_username.as_deref()), db_encrypted_pw, data.db_os_user.as_deref().or(device.db_os_user.as_deref()), device_id],
    ).map_err(|e| format!("更新设备失败: {}", e))?;
    db.query_row("SELECT * FROM devices WHERE id = ?1", rusqlite::params![device_id], device_from_row).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_device(device_id: i64, state: State<AppState>) -> Result<(), String> {
    let db = state.db.lock();
    let exists: bool = db.query_row("SELECT COUNT(*)>0 FROM devices WHERE id=?1", rusqlite::params![device_id], |r| r.get(0)).unwrap_or(false);
    if !exists { return Err("设备不存在".into()); }
    db.execute("DELETE FROM device_status_logs WHERE device_id=?1", rusqlite::params![device_id]).ok();
    db.execute("DELETE FROM devices WHERE id=?1", rusqlite::params![device_id]).ok();
    Ok(())
}

#[tauri::command]
pub fn batch_delete_devices(ids: Vec<i64>, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    for id in ids {
        db.execute("DELETE FROM device_status_logs WHERE device_id=?1", rusqlite::params![id]).ok();
        db.execute("DELETE FROM devices WHERE id=?1", rusqlite::params![id]).ok();
    }
    Ok(serde_json::json!({"success": true}))
}

#[tauri::command]
pub fn get_device_status_log(device_id: i64, limit: Option<i64>, state: State<AppState>) -> Result<Vec<DeviceStatusLog>, String> {
    let db = state.db.lock();
    let limit = limit.unwrap_or(50);
    let mut stmt = db.prepare(
        "SELECT id, device_id, old_status, new_status, checked_at FROM device_status_logs WHERE device_id=?1 ORDER BY checked_at DESC LIMIT ?2"
    ).map_err(|e| e.to_string())?;
    let rows: Vec<DeviceStatusLog> = stmt.query_map(rusqlite::params![device_id, limit], |row| Ok(DeviceStatusLog {
        id: row.get(0)?, device_id: row.get(1)?, old_status: row.get(2)?, new_status: row.get(3)?, checked_at: row.get(4)?,
    })).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

fn ping_device(ip: &str, port: i64) -> String {
    let ping_ok = Command::new("ping").arg("-c").arg("1").arg("-W").arg("2").arg(ip)
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .status().map(|s| s.success()).unwrap_or(false);
    let addr = format!("{}:{}", ip, port);
    let tcp_ok = std::net::TcpStream::connect_timeout(
        &addr.parse().unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap()),
        std::time::Duration::from_secs(3),
    ).is_ok();
    if ping_ok || tcp_ok { "online".into() } else { "offline".into() }
}

#[tauri::command]
pub fn check_device_status(device_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let device = db.query_row("SELECT * FROM devices WHERE id=?1", rusqlite::params![device_id], device_from_row)
        .map_err(|_| "设备不存在".to_string())?;
    let old_status = device.status.clone();
    let new_status = ping_device(&device.ip, device.ssh_port);
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let changed = old_status != new_status;
    if changed {
        db.execute("INSERT INTO device_status_logs (device_id, old_status, new_status, checked_at) VALUES (?1,?2,?3,?4)",
            rusqlite::params![device_id, old_status, new_status, now]).ok();
    }
    db.execute("UPDATE devices SET status=?1, last_checked_at=?2 WHERE id=?3", rusqlite::params![new_status, now, device_id]).ok();
    Ok(serde_json::json!({"success": true, "device_id": device_id, "status": new_status, "changed": changed}))
}

#[tauri::command]
pub fn check_all_devices_status(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let mut stmt = db.prepare("SELECT * FROM devices").map_err(|e| e.to_string())?;
    let devices: Vec<Device> = stmt.query_map([], device_from_row).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let total = devices.len();
    let mut changed = 0i64;
    for device in devices {
        let old = device.status.clone();
        let new = ping_device(&device.ip, device.ssh_port);
        if old != new {
            db.execute("INSERT INTO device_status_logs (device_id, old_status, new_status, checked_at) VALUES (?1,?2,?3,?4)",
                rusqlite::params![device.id, old, new, now]).ok();
            changed += 1;
        }
        db.execute("UPDATE devices SET status=?1, last_checked_at=?2 WHERE id=?3", rusqlite::params![new, now, device.id]).ok();
    }
    Ok(serde_json::json!({"success": true, "total": total, "changed": changed}))
}
