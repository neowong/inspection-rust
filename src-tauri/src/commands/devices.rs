use std::sync::Arc;
use tauri::State;
use rusqlite::types::ToSql;

use crate::AppState;
use crate::db::models::{
    Device, DeviceCreate, DeviceUpdate, DeviceStatusLog,
    DEVICE_COLUMNS, device_from_row, status_log_from_row,
};
use crate::services::crypto::CryptoService;

use std::net::{TcpStream, IpAddr, SocketAddr};
use std::str::FromStr;
use std::time::Duration;

// ============================================================
// Helpers
// ============================================================

/// 验证 IP 地址格式（IPv4，4 段，每段 0-255）
fn validate_ip(ip: &str) -> Result<(), String> {
    let parts: Vec<&str> = ip.trim().split('.').collect();
    if parts.len() != 4 {
        return Err(format!("无效的 IP 地址格式: {}", ip));
    }
    for part in &parts {
        let num: u16 = part
            .parse()
            .map_err(|_| format!("无效的 IP 地址段: {}", part))?;
        if num > 255 {
            return Err(format!("IP 地址段超出范围 (0-255): {}", part));
        }
    }
    Ok(())
}

/// 检查设备名称或 IP 是否唯一
fn check_unique(
    conn: &rusqlite::Connection,
    name: &str,
    ip: &str,
    exclude_id: Option<i64>,
) -> Result<(), String> {
    let count: i64 = if let Some(eid) = exclude_id {
        conn.query_row(
            "SELECT COUNT(*) FROM devices WHERE (name = ?1 OR ip = ?2) AND id != ?3",
            rusqlite::params![name, ip, eid],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?
    } else {
        conn.query_row(
            "SELECT COUNT(*) FROM devices WHERE name = ?1 OR ip = ?2",
            rusqlite::params![name, ip],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?
    };

    if count > 0 {
        let name_count: i64 = if let Some(eid) = exclude_id {
            conn.query_row(
                "SELECT COUNT(*) FROM devices WHERE name = ?1 AND id != ?2",
                rusqlite::params![name, eid],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM devices WHERE name = ?1",
                rusqlite::params![name],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?
        };

        if name_count > 0 {
            return Err(format!("设备名称 '{}' 已存在", name));
        }
        return Err(format!("IP 地址 '{}' 已存在", ip));
    }

    Ok(())
}

// ============================================================
// Query Commands
// ============================================================

/// 获取设备列表，支持按厂商和状态筛选
#[tauri::command]
pub fn list_devices(
    vendor: Option<String>,
    status: Option<String>,
    state: State<AppState>,
) -> Result<Vec<Device>, String> {
    let conn = state.db.lock();

    let mut sql = format!("SELECT {} FROM devices WHERE 1=1", DEVICE_COLUMNS);
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(ref v) = vendor {
        sql.push_str(" AND vendor = ?");
        params.push(Box::new(v.clone()));
    }
    if let Some(ref s) = status {
        sql.push_str(" AND status = ?");
        params.push(Box::new(s.clone()));
    }

    sql.push_str(" ORDER BY created_at DESC");

    let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
    crate::db::query::query_all(&conn, &sql, &param_refs, device_from_row)
}

/// 获取单个设备详情
#[tauri::command]
pub fn get_device(device_id: i64, state: State<AppState>) -> Result<Device, String> {
    let conn = state.db.lock();

    let sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
    crate::db::query::query_one(&conn, &sql, rusqlite::params![device_id], device_from_row)?
        .ok_or_else(|| format!("设备 ID {} 不存在", device_id))
}

// ============================================================
// Mutate Commands
// ============================================================

/// 创建设备
#[tauri::command]
pub fn create_device(data: DeviceCreate, state: State<AppState>) -> Result<Device, String> {
    // 1. 验证 IP 地址
    validate_ip(&data.ip)?;

    let conn = state.db.lock();

    // 2. 检查名称和 IP 唯一性
    check_unique(&conn, &data.name, &data.ip, None)?;

    // 3. 加密 SSH 密码（如果提供）
    let encrypted_password = match data.ssh_password_encrypted {
        Some(ref pass) if !pass.is_empty() => Some(CryptoService::encrypt(pass)?),
        _ => None,
    };

    let ssh_port = data.ssh_port.unwrap_or(22);
    let status = data.status.as_deref().unwrap_or("unknown");

    // 4. 插入数据库
    conn.execute(
        "INSERT INTO devices (name, ip, device_type, vendor, model, ssh_username, ssh_password_encrypted, ssh_port, template_id, status, last_checked_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            data.name,
            data.ip,
            data.device_type,
            data.vendor,
            data.model,
            data.ssh_username,
            encrypted_password,
            ssh_port,
            data.template_id,
            status,
            data.last_checked_at,
        ],
    )
    .map_err(|e| e.to_string())?;

    let last_id = conn.last_insert_rowid();

    // 5. 返回新创建的设备
    let sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
    crate::db::query::query_one(&conn, &sql, rusqlite::params![last_id], device_from_row)?
        .ok_or_else(|| "创建设备后查询失败".to_string())
}

/// 更新设备信息
#[tauri::command]
pub fn update_device(
    device_id: i64,
    data: DeviceUpdate,
    state: State<AppState>,
) -> Result<Device, String> {
    let conn = state.db.lock();

    // 验证设备存在
    let sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
    let existing = crate::db::query::query_one(
        &conn,
        &sql,
        rusqlite::params![device_id],
        device_from_row,
    )?
    .ok_or_else(|| format!("设备 ID {} 不存在", device_id))?;

    // 验证 IP（如果提供）
    if let Some(ref ip) = data.ip {
        validate_ip(ip)?;
    }

    // 检查唯一性（如果名称或 IP 变更）
    let new_name = data.name.as_deref().unwrap_or(&existing.name);
    let new_ip = data.ip.as_deref().unwrap_or(&existing.ip);
    if data.name.is_some() || data.ip.is_some() {
        check_unique(&conn, new_name, new_ip, Some(device_id))?;
    }

    // 构建动态 UPDATE
    let mut set_parts: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    let mut idx = 1i32;

    macro_rules! push_field {
        ($field:ident, $col:expr) => {
            if let Some(ref val) = data.$field {
                set_parts.push(format!("{} = ?{}", $col, idx));
                params.push(Box::new(val.clone()));
                idx += 1;
            }
        };
    }

    push_field!(name, "name");
    push_field!(ip, "ip");
    push_field!(device_type, "device_type");
    push_field!(vendor, "vendor");
    push_field!(model, "model");
    push_field!(ssh_username, "ssh_username");
    push_field!(ssh_port, "ssh_port");
    push_field!(template_id, "template_id");
    push_field!(status, "status");
    push_field!(last_checked_at, "last_checked_at");

    // 处理密码加密
    if let Some(ref pass) = data.ssh_password_encrypted {
        if !pass.is_empty() {
            let encrypted = CryptoService::encrypt(pass)?;
            set_parts.push(format!("ssh_password_encrypted = ?{}", idx));
            params.push(Box::new(encrypted));
            idx += 1;
        }
    }

    if set_parts.is_empty() {
        return Ok(existing);
    }

    set_parts.push("updated_at = datetime('now')".to_string());

    let update_sql = format!(
        "UPDATE devices SET {} WHERE id = ?{}",
        set_parts.join(", "),
        idx
    );
    params.push(Box::new(device_id));

    let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&update_sql, param_refs.as_slice())
        .map_err(|e| e.to_string())?;

    // 返回更新后的设备
    let query_sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
    crate::db::query::query_one(
        &conn,
        &query_sql,
        rusqlite::params![device_id],
        device_from_row,
    )?
    .ok_or_else(|| format!("更新后查询设备 ID {} 失败", device_id))
}

/// 删除设备
#[tauri::command]
pub fn delete_device(device_id: i64, state: State<AppState>) -> Result<(), String> {
    let mut conn = state.db.lock();

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // 先删除关联的巡检记录
    tx.execute(
        "DELETE FROM inspection_records WHERE device_id = ?1",
        rusqlite::params![device_id],
    )
    .map_err(|e| e.to_string())?;

    // 再删除设备
    let affected = tx
        .execute(
            "DELETE FROM devices WHERE id = ?1",
            rusqlite::params![device_id],
        )
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("设备 ID {} 不存在", device_id));
    }

    tx.commit().map_err(|e| e.to_string())?;

    Ok(())
}

