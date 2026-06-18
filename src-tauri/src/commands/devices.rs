use rusqlite::types::ToSql;
use std::sync::Arc;
use tauri::State;

use crate::db::models::{
    device_from_row, now_str, Device, DeviceCreate, DeviceUpdate,
    DEVICE_COLUMNS,
};
use crate::services::crypto::CryptoService;
use crate::AppState;

use std::net::{IpAddr, SocketAddr, TcpStream};
use std::str::FromStr;
use std::time::Duration;

// ============================================================
// Helpers
// ============================================================

/// 验证 IP 地址格式
fn validate_ip(ip: &str) -> Result<(), String> {
    if ip.trim().is_empty() || ip.trim().parse::<std::net::IpAddr>().is_err() {
        Err(format!("请输入有效的 IP 地址: {}", ip))
    } else {
        Ok(())
    }
}

/// 校验 SSH 端口范围（1..=65535），返回 u16。
/// `ssh_port` 在 DB 中以 i64 存储，`as u16` 会静默截断越界值（如 65537→1、负数→大值），
/// 导致连接到错误端口且难以排查，故入库前必须校验。
fn validate_port(port: i64) -> Result<u16, String> {
    u16::try_from(port)
        .map_err(|_| format!("SSH 端口必须在 1..=65535 范围内，当前为 {}", port))
}

