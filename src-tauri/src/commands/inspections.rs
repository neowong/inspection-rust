use tauri::State;
use rusqlite::types::ToSql;

use crate::AppState;
use crate::db::models::{
    BatchCreate, CommandPool, Device, InspectionBatch, InspectionRecord, InspectionTemplate,
};
use crate::services::crypto::CryptoService;
use crate::services::inspection_runner::{self, SSHSessionSource};

// ============================================================
// Constants
// ============================================================

const BATCH_COLUMNS: &str =
    "id, name, status, triggered_by, device_ids, started_at, completed_at, created_at, updated_at";

const RECORD_COLUMNS: &str =
    "id, batch_id, device_id, status, error_message, command_outputs, ai_status, ai_result, \
     ai_analysis, ai_suggestions, command_judgments, summary_judgment, report_path, \
     started_at, completed_at, created_at, updated_at";

const DEVICE_COLUMNS: &str =
    "id, name, ip, device_type, vendor, model, ssh_username, ssh_password_encrypted, \
     ssh_port, template_id, status, last_checked_at, created_at, updated_at";

const TEMPLATE_COLUMNS: &str =
    "id, name, vendor, model, device_type, config, description, report_template_id, \
     created_at, updated_at";

const COMMAND_COLUMNS: &str =
    "id, vendor, command, description, category, model, created_at, updated_at";

// ============================================================
// Row Helpers
// ============================================================