/// 批量删除设备
#[tauri::command]
pub fn batch_delete_devices(ids: Vec<i64>, state: State<AppState>) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    let mut conn = state.db.lock();

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    for id in &ids {
        // 先删除关联的巡检记录
        tx.execute(
            "DELETE FROM inspection_records WHERE device_id = ?1",
            rusqlite::params![id],
        )
        .map_err(|e| e.to_string())?;

        tx.execute("DELETE FROM devices WHERE id = ?1", rusqlite::params![id])
            .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;

    Ok(())
}

// ============================================================
// Status Check Commands
// ============================================================

/// 检查单个设备连通状态（内部实现）
/// 拆分为：读取设备信息 → TCP 检测（锁外）→ 写入结果
fn check_device_status_inner(
    app_state: &AppState,
    device_id: i64,
) -> Result<serde_json::Value, String> {
    // 1. 读取设备信息（短暂获锁）
    let device = {
        let conn = app_state.db.lock();
        let sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
        crate::db::query::query_one(
            &conn,
            &sql,
            rusqlite::params![device_id],
            device_from_row,
        )?
        .ok_or_else(|| format!("设备 ID {} 不存在", device_id))?
    }; // 锁释放

    // 2. TCP 连接检测（锁外，5 秒超时）
    let ip_addr = IpAddr::from_str(&device.ip)
        .map_err(|_| format!("无法解析设备 IP 地址: {}", device.ip))?;
    let socket_addr = SocketAddr::new(ip_addr, device.ssh_port as u16);

    let new_status = match TcpStream::connect_timeout(&socket_addr, Duration::from_secs(5)) {
        Ok(_stream) => "online",
        Err(_) => "offline",
    };

    let now = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    // 3. 写入结果（短暂获锁）
    {
        let conn = app_state.db.lock();

        conn.execute(
            "INSERT INTO device_status_logs (device_id, old_status, new_status, checked_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![device_id, device.status, new_status, now],
        )
        .map_err(|e| e.to_string())?;

        conn.execute(
            "UPDATE devices SET status = ?1, last_checked_at = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![new_status, now, now, device_id],
        )
        .map_err(|e| e.to_string())?;
    } // 锁释放

    Ok(serde_json::json!({
        "device_id": device_id,
        "old_status": device.status,
        "new_status": new_status,
        "checked_at": now,
    }))
}