/// 将 DB 中的 i64 端口安全转换为 u16，越界则返回 None（调用方按离线处理）。
fn port_u16(port: i64) -> Option<u16> {
    u16::try_from(port).ok().filter(|&p| p > 0)
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

    let ssh_port = validate_port(data.ssh_port.unwrap_or(22))?;
    let status = data.status.as_deref().unwrap_or("unknown");

    // 4. 插入数据库
    conn.execute(
        "INSERT INTO devices (name, ip, device_type, vendor, model, ssh_username, ssh_password_encrypted, ssh_port, template_id, status, last_checked_at, serial_number, manufacturing_date, sysname) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        rusqlite::params![
            data.name,
            data.ip,
            data.device_type,
            data.vendor,
            data.model,
            data.ssh_username,
            encrypted_password,
            i64::from(ssh_port),
            data.template_id,
            status,
            data.last_checked_at,
            data.serial_number,
            data.manufacturing_date,
            data.sysname,
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
    let existing =
        crate::db::query::query_one(&conn, &sql, rusqlite::params![device_id], device_from_row)?
            .ok_or_else(|| format!("设备 ID {} 不存在", device_id))?;

    // 验证 IP（如果提供）
    if let Some(ref ip) = data.ip {
        validate_ip(ip)?;
    }

    // 验证 SSH 端口（如果提供）
    if let Some(port) = data.ssh_port {
        validate_port(port)?;
    }

    // 检查唯一性（如果名称或 IP 变更）
    let new_name = data.name.as_deref().unwrap_or(&existing.name);
    let new_ip = data.ip.as_deref().unwrap_or(&existing.ip);
    if data.name.is_some() || data.ip.is_some() {
        check_unique(&conn, new_name, new_ip, Some(device_id))?;
    }

    // 构建动态 UPDATE
    let mut updater = crate::db::db_helpers::DynamicUpdate::new();
    updater.push_opt("name", &data.name);
    updater.push_opt("ip", &data.ip);
    updater.push_opt("device_type", &data.device_type);
    updater.push_opt("vendor", &data.vendor);
    updater.push_opt("model", &data.model);
    updater.push_opt("ssh_username", &data.ssh_username);
    updater.push_opt("ssh_port", &data.ssh_port);
    updater.push_opt("template_id", &data.template_id);
    updater.push_opt("status", &data.status);
    updater.push_opt("last_checked_at", &data.last_checked_at);
    updater.push_opt("serial_number", &data.serial_number);
    updater.push_opt("manufacturing_date", &data.manufacturing_date);
    updater.push_opt("sysname", &data.sysname);

    // 处理密码加密
    if let Some(ref pass) = data.ssh_password_encrypted {
        if !pass.is_empty() {
            let encrypted = CryptoService::encrypt(pass)?;
            updater.push_raw("ssh_password_encrypted", encrypted);
        }
    }

    if updater.is_empty() {
        return Ok(existing);
    }

    // 统一用 now_str()（Local 时间），与 create/check 等路径一致，避免 updated_at 时区不一致
    updater.push_raw("updated_at", now_str());

    let (set_parts, mut params) = updater.finish();
    let idx = params.len() as i32 + 1;

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
        crate::db::query::query_one(&conn, &sql, rusqlite::params![device_id], device_from_row)?
            .ok_or_else(|| format!("设备 ID {} 不存在", device_id))?
    }; // 锁释放

    // 2. TCP 连接检测（锁外，5 秒超时）
    let ip_addr =
        IpAddr::from_str(&device.ip).map_err(|_| format!("无法解析设备 IP 地址: {}", device.ip))?;
    let port = port_u16(device.ssh_port).ok_or_else(|| {
        format!("设备 SSH 端口非法: {}", device.ssh_port)
    })?;
    let socket_addr = SocketAddr::new(ip_addr, port);

    let new_status = match TcpStream::connect_timeout(&socket_addr, Duration::from_secs(5)) {
        Ok(_stream) => "online",
        Err(_) => "offline",
    };

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

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
pub async fn check_all_devices_status(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
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
            let socket_addr = ip_addr.and_then(|ip| port_u16(device.ssh_port).map(|p| std::net::SocketAddr::new(ip, p)));
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
                // 仅在状态变更时记录日志，避免状态日志表无限增长
                if device.status.as_str() != new_status {
                    let _ = conn.execute(
                        "INSERT INTO device_status_logs (device_id, old_status, new_status, checked_at) VALUES (?1, ?2, ?3, ?4)",
                        rusqlite::params![device.id, device.status, new_status, now],
                    );
                }
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
                if status == "online" {
                    online_count += 1;
                } else {
                    offline_count += 1;
                }
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
// Device Model Detection
// ============================================================

/// 自动检测设备型号（通过 SSH 登录执行厂商命令）
#[tauri::command]
pub async fn detect_device_model(
    ip: String,
    ssh_port: u16,
    ssh_username: String,
    ssh_password: String,
    vendor: String,
) -> Result<String, String> {
    use crate::services::inspection_runner::{self, SSHSessionSource};

    // 仅支持 H3C
    let manu_cmd = match vendor.as_str() {
        "H3C" | "华三" => "display device manuinfo",
        _ => return Err("当前仅支持 H3C 设备自动检测型号".to_string()),
    };
    let sysname_cmd = "display current-configuration | include sysname";

    tokio::task::spawn_blocking(move || {
        let source = SSHSessionSource {
            host: ip,
            port: ssh_port,
            username: ssh_username,
            password: ssh_password.clone(),
        };

        let session = inspection_runner::connect_session(&source)?;
        let (base_prompt, mut channel) = inspection_runner::open_shell_session(
            &session,
            &vendor,
            &source.password,
            &source.host,
            None,
        )?;

        // 用闭包执行命令，确保即便 send_command 失败也能关闭 channel / 断开会话，避免 SSH 资源泄漏
        let cmd_result: Result<(String, String), String> = (|| {
            let output = inspection_runner::send_command(
                &mut channel,
                manu_cmd,
                &base_prompt,
                &source.password,
                &source.host,
                &vendor,
                None,
            )?;

            let sysname_output = inspection_runner::send_command(
                &mut channel,
                sysname_cmd,
                &base_prompt,
                &source.password,
                &source.host,
                &vendor,
                None,
            )
            .unwrap_or_default();

            Ok((output, sysname_output))
        })();

        // 无论命令是否成功都清理 SSH 资源
        let _ = channel.close();
        let _ = session.disconnect(None, "done", None);

        let (output, sysname_output) = cmd_result?;

        // 从输出中提取信息
        let cleaned = inspection_runner::clean_output(&output, &base_prompt);
        let sysname_cleaned = inspection_runner::clean_output(&sysname_output, &base_prompt);
        let mut info = parse_h3c_device_info(&cleaned);
        info.sysname = parse_sysname(&sysname_cleaned);
        Ok(serde_json::json!(info).to_string())
    })
    .await
    .map_err(|e| format!("检测任务失败: {}", e))?
}

/// H3C 设备信息
#[derive(serde::Serialize)]
struct H3cDeviceInfo {
    model: Option<String>,
    serial_number: Option<String>,
    manufacturing_date: Option<String>,
    sysname: Option<String>,
}

/// 从 H3C display device manuinfo 输出中解析设备信息
fn parse_h3c_device_info(output: &str) -> H3cDeviceInfo {
    let mut model = None;
    let mut serial_number = None;
    let mut manufacturing_date = None;

    for line in output.lines() {
        let trimmed = line.trim();
        if model.is_none() {
            if let Some(v) = extract_h3c_field(trimmed, &["DEVICE_NAME", "PRODUCT_NAME"]) {
                model = Some(v);
            }
        }
        if serial_number.is_none() {
            if let Some(v) = extract_h3c_field(trimmed, &["DEVICE_SERIAL_NUMBER", "SERIAL_NUMBER"])
            {
                serial_number = Some(v);
            }
        }
        if manufacturing_date.is_none() {
            if let Some(v) = extract_h3c_field(trimmed, &["MANUFACTURING_DATE"]) {
                manufacturing_date = Some(v);
            }
        }
    }

    H3cDeviceInfo {
        model,
        serial_number,
        manufacturing_date,
        sysname: None,
    }
}

fn parse_sysname(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.to_lowercase().starts_with("sysname ") {
            let name = trimmed.split_whitespace().nth(1).unwrap_or("").trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// 从 H3C 键值行中提取值，格式: "KEY          : VALUE"
fn extract_h3c_field(line: &str, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(rest) = line.strip_prefix(key) {
            // rest = "          : VALUE" 或 ": VALUE"
            let after_colon = rest.trim_start().strip_prefix(':')?;
            let value = after_colon.trim();
            // 过滤空值和表头分隔线（如 "----"）
            if !value.is_empty() && !value.contains("----") {
                return Some(value.to_string());
            }
        }
    }
    None
}