fn batch_from_row(row: &rusqlite::Row) -> rusqlite::Result<InspectionBatch> {
    Ok(InspectionBatch {
        id: row.get(0)?,
        name: row.get(1)?,
        status: row.get(2)?,
        triggered_by: row.get(3)?,
        device_ids: row.get(4)?,
        started_at: row.get(5)?,
        completed_at: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn record_from_row(row: &rusqlite::Row) -> rusqlite::Result<InspectionRecord> {
    Ok(InspectionRecord {
        id: row.get(0)?,
        batch_id: row.get(1)?,
        device_id: row.get(2)?,
        status: row.get(3)?,
        error_message: row.get(4)?,
        command_outputs: row.get(5)?,
        ai_status: row.get(6)?,
        ai_result: row.get(7)?,
        ai_analysis: row.get(8)?,
        ai_suggestions: row.get(9)?,
        command_judgments: row.get(10)?,
        summary_judgment: row.get(11)?,
        report_path: row.get(12)?,
        started_at: row.get(13)?,
        completed_at: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
}

fn device_from_row(row: &rusqlite::Row) -> rusqlite::Result<Device> {
    Ok(Device {
        id: row.get(0)?,
        name: row.get(1)?,
        ip: row.get(2)?,
        device_type: row.get(3)?,
        vendor: row.get(4)?,
        model: row.get(5)?,
        ssh_username: row.get(6)?,
        ssh_password_encrypted: row.get(7)?,
        ssh_port: row.get(8)?,
        template_id: row.get(9)?,
        status: row.get(10)?,
        last_checked_at: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn template_from_row(row: &rusqlite::Row) -> rusqlite::Result<InspectionTemplate> {
    Ok(InspectionTemplate {
        id: row.get(0)?,
        name: row.get(1)?,
        vendor: row.get(2)?,
        model: row.get(3)?,
        device_type: row.get(4)?,
        config: row.get(5)?,
        description: row.get(6)?,
        report_template_id: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn command_from_row(row: &rusqlite::Row) -> rusqlite::Result<CommandPool> {
    Ok(CommandPool {
        id: row.get(0)?,
        vendor: row.get(1)?,
        command: row.get(2)?,
        description: row.get(3)?,
        category: row.get(4)?,
        model: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

// ============================================================
// Internal Helpers
// ============================================================

/// Returns the current timestamp as a formatted string.
fn now_str() -> String {
    chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

/// Execute inspection commands on a single device via SSH synchronously.
///
/// 1. Looks up the device and decrypts its SSH password.
/// 2. Looks up the associated template and parses command IDs from its config.
/// 3. Fetches each command from the command pool.
/// 4. Creates or updates the inspection record to "running".
/// 5. Calls `inspection_runner::run_commands` to execute SSH commands.
/// 6. Updates the record with outputs (status = "completed") or error (status = "failed").
fn execute_device_inspection(
    conn: &rusqlite::Connection,
    device_id: i64,
    batch_id: i64,
) -> Result<(), String> {
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

    // 4. Parse template config for command IDs
    let config_str = template
        .config
        .ok_or_else(|| format!("模板 '{}' 配置为空", template.name))?;
    let config: serde_json::Value = serde_json::from_str(&config_str)
        .map_err(|e| format!("解析模板配置 JSON 失败: {}", e))?;

    let command_ids: Vec<i64> = config["command_ids"]
        .as_array()
        .ok_or_else(|| format!("模板 '{}' 配置缺少 command_ids", template.name))?
        .iter()
        .filter_map(|v| v.as_i64())
        .collect();

    // 5. Fetch commands from command_pool by ID
    let mut commands: Vec<String> = Vec::new();
    for cmd_id in &command_ids {
        let cmd_sql = format!("SELECT {} FROM command_pool WHERE id = ?1", COMMAND_COLUMNS);
        let cmd = crate::db::query::query_one(
            conn,
            &cmd_sql,
            rusqlite::params![cmd_id],
            command_from_row,
        )?
        .ok_or_else(|| format!("命令 ID {} 不存在", cmd_id))?;
        commands.push(cmd.command);
    }

    if commands.is_empty() {
        return Err(format!(
            "设备 '{}' 的巡检模板 '{}' 未包含有效命令",
            device.name, template.name
        ));
    }

    // 6. Create or update inspection record to "running"
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
                 command_outputs = '{}', started_at = ?1, updated_at = ?1 WHERE id = ?2",
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

    // 7. Execute SSH commands
    let source = SSHSessionSource {
        host: device.ip.clone(),
        port: device.ssh_port as u16,
        username,
        password,
    };

    match inspection_runner::run_commands(&source, &device.vendor, &commands) {
        Ok(outputs) => {
            let outputs_json = serde_json::to_string(&outputs)
                .map_err(|e| format!("序列化命令输出失败: {}", e))?;
            let completed_at = now_str();
            conn.execute(
                "UPDATE inspection_records SET status = 'completed', command_outputs = ?1, \
                 completed_at = ?2, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![outputs_json, completed_at, record_id],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(err) => {
            let completed_at = now_str();
            conn.execute(
                "UPDATE inspection_records SET status = 'failed', error_message = ?1, \
                 completed_at = ?2, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![&err, completed_at, record_id],
            )
            .map_err(|e| e.to_string())?;
            Err(err)
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

    let mut sql = format!(
        "SELECT {} FROM inspection_batches WHERE 1=1",
        BATCH_COLUMNS
    );
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(ref s) = status {
        sql.push_str(" AND status = ?");
        params.push(Box::new(s.clone()));
    }

    sql.push_str(" ORDER BY created_at DESC LIMIT 50");

    let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let batches = crate::db::query::query_all(&conn, &sql, &param_refs, batch_from_row)?;

    let mut results = Vec::new();
    for batch in batches {
        let records_sql = format!(
            "SELECT {} FROM inspection_records WHERE batch_id = ?1",
            RECORD_COLUMNS
        );
        let records = crate::db::query::query_all(
            &conn,
            &records_sql,
            rusqlite::params![batch.id],
            record_from_row,
        )?;

        let record_summaries: Vec<serde_json::Value> = records
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "batch_id": r.batch_id,
                    "device_id": r.device_id,
                    "status": r.status,
                    "ai_status": r.ai_status,
                    "report_path": r.report_path,
                    "error_message": r.error_message,
                })
            })
            .collect();

        results.push(serde_json::json!({
            "id": batch.id,
            "name": batch.name,
            "status": batch.status,
            "triggered_by": batch.triggered_by,
            "device_ids": batch.device_ids,
            "started_at": batch.started_at,
            "completed_at": batch.completed_at,
            "created_at": batch.created_at,
            "updated_at": batch.updated_at,
            "records": record_summaries,
        }));
    }

    Ok(results)
}

/// 获取单个巡检批次详情，包含完整的记录列表。
#[tauri::command]
pub fn get_batch(
    batch_id: i64,
    state: State<AppState>,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock();

    let sql = format!(
        "SELECT {} FROM inspection_batches WHERE id = ?1",
        BATCH_COLUMNS
    );
    let batch = crate::db::query::query_one(
        &conn,
        &sql,
        rusqlite::params![batch_id],
        batch_from_row,
    )?
    .ok_or_else(|| format!("巡检批次 ID {} 不存在", batch_id))?;

    let records_sql = format!(
        "SELECT {} FROM inspection_records WHERE batch_id = ?1 ORDER BY id",
        RECORD_COLUMNS
    );
    let records = crate::db::query::query_all(
        &conn,
        &records_sql,
        rusqlite::params![batch_id],
        record_from_row,
    )?;

    Ok(serde_json::json!({
        "id": batch.id,
        "name": batch.name,
        "status": batch.status,
        "triggered_by": batch.triggered_by,
        "device_ids": batch.device_ids,
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

/// 创建巡检批次。若 auto_start = true，则为每台设备创建记录并立即执行 SSH 巡检。
#[tauri::command]
pub fn create_batch(
    data: BatchCreate,
    auto_start: Option<bool>,
    state: State<AppState>,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock();

    let device_ids = data.device_ids.clone().unwrap_or_else(|| "[]".to_string());

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

    let batch_id = conn.last_insert_rowid();

    if auto_start.unwrap_or(false) {
        let parsed_ids: Vec<i64> = serde_json::from_str(&device_ids)
            .map_err(|e| format!("解析设备ID列表失败: {}", e))?;

        if parsed_ids.is_empty() {
            let now = now_str();
            conn.execute(
                "UPDATE inspection_batches SET status = 'completed', started_at = ?1, \
                 completed_at = ?1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, batch_id],
            )
            .map_err(|e| e.to_string())?;
        } else {
            let now = now_str();
            conn.execute(
                "UPDATE inspection_batches SET status = 'running', started_at = ?1, \
                 updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, batch_id],
            )
            .map_err(|e| e.to_string())?;

            let mut completed_count = 0;
            let mut failed_count = 0;

            for device_id in &parsed_ids {
                conn.execute(
                    "INSERT INTO inspection_records (batch_id, device_id, status) \
                     VALUES (?1, ?2, 'pending')",
                    rusqlite::params![batch_id, device_id],
                )
                .map_err(|e| e.to_string())?;

                match execute_device_inspection(&conn, *device_id, batch_id) {
                    Ok(_) => completed_count += 1,
                    Err(e) => {
                        failed_count += 1;
                        eprintln!("设备 {} 巡检失败: {}", device_id, e);
                    }
                }
            }

            let final_status = if failed_count == 0 {
                "completed"
            } else if completed_count == 0 {
                "failed"
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
        }
    }

    // Return the created batch
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

/// 运行指定批次，对批次内的每台设备执行 SSH 巡检命令。
/// 同步执行（顺序处理每台设备），前端可通过定时查询获取中间状态。
#[tauri::command]
pub fn run_batch(batch_id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock();

    // 1. Get batch
    let sql = format!(
        "SELECT {} FROM inspection_batches WHERE id = ?1",
        BATCH_COLUMNS
    );
    let batch = crate::db::query::query_one(
        &conn,
        &sql,
        rusqlite::params![batch_id],
        batch_from_row,
    )?
    .ok_or_else(|| format!("巡检批次 ID {} 不存在", batch_id))?;

    let device_ids_str = batch.device_ids.unwrap_or_else(|| "[]".to_string());
    let device_ids: Vec<i64> = serde_json::from_str(&device_ids_str)
        .map_err(|e| format!("解析设备ID列表失败: {}", e))?;

    if device_ids.is_empty() {
        let now = now_str();
        conn.execute(
            "UPDATE inspection_batches SET status = 'completed', started_at = ?1, \
             completed_at = ?1, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, batch_id],
        )
        .map_err(|e| e.to_string())?;
        return Ok(());
    }

    // 2. Update batch status to "running"
    let now = now_str();
    conn.execute(
        "UPDATE inspection_batches SET status = 'running', started_at = ?1, \
         updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, batch_id],
    )
    .map_err(|e| e.to_string())?;

    // 3. Execute for each device sequentially
    let mut completed_count = 0;
    let mut failed_count = 0;

    for device_id in &device_ids {
        match execute_device_inspection(&conn, *device_id, batch_id) {
            Ok(_) => completed_count += 1,
            Err(e) => {
                failed_count += 1;
                eprintln!("设备 {} 巡检失败: {}", device_id, e);
            }
        }
    }

    // 4. Update batch final status
    let final_status = if failed_count == 0 {
        "completed"
    } else if completed_count == 0 {
        "failed"
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

    Ok(())
}

/// 暂停指定批次（仅允许 running 状态的批次）。
#[tauri::command]
pub fn pause_batch(batch_id: i64, state: State<AppState>) -> Result<(), String> {
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
        return Err(format!(
            "巡检批次 ID {} 不存在或状态不是 running",
            batch_id
        ));
    }

    Ok(())
}

/// 停止指定批次，同时将批次内所有 running 状态的记录改为 stopped。
#[tauri::command]
pub fn stop_batch(batch_id: i64, state: State<AppState>) -> Result<(), String> {
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

    // Set any "running" records to "stopped"
    conn.execute(
        "UPDATE inspection_records SET status = 'stopped', updated_at = ?1 \
         WHERE batch_id = ?2 AND status = 'running'",
        rusqlite::params![now, batch_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// 重启指定批次，重置所有失败/已停止的记录为 pending，批次状态重置为 pending。
#[tauri::command]
pub fn restart_batch(batch_id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock();

    // Verify batch exists
    let sql = format!(
        "SELECT {} FROM inspection_batches WHERE id = ?1",
        BATCH_COLUMNS
    );
    crate::db::query::query_one(&conn, &sql, rusqlite::params![batch_id], batch_from_row)?
        .ok_or_else(|| format!("巡检批次 ID {} 不存在", batch_id))?;

    let now = now_str();

    // Reset failed/stopped records to "pending"
    conn.execute(
        "UPDATE inspection_records SET status = 'pending', error_message = NULL, \
         command_outputs = '{}', completed_at = NULL, updated_at = ?1 \
         WHERE batch_id = ?2 AND (status = 'failed' OR status = 'stopped')",
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

/// 重试单条巡检记录，重置为 pending 后立即重新执行 SSH 巡检。
#[tauri::command]
pub fn retry_device(record_id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock();

    // Get the record
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

    // Reset record to pending
    let now = now_str();
    conn.execute(
        "UPDATE inspection_records SET status = 'pending', error_message = NULL, \
         command_outputs = '{}', completed_at = NULL, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, record_id],
    )
    .map_err(|e| e.to_string())?;

    // Set batch to running
    conn.execute(
        "UPDATE inspection_batches SET status = 'running', updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, batch_id],
    )
    .map_err(|e| e.to_string())?;

    // Re-execute
    execute_device_inspection(&conn, device_id, batch_id)
}

/// 删除指定批次及其关联的所有记录。
#[tauri::command]
pub fn delete_batch(batch_id: i64, state: State<AppState>) -> Result<(), String> {
    let mut conn = state.db.lock();

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

/// 批量删除巡检批次及其关联记录（事务安全）。
#[tauri::command]
pub fn batch_delete_batches(ids: Vec<i64>, state: State<AppState>) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    for id in &ids {
        tx.execute(
            "DELETE FROM inspection_records WHERE batch_id = ?1",
            rusqlite::params![id],
        )
        .map_err(|e| e.to_string())?;

        tx.execute(
            "DELETE FROM inspection_batches WHERE id = ?1",
            rusqlite::params![id],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;

    Ok(())
}

// ============================================================
// Record Commands
// ============================================================

/// 删除单条巡检记录。
#[tauri::command]
pub fn delete_record(record_id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock();

    let affected = conn
        .execute(
            "DELETE FROM inspection_records WHERE id = ?1",
            rusqlite::params![record_id],
        )
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("巡检记录 ID {} 不存在", record_id));
    }

    Ok(())
}

/// 批量删除巡检记录（事务安全）。
#[tauri::command]
pub fn batch_delete_records(ids: Vec<i64>, state: State<AppState>) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    for id in &ids {
        tx.execute(
            "DELETE FROM inspection_records WHERE id = ?1",
            rusqlite::params![id],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;

    Ok(())
}
