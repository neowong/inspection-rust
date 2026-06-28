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

/// 校验会被拼入远端 shell 的标识符（容器名/Pod名/DB用户名）。
/// 只允许 [A-Za-z0-9_.:-]，防止 `;` `$()` `|` 等造成命令注入。
/// 空串允许（由调用方走默认值兜底）。
pub(crate) fn validate_shell_identifier(field: &str, label: &str) -> Result<(), String> {
    if field.is_empty() {
        return Ok(());
    }
    let ok = field.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'));
    if ok {
        Ok(())
    } else {
        Err(format!(
            "{} 只能包含字母、数字和 _ . : - ，不能包含空格或特殊符号（防止命令注入）",
            label
        ))
    }
}

/// 单引号包裹并转义内部单引号（用于 sh -c 内层 `'...'` 段）。
pub(crate) fn shell_quote_single(s: &str) -> String {
    s.replace('\'', "'\\''")
}

/// 检查设备名称或 IP 是否唯一
fn check_unique(
    conn: &rusqlite::Connection,
    name: &str,
    ip: &str,
    device_type: &str,
    exclude_id: Option<i64>,
) -> Result<(), String> {
    // 名称全局唯一
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

    // IP 按设备类型唯一（允许同 IP 不同类型：如服务器 192.168.1.10 + 数据库 192.168.1.10）
    let ip_count: i64 = if let Some(eid) = exclude_id {
        conn.query_row(
            "SELECT COUNT(*) FROM devices WHERE ip = ?1 AND device_type = ?2 AND id != ?3",
            rusqlite::params![ip, device_type, eid],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?
    } else {
        conn.query_row(
            "SELECT COUNT(*) FROM devices WHERE ip = ?1 AND device_type = ?2",
            rusqlite::params![ip, device_type],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?
    };
    if ip_count > 0 {
        return Err(format!("同类型设备中 IP '{}' 已存在", ip));
    }

    Ok(())
}

// ============================================================
// Query Commands
// ============================================================

/// 获取设备列表，支持按厂商、设备类型和状态筛选
#[tauri::command]
pub fn list_devices(
    vendor: Option<String>,
    device_type: Option<String>,
    status: Option<String>,
    state: State<AppState>,
) -> Result<Vec<Device>, String> {
    tracing::debug!("[list_devices] vendor={:?}, device_type={:?}, status={:?}", vendor, device_type, status);
    let conn = state.db.lock();

    let mut sql = format!("SELECT {} FROM devices WHERE 1=1", DEVICE_COLUMNS);
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(ref v) = vendor {
        sql.push_str(" AND vendor = ?");
        params.push(Box::new(v.clone()));
    }
    if let Some(ref dt) = device_type {
        if dt == "other" {
            // "其它"：排除已知类型
            sql.push_str(" AND device_type NOT IN ('switch','router','firewall','loadbalancer','server','database')");
        } else {
            // 支持逗号分隔的多值过滤，如 "switch,router"
            let types: Vec<&str> = dt.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if !types.is_empty() {
                let placeholders: Vec<String> = types.iter().enumerate().map(|(i, _)| format!("?{}", params.len() + i + 1)).collect();
                sql.push_str(&format!(" AND device_type IN ({})", placeholders.join(",")));
                for t in types {
                    params.push(Box::new(t.to_string()));
                }
            }
        }
    }
    if let Some(ref s) = status {
        sql.push_str(" AND status = ?");
        params.push(Box::new(s.clone()));
    }

    sql.push_str(" ORDER BY created_at DESC");

    let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
    crate::db::query::query_all(&conn, &sql, &param_refs, device_from_row)
}

// ============================================================
// Mutate Commands
// ============================================================

/// 创建设备
#[tauri::command]
pub fn create_device(data: DeviceCreate, state: State<AppState>) -> Result<Device, String> {
    // 1. 验证 IP 地址
    validate_ip(&data.ip)?;

    // 1b. 校验会被拼入远端 shell 的标识符（防止命令注入）
    if let Some(ref n) = data.instance_name {
        validate_shell_identifier(n, "容器/实例名")?;
    }
    if let Some(ref u) = data.db_username {
        validate_shell_identifier(u, "数据库用户名")?;
    }

    let conn = state.db.lock();

    // 2. 检查名称和 IP 唯一性
    check_unique(&conn, &data.name, &data.ip, &data.device_type, None)?;

    // 3. 加密 SSH 密码（如果提供）
    let encrypted_password = match data.ssh_password_encrypted {
        Some(ref pass) if !pass.is_empty() => Some(CryptoService::encrypt(pass)?),
        _ => None,
    };
    // 3b. 加密数据库密码（如果提供）
    let encrypted_db_password = match data.db_password_encrypted {
        Some(ref pass) if !pass.is_empty() => Some(CryptoService::encrypt(pass)?),
        _ => None,
    };

    let ssh_port = validate_port(data.ssh_port.unwrap_or(22))?;
    let db_port = validate_port(data.db_port.unwrap_or(3306))?;
    let status = data.status.as_deref().unwrap_or("unknown");

    // 4. 插入数据库
    conn.execute(
        "INSERT INTO devices (name, ip, device_type, vendor, model, ssh_username, ssh_password_encrypted, ssh_port, template_id, status, last_checked_at, serial_number, manufacturing_date, sysname, cpu_cores, memory_gb, deployment, db_version, instance_name, db_username, db_password_encrypted, db_port, kernel_version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
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
            data.cpu_cores,
            data.memory_gb,
            data.deployment,
            data.db_version,
            data.instance_name,
            data.db_username,
            encrypted_db_password,
            i64::from(db_port),
            data.kernel_version,
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
    // 验证数据库端口（如果提供）
    if let Some(port) = data.db_port {
        validate_port(port)?;
    }

    // 校验会被拼入远端 shell 的标识符（防止命令注入）
    if let Some(ref n) = data.instance_name {
        validate_shell_identifier(n, "容器/实例名")?;
    }
    if let Some(ref u) = data.db_username {
        validate_shell_identifier(u, "数据库用户名")?;
    }

    // 检查唯一性（如果名称或 IP 变更）
    let new_name = data.name.as_deref().unwrap_or(&existing.name);
    let new_ip = data.ip.as_deref().unwrap_or(&existing.ip);
    if data.name.is_some() || data.ip.is_some() {
        let device_type = data.device_type.as_deref().unwrap_or(&existing.device_type);
        check_unique(&conn, new_name, new_ip, device_type, Some(device_id))?;
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
    updater.push_opt("cpu_cores", &data.cpu_cores);
    updater.push_opt("memory_gb", &data.memory_gb);
    updater.push_opt("deployment", &data.deployment);
    updater.push_opt("db_version", &data.db_version);
    updater.push_opt("instance_name", &data.instance_name);
    updater.push_opt("db_username", &data.db_username);
    updater.push_opt("db_port", &data.db_port);
    updater.push_opt("kernel_version", &data.kernel_version);
    if let Some(ref pass) = data.db_password_encrypted {
        if !pass.is_empty() {
            let enc = CryptoService::encrypt(pass)?;
            updater.push_raw("db_password_encrypted", enc);
            // 数据库密码变更后旧的 auth_status 失效，重置为 unknown 等待重新验证
            updater.push_raw("auth_status", "unknown".to_string());
            updater.push_raw("auth_message", Option::<String>::None);
        }
    }

    // 处理密码加密
    if let Some(ref pass) = data.ssh_password_encrypted {
        if !pass.is_empty() {
            let encrypted = CryptoService::encrypt(pass)?;
            updater.push_raw("ssh_password_encrypted", encrypted);
            // 密码变更后旧的 auth_status 失效，重置为 unknown 等待重新验证
            updater.push_raw("auth_status", "unknown".to_string());
            updater.push_raw("auth_message", Option::<String>::None);
        }
    }
    // 用户名变更也重置 auth_status
    if data.ssh_username.is_some() {
        updater.push_raw("auth_status", "unknown".to_string());
        updater.push_raw("auth_message", Option::<String>::None);
    }

    // 静态信息相关字段变更时清除已缓存的旧数据，下次检测时重新获取
    let auth_changed = data.ssh_password_encrypted.is_some()
        || data.ssh_username.is_some()
        || data.db_password_encrypted.is_some()
        || data.db_username.is_some()
        || data.ip.is_some()
        || data.ssh_port.is_some()
        || data.db_port.is_some()
        || data.deployment.is_some()
        || data.vendor.is_some();
    if auth_changed {
        updater.push_raw("auth_status", "unknown".to_string());
        updater.push_raw("auth_message", Option::<String>::None);
        // 清除旧的静态信息，避免用错误凭据获取的数据持久化
        updater.push_raw("sysname", Option::<String>::None);
        updater.push_raw("model", Option::<String>::None);
        updater.push_raw("serial_number", Option::<String>::None);
        updater.push_raw("manufacturing_date", Option::<String>::None);
        updater.push_raw("cpu_cores", Option::<i64>::None);
        updater.push_raw("memory_gb", Option::<i64>::None);
        updater.push_raw("kernel_version", Option::<String>::None);
        updater.push_raw("db_version", Option::<String>::None);
        updater.push_raw("instance_name", Option::<String>::None);
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

    // 先收集关联的巡检记录 report_path，用于事务提交后清理磁盘文件
    let report_files: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT report_path FROM inspection_records WHERE device_id = ?1 AND report_path IS NOT NULL")
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map(rusqlite::params![device_id], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

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

    // 事务成功后清理磁盘上的报告文件（失败不影响 DB 一致性）
    for path in &report_files {
        crate::commands::reports::safe_remove_report(path);
    }

    Ok(())
}

/// 批量删除设备
#[tauri::command]
pub fn batch_delete_devices(ids: Vec<i64>, state: State<AppState>) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    let mut conn = state.db.lock();

    // 先收集所有关联的巡检记录 report_path，用于事务提交后清理磁盘文件
    let mut report_files: Vec<String> = Vec::new();
    for id in &ids {
        let mut stmt = conn
            .prepare("SELECT report_path FROM inspection_records WHERE device_id = ?1 AND report_path IS NOT NULL")
            .map_err(|e| e.to_string())?;
        let paths: Vec<String> = stmt
            .query_map(rusqlite::params![id], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        report_files.extend(paths);
    }

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

    // 事务成功后清理磁盘上的报告文件（失败不影响 DB 一致性）
    for path in &report_files {
        crate::commands::reports::safe_remove_report(path);
    }

    Ok(())
}

// ============================================================
// Status Check Commands
// ============================================================

/// 检查单个设备连通状态（内部实现）
/// 参数为 Arc<Mutex<Connection>> 以支持 spawn_blocking 跨线程
fn check_device_status_inner(
    db: &Arc<parking_lot::Mutex<rusqlite::Connection>>,
    device_id: i64,
) -> Result<serde_json::Value, String> {
    // 1. 读取设备信息（短暂获锁）
    let device = {
        let conn = db.lock();
        let sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
        crate::db::query::query_one(&conn, &sql, rusqlite::params![device_id], device_from_row)?
            .ok_or_else(|| format!("设备 ID {} 不存在", device_id))?
    }; // 锁释放

    // 2. TCP 连接检测（锁外，5 秒超时）
    // 数据库设备额外检测数据库端口
    let ip_addr =
        IpAddr::from_str(&device.ip).map_err(|_| format!("无法解析设备 IP 地址: {}", device.ip))?;
    let ssh_port = port_u16(device.ssh_port).ok_or_else(|| {
        format!("设备 SSH 端口非法: {}", device.ssh_port)
    })?;
    let ssh_addr = SocketAddr::new(ip_addr, ssh_port);

    let new_status = match TcpStream::connect_timeout(&ssh_addr, Duration::from_secs(5)) {
        Ok(_stream) => "online",
        Err(_) => "offline",
    };

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // 3. 写入结果（短暂获锁）
    {
        let conn = db.lock();

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

/// 检查单个设备连通状态（TCP 放在 spawn_blocking 中，不阻塞 Tauri 线程池）
#[tauri::command]
pub async fn check_device_status(
    device_id: i64,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let db = state.db.clone();
    let db_for_check = db.clone();
    let inner = tokio::task::spawn_blocking(move || check_device_status_inner(&db_for_check, device_id))
        .await
        .map_err(|e| {
            let msg = format!("检测任务失败: {}", e);
            tracing::error!("[check_device_status] {}", msg);
            msg
        })?;
    let result = inner?;
    // 如果设备在线，清除旧静态信息后重新检测（解决凭据/连接信息变更后旧数据过期的问题）
    if result.get("status").and_then(|v| v.as_str()) == Some("online") {
        let _ = db.lock().execute(
            "UPDATE devices SET sysname=NULL, model=NULL, serial_number=NULL, manufacturing_date=NULL, \
             cpu_cores=NULL, memory_gb=NULL, kernel_version=NULL, db_version=NULL, instance_name=NULL \
             WHERE id=?1 AND (sysname IS NOT NULL OR model IS NOT NULL OR db_version IS NOT NULL)",
            rusqlite::params![device_id],
        );
        detect_static_info_if_missing(device_id, &db);
    }
    tracing::info!("[check_device_status] device_id={} 完成", device_id);
    Ok(result)
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
                    if let Err(e) = conn.execute(
                        "INSERT INTO device_status_logs (device_id, old_status, new_status, checked_at) VALUES (?1, ?2, ?3, ?4)",
                        rusqlite::params![device.id, device.status, new_status, now],
                    ) {
                        tracing::error!("设备状态日志写入失败 (device_id={}): {}", device.id, e);
                    }
                }
                if let Err(e) = conn.execute(
                    "UPDATE devices SET status = ?1, last_checked_at = ?2, updated_at = ?3 WHERE id = ?4",
                    rusqlite::params![new_status, now, now, device.id],
                ) {
                    tracing::error!("设备状态更新失败 (device_id={}): {}", device.id, e);
                }
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
/// 非前端 invoke，由 detect_device_model_by_id 内部调用
#[allow(clippy::too_many_arguments)]
pub async fn detect_device_model(
    ip: String,
    ssh_port: u16,
    ssh_username: String,
    ssh_password: String,
    vendor: String,
    deployment: String,
    device_name: String,
    db_username: String,
    db_password: String,
    instance_name: String,
    db_port: i64,
) -> Result<String, String> {
    use crate::services::vendor_profile::{self, ExecMode};

    let profile = vendor_profile::get_profile(&vendor);
    tracing::info!(
        "[detect] 开始静态信息检测 [{}@{}:{}] vendor={}, exec_mode={:?}",
        ssh_username, ip, ssh_port, vendor, profile.exec_mode
    );

    // 数据库设备：检测 OS 信息 + 数据库信息
    let is_db = ["mysql","postgres","oracle","sql","达梦","redis","mongo"]
        .iter().any(|o| vendor.to_lowercase().contains(o));

    let result = if is_db {
        detect_db_info(ip.clone(), ssh_port, ssh_username.clone(), ssh_password, vendor.clone(), deployment.clone(), device_name.clone(), db_username.clone(), db_password.clone(), instance_name.clone(), db_port).await
    } else {
        match profile.exec_mode {
            ExecMode::Exec => {
                detect_linux_info(ip.clone(), ssh_port, ssh_username.clone(), ssh_password).await
            }
            ExecMode::Shell => {
                detect_network_device_info(ip.clone(), ssh_port, ssh_username.clone(), ssh_password, vendor.clone()).await
            }
        }
    };
    match &result {
        Ok(json) => tracing::info!("[detect] 完成 [{}@{}]: {}", ssh_username, ip, json),
        Err(e) => tracing::error!("[detect] 失败 [{}@{}]: {}", ssh_username, ip, e),
    }
    result
}

/// 使用 DB 中存储的设备凭据触发检测（编辑模式下用户不必重新输入密码）
#[tauri::command]
pub async fn detect_device_model_by_id(
    device_id: i64,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // 读取设备后打包的检测上下文（ip, ssh_port, ssh_user, ssh_pwd, vendor, deployment, device_name,
    // db_user, db_pwd, instance_name, db_port）
    type DetectContext = (String, u16, String, String, String, String, String, String, String, String, i64);
    // 1. 读取设备 + 解密密码
    let read_result: Result<DetectContext, String> = {
        let conn = state.db.lock();
        let sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
        let device =
            crate::db::query::query_one(&conn, &sql, rusqlite::params![device_id], device_from_row)?
                .ok_or_else(|| format!("设备 ID {} 不存在", device_id))?;

        let port = match port_u16(device.ssh_port) {
            Some(p) => p,
            None => return Err(format!("设备 SSH 端口非法: {}", device.ssh_port)),
        };
        let pwd = match device.ssh_password_encrypted.as_deref() {
            Some(p) if !p.is_empty() => match CryptoService::decrypt(p) {
                Ok(s) => s,
                Err(e) => return Err(e),
            },
            _ => {
                // 凭据缺失：写入 auth_status 然后返回错误
                let now = now_str();
                let _ = conn.execute(
                    "UPDATE devices SET auth_status='no_credential', auth_message=?1, updated_at=?2 WHERE id=?3",
                    rusqlite::params!["未保存 SSH 密码", now, device_id],
                );
                return Err("设备未保存 SSH 密码，请编辑后重新输入".to_string());
            }
        };
        let deployment = device.deployment.unwrap_or_default();
        let device_name = device.name.clone();
        let db_username = device.db_username.unwrap_or_default();
        let db_password_raw = device.db_password_encrypted
            .as_deref()
            .filter(|p| !p.is_empty())
            .and_then(|p| CryptoService::decrypt(p).ok())
            .unwrap_or_default();
        let instance_name = device.instance_name.unwrap_or_default();
        let db_port = device.db_port.unwrap_or(3306);
        Ok((
            device.ip,
            port,
            device.ssh_username.unwrap_or_default(),
            pwd,
            device.vendor,
            deployment,
            device_name,
            db_username,
            db_password_raw,
            instance_name,
            db_port,
        ))
    };
    let (ip, ssh_port, ssh_username, ssh_password, vendor, deployment, device_name, db_username, db_password, instance_name, db_port) = read_result?;

    // 2. 如果同 IP 已有服务器设备且有 OS 信息，直接复制（避免重复 SSH + sudo 问题）
    let os_info_from_sibling = {
        let conn = state.db.lock();
        let sql = "SELECT model, sysname, cpu_cores, memory_gb, kernel_version FROM devices \
             WHERE ip = ?1 AND device_type != 'database' AND (model IS NOT NULL OR sysname IS NOT NULL) LIMIT 1";
        let r = conn.query_row(sql, rusqlite::params![ip], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<f64>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        });
        match &r {
            Ok((m, s, c, mem, kv)) => tracing::info!(
                "[detect_db] 找到同 IP 服务器设备 OS 信息: model={:?}, sysname={:?}, cpu={:?}, mem={:?}, kernel={:?}",
                m, s, c, mem, kv
            ),
            Err(e) => tracing::warn!("[detect_db] 未找到同 IP 服务器设备 OS 信息 (ip={}): {}", ip, e),
        }
        r.ok()
    };

    // 3. 调用底层检测（OS 信息 + DB 版本）
    let detect_result =
        detect_device_model(ip, ssh_port, ssh_username, ssh_password, vendor, deployment.clone(), device_name.clone(), db_username, db_password, instance_name, db_port).await;
    tracing::info!("[detect_db] detect_device_model 结果: {:?}", detect_result.as_ref().err().map(|e| e.as_str()).unwrap_or("Ok"));

    // 失败：分类并写入 auth_status
    // 无论 SSH 成功与否，都用同 IP 服务器的 OS 信息补全缺失字段
    // （dmidecode 内存检测可能因 sudo 权限失败，兄弟设备的信息是可靠补充）
    let json = match detect_result {
        Ok(j) => j,
        Err(e) => {
            if os_info_from_sibling.is_some() {
                tracing::warn!("[detect_db] SSH 检测失败，使用同 IP 服务器的 OS 信息: {}", e);
                let (model, sysname, cpu_cores, memory_gb, kernel_version) = os_info_from_sibling.clone().unwrap();
                let mut map = serde_json::Map::new();
                if let Some(m) = model { map.insert("model".into(), serde_json::Value::String(m)); }
                if let Some(s) = sysname { map.insert("sysname".into(), serde_json::Value::String(s)); }
                if let Some(c) = cpu_cores { map.insert("cpu_cores".into(), serde_json::Value::String(c.to_string())); }
                if let Some(m) = memory_gb { map.insert("memory_gb".into(), serde_json::Value::String(m.to_string())); }
                if let Some(k) = kernel_version { map.insert("kernel_version".into(), serde_json::Value::String(k)); }
                serde_json::Value::Object(map).to_string()
            } else {
                // SSH 检测失败：仅认证类错误写 auth_status，其他错误不污染状态列
                let (auth_status, brief) = classify_detect_error(&e);
                let now = now_str();
                let conn = state.db.lock();
                // 只有认证失败/无凭据才标记 auth_status，避免命令执行异常误报为"账号错误"
                if auth_status == "auth_failed" || auth_status == "no_credential" {
                    let _ = conn.execute(
                        "UPDATE devices SET auth_status=?1, auth_message=?2, updated_at=?3 WHERE id=?4",
                        rusqlite::params![auth_status, brief, now, device_id],
                    );
                }
                return Err(e);
            }
        }
    };

    // SSH 成功时，用兄弟设备信息补全缺失字段（特别是 memory）
    let json = if let Some((ref s_model, ref s_sysname, s_cpu, s_mem, s_kernel)) = os_info_from_sibling {
        let mut map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&json).unwrap_or_else(|e| {
            tracing::warn!("[detect] 检测结果 JSON 解析失败: {}，原始: {}", e, json.chars().take(200).collect::<String>());
            serde_json::Map::new()
        });
        if !map.contains_key("memory_gb") {
            if let Some(m) = s_mem { map.insert("memory_gb".into(), serde_json::Value::String(m.to_string())); }
        }
        if !map.contains_key("cpu_cores") {
            if let Some(c) = s_cpu { map.insert("cpu_cores".into(), serde_json::Value::String(c.to_string())); }
        }
        if !map.contains_key("model") {
            if let Some(m) = s_model.clone() { map.insert("model".into(), serde_json::Value::String(m)); }
        }
        if !map.contains_key("sysname") {
            if let Some(s) = s_sysname.clone() { map.insert("sysname".into(), serde_json::Value::String(s)); }
        }
        if !map.contains_key("kernel_version") {
            if let Some(k) = s_kernel.clone() { map.insert("kernel_version".into(), serde_json::Value::String(k)); }
        }
        serde_json::Value::Object(map).to_string()
    } else {
        json
    };

    // 3. 把检测到的字段写回数据库（同时把 auth_status 标记为 ok）
    let parsed: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| format!("解析检测结果失败: {}", e))?;
    {
        let conn = state.db.lock();
        let mut updater = crate::db::db_helpers::DynamicUpdate::new();
        if let Some(obj) = parsed.as_object() {
            if let Some(s) = obj.get("model").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                updater.push_raw("model", s.to_string());
            }
            if let Some(s) = obj
                .get("serial_number")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                updater.push_raw("serial_number", s.to_string());
            }
            if let Some(s) = obj
                .get("manufacturing_date")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                updater.push_raw("manufacturing_date", s.to_string());
            }
            if let Some(s) = obj.get("sysname").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                updater.push_raw("sysname", s.to_string());
            }
            // 数据库专属字段
            if let Some(s) = obj.get("db_version").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                updater.push_raw("db_version", s.to_string());
            }
            if let Some(s) = obj.get("instance_name").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                updater.push_raw("instance_name", s.to_string());
            }
            if let Some(p) = obj.get("db_port").and_then(|v| v.as_i64()).filter(|&p| p > 0) {
                updater.push_raw("db_port", p);
            }
            if let Some(s) = obj.get("kernel_version").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                updater.push_raw("kernel_version", s.to_string());
            }
            if let Some(s) = obj
                .get("cpu_cores")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                if let Ok(n) = s.parse::<i64>() {
                    updater.push_raw("cpu_cores", n);
                }
            }
            if let Some(s) = obj
                .get("memory_gb")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                // dmidecode 返回如 "7.7Gi"，DB 列是 REAL，需提取数字部分
                let num_str: String = s
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                if let Ok(n) = num_str.parse::<f64>() {
                    updater.push_raw("memory_gb", n);
                }
            }
        }
        // 检测成功 → 标记 auth_status=ok
        updater.push_raw("auth_status", "ok".to_string());
        updater.push_raw("auth_message", Option::<String>::None);
        updater.push_raw("updated_at", now_str());
        let (set_parts, mut params) = updater.finish();
        let idx = params.len() as i32 + 1;
        let sql = format!(
            "UPDATE devices SET {} WHERE id = ?{}",
            set_parts.join(", "),
            idx
        );
        params.push(Box::new(device_id));
        let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, param_refs.as_slice())
            .map_err(|e| e.to_string())?;
    }

    Ok(json)
}

