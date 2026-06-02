use std::sync::Arc;
use tauri::State;
use rusqlite::types::ToSql;

use crate::AppState;
use crate::db::models::{
    BatchCreate, Device,
    BATCH_COLUMNS, RECORD_COLUMNS, DEVICE_COLUMNS, TEMPLATE_COLUMNS, COMMAND_COLUMNS,
    batch_from_row, record_from_row, device_from_row, template_from_row, command_from_row,
    now_str,
};
use crate::services::crypto::CryptoService;
use crate::services::inspection_runner::{self, SSHSessionSource};

// ============================================================
// Internal Helpers
// ============================================================

/// 从数据库读取设备巡检所需的全部信息（在锁内调用）
fn read_device_inspection_data(
    conn: &rusqlite::Connection,
    device_id: i64,
) -> Result<(Device, String, String, Vec<String>), String> {
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
    Ok(record_id)
}

/// 更新巡检记录结果（在锁内调用）
fn update_record_result(
    conn: &rusqlite::Connection,
    record_id: i64,
    status: &str,
    outputs_json: Option<&str>,
    error: Option<&str>,
) -> Result<(), String> {
    let completed_at = now_str();
    match (outputs_json, error) {
        (Some(json), _) => {
            // Parse output count for summary
            let cmd_count = serde_json::from_str::<serde_json::Value>(json)
                .ok()
                .and_then(|v| v.as_object().map(|o| o.len()))
                .unwrap_or(0);
            let summary = format!("完成 {} 条命令", cmd_count);
            conn.execute(
                "UPDATE inspection_records SET status = ?1, command_outputs = ?2, \
                 error_message = ?3, completed_at = ?4, updated_at = ?4 WHERE id = ?5",
                rusqlite::params![status, json, summary, completed_at, record_id],
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

/// 执行单台设备的 SSH 巡检（锁外调用，包含耗时的 SSH 操作）
fn execute_device_ssh(
    device: &Device,
    username: &str,
    password: &str,
    commands: &[String],
    on_progress: Option<Arc<std::sync::Mutex<String>>>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let source = SSHSessionSource {
        host: device.ip.clone(),
        port: device.ssh_port as u16,
        username: username.to_string(),
        password: password.to_string(),
    };
    inspection_runner::run_commands(&source, &device.vendor, commands, on_progress)
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

        // Parse device_ids from JSON string to array
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
fn validate_devices_ready(
    conn: &rusqlite::Connection,
    device_ids: &[i64],
) -> Result<(), String> {
    let mut no_password: Vec<String> = Vec::new();
    for &id in device_ids {
        let sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
        if let Ok(Some(device)) = crate::db::query::query_one(
            conn, &sql, rusqlite::params![id], device_from_row,
        ) {
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
    let parsed_ids: Vec<i64> = serde_json::from_str(&device_ids)
        .map_err(|e| format!("解析设备ID列表失败: {}", e))?;

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

    tracing::info!("批次 #{} 创建成功, auto_start={}", batch_id, auto_start.unwrap_or(false));

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
            tracing::info!("批次 #{} 自动开始执行, 共 {} 台设备并发", batch_id, device_count);

            let db = state.db.clone();
            let handles: Vec<_> = parsed_ids.into_iter().map(|device_id| {
                let db = Arc::clone(&db);
                tokio::spawn(async move {
                    inspect_one_device(batch_id, device_id, db).await
                })
            }).collect();

            let mut completed_count = 0u32;
            let mut failed_count = 0u32;
            for handle in handles {
                match handle.await {
                    Ok(Ok(_)) => completed_count += 1,
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

            let final_status = if failed_count == 0 {
                "completed"
            } else if completed_count == 0 {
                "failed"
            } else {
                "partially_completed"
            };

            tracing::info!(
                "批次 #{} 执行完毕: status={}, completed={}, failed={}",
                batch_id, final_status, completed_count, failed_count
            );

            {
                let conn = state.db.lock();
                let now = now_str();
                conn.execute(
                    "UPDATE inspection_batches SET status = ?1, completed_at = ?2, updated_at = ?2 \
                     WHERE id = ?3",
                    rusqlite::params![final_status, now, batch_id],
                )
                .map_err(|e| e.to_string())?;
            }
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
        let batch = crate::db::query::query_one(
            &conn,
            &sql,
            rusqlite::params![batch_id],
            batch_from_row,
        )?
        .ok_or_else(|| format!("巡检批次 ID {} 不存在", batch_id))?;

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

    // 2. 并发执行各设备
    let db = state.db.clone();

    let handles: Vec<_> = device_ids.into_iter().map(|device_id| {
        let db = Arc::clone(&db);
        tokio::spawn(async move {
            inspect_one_device(batch_id, device_id, db).await
        })
    }).collect();

    let mut completed_count = 0u32;
    let mut failed_count = 0u32;

    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => completed_count += 1,
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

    // 3. 更新批次最终状态（短暂获锁）
    let final_status = if failed_count == 0 {
        "completed"
    } else if completed_count == 0 {
        "failed"
    } else {
        "partially_completed"
    };

    tracing::info!(
        "批次 #{} 执行完毕: status={}, completed={}, failed={}",
        batch_id, final_status, completed_count, failed_count
    );

    {
        let conn = state.db.lock();
        let now = now_str();
        conn.execute(
            "UPDATE inspection_batches SET status = ?1, completed_at = ?2, updated_at = ?2 \
             WHERE id = ?3",
            rusqlite::params![final_status, now, batch_id],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// 执行单台设备的巡检流程：读数据 → 建记录 → SSH → 写结果。
/// 在 tokio 任务中运行，多设备并发调用。
/// 通过 Arc<Mutex> 将当前执行命令回写 DB，前端可实时看到进度。
async fn inspect_one_device(
    batch_id: i64,
    device_id: i64,
    db: Arc<parking_lot::Mutex<rusqlite::Connection>>,
) -> Result<(), (i64, String)> {
    // 读取数据
    let (device, username, password, commands) = {
        let conn = db.lock();
        read_device_inspection_data(&conn, device_id)
            .map_err(|e| (device_id, e))?
    };

    // 创建/重置记录
    let record_id = {
        let conn = db.lock();
        create_or_reset_record(&conn, batch_id, device_id)
            .map_err(|e| (device_id, e))?
    };

    tracing::info!("批次 #{} 设备 #{} ({}) [{}] SSH 开始, {} 条命令", batch_id, device_id, device.name, device.ip, commands.len());

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
        tokio::task::spawn_blocking(move || {
            execute_device_ssh(&device_clone, &username_clone, &password_clone, &commands_clone, Some(progress_ssh))
        })
        .await
        .map_err(|e| (device_id, format!("SSH 任务调度失败: {}", e)))?
    };

    // 停止 poller
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = poller.await;

    // 写入结果
    {
        let conn = db.lock();
        match &ssh_result {
            Ok(outputs) => {
                let outputs_json = serde_json::to_string(outputs)
                    .map_err(|e| (device_id, format!("序列化命令输出失败: {}", e)))?;
                update_record_result(&conn, record_id, "completed", Some(&outputs_json), None)
                    .map_err(|e| (device_id, e))?;
                tracing::info!("批次 #{} 设备 #{} OK, 获得 {} 条命令输出", batch_id, device_id, outputs.len());
            }
            Err(err) => {
                update_record_result(&conn, record_id, "failed", None, Some(err))
                    .map_err(|e| (device_id, e))?;
                return Err((device_id, err.clone()));
            }
        }
    }

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
pub async fn retry_device(record_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    // 读取记录信息并重置状态（短暂获锁）
    let (record_id, device, username, password, commands) = {
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
             command_outputs = '{}', started_at = ?1, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, record_id],
        )
        .map_err(|e| e.to_string())?;

        // Set batch to running
        conn.execute(
            "UPDATE inspection_batches SET status = 'running', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, batch_id],
        )
        .map_err(|e| e.to_string())?;

        let (device, username, password, commands) =
            read_device_inspection_data(&conn, device_id)?;
        (record_id, device, username, password, commands)
    }; // 锁释放

    // SSH 执行（锁外）
    let ssh_result = {
        let device_clone = device.clone();
        let username_clone = username.clone();
        let password_clone = password.clone();
        let commands_clone = commands.clone();
        tokio::task::spawn_blocking(move || {
            execute_device_ssh(&device_clone, &username_clone, &password_clone, &commands_clone, None)
        })
        .await
        .map_err(|e| format!("SSH 任务调度失败: {}", e))?
    };

    // 写入结果（短暂获锁）
    let conn = state.db.lock();
    match ssh_result {
        Ok(outputs) => {
            let outputs_json = serde_json::to_string(&outputs)
                .map_err(|e| format!("序列化命令输出失败: {}", e))?;
            update_record_result(&conn, record_id, "completed", Some(&outputs_json), None)?;
        }
        Err(err) => {
            update_record_result(&conn, record_id, "failed", None, Some(&err))?;
        }
    }

    Ok(())
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