/// 检查单个设备连通状态
#[tauri::command]
pub fn check_device_status(
    device_id: i64,
    state: State<AppState>,
) -> Result<serde_json::Value, String> {
    check_device_status_inner(&*state, device_id)
}

/// 检查所有设备连通状态（并发）
#[tauri::command]
pub async fn check_all_devices_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let devices = {
        let conn = state.db.lock();
        let sql = format!("SELECT {} FROM devices", DEVICE_COLUMNS);
        crate::db::query::query_all(&conn, &sql, &[], device_from_row)?
    };

    let total = devices.len();
    let db = state.db.clone();

    let handles: Vec<_> = devices.into_iter().map(|device| {
        let db = Arc::clone(&db);
        tokio::task::spawn_blocking(move || {
            let ip_addr = std::net::IpAddr::from_str(&device.ip).ok();
            let socket_addr = ip_addr.map(|ip| std::net::SocketAddr::new(ip, device.ssh_port as u16));
            let new_status = match socket_addr {
                Some(addr) => match std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
                    Ok(_) => "online",
                    Err(_) => "offline",
                },
                None => "offline",
            };
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            {
                let conn = db.lock();
                let _ = conn.execute(
                    "INSERT INTO device_status_logs (device_id, old_status, new_status, checked_at) VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![device.id, device.status, new_status, now],
                );
                let _ = conn.execute(
                    "UPDATE devices SET status = ?1, last_checked_at = ?2, updated_at = ?3 WHERE id = ?4",
                    rusqlite::params![new_status, now, now, device.id],
                );
            }
            (device.id, device.name.clone(), new_status.to_string())
        })
    }).collect();

    let mut online_count = 0u32;
    let mut offline_count = 0u32;

    for handle in handles {
        match handle.await {
            Ok((_id, _name, status)) => {
                if status == "online" { online_count += 1; }
                else { offline_count += 1; }
            }
            Err(e) => {
                tracing::warn!("设备状态检测任务失败: {}", e);
                offline_count += 1;
            }
        }
    }

    Ok(serde_json::json!({
        "total": total,
        "online": online_count,
        "offline": offline_count,
    }))
}

// ============================================================
// Status Log Commands
// ============================================================

/// 获取设备状态变更日志
#[tauri::command]
pub fn get_device_status_log(
    device_id: i64,
    state: State<AppState>,
) -> Result<Vec<DeviceStatusLog>, String> {
    let conn = state.db.lock();

    crate::db::query::query_all(
        &conn,
        "SELECT id, device_id, old_status, new_status, checked_at FROM device_status_logs WHERE device_id = ?1 ORDER BY checked_at DESC",
        rusqlite::params![device_id],
        status_log_from_row,
    )
}