/// 后台轮询专用：检测单台设备的静态信息（同步，可从 std::thread 调用）。
///
/// 仅对在线且尚缺静态信息（model/sysname 均为空）的设备执行。
/// 内部完成 DB 读取→解密→SSH→DB 写入。
pub fn detect_static_info_if_missing(
    device_id: i64,
    db: &Arc<parking_lot::Mutex<rusqlite::Connection>>,
) {
    use crate::services::crypto::CryptoService;
    use crate::services::vendor_profile::{self, ExecMode};

    // 1. 读取设备信息 + 检查是否需要检测
    let device_info = {
        let conn = db.lock();
        let sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
        match crate::db::query::query_one(&conn, &sql, rusqlite::params![device_id], device_from_row) {
            Ok(Some(d)) => d,
            _ => return,
        }
    };

    let is_db = ["mysql","postgres","oracle","sql","达梦","redis","mongo"]
        .iter().any(|o| device_info.vendor.to_lowercase().contains(o));
    let has_os_info = device_info.model.as_ref().filter(|s| !s.is_empty()).is_some()
        || device_info.sysname.as_ref().filter(|s| !s.is_empty()).is_some();
    let has_db_info = device_info.db_version.as_ref().filter(|s| !s.is_empty()).is_some();

    // 普通设备：已有型号/主机名 → 跳过
    // 数据库设备：OS 信息和 DB 版本都需要有才跳过
    if is_db {
        if has_os_info && has_db_info {
            tracing::info!("[bg-detect] 设备 #{} ({}) 已有完整信息，跳过", device_id, device_info.name);
            return;
        }
    } else {
        if has_os_info {
            tracing::info!("[bg-detect] 设备 #{} ({}) 已有 OS 信息，跳过", device_id, device_info.name);
            return;
        }
    }

    let port = match port_u16(device_info.ssh_port) {
        Some(p) => p,
        None => {
            tracing::warn!("[bg-detect] 设备 #{} ({}) SSH 端口非法: {}，跳过", device_id, device_info.name, device_info.ssh_port);
            return;
        }
    };
    let password = match device_info.ssh_password_encrypted.as_deref() {
        Some(p) if !p.is_empty() => match CryptoService::decrypt(p) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("[bg-detect] 设备 #{} ({}) SSH 密码解密失败: {}，跳过", device_id, device_info.name, e);
                return;
            }
        },
        _ => {
            tracing::warn!("[bg-detect] 设备 #{} ({}) 未保存 SSH 密码，跳过", device_id, device_info.name);
            return;
        }
    };
    let ip = device_info.ip.clone();
    let vendor = device_info.vendor.clone();
    let username = device_info.ssh_username.clone().unwrap_or_default();
    let deployment = device_info.deployment.clone().unwrap_or_default();
    let device_name = device_info.name.clone();
    let db_username = device_info.db_username.clone().unwrap_or_default();
    let db_password = device_info.db_password_encrypted
        .as_deref()
        .filter(|p| !p.is_empty())
        .and_then(|p| CryptoService::decrypt(p).ok())
        .unwrap_or_default();
    let instance_name = device_info.instance_name.clone().unwrap_or_default();
    let db_port = device_info.db_port.unwrap_or(3306);
    drop(device_info);

    // 2. SSH 检测（锁外）
    tracing::info!("[bg-detect] 设备 #{} ({}) 开始检测, vendor={}, deployment={}, db_port={}",
        device_id, device_name, vendor, deployment, db_port);
    let is_db = ["mysql","postgres","oracle","sql","达梦","redis","mongo"]
        .iter().any(|o| vendor.to_lowercase().contains(o));
    let result = if is_db {
        detect_db_info_sync(&ip, port, &username, &password, &vendor, &deployment, &device_name, &db_username, &db_password, &instance_name, db_port)
    } else {
        let profile = vendor_profile::get_profile(&vendor);
        match profile.exec_mode {
            ExecMode::Exec => detect_linux_info_sync(&ip, port, &username, &password),
            ExecMode::Shell => detect_network_device_info_sync(&ip, port, &username, &password, &vendor),
        }
    };

    // 非认证类失败（SSH 超时/网络抖动等）短间隔重试一次；认证失败不重试
    let result = match result {
        Ok(_) => result,
        Err(ref e) => {
            let err_lower = e.to_lowercase();
            let is_auth_err = err_lower.contains("密码") || err_lower.contains("认证")
                || err_lower.contains("auth") || err_lower.contains("no_credential");
            if is_auth_err {
                result
            } else {
                tracing::warn!("[bg-detect] 设备 #{} ({}) 首次检测失败（{}），3s 后重试一次", device_id, device_name, e);
                std::thread::sleep(std::time::Duration::from_secs(3));
                // 重新执行（result 已 move，用 Err 引用判断后重新调用）
                let retry = if is_db {
                    detect_db_info_sync(&ip, port, &username, &password, &vendor, &deployment, &device_name, &db_username, &db_password, &instance_name, db_port)
                } else {
                    let profile = vendor_profile::get_profile(&vendor);
                    match profile.exec_mode {
                        ExecMode::Exec => detect_linux_info_sync(&ip, port, &username, &password),
                        ExecMode::Shell => detect_network_device_info_sync(&ip, port, &username, &password, &vendor),
                    }
                };
                match &retry {
                    Ok(_) => {
                        tracing::info!("[bg-detect] 设备 #{} ({}) 重试成功", device_id, device_name);
                        retry
                    }
                    Err(re) => {
                        tracing::warn!("[bg-detect] 设备 #{} ({}) 重试仍失败: {}", device_id, device_name, re);
                        retry
                    }
                }
            }
        }
    };

    // 3. 写入 DB（短暂获锁）
    match result {
        Ok(json) => {
            let parsed: serde_json::Value = match serde_json::from_str(&json) {
                Ok(v) => v,
                Err(_) => return,
            };
            let conn = db.lock();
            let mut updater = crate::db::db_helpers::DynamicUpdate::new();
            if let Some(obj) = parsed.as_object() {
                if let Some(s) = obj.get("model").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                    updater.push_raw("model", s.to_string());
                }
                if let Some(s) = obj.get("sysname").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                    updater.push_raw("sysname", s.to_string());
                }
                if let Some(s) = obj.get("serial_number").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                    updater.push_raw("serial_number", s.to_string());
                }
                if let Some(s) = obj.get("manufacturing_date").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                    updater.push_raw("manufacturing_date", s.to_string());
                }
                if let Some(s) = obj.get("cpu_cores").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                    if let Ok(n) = s.parse::<i64>() {
                        updater.push_raw("cpu_cores", n);
                    }
                }
                if let Some(s) = obj.get("memory_gb").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                    let num_str: String = s.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                    if let Ok(n) = num_str.parse::<f64>() {
                        updater.push_raw("memory_gb", n);
                    }
                }
                if let Some(s) = obj.get("db_version").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                    updater.push_raw("db_version", s.to_string());
                }
                if let Some(s) = obj.get("instance_name").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                    updater.push_raw("instance_name", s.to_string());
                }
                if let Some(s) = obj.get("kernel_version").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                    updater.push_raw("kernel_version", s.to_string());
                }
            }
            if !updater.is_empty() {
                updater.push_raw("auth_status", "ok".to_string());
                updater.push_raw("auth_message", Option::<String>::None);
                updater.push_raw("updated_at", now_str());
                let (set_parts, mut params) = updater.finish();
                let idx = params.len() as i32 + 1;
                let sql = format!("UPDATE devices SET {} WHERE id = ?{}", set_parts.join(", "), idx);
                params.push(Box::new(device_id));
                let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
                let _ = conn.execute(&sql, param_refs.as_slice());
            }
            tracing::info!("[bg-detect] 设备 #{} 静态信息后台采集成功", device_id);
        }
        Err(e) => {
            let (auth_status, brief) = classify_detect_error(&e);
            let conn = db.lock();
            let _ = conn.execute(
                "UPDATE devices SET auth_status=?1, auth_message=?2, updated_at=?3 WHERE id=?4",
                rusqlite::params![auth_status, brief, now_str(), device_id],
            );
            tracing::warn!("[bg-detect] 设备 #{} 静态信息后台采集失败: {}", device_id, e);
        }
    }
}

