use rusqlite::types::ToSql;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::State;

use crate::db::models::{
    batch_from_row, device_from_row, now_str, record_from_row, template_from_row,
    BatchCreate, Device, BATCH_COLUMNS, DEVICE_COLUMNS, RECORD_COLUMNS,
    TEMPLATE_COLUMNS,
};
use crate::services::crypto::CryptoService;
use crate::services::inspection_runner::{self, SSHSessionSource};
use crate::AppState;

// ============================================================
// Internal Helpers
// ============================================================

#[derive(Debug, Clone)]
struct TemplateCommandSpec {
    command: String,
    show_in_report: bool,
    extract_fields: Vec<String>,
    needs_root: bool,
}

/// 从数据库读取设备巡检所需的全部信息（在锁内调用）
fn read_device_inspection_data(
    conn: &rusqlite::Connection,
    device_id: i64,
) -> Result<(Device, String, String, Vec<TemplateCommandSpec>), String> {
    // 1. Look up device
    let device_sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
    let device = crate::db::query::query_one(
        conn,
        &device_sql,
        rusqlite::params![device_id],
        device_from_row,
    )?
    .ok_or_else(|| format!("设备 ID {} 不存在", device_id))?;

    // 2. Decrypt SSH password
    let password = match &device.ssh_password_encrypted {
        Some(enc) if !enc.is_empty() => CryptoService::decrypt(enc)?,
        _ => return Err(format!("设备 '{}' 未配置 SSH 密码", device.name)),
    };
    let username = device.ssh_username.clone().unwrap_or_default();

    // 3. Look up template
    let template_id = device
        .template_id
        .ok_or_else(|| format!("设备 '{}' 未关联巡检模板", device.name))?;
    let template_sql = format!(
        "SELECT {} FROM inspection_templates WHERE id = ?1",
        TEMPLATE_COLUMNS
    );
    let template = crate::db::query::query_one(
        conn,
        &template_sql,
        rusqlite::params![template_id],
        template_from_row,
    )?
    .ok_or_else(|| format!("巡检模板 ID {} 不存在", template_id))?;

    // 4. Parse template config for command specs
    let config_str = template
        .config
        .ok_or_else(|| format!("模板 '{}' 配置为空", template.name))?;
    let config: serde_json::Value =
        serde_json::from_str(&config_str).map_err(|e| format!("解析模板配置 JSON 失败: {}", e))?;

    let specs_json = config["commands"]
        .as_array()
        .ok_or_else(|| format!("模板 '{}' 配置缺少 commands", template.name))?;

    // 5. Fetch commands from command_pool — 单次批量查询避免 N+1（N 台设备并发时锁竞争放大）
    // 先按顺序收集每个 spec 的 command_id 与元数据，再一次性 IN 查询补全命令文本
    let mut spec_entries: Vec<(i64, &serde_json::Value)> = Vec::new();
    for spec in specs_json {
        let cmd_id = spec
            .get("command_id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| format!("模板 '{}' 命令配置缺少 command_id", template.name))?;
        spec_entries.push((cmd_id, spec));
    }

    let cmd_texts: std::collections::HashMap<i64, (String, bool)> = if spec_entries.is_empty() {
        std::collections::HashMap::new()
    } else {
        let ids: Vec<String> = spec_entries.iter().map(|(id, _)| id.to_string()).collect();
        let sql = format!(
            "SELECT id, command, COALESCE(needs_root, 0) FROM command_pool WHERE id IN ({})",
            ids.join(",")
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(
                &[] as &[&dyn rusqlite::types::ToSql],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, bool>(2)?)),
            )
            .map_err(|e| e.to_string())?;
        let mut m = std::collections::HashMap::new();
        for r in rows {
            if let Ok((id, command, needs_root)) = r {
                m.insert(id, (command, needs_root));
            }
        }
        m
    };

    let mut commands: Vec<TemplateCommandSpec> = Vec::new();
    for (cmd_id, spec) in &spec_entries {
        let (command, needs_root) = cmd_texts
            .get(cmd_id)
            .cloned()
            .ok_or_else(|| format!("命令 ID {} 不存在", cmd_id))?;

        let purpose = spec
            .get("purpose")
            .and_then(|v| v.as_str())
            .unwrap_or("inspection");
        let show_in_report = spec
            .get("show_in_report")
            .and_then(|v| v.as_bool())
            .unwrap_or(purpose != "static_info");
        let extract_fields = spec
            .get("extract_fields")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_else(Vec::new);

        commands.push(TemplateCommandSpec {
            command,
            show_in_report,
            extract_fields,
            needs_root,
        });
    }

    if commands.is_empty() {
        return Err(format!(
            "设备 '{}' 的巡检模板 '{}' 未包含有效命令",
            device.name, template.name
        ));
    }

    Ok((device, username, password, commands))
}

/// 将巡检记录创建或更新为 running 状态（在锁内调用）
fn create_or_reset_record(
    conn: &rusqlite::Connection,
    batch_id: i64,
    device_id: i64,
) -> Result<i64, String> {
    let now = now_str();
    let existing: Result<i64, _> = conn.query_row(
        "SELECT id FROM inspection_records WHERE batch_id = ?1 AND device_id = ?2",
        rusqlite::params![batch_id, device_id],
        |row| row.get(0),
    );

    let record_id = match existing {
        Ok(id) => {
            conn.execute(
                "UPDATE inspection_records SET status = 'running', error_message = NULL, \
                 command_outputs = '{}', static_info = '{}', started_at = ?1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, id],
            )
            .map_err(|e| e.to_string())?;
            id
        }
        Err(_) => {
            conn.execute(
                "INSERT INTO inspection_records (batch_id, device_id, status, started_at) \
                 VALUES (?1, ?2, 'running', ?3)",
                rusqlite::params![batch_id, device_id, now],
            )
            .map_err(|e| e.to_string())?;
            conn.last_insert_rowid()
        }
    };
    Ok(record_id)
}