/// 把 SSH 检测错误分类为 auth_status 标记和简短中文消息。
/// 返回 (status_code, brief_message)。
fn classify_detect_error(err: &str) -> (&'static str, String) {
    let lower = err.to_lowercase();
    if err.contains("SSH密码认证失败")
        || err.contains("SSH认证未通过")
        || lower.contains("authentication")
    {
        ("auth_failed", "SSH 账号或密码错误".to_string())
    } else if err.contains("TCP连接失败") || lower.contains("connection refused") {
        ("unreachable", "无法连接（端口/网络）".to_string())
    } else if lower.contains("timeout") || err.contains("超时") {
        ("timeout", "连接超时".to_string())
    } else if err.contains("地址解析失败") || err.contains("无法解析主机地址") {
        ("dns_fail", "地址解析失败".to_string())
    } else if err.contains("未保存 SSH 密码") {
        ("no_credential", "未保存 SSH 密码".to_string())
    } else {
        // 截断到 80 字符
        let trimmed = if err.chars().count() > 80 {
            err.chars().take(80).collect::<String>() + "…"
        } else {
            err.to_string()
        };
        ("error", trimmed)
    }
}

/// Linux 服务器静态信息检测（exec channel）— async 版本（供 Tauri command 调用）
async fn detect_linux_info(
    ip: String,
    ssh_port: u16,
    ssh_username: String,
    ssh_password: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        detect_linux_info_sync(&ip, ssh_port, &ssh_username, &ssh_password)
    })
    .await
    .map_err(|e| format!("检测任务失败: {}", e))?
}

/// detect_linux_info 的同步版本（供后台 std::thread 直接调用）
fn detect_linux_info_sync(
    ip: &str,
    ssh_port: u16,
    ssh_username: &str,
    ssh_password: &str,
) -> Result<String, String> {
    use crate::services::inspection_runner::SSHSessionSource;
    use crate::services::linux_runner;
    use std::collections::HashMap;

    tracing::info!("[detect_linux] 开始: {}@{}:{}", ssh_username, ip, ssh_port);

    let commands: Vec<String> = vec![
        "hostnamectl".to_string(),
        "cat /etc/os-release".to_string(),
        "uname -r".to_string(),
        "nproc".to_string(),
        "lscpu".to_string(),
        "sudo dmidecode -t memory 2>/dev/null | grep -i Size".to_string(),
    ];
    let mut needs_root_map = HashMap::new();
    needs_root_map.insert("sudo dmidecode -t memory 2>/dev/null | grep -i Size".to_string(), true);

    let source = SSHSessionSource {
        host: ip.to_string(),
        port: ssh_port,
        username: ssh_username.to_string(),
        password: ssh_password.to_string(),
    };

    let outputs = linux_runner::run_commands_exec(
        &source, &commands, &needs_root_map, None, None,
    )?;

    tracing::info!("[detect_linux] 命令执行完成，输出数: {}", outputs.len());
    for (cmd, output) in &outputs {
        let mut end = output.len().min(200);
        while end > 0 && !output.is_char_boundary(end) {
            end -= 1;
        }
        tracing::debug!("[detect_linux] 命令 '{}' → {}", cmd, &output[..end]);
    }

    let mut info = serde_json::Map::new();

    if let Some(output) = outputs.get("hostnamectl") {
        if let Some(hostname) = extract_hostnamectl_value(output, "Static hostname") {
            info.insert("sysname".to_string(), serde_json::Value::String(hostname.clone()));
            info.insert("hostname".to_string(), serde_json::Value::String(hostname));
        }
        if let Some(os) = extract_hostnamectl_value(output, "Operating System") {
            info.insert("model".to_string(), serde_json::Value::String(os));
        }
    }

    if !info.contains_key("model") {
        if let Some(output) = outputs.get("cat /etc/os-release") {
            if let Some(name) = extract_os_release_value(output, "PRETTY_NAME") {
                info.insert("model".to_string(), serde_json::Value::String(name));
            }
        }
    }

    if let Some(output) = outputs.get("uname -r") {
        let trimmed = output.trim().to_string();
        if !trimmed.is_empty() {
            info.insert("kernel_version".to_string(), serde_json::Value::String(trimmed));
        }
    }

    if let Some(output) = outputs.get("nproc") {
        let trimmed = output.trim();
        if trimmed.parse::<i64>().is_ok() {
            info.insert("cpu_cores".to_string(), serde_json::Value::String(trimmed.to_string()));
        }
    }
    if !info.contains_key("cpu_cores") {
        if let Some(output) = outputs.get("lscpu") {
            if let Some(cores) = extract_lscpu_cores(output) {
                info.insert("cpu_cores".to_string(), serde_json::Value::String(cores));
            }
        }
    }

    if let Some(output) = outputs.get("sudo dmidecode -t memory 2>/dev/null | grep -i Size") {
        if let Some(mem) = extract_dmidecode_memory(output) {
            info.insert("memory_gb".to_string(), serde_json::Value::String(mem));
        }
    }

    let result = serde_json::Value::Object(info).to_string();
    tracing::info!("[detect_linux] 返回结果: {}", result);
    Ok(result)
}