/// 更新巡检记录结果（在锁内调用）
fn update_record_result(
    conn: &rusqlite::Connection,
    record_id: i64,
    status: &str,
    outputs_json: Option<&str>,
    static_info_json: Option<&str>,
    error: Option<&str>,
) -> Result<(), String> {
    let completed_at = now_str();
    match (outputs_json, error) {
        (Some(json), _) => {
            conn.execute(
                "UPDATE inspection_records SET status = ?1, command_outputs = ?2, static_info = ?3, \
                 error_message = NULL, completed_at = ?4, updated_at = ?4 WHERE id = ?5",
                rusqlite::params![status, json, static_info_json.unwrap_or("{}"), completed_at, record_id],
            )
            .map_err(|e| e.to_string())?;
        }
        (_, Some(err)) => {
            conn.execute(
                "UPDATE inspection_records SET status = ?1, error_message = ?2, \
                 completed_at = ?3, updated_at = ?3 WHERE id = ?4",
                rusqlite::params![status, err, completed_at, record_id],
            )
            .map_err(|e| e.to_string())?;
        }
        _ => {}
    }
    Ok(())
}

/// 根据批次内所有记录的当前状态，重新评估并更新批次整体状态。
/// 仅在无 running/pending 记录时才收尾，否则不修改批次状态。
fn finalize_batch_status(conn: &rusqlite::Connection, batch_id: i64) -> Result<(), String> {
    let counts_sql = "SELECT status, COUNT(*) FROM inspection_records WHERE batch_id = ?1 \
                      GROUP BY status";
    let mut stmt = conn.prepare(counts_sql).map_err(|e| e.to_string())?;
    let rows: Vec<(String, i64)> = stmt
        .query_map(rusqlite::params![batch_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut completed = 0i64;
    let mut failed = 0i64;
    let mut stopped = 0i64;
    let mut running = 0i64;
    let mut pending = 0i64;

    for (status, count) in &rows {
        match status.as_str() {
            "completed" => completed += count,
            "failed" => failed += count,
            "stopped" => stopped += count,
            "running" => running += count,
            "pending" => pending += count,
            other => {
                tracing::warn!("finalize_batch_status: 未知记录状态 '{}', 视为 failed", other);
                failed += count;
            }
        }
    }

    // 还有进行中的任务，不收尾批次
    if running > 0 || pending > 0 {
        return Ok(());
    }

    // 若批次已被用户暂停，保持 paused 状态，不按记录结果自动收尾。
    // pause_batch 会设置 cancel flag 让 running 设备停止（记录变 stopped），
    // 此处若按记录判定会覆盖为 stopped/partially_completed，违背用户暂停意图。
    let current_status: Option<String> = conn
        .query_row(
            "SELECT status FROM inspection_batches WHERE id = ?1",
            rusqlite::params![batch_id],
            |row| row.get(0),
        )
        .ok();
    if current_status.as_deref() == Some("paused") {
        tracing::info!("批次 #{} 当前为 paused，保持暂停状态不自动收尾", batch_id);
        return Ok(());
    }

    // 无记录（空批次）视为完成
    if rows.is_empty() {
        let now = now_str();
        conn.execute(
            "UPDATE inspection_batches SET status = 'completed', completed_at = ?1, \
             updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, batch_id],
        )
        .map_err(|e| e.to_string())?;
        return Ok(());
    }

    let final_status = if failed == 0 && stopped == 0 {
        "completed"
    } else if completed == 0 && stopped == 0 {
        "failed"
    } else if stopped > 0 && failed == 0 && completed == 0 {
        "stopped"
    } else if stopped > 0 && failed == 0 {
        // 有 completed 和 stopped → 部分完成
        "partially_completed"
    } else {
        "partially_completed"
    };

    let now = now_str();
    conn.execute(
        "UPDATE inspection_batches SET status = ?1, completed_at = ?2, updated_at = ?2 \
         WHERE id = ?3",
        rusqlite::params![final_status, now, batch_id],
    )
    .map_err(|e| e.to_string())?;

    tracing::info!(
        "批次 #{} 状态重评估: status={}, completed={}, failed={}, stopped={}",
        batch_id,
        final_status,
        completed,
        failed,
        stopped,
    );
    Ok(())
}

fn build_static_info(
    all_outputs: &indexmap::IndexMap<String, String>,
    specs: &[TemplateCommandSpec],
) -> serde_json::Map<String, serde_json::Value> {
    let mut info = serde_json::Map::new();
    for spec in specs {
        if spec.extract_fields.is_empty() {
            continue;
        }
        let Some(output) = all_outputs.get(&spec.command) else {
            continue;
        };
        for field in &spec.extract_fields {
            if info.contains_key(field) {
                continue;
            }
            let val = match field.as_str() {
                "sysname" => extract_sysname(output)
                    .or_else(|| extract_hostnamectl_field(output, "Static hostname")),
                "serial_number" | "sn" => extract_by_patterns(
                    output,
                    &["DEVICE_SERIAL_NUMBER", "SERIAL_NUMBER", "Serial Number", "Serial-Number"],
                ),
                "manufacturing_date" | "mfg_date" => {
                    extract_by_patterns(output, &["MANUFACTURING_DATE", "Manufacturing Date"])
                }
                "model" => extract_model(output),
                _ => None,
            };
            if let Some(v) = val.filter(|s| !s.trim().is_empty()) {
                let key = match field.as_str() {
                    "sn" => "serial_number",
                    "mfg_date" => "manufacturing_date",
                    other => other,
                };
                info.insert(key.to_string(), serde_json::Value::String(v));
            }
        }
    }
    info
}

fn extract_sysname(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.to_lowercase().starts_with("sysname ") {
            let name = trimmed.split_whitespace().nth(1).unwrap_or("").trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
        if trimmed.to_lowercase().starts_with("hostname ") {
            let name = trimmed.split_whitespace().nth(1).unwrap_or("").trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
        if let Some(v) = extract_by_patterns(trimmed, &["Hostname", "Host name"])
            .filter(|s| !s.trim().is_empty())
        {
            return Some(v);
        }
    }
    None
}

/// 从 hostnamectl 输出中提取字段值
fn extract_hostnamectl_field(output: &str, field_name: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(field_name) {
            let value = rest.trim_start().strip_prefix(':').unwrap_or(rest).trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn extract_model(output: &str) -> Option<String> {
    extract_by_patterns(
        output,
        &[
            "DEVICE_NAME",
            "PRODUCT_NAME",
            "Product Name",
            "Version",
            "Platform Type",
            "Model",
            "Operating System",
            "Manufacturer",
            "PRETTY_NAME",
        ],
    )
    .map(|value| {
        value
            .split_whitespace()
            .next()
            .unwrap_or(value.as_str())
            .trim_matches(',')
            .to_string()
    })
}

fn extract_by_patterns(output: &str, keys: &[&str]) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        for key in keys {
            if let Some(rest) = trimmed.strip_prefix(key) {
                let rest_trimmed = rest.trim_start();
                let value = rest_trimmed
                    .strip_prefix(':')
                    .or_else(|| rest_trimmed.strip_prefix('='))
                    .unwrap_or(rest_trimmed)
                    .trim()
                    .trim_matches('"');
                if !value.is_empty() && !value.contains("----") {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

fn visible_outputs(
    all_outputs: &indexmap::IndexMap<String, String>,
    specs: &[TemplateCommandSpec],
) -> indexmap::IndexMap<String, String> {
    let mut visible = indexmap::IndexMap::new();
    for spec in specs {
        if !spec.show_in_report {
            continue;
        }
        if let Some(output) = all_outputs.get(&spec.command) {
            visible.insert(spec.command.clone(), output.clone());
        }
    }
    visible
}

fn sync_device_static_info(
    conn: &rusqlite::Connection,
    device_id: i64,
    static_info: &serde_json::Map<String, serde_json::Value>,
) {
    let get = |key: &str| {
        static_info
            .get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
    };
    let sysname = get("sysname");
    let model = get("model");
    let serial_number = get("serial_number");
    let manufacturing_date = get("manufacturing_date");
    if sysname.is_none()
        && model.is_none()
        && serial_number.is_none()
        && manufacturing_date.is_none()
    {
        return;
    }
    let _ = conn.execute(
        "UPDATE devices SET \
         sysname = COALESCE(?1, sysname), \
         model = COALESCE(?2, model), \
         serial_number = COALESCE(?3, serial_number), \
         manufacturing_date = COALESCE(?4, manufacturing_date), \
         updated_at = ?5 WHERE id = ?6",
        rusqlite::params![sysname, model, serial_number, manufacturing_date, now_str(), device_id],
    );
}

/// 执行单台设备的 SSH 巡检（锁外调用，包含耗时的 SSH 操作）
///
/// 根据厂商 Profile 决定执行模式：
/// - Shell 模式（网络设备）：使用交互式 PTY 会话
/// - Exec 模式（Linux 服务器）：每条命令独立 exec channel
fn execute_device_ssh(
    device: &Device,
    username: &str,
    password: &str,
    commands: &[TemplateCommandSpec],
    on_progress: Option<Arc<std::sync::Mutex<String>>>,
    cancel: Arc<AtomicBool>,
) -> Result<indexmap::IndexMap<String, String>, String> {
    let port = u16::try_from(device.ssh_port)
        .ok()
        .filter(|&p| p > 0)
        .ok_or_else(|| format!("设备 '{}' SSH 端口非法: {}", device.name, device.ssh_port))?;
    let source = SSHSessionSource {
        host: device.ip.clone(),
        port,
        username: username.to_string(),
        password: password.to_string(),
    };

    let profile = crate::services::vendor_profile::get_profile(&device.vendor);

    match profile.exec_mode {
        crate::services::vendor_profile::ExecMode::Exec => {
            let cmd_strings: Vec<String> = commands.iter().map(|s| s.command.clone()).collect();
            let needs_root_map: std::collections::HashMap<String, bool> = commands
                .iter()
                .map(|s| (s.command.clone(), s.needs_root))
                .collect();
            crate::services::linux_runner::run_commands_exec(
                &source,
                &cmd_strings,
                &needs_root_map,
                Some(cancel),
                on_progress,
            )
        }
        crate::services::vendor_profile::ExecMode::Shell => {
            let cmd_strings: Vec<String> = commands.iter().map(|s| s.command.clone()).collect();
            inspection_runner::run_commands_with_cancel(
                &source,
                &device.vendor,
                &cmd_strings,
                on_progress,
                Some(cancel),
            )
        }
    }
}

// ============================================================
// Batch Query Commands
// ============================================================

/// 获取巡检批次列表，支持按状态筛选，最多返回 50 条。
/// 每条记录包含完整的批次字段 + 记录摘要数组。
#[tauri::command]
pub fn list_batches(
    status: Option<String>,
    state: State<AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = state.db.lock();

    let mut sql = format!("SELECT {} FROM inspection_batches WHERE 1=1", BATCH_COLUMNS);
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(ref s) = status {
        sql.push_str(" AND status = ?");
        params.push(Box::new(s.clone()));
    }

    sql.push_str(" ORDER BY created_at DESC LIMIT 50");

    let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let batches = crate::db::query::query_all(&conn, &sql, &param_refs, batch_from_row)?;

    // Load all records for all batches in a single query
    let mut records_by_batch: std::collections::HashMap<i64, Vec<serde_json::Value>> =
        std::collections::HashMap::new();
    if !batches.is_empty() {
        let batch_ids: Vec<String> = batches.iter().map(|b| b.id.to_string()).collect();
        let all_records_sql = format!(
            "SELECT {} FROM inspection_records WHERE batch_id IN ({}) ORDER BY batch_id, id",
            crate::db::models::RECORD_SUMMARY_COLUMNS,
            batch_ids.join(",")
        );
        let all_records = crate::db::query::query_all(
            &conn,
            &all_records_sql,
            &[] as &[&dyn rusqlite::types::ToSql],
            crate::db::models::record_summary_from_row,
        )?;
        for r in all_records {
            records_by_batch
                .entry(r.batch_id)
                .or_default()
                .push(serde_json::json!({
                    "id": r.id,
                    "batch_id": r.batch_id,
                    "device_id": r.device_id,
                    "status": r.status,
                    "ai_status": r.ai_status,
                    "report_path": r.report_path,
                    "error_message": r.error_message,
                    "started_at": r.started_at,
                    "completed_at": r.completed_at,
                }));
        }
    }

    let mut results = Vec::new();
    for batch in batches {
        let device_ids_value = batch
            .device_ids
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .unwrap_or(serde_json::json!([]));

        results.push(serde_json::json!({
            "id": batch.id,
            "name": batch.name,
            "status": batch.status,
            "triggered_by": batch.triggered_by,
            "device_ids": device_ids_value,
            "started_at": batch.started_at,
            "completed_at": batch.completed_at,
            "created_at": batch.created_at,
            "updated_at": batch.updated_at,
            "records": records_by_batch.remove(&batch.id).unwrap_or_default(),
        }));
    }

    Ok(results)
}

/// 获取单个巡检批次详情，包含完整的记录列表。
#[tauri::command]
pub fn get_batch(batch_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock();

    let sql = format!(
        "SELECT {} FROM inspection_batches WHERE id = ?1",
        BATCH_COLUMNS
    );
    let batch =
        crate::db::query::query_one(&conn, &sql, rusqlite::params![batch_id], batch_from_row)?
            .ok_or_else(|| format!("巡检批次 ID {} 不存在", batch_id))?;

    let records_sql = format!(
        "SELECT {} FROM inspection_records WHERE batch_id = ?1 ORDER BY id",
        crate::db::models::RECORD_SUMMARY_COLUMNS
    );
    let records = crate::db::query::query_all(
        &conn,
        &records_sql,
        rusqlite::params![batch_id],
        crate::db::models::record_summary_from_row,
    )?;

    // Parse device_ids from JSON string to array
    let device_ids_value = batch
        .device_ids
        .as_deref()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .unwrap_or(serde_json::json!([]));

    Ok(serde_json::json!({
        "id": batch.id,
        "name": batch.name,
        "status": batch.status,
        "triggered_by": batch.triggered_by,
        "device_ids": device_ids_value,
        "started_at": batch.started_at,
        "completed_at": batch.completed_at,
        "created_at": batch.created_at,
        "updated_at": batch.updated_at,
        "records": records,
    }))
}

// ============================================================
// Batch Mutate Commands
// ============================================================

/// 校验设备是否都配置了模板和 SSH 密码
fn validate_devices_ready(conn: &rusqlite::Connection, device_ids: &[i64]) -> Result<(), String> {
    let mut no_password: Vec<String> = Vec::new();
    for &id in device_ids {
        let sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
        if let Ok(Some(device)) =
            crate::db::query::query_one(conn, &sql, rusqlite::params![id], device_from_row)
        {
            let has_pwd = device
                .ssh_password_encrypted
                .as_ref()
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            if !has_pwd {
                no_password.push(format!("{} ({})", device.name, device.ip));
            }
        }
    }
    if !no_password.is_empty() {
        return Err(format!(
            "以下设备未配置 SSH 密码: {}",
            no_password.join("、")
        ));
    }
    Ok(())
}

/// 创建巡检批次。若 auto_start = true，则为每台设备创建记录并立即执行 SSH 巡检。
#[tauri::command]
pub async fn create_batch(
    data: BatchCreate,
    auto_start: Option<bool>,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let device_ids = data.device_ids.clone().unwrap_or_else(|| "[]".to_string());
    let parsed_ids: Vec<i64> =
        serde_json::from_str(&device_ids).map_err(|e| format!("解析设备ID列表失败: {}", e))?;

    // 前置校验：检查所有设备是否有模板和 SSH 密码
    {
        let conn = state.db.lock();
        validate_devices_ready(&conn, &parsed_ids)?;
    }

    // 插入批次记录（短暂获锁）
    let batch_id = {
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO inspection_batches (name, status, triggered_by, device_ids, started_at, completed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                data.name,
                data.status.as_deref().unwrap_or("pending"),
                data.triggered_by.as_deref().unwrap_or("manual"),
                device_ids,
                data.started_at,
                data.completed_at,
            ],
        )
        .map_err(|e| e.to_string())?;
        conn.last_insert_rowid()
    };

    tracing::info!(
        "批次 #{} 创建成功, auto_start={}",
        batch_id,
        auto_start.unwrap_or(false)
    );

    if auto_start.unwrap_or(false) {
        if parsed_ids.is_empty() {
            let conn = state.db.lock();
            let now = now_str();
            conn.execute(
                "UPDATE inspection_batches SET status = 'completed', started_at = ?1, \
                 completed_at = ?1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, batch_id],
            )
            .map_err(|e| e.to_string())?;
        } else {
            // 更新为 running
            {
                let conn = state.db.lock();
                let now = now_str();
                conn.execute(
                    "UPDATE inspection_batches SET status = 'running', started_at = ?1, \
                     updated_at = ?1 WHERE id = ?2",
                    rusqlite::params![now, batch_id],
                )
                .map_err(|e| e.to_string())?;
            }

            let device_count = parsed_ids.len();
            tracing::info!(
                "批次 #{} 自动开始执行, 共 {} 台设备并发",
                batch_id,
                device_count
            );

            // 注册取消标志
            let db = state.db.clone();
            let batch_cancels = state.batch_cancels.clone();
            let cancel = {
                let mut cancels = batch_cancels.lock();
                let flag = Arc::new(AtomicBool::new(false));
                cancels.insert(batch_id, Arc::clone(&flag));
                flag
            };

            // 后台执行：不阻塞 create_batch 返回，前端立即可见
            tokio::spawn(async move {
                let handles: Vec<_> = parsed_ids
                    .into_iter()
                    .map(|device_id| {
                        let db = Arc::clone(&db);
                        let cancel = Arc::clone(&cancel);
                        tokio::spawn(async move {
                            inspect_one_device(batch_id, device_id, db, cancel).await
                        })
                    })
                    .collect();

                await_handles_and_finalize(batch_id, handles, db, cancel, batch_cancels).await;
            });
        }
    }

    // Return the created batch
    let conn = state.db.lock();
    let query_sql = format!(
        "SELECT {} FROM inspection_batches WHERE id = ?1",
        BATCH_COLUMNS
    );
    let batch = crate::db::query::query_one(
        &conn,
        &query_sql,
        rusqlite::params![batch_id],
        batch_from_row,
    )?
    .ok_or_else(|| "创建巡检批次后查询失败".to_string())?;

    Ok(serde_json::json!(batch))
}

/// 运行指定批次，并发对每台设备执行 SSH 巡检命令。
/// 各设备的命令在各自 shell 内串行，但设备之间并行。
#[tauri::command]
pub async fn run_batch(batch_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    tracing::info!("执行批次 #{} 开始", batch_id);

    // 1. 读取批次信息和设备列表（短暂获锁）
    let device_ids = {
        let conn = state.db.lock();
        let sql = format!(
            "SELECT {} FROM inspection_batches WHERE id = ?1",
            BATCH_COLUMNS
        );
        let batch =
            crate::db::query::query_one(&conn, &sql, rusqlite::params![batch_id], batch_from_row)?
                .ok_or_else(|| format!("巡检批次 ID {} 不存在", batch_id))?;

        // 防止重复执行：已在运行中的批次不可再次启动
        if batch.status == "running" {
            return Err(format!("巡检批次 #{} 正在运行中，请勿重复执行", batch_id));
        }

        let device_ids_str = batch.device_ids.unwrap_or_else(|| "[]".to_string());
        let ids: Vec<i64> = serde_json::from_str(&device_ids_str)
            .map_err(|e| format!("解析设备ID列表失败: {}", e))?;

        if ids.is_empty() {
            let now = now_str();
            conn.execute(
                "UPDATE inspection_batches SET status = 'completed', started_at = ?1, \
                 completed_at = ?1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, batch_id],
            )
            .map_err(|e| e.to_string())?;
            tracing::info!("批次 #{} 无设备，直接标记为完成", batch_id);
            return Ok(());
        }

        // 前置校验：设备是否配置了模板和 SSH 密码
        validate_devices_ready(&conn, &ids)?;

        // 更新批次状态为 running
        let now = now_str();
        conn.execute(
            "UPDATE inspection_batches SET status = 'running', started_at = ?1, \
             updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, batch_id],
        )
        .map_err(|e| e.to_string())?;

        tracing::info!("批次 #{} 状态→running, 设备数={}", batch_id, ids.len());
        ids
    }; // 锁释放

    // 2. 并发执行各设备（后台运行，不阻塞 run_batch 返回）
    let db = state.db.clone();
    let batch_cancels = state.batch_cancels.clone();
    let cancel = {
        let mut cancels = batch_cancels.lock();
        let flag = Arc::new(AtomicBool::new(false));
        cancels.insert(batch_id, Arc::clone(&flag));
        flag
    };

    let handles: Vec<_> = device_ids
        .into_iter()
        .map(|device_id| {
            let db = Arc::clone(&db);
            let cancel = Arc::clone(&cancel);
            tokio::spawn(async move { inspect_one_device(batch_id, device_id, db, cancel).await })
        })
        .collect();

    tokio::spawn(async move {
        await_handles_and_finalize(batch_id, handles, db, cancel, batch_cancels).await;
    });

    Ok(())
}

/// 后台等待所有设备任务完成，更新批次最终状态
async fn await_handles_and_finalize(
    batch_id: i64,
    handles: Vec<tokio::task::JoinHandle<Result<(), (i64, String)>>>,
    db: Arc<parking_lot::Mutex<rusqlite::Connection>>,
    cancel: Arc<AtomicBool>,
    batch_cancels: Arc<parking_lot::Mutex<std::collections::HashMap<i64, Arc<AtomicBool>>>>,
) {
    let mut completed_count = 0u32;
    let mut failed_count = 0u32;
    let mut stopped_count = 0u32;
    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => completed_count += 1,
            Ok(Err((_id, err))) if err == "巡检已停止" => {
                stopped_count += 1;
                tracing::info!("批次 #{} 设备巡检已停止", batch_id);
            }
            Ok(Err((_id, err))) => {
                failed_count += 1;
                tracing::warn!("批次 #{} 设备巡检失败: {}", batch_id, err);
            }
            Err(join_err) => {
                failed_count += 1;
                tracing::error!("批次 #{} 设备任务 panic: {}", batch_id, join_err);
            }
        }
    }

    tracing::info!(
        "批次 #{} 执行任务已结束: completed={}, failed={}, stopped={}, cancel={}",
        batch_id,
        completed_count,
        failed_count,
        stopped_count,
        cancel.load(Ordering::Relaxed),
    );

    {
        let conn = db.lock();
        if let Err(err) = finalize_batch_status(&conn, batch_id) {
            tracing::error!("批次 #{} 状态收尾失败: {}", batch_id, err);
        }
    }

    batch_cancels.lock().remove(&batch_id);
}

/// 执行单台设备的巡检流程：读数据 → 建记录 → SSH → 写结果。
/// 在 tokio 任务中运行，多设备并发调用。
/// 通过 Arc<Mutex> 将当前执行命令回写 DB，前端可实时看到进度。
async fn inspect_one_device(
    batch_id: i64,
    device_id: i64,
    db: Arc<parking_lot::Mutex<rusqlite::Connection>>,
    cancel: Arc<AtomicBool>,
) -> Result<(), (i64, String)> {
    if cancel.load(Ordering::Relaxed) {
        return Err((device_id, "巡检已停止".to_string()));
    }

    // 读取数据
    let (device, username, password, commands) = {
        let conn = db.lock();
        read_device_inspection_data(&conn, device_id).map_err(|e| (device_id, e))?
    };

    // 创建/重置记录
    let record_id = {
        let conn = db.lock();
        create_or_reset_record(&conn, batch_id, device_id).map_err(|e| (device_id, e))?
    };

    if cancel.load(Ordering::Relaxed) {
        let conn = db.lock();
        update_record_result(&conn, record_id, "stopped", None, None, Some("巡检已停止"))
            .map_err(|e| (device_id, e))?;
        return Err((device_id, "巡检已停止".to_string()));
    }

    tracing::info!(
        "批次 #{} 设备 #{} ({}) [{}] SSH 开始, {} 条命令",
        batch_id,
        device_id,
        device.name,
        device.ip,
        commands.len()
    );

    // 进度共享：SSH runner 写入当前命令，poller 每隔 2 秒刷新到 DB
    let progress = Arc::new(std::sync::Mutex::new(String::new()));
    let progress_clone = Arc::clone(&progress);
    let db_clone = Arc::clone(&db);
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);

    // 后台任务：定期将进度写入 DB 的 error_message 字段
    let poller = tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if stop_clone.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let msg = progress_clone.lock().unwrap().clone();
            if !msg.is_empty() {
                let conn = db_clone.lock();
                let _ = conn.execute(
                    "UPDATE inspection_records SET error_message = ?1 WHERE id = ?2",
                    rusqlite::params![format!("正在执行: {}", msg), record_id],
                );
            }
        }
    });

    // SSH 执行（spawn_blocking 避免阻塞 tokio）
    let progress_ssh = Arc::clone(&progress);
    let ssh_result = {
        let device_clone = device.clone();
        let username_clone = username.clone();
        let password_clone = password.clone();
        let commands_clone = commands.clone();
        let cancel_ssh = Arc::clone(&cancel);
        tokio::task::spawn_blocking(move || {
            execute_device_ssh(
                &device_clone,
                &username_clone,
                &password_clone,
                &commands_clone,
                Some(progress_ssh),
                cancel_ssh,
            )
        })
        .await
        .map_err(|e| (device_id, format!("SSH 任务调度失败: {}", e)))?
    };

    // 先停止 poller 并等待其完全退出，避免进度信息覆盖最终结果
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = poller.await;

    // 写入结果
    {
        let conn = db.lock();
        match &ssh_result {
            Ok(outputs) => {
                let static_info = build_static_info(outputs, &commands);
                let report_outputs = visible_outputs(outputs, &commands);
                let outputs_json = serde_json::to_string(&report_outputs)
                    .map_err(|e| (device_id, format!("序列化命令输出失败: {}", e)))?;
                let static_info_json = serde_json::to_string(&static_info)
                    .map_err(|e| (device_id, format!("序列化静态信息失败: {}", e)))?;
                update_record_result(
                    &conn,
                    record_id,
                    "completed",
                    Some(&outputs_json),
                    Some(&static_info_json),
                    None,
                )
                .map_err(|e| (device_id, e))?;
                sync_device_static_info(&conn, device_id, &static_info);
                tracing::info!(
                    "批次 #{} 设备 #{} OK, 获得 {} 条命令输出（报告显示 {} 条）",
                    batch_id,
                    device_id,
                    outputs.len(),
                    report_outputs.len()
                );
            }
            Err(err) => {
                if cancel.load(Ordering::Relaxed) || err == "巡检已停止" {
                    update_record_result(
                        &conn,
                        record_id,
                        "stopped",
                        None,
                        None,
                        Some("巡检已停止"),
                    )
                    .map_err(|e| (device_id, e))?;
                    return Err((device_id, "巡检已停止".to_string()));
                }
                update_record_result(&conn, record_id, "failed", None, None, Some(err))
                    .map_err(|e| (device_id, e))?;
                return Err((device_id, err.clone()));
            }
        }
    }

    Ok(())
}

/// 暂停指定批次（仅允许 running 状态的批次）。
///
/// 通过设置 cancel flag 让正在执行的设备在当前命令边界停止（记录变为 stopped），
/// 批次状态置为 paused。finalize_batch_status 会保留 paused 状态不被自动覆盖。
/// 恢复巡检需通过 restart_and_run_batch 重新执行。
#[tauri::command]
pub fn pause_batch(batch_id: i64, state: State<AppState>) -> Result<(), String> {
    // 设置 cancel flag，让 running 设备的 SSH 任务在下个命令边界停止
    if let Some(cancel) = state.batch_cancels.lock().get(&batch_id) {
        cancel.store(true, Ordering::Relaxed);
    }

    let conn = state.db.lock();

    let now = now_str();
    let affected = conn
        .execute(
            "UPDATE inspection_batches SET status = 'paused', updated_at = ?1 \
             WHERE id = ?2 AND status = 'running'",
            rusqlite::params![now, batch_id],
        )
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("巡检批次 ID {} 不存在或状态不是 running", batch_id));
    }

    // 将仍在 running 的记录标记为 stopped（pending 记录保持，待恢复时执行）
    conn.execute(
        "UPDATE inspection_records SET status = 'stopped', updated_at = ?1 \
         WHERE batch_id = ?2 AND status = 'running'",
        rusqlite::params![now, batch_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// 停止指定批次，同时将批次内所有 running 状态的记录改为 stopped。
#[tauri::command]
pub fn stop_batch(batch_id: i64, state: State<AppState>) -> Result<(), String> {
    if let Some(cancel) = state.batch_cancels.lock().get(&batch_id) {
        cancel.store(true, Ordering::Relaxed);
    }

    let conn = state.db.lock();

    // Verify batch exists
    let sql = format!(
        "SELECT {} FROM inspection_batches WHERE id = ?1",
        BATCH_COLUMNS
    );
    crate::db::query::query_one(&conn, &sql, rusqlite::params![batch_id], batch_from_row)?
        .ok_or_else(|| format!("巡检批次 ID {} 不存在", batch_id))?;

    let now = now_str();

    // Set batch status to "stopped"
    conn.execute(
        "UPDATE inspection_batches SET status = 'stopped', updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, batch_id],
    )
    .map_err(|e| e.to_string())?;

    // Set any "running"/"pending" records to "stopped"
    conn.execute(
        "UPDATE inspection_records SET status = 'stopped', updated_at = ?1 \
         WHERE batch_id = ?2 AND status IN ('running', 'pending')",
        rusqlite::params![now, batch_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// 重启指定批次，重置所有非 running/pending 的记录为 pending，批次状态重置为 pending。
/// 支持对已完成批次重新巡检。
#[tauri::command]
pub fn restart_batch(batch_id: i64, state: State<AppState>) -> Result<(), String> {
    // 若批次正在运行，设置取消标志停止在途 SSH 任务，并清除旧 flag。
    // 否则在途任务会继续向已重置的记录写入结果，且 run_batch 会插入新 flag 与之冲突。
    {
        let mut cancels = state.batch_cancels.lock();
        if let Some(cancel) = cancels.get(&batch_id) {
            cancel.store(true, Ordering::Relaxed);
        }
        cancels.remove(&batch_id);
    }

    let conn = state.db.lock();

    // Verify batch exists
    let sql = format!(
        "SELECT {} FROM inspection_batches WHERE id = ?1",
        BATCH_COLUMNS
    );
    crate::db::query::query_one(&conn, &sql, rusqlite::params![batch_id], batch_from_row)?
        .ok_or_else(|| format!("巡检批次 ID {} 不存在", batch_id))?;

    let now = now_str();

    // Reset all non-running/pending records to "pending" (包括 completed/failed/stopped)
    // 清除巡检输出、AI 分析结果和报告路径，回到初始状态
    conn.execute(
        "UPDATE inspection_records SET status = 'pending', error_message = NULL, \
         command_outputs = '{}', static_info = '{}', completed_at = NULL, \
         ai_status = 'pending', ai_result = NULL, ai_analysis = NULL, \
         ai_suggestions = NULL, command_judgments = NULL, summary_judgment = NULL, \
         report_path = NULL, updated_at = ?1 \
         WHERE batch_id = ?2 AND status NOT IN ('pending', 'running')",
        rusqlite::params![now, batch_id],
    )
    .map_err(|e| e.to_string())?;

    // Set batch status to "pending"
    conn.execute(
        "UPDATE inspection_batches SET status = 'pending', started_at = NULL, \
         completed_at = NULL, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, batch_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// 重新巡检：重置记录后立即执行，无需再手动点"执行"。
#[tauri::command]
pub async fn restart_and_run_batch(
    batch_id: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // 1. 重置
    restart_batch(batch_id, state.clone())?;
    // 2. 立即执行
    run_batch(batch_id, state).await
}

/// 重试单条巡检记录，重置为 running 后立即重新执行 SSH 巡检。
///
/// 注册取消标志到 batch_cancels（以 batch_id 为 key），使 stop_batch 可中止重试；
/// 重试完成后调用 finalize_batch_status 收尾批次，避免批次永久卡在 running。
#[tauri::command]
pub async fn retry_device(record_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let batch_cancels = state.batch_cancels.clone();
    let db = state.db.clone();

    // 读取记录信息并重置状态（短暂获锁）
    let (record_id, batch_id, device, username, password, commands) = {
        let conn = state.db.lock();

        let record_sql = format!(
            "SELECT {} FROM inspection_records WHERE id = ?1",
            RECORD_COLUMNS
        );
        let record = crate::db::query::query_one(
            &conn,
            &record_sql,
            rusqlite::params![record_id],
            record_from_row,
        )?
        .ok_or_else(|| format!("巡检记录 ID {} 不存在", record_id))?;

        let batch_id = record.batch_id;
        let device_id = record.device_id;

        // Reset record to running
        let now = now_str();
        conn.execute(
            "UPDATE inspection_records SET status = 'running', error_message = NULL, \
             command_outputs = '{}', static_info = '{}', started_at = ?1, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, record_id],
        )
        .map_err(|e| e.to_string())?;

        // Set batch to running
        conn.execute(
            "UPDATE inspection_batches SET status = 'running', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, batch_id],
        )
        .map_err(|e| e.to_string())?;

        let (device, username, password, commands) = read_device_inspection_data(&conn, device_id)?;
        (record_id, batch_id, device, username, password, commands)
    }; // 锁释放

    // 注册取消标志：若批次已有 flag（批次正在运行）则复用，否则新建。
    // we_registered 标记是否由本次重试创建，决定收尾时是否清理 flag 与收尾批次。
    let (cancel, we_registered) = {
        let mut cancels = batch_cancels.lock();
        if let Some(existing) = cancels.get(&batch_id) {
            (Arc::clone(existing), false)
        } else {
            let flag = Arc::new(AtomicBool::new(false));
            cancels.insert(batch_id, Arc::clone(&flag));
            (flag, true)
        }
    };

    // SSH 执行（锁外）
    let ssh_result = {
        let device_clone = device.clone();
        let username_clone = username.clone();
        let password_clone = password.clone();
        let commands_clone = commands.clone();
        let cancel = Arc::clone(&cancel);
        tokio::task::spawn_blocking(move || {
            execute_device_ssh(
                &device_clone,
                &username_clone,
                &password_clone,
                &commands_clone,
                None,
                cancel,
            )
        })
        .await
        .map_err(|e| format!("SSH 任务调度失败: {}", e))?
    };

    // 写入结果并收尾批次（短暂获锁）
    {
        let conn = db.lock();
        match ssh_result {
            Ok(outputs) => {
                let static_info = build_static_info(&outputs, &commands);
                let report_outputs = visible_outputs(&outputs, &commands);
                let outputs_json = serde_json::to_string(&report_outputs)
                    .map_err(|e| format!("序列化命令输出失败: {}", e))?;
                let static_info_json = serde_json::to_string(&static_info)
                    .map_err(|e| format!("序列化静态信息失败: {}", e))?;
                update_record_result(
                    &conn,
                    record_id,
                    "completed",
                    Some(&outputs_json),
                    Some(&static_info_json),
                    None,
                )?;
                sync_device_static_info(&conn, device.id, &static_info);
            }
            Err(err) => {
                update_record_result(&conn, record_id, "failed", None, None, Some(&err))?;
            }
        }

        // 仅当本次重试独占批次（非批次运行中触发）时收尾批次并清理 flag，
        // 否则交由批次的 await_handles_and_finalize 管理。
        if we_registered {
            if let Err(e) = finalize_batch_status(&conn, batch_id) {
                tracing::warn!("重试后批次 #{} 收尾失败: {}", batch_id, e);
            }
        }
    }

    if we_registered {
        batch_cancels.lock().remove(&batch_id);
    }

    Ok(())
}

/// 删除指定批次及其关联的所有记录。
#[tauri::command]
pub fn delete_batch(batch_id: i64, state: State<AppState>) -> Result<(), String> {
    let mut conn = state.db.lock();

    // 删除前检查批次状态，运行中的批次需先停止
    let sql = format!(
        "SELECT {} FROM inspection_batches WHERE id = ?1",
        BATCH_COLUMNS
    );
    if let Some(batch) = crate::db::query::query_one(&conn, &sql, rusqlite::params![batch_id], batch_from_row)? {
        if batch.status == "running" {
            return Err("巡检批次正在运行中，请先停止后再删除".to_string());
        }
    }

    // 设置取消标志，确保残留 SSH 任务停止
    if let Some(cancel) = state.batch_cancels.lock().get(&batch_id) {
        cancel.store(true, Ordering::Relaxed);
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // Delete associated records
    tx.execute(
        "DELETE FROM inspection_records WHERE batch_id = ?1",
        rusqlite::params![batch_id],
    )
    .map_err(|e| e.to_string())?;

    // Delete the batch
    let affected = tx
        .execute(
            "DELETE FROM inspection_batches WHERE id = ?1",
            rusqlite::params![batch_id],
        )
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("巡检批次 ID {} 不存在", batch_id));
    }

    tx.commit().map_err(|e| e.to_string())?;

    Ok(())
}