/// 数据库设备静态信息检测：先检测 OS 信息，再检测数据库版本/实例名
#[allow(clippy::too_many_arguments)]
async fn detect_db_info(
    ip: String,
    ssh_port: u16,
    ssh_username: String,
    ssh_password: String,
    vendor: String,
    deployment: String,
    device_name: String,
    db_username: String,
    db_password: String,
    instance_name: String,
    db_port: i64,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        detect_db_info_sync(&ip, ssh_port, &ssh_username, &ssh_password, &vendor, &deployment, &device_name, &db_username, &db_password, &instance_name, db_port)
    })
    .await
    .map_err(|e| format!("检测任务失败: {}", e))?
}

#[allow(clippy::too_many_arguments)]
fn detect_db_info_sync(
    ip: &str,
    ssh_port: u16,
    ssh_username: &str,
    ssh_password: &str,
    vendor: &str,
    deployment: &str,
    device_name: &str,
    db_username: &str,
    db_password: &str,
    instance_name: &str,
    db_port: i64,
) -> Result<String, String> {
    use crate::services::linux_runner;
    use crate::services::inspection_runner::SSHSessionSource;
    use std::collections::HashMap;

    tracing::info!("[detect_db] 开始: {}@{}:{}, vendor={}, deployment={}, db_user={}, instance={}",
        ssh_username, ip, ssh_port, vendor, deployment, db_username, instance_name);

    let vendor_lower = vendor.to_lowercase();
    let is_container = deployment == "docker" || deployment == "podman";
    let runtime = if deployment == "podman" { "podman" } else { "docker" };

    // ── MySQL / MariaDB 数据库命令 ──
    // 全部存裸命令，由下方 wrap_cmd 统一包装（避免预包装+跳过的条件不一致 bug）
    let mut db_cmds: Vec<(String, String)> = Vec::new(); // (label, raw_cmd)

    if vendor_lower.contains("mysql") || vendor_lower.contains("mariadb") {
        // 用 MYSQL_PWD 环境变量传密码，避免命令行参数暴露（ps 可见）
        // db_username 已在入库时校验为安全字符集，单引号包裹防注入
        let mysql_auth = if !db_username.is_empty() { format!("-u'{}'", shell_quote_single(db_username)) } else { String::new() };
        let mysql_pwd_prefix = if !db_password.is_empty() {
            // wrap_cmd 使用 sh -c '...'（单引号），只做单引号转义即可
            let escaped = shell_quote_single(db_password);
            format!("MYSQL_PWD='{}' ", escaped)
        } else { String::new() };
        db_cmds.push(("db_detail".to_string(), format!(
            "{}mysql {} -N -B -e \"SELECT VERSION(), @@hostname, @@port, @@datadir\"", mysql_pwd_prefix, mysql_auth)));
    } else if vendor_lower.contains("postgres") {
        // 服务端版本：连库执行 SELECT version()（psql --version 只取客户端版本）
        if !db_username.is_empty() {
            let pg_env = if !db_password.is_empty() {
                let escaped = shell_quote_single(db_password);
                format!("PGPASSWORD='{}' ", escaped)
            } else { String::new() };
            db_cmds.push(("db_version".to_string(), format!(
                "{}psql -U '{}' -h localhost -p {} -t -c 'SHOW server_version'",
                pg_env, shell_quote_single(db_username), db_port)));
            db_cmds.push(("db_detail".to_string(), format!(
                "{}psql -U '{}' -h localhost -p {} -c \"SELECT version(), inet_server_addr(), inet_server_port(), current_database()\"",
                pg_env, shell_quote_single(db_username), db_port)));
        } else {
            db_cmds.push(("db_version".to_string(), "psql --version".to_string()));
        }
    } else if vendor_lower.contains("oracle") {
        db_cmds.push(("db_version".to_string(), "sqlplus -v".to_string()));
    } else if vendor_lower.contains("sql") || vendor_lower.contains("mssql") {
        db_cmds.push(("db_version".to_string(), "sqlcmd -Q 'SELECT @@VERSION' -W".to_string()));
    } else if vendor_lower.contains("达梦") {
        db_cmds.push(("db_version".to_string(), "/opt/dmdbms/bin/disql -v".to_string()));
    } else if vendor_lower.contains("redis") {
        db_cmds.push(("db_version".to_string(), "redis-cli --version".to_string()));
    } else if vendor_lower.contains("mongo") {
        db_cmds.push(("db_version".to_string(), "mongosh --version".to_string()));
    } else {
        db_cmds.push(("db_version".to_string(), "echo unknown_db_vendor".to_string()));
    }

    let wrap_cmd = |raw: &str| -> Result<String, String> {
        if is_container {
            let cname = if instance_name.is_empty() { device_name } else { instance_name };
            // 防御性校验：容器名会拼入远端 shell，必须为安全字符集
            // （设备名作为兜底也校验，拦截绕过入库校验的旧数据）
            validate_shell_identifier(cname, "容器/实例名")?;
            // 用单引号包裹命令体（单引号内一切字面值，不展开 $ ` \），
            // 内部单引号用 '\'' 转义（经典 shell 退出-转义-重入模式）
            let sq = raw.replace('\'', "'\\''");
            Ok(format!("{} exec {} sh -c '{}' 2>&1; E=$?; [ $E -eq 127 ] && echo client_not_found || [ $E -ne 0 ] && echo container_not_found",
                runtime, cname, sq))
        } else {
            Ok(format!("{} 2>&1", raw))
        }
    };

    // 一次 SSH 批次：OS 命令 + DB 命令
    let mut commands: Vec<String> = vec![
        "hostnamectl".to_string(),
        "cat /etc/os-release".to_string(),
        "uname -r".to_string(),
        "nproc".to_string(),
        "lscpu".to_string(),
        "sudo dmidecode -t memory 2>/dev/null | grep -i Size".to_string(),
    ];
    for (_, raw_cmd) in &db_cmds {
        commands.push(wrap_cmd(raw_cmd)?);
    }

    let mut needs_root_map = HashMap::new();
    needs_root_map.insert("sudo dmidecode -t memory 2>/dev/null | grep -i Size".to_string(), true);

    let source = SSHSessionSource {
        host: ip.to_string(),
        port: ssh_port,
        username: ssh_username.to_string(),
        password: ssh_password.to_string(),
    };

    let outputs = linux_runner::run_commands_exec(
        &source, &commands, &needs_root_map, None, None,
    )?;

    tracing::info!("[detect_db] 命令执行完成，输出数: {}", outputs.len());
    for (cmd, output) in &outputs {
        let mut end = output.len().min(200);
        while end > 0 && !output.is_char_boundary(end) {
            end -= 1;
        }
        tracing::debug!("[detect_db] 命令 '{}' → {}", cmd, &output[..end]);
    }

    let mut info = serde_json::Map::new();

    // ── OS 信息解析 ──
    if let Some(output) = outputs.get("hostnamectl") {
        if let Some(hostname) = extract_hostnamectl_value(output, "Static hostname") {
            info.insert("sysname".to_string(), serde_json::Value::String(hostname.clone()));
            info.insert("hostname".to_string(), serde_json::Value::String(hostname));
        }
        if let Some(os) = extract_hostnamectl_value(output, "Operating System") {
            info.insert("model".to_string(), serde_json::Value::String(os));
        }
    }
    if !info.contains_key("model") {
        if let Some(output) = outputs.get("cat /etc/os-release") {
            if let Some(name) = extract_os_release_value(output, "PRETTY_NAME") {
                info.insert("model".to_string(), serde_json::Value::String(name));
            }
        }
    }
    if let Some(output) = outputs.get("uname -r") {
        let trimmed = output.trim().to_string();
        if !trimmed.is_empty() {
            info.insert("kernel_version".to_string(), serde_json::Value::String(trimmed));
        }
    }
    if let Some(output) = outputs.get("nproc") {
        let trimmed = output.trim();
        if trimmed.parse::<i64>().is_ok() {
            info.insert("cpu_cores".to_string(), serde_json::Value::String(trimmed.to_string()));
        }
    }
    if !info.contains_key("cpu_cores") {
        if let Some(output) = outputs.get("lscpu") {
            if let Some(cores) = extract_lscpu_cores(output) {
                info.insert("cpu_cores".to_string(), serde_json::Value::String(cores));
            }
        }
    }
    if let Some(output) = outputs.get("sudo dmidecode -t memory 2>/dev/null | grep -i Size") {
        if let Some(mem) = extract_dmidecode_memory(output) {
            info.insert("memory_gb".to_string(), serde_json::Value::String(mem));
        }
    }

    // ── 数据库信息解析 ──
    for (label, raw_cmd) in &db_cmds {
        let key = wrap_cmd(raw_cmd)?;
        if let Some(output) = outputs.get(&key) {
            let trimmed = output.trim();
            if trimmed.is_empty()
                || trimmed.contains("container_not_found")
                || trimmed.contains("client_not_found")
                || trimmed.contains("unknown_db_vendor")
                || trimmed.contains("command not found")
                || trimmed.contains("No such file")
                || trimmed.starts_with("ERROR ")
            {
                continue;
            }

            match label.as_str() {
                "db_version" => {
                    let ver = trimmed.lines().next().unwrap_or(trimmed).to_string();
                    info.insert("db_version".to_string(), serde_json::Value::String(ver));
                }
                "db_detail" => {
                    // MySQL -N -B 输出：一行 tab 分隔，无表头无边框
                    // 非 MySQL 的其它数据库可能有表头或 warning，跳过短行
                    for line in trimmed.lines() {
                        let parts: Vec<&str> = line.split('\t').collect();
                        if parts.len() < 4 { continue; }
                        if parts[0].trim().eq_ignore_ascii_case("version") { continue; }
                        if !parts[0].trim().is_empty() {
                            info.insert("db_version".to_string(), serde_json::Value::String(parts[0].trim().to_string()));
                        }
                        if !parts[1].trim().is_empty() {
                            info.insert("instance_name".to_string(), serde_json::Value::String(parts[1].trim().to_string()));
                        }
                        if let Ok(p) = parts[2].trim().parse::<i64>() {
                            info.insert("db_port".to_string(), serde_json::Value::Number(serde_json::Number::from(p)));
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    // ── 诊断失败原因（必须在 db_version 解析之后，才能正确判断是否真的缺失）──
    let has_db_result = info.contains_key("db_version");
    let mut db_error: Option<String> = None;
    if !has_db_result {
        for (_label, raw_cmd) in &db_cmds {
            let key = wrap_cmd(raw_cmd)?;
            if let Some(output) = outputs.get(&key) {
                let trimmed = output.trim();
                if trimmed.is_empty() { continue; }
                if trimmed.contains("Access denied") {
                    db_error = Some(format!("数据库密码错误（用户: {}）", db_username));
                    break;
                }
                if trimmed.contains("client_not_found") {
                    db_error = Some(format!("容器内未安装 mysql 客户端：请在容器 '{}' 内安装 mysql-client", if instance_name.is_empty() { device_name } else { instance_name }));
                    break;
                }
                if trimmed.contains("container_not_found") {
                    db_error = Some(format!("容器 '{}' 未运行或名称错误，请确认容器名正确", if instance_name.is_empty() { device_name } else { instance_name }));
                    break;
                }
                if trimmed.contains("command not found") || trimmed.contains("No such file") {
                    db_error = Some("mysql 客户端未安装".to_string());
                    break;
                }
                if trimmed.starts_with("ERROR ") {
                    db_error = Some(trimmed.lines().next().unwrap_or(trimmed).to_string());
                    break;
                }
            }
        }
    }

    // 附加警告到结果
    if let Some(ref warn) = db_error {
        info.insert("_warn".to_string(), serde_json::Value::String(warn.clone()));
    } else if !has_db_result && is_container {
        info.insert("_warn".to_string(), serde_json::Value::String(
            format!("数据库版本获取失败：请确认容器名 '{}' 正确且密码无误", if instance_name.is_empty() { device_name } else { instance_name })
        ));
    } else if !has_db_result {
        info.insert("_warn".to_string(), serde_json::Value::String(
            "数据库版本获取失败：请确认客户端已安装且密码正确".to_string()
        ));
    }

    if info.is_empty() {
        return Err("数据库设备 OS 信息和 DB 版本检测均失败".to_string());
    }
    let result = serde_json::Value::Object(info).to_string();
    tracing::info!("[detect_db] 返回结果: {}", result);
    Ok(result)
}

async fn detect_network_device_info(
    ip: String,
    ssh_port: u16,
    ssh_username: String,
    ssh_password: String,
    vendor: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        detect_network_device_info_sync(&ip, ssh_port, &ssh_username, &ssh_password, &vendor)
    })
    .await
    .map_err(|e| format!("检测任务失败: {}", e))?
}

/// detect_network_device_info 的同步版本（供后台 std::thread 直接调用）
fn detect_network_device_info_sync(
    ip: &str,
    ssh_port: u16,
    ssh_username: &str,
    ssh_password: &str,
    vendor: &str,
) -> Result<String, String> {
    use crate::services::inspection_runner::{self, SSHSessionSource};

    let vendor_lower = vendor.to_lowercase();
    let is_fortinet = vendor == "飞塔"
        || vendor_lower == "fortinet"
        || vendor_lower == "fortigate";
    let is_h3c = vendor == "H3C" || vendor == "华三" || vendor_lower == "h3c";

    let (manu_cmd, sysname_cmd_opt): (&str, Option<&str>) = if is_h3c {
        (
            "display device manuinfo",
            Some("display current-configuration | include sysname"),
        )
    } else if is_fortinet {
        ("get system status", None)
    } else {
        return Err(format!("暂不支持 {} 设备自动检测型号", vendor));
    };

    let source = SSHSessionSource {
        host: ip.to_string(),
        port: ssh_port,
        username: ssh_username.to_string(),
        password: ssh_password.to_string(),
    };

    let session = inspection_runner::connect_session(&source)?;
    let (base_prompt, mut channel) = inspection_runner::open_shell_session(
        &session, vendor, &source.password, &source.host, None,
    )?;

    let cmd_result: Result<(String, String), String> = (|| {
        let output = inspection_runner::send_command(
            &mut channel, manu_cmd, &base_prompt, &source.password, &source.host, vendor, None,
        )?;
        let sysname_output = if let Some(scmd) = sysname_cmd_opt {
            inspection_runner::send_command(
                &mut channel, scmd, &base_prompt, &source.password, &source.host, vendor, None,
            )
            .unwrap_or_default()
        } else {
            String::new()
        };
        Ok((output, sysname_output))
    })();

    let _ = channel.close();
    let _ = session.disconnect(None, "done", None);

    let (output, sysname_output) = cmd_result?;
    let cleaned = inspection_runner::clean_output(&output, &base_prompt);

    let info_json = if is_h3c {
        let sysname_cleaned = inspection_runner::clean_output(&sysname_output, &base_prompt);
        let mut info = parse_h3c_device_info(&cleaned);
        info.sysname = parse_sysname(&sysname_cleaned);
        serde_json::json!(info)
    } else if is_fortinet {
        serde_json::json!(parse_fortinet_device_info(&cleaned))
    } else {
        serde_json::json!({})
    };
    Ok(info_json.to_string())
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

/// 飞塔 (FortiGate) 设备信息
#[derive(serde::Serialize)]
struct FortinetDeviceInfo {
    model: Option<String>,
    serial_number: Option<String>,
    manufacturing_date: Option<String>,
    sysname: Option<String>,
}

/// 从飞塔 `get system status` 输出中解析设备信息
///
/// 关键字段示例：
/// ```text
/// Version: FortiGate-80F v7.0.17,build0682,250113 (GA.M)
/// Serial-Number: FGT80FTK24011463
/// Hostname: aHope-FW
/// BIOS version: 05000100
/// ```
///
/// 注意：飞塔 CLI 不暴露真正的出厂日期（`get system status` 中的 250113 是固件 build 日期），
/// 因此 `manufacturing_date` 留空。
fn parse_fortinet_device_info(output: &str) -> FortinetDeviceInfo {
    let mut model = None;
    let mut serial_number = None;
    let mut sysname = None;

    for line in output.lines() {
        let trimmed = line.trim();
        let (key, value) = match trimmed.find(':') {
            Some(idx) => (
                trimmed[..idx].trim().to_lowercase(),
                trimmed[idx + 1..].trim(),
            ),
            None => continue,
        };
        if value.is_empty() {
            continue;
        }
        match key.as_str() {
            "version" if model.is_none() => {
                // "FortiGate-80F v7.0.17,build0682,250113 (GA.M)"
                // 取首个逗号前作为型号+版本
                let m = value.split(',').next().unwrap_or(value).trim();
                if !m.is_empty() {
                    model = Some(m.to_string());
                }
            }
            "serial-number" | "serial number" if serial_number.is_none() => {
                serial_number = Some(value.to_string());
            }
            "hostname" if sysname.is_none() => {
                sysname = Some(value.to_string());
            }
            _ => {}
        }
    }

    FortinetDeviceInfo {
        model,
        serial_number,
        manufacturing_date: None,
        sysname,
    }
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

// ============================================================
// Linux 检测辅助函数
// ============================================================

/// 从 hostnamectl 输出提取字段值（格式: "   Field: value"）
fn extract_hostnamectl_value(output: &str, field: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(field) {
            let value = rest.trim_start().strip_prefix(':').unwrap_or(rest).trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// 从 /etc/os-release 提取值（格式: KEY=VALUE 或 KEY="VALUE"）
fn extract_os_release_value(output: &str, key: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(key) {
            let value = rest.trim_start().strip_prefix('=').unwrap_or(rest).trim().trim_matches('"');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// 从 lscpu 输出提取 CPU 核心数
/// 兼容英文 "CPU(s):" 和中文 locale "CPU:" 等多种格式
fn extract_lscpu_cores(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        // 匹配 "CPU(s):" 或行首是 "CPU" 后跟冒号的字段
        let rest = trimmed
            .strip_prefix("CPU(s):")
            .or_else(|| {
                // 兼容 "CPU :" / "CPU:"，但要排除 "CPU MHz:" / "CPU family:" 等
                if let Some(idx) = trimmed.find(':') {
                    let key = trimmed[..idx].trim();
                    if key.eq_ignore_ascii_case("cpu") {
                        return Some(&trimmed[idx + 1..]);
                    }
                }
                None
            });
        if let Some(rest) = rest {
            let val = rest.split_whitespace().next()?;
            if val.parse::<i64>().is_ok() {
                return Some(val.to_string());
            }
        }
    }
    None
}

/// 从 dmidecode -t memory | grep Size 输出提取总物理内存
fn extract_dmidecode_memory(output: &str) -> Option<String> {
    let mut total_mb: u64 = 0;
    let mut found = false;
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Size:") {
            let val = rest.trim();
            if val.contains("No Module") || val.is_empty() {
                continue;
            }
            let parts: Vec<&str> = val.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(size) = parts[0].parse::<u64>() {
                    let unit = parts[1].to_uppercase();
                    if unit.starts_with("MB") {
                        total_mb += size;
                        found = true;
                    } else if unit.starts_with("GB") {
                        total_mb += size * 1024;
                        found = true;
                    }
                }
            }
        }
    }
    if found && total_mb > 0 {
        let gb = total_mb as f64 / 1024.0;
        Some(format!("{:.1}Gi", gb))
    } else {
        None
    }
}
