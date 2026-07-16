use tauri::State;
use tracing::{info, warn};

use crate::db::models::{
    now_str, scheduled_task_from_row, ScheduledTask, ScheduledTaskCreate, ScheduledTaskUpdate,
    SCHEDULED_TASK_COLUMNS,
};
use crate::AppState;

// ============================================================
// Tauri Commands
// ============================================================

/// 创建定时任务
#[tauri::command]
pub fn create_scheduled_task(
    task: ScheduledTaskCreate,
    state: State<AppState>,
) -> Result<ScheduledTask, String> {
    info!("创建定时任务: name={}, type={}, cron={}", task.name, task.task_type, task.cron_expr);
    let conn = state.db.lock();
    let now = now_str();

    let enabled = if task.enabled.unwrap_or(true) { 1 } else { 0 };
    let config_json = task.config_json.unwrap_or_else(|| "{}".to_string());

    // 计算下次执行时间
    let next_run = calculate_next_run_from_cron(&task.cron_expr);

    conn.execute(
        "INSERT INTO scheduled_tasks (name, task_type, cron_expr, enabled, config_json, next_run_at, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        rusqlite::params![task.name, task.task_type, task.cron_expr, enabled, config_json, next_run, now],
    ).map_err(|e| format!("创建定时任务失败: {}", e))?;

    let id = conn.last_insert_rowid();
    info!("定时任务创建成功: id={}", id);
    let sql = format!("SELECT {} FROM scheduled_tasks WHERE id = ?1", SCHEDULED_TASK_COLUMNS);
    crate::db::query::query_one(&conn, &sql, rusqlite::params![id], scheduled_task_from_row)?
        .ok_or_else(|| format!("定时任务 ID {} 不存在", id))
}

/// 列出所有定时任务
#[tauri::command]
pub fn list_scheduled_tasks(
    task_type: Option<String>,
    state: State<AppState>,
) -> Result<Vec<ScheduledTask>, String> {
    let conn = state.db.lock();

    let (sql, params): (String, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(ref tt) = task_type {
        (
            format!("SELECT {} FROM scheduled_tasks WHERE task_type = ?1 ORDER BY created_at DESC", SCHEDULED_TASK_COLUMNS),
            vec![Box::new(tt.clone())],
        )
    } else {
        (
            format!("SELECT {} FROM scheduled_tasks ORDER BY created_at DESC", SCHEDULED_TASK_COLUMNS),
            vec![],
        )
    };

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    crate::db::query::query_all(&conn, &sql, param_refs.as_slice(), scheduled_task_from_row)
}

/// 获取单条定时任务详情
#[tauri::command]
pub fn get_scheduled_task(task_id: i64, state: State<AppState>) -> Result<ScheduledTask, String> {
    let conn = state.db.lock();
    let sql = format!("SELECT {} FROM scheduled_tasks WHERE id = ?1", SCHEDULED_TASK_COLUMNS);
    crate::db::query::query_one(&conn, &sql, rusqlite::params![task_id], scheduled_task_from_row)?
        .ok_or_else(|| format!("定时任务 ID {} 不存在", task_id))
}

/// 更新定时任务
#[tauri::command]
pub fn update_scheduled_task(
    task_id: i64,
    update: ScheduledTaskUpdate,
    state: State<AppState>,
) -> Result<ScheduledTask, String> {
    let conn = state.db.lock();
    let now = now_str();

    // 获取现有任务
    let sql = format!("SELECT {} FROM scheduled_tasks WHERE id = ?1", SCHEDULED_TASK_COLUMNS);
    let existing = crate::db::query::query_one(&conn, &sql, rusqlite::params![task_id], scheduled_task_from_row)?
        .ok_or_else(|| format!("定时任务 ID {} 不存在", task_id))?;

    let name = update.name.unwrap_or(existing.name);
    let cron_expr_changed = update.cron_expr.is_some();
    let cron_expr = update.cron_expr.unwrap_or(existing.cron_expr);
    let enabled = update.enabled.map(|e| if e { 1 } else { 0 }).unwrap_or(existing.enabled);
    let config_json = update.config_json.unwrap_or(existing.config_json);

    // 如果 cron 表达式变化，重新计算下次执行时间
    let next_run = if cron_expr_changed {
        calculate_next_run_from_cron(&cron_expr)
    } else {
        existing.next_run_at
    };

    conn.execute(
        "UPDATE scheduled_tasks SET name = ?1, cron_expr = ?2, enabled = ?3, config_json = ?4, \
         next_run_at = ?5, updated_at = ?6 WHERE id = ?7",
        rusqlite::params![name, cron_expr, enabled, config_json, next_run, now, task_id],
    ).map_err(|e| format!("更新定时任务失败: {}", e))?;
    info!("定时任务更新成功: id={}", task_id);

    let sql = format!("SELECT {} FROM scheduled_tasks WHERE id = ?1", SCHEDULED_TASK_COLUMNS);
    crate::db::query::query_one(&conn, &sql, rusqlite::params![task_id], scheduled_task_from_row)?
        .ok_or_else(|| format!("定时任务 ID {} 不存在", task_id))
}

/// 删除定时任务
#[tauri::command]
pub fn delete_scheduled_task(task_id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute("DELETE FROM scheduled_tasks WHERE id = ?1", rusqlite::params![task_id])
        .map_err(|e| format!("删除定时任务失败: {}", e))?;
    info!("定时任务已删除: id={}", task_id);
    Ok(())
}

/// 启用/禁用定时任务
#[tauri::command]
pub fn toggle_scheduled_task(task_id: i64, enabled: bool, state: State<AppState>) -> Result<ScheduledTask, String> {
    let conn = state.db.lock();
    let now = now_str();

    let enabled_val = if enabled { 1 } else { 0 };
    conn.execute(
        "UPDATE scheduled_tasks SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![enabled_val, now, task_id],
    ).map_err(|e| format!("切换定时任务状态失败: {}", e))?;
    info!("定时任务状态切换: id={}, enabled={}", task_id, enabled);

    let sql = format!("SELECT {} FROM scheduled_tasks WHERE id = ?1", SCHEDULED_TASK_COLUMNS);
    crate::db::query::query_one(&conn, &sql, rusqlite::params![task_id], scheduled_task_from_row)?
        .ok_or_else(|| format!("定时任务 ID {} 不存在", task_id))
}

/// 手动触发执行定时任务
#[tauri::command]
pub async fn run_scheduled_task(task_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    info!("手动触发定时任务: task_id={}", task_id);
    let (task_type, config_json) = {
        let conn = state.db.lock();
        let sql = format!("SELECT {} FROM scheduled_tasks WHERE id = ?1", SCHEDULED_TASK_COLUMNS);
        let task = crate::db::query::query_one(&conn, &sql, rusqlite::params![task_id], scheduled_task_from_row)?
            .ok_or_else(|| format!("定时任务 ID {} 不存在", task_id))?;
        (task.task_type, task.config_json)
    };

    let now = now_str();
    {
        let conn = state.db.lock();
        let _ = conn.execute(
            "UPDATE scheduled_tasks SET last_run_at = ?1, run_count = run_count + 1, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, task_id],
        );
    }

    // 根据任务类型执行
    match task_type.as_str() {
        "inspection" => {
            // 创建并执行巡检批次
            let config: serde_json::Value = serde_json::from_str(&config_json)
                .map_err(|e| format!("解析任务配置失败: {}", e))?;

            let device_ids: Vec<i64> = config.get("device_ids")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect())
                .unwrap_or_default();

            if device_ids.is_empty() {
                return Err("未指定设备".to_string());
            }

            let batch_id = {
                let conn = state.db.lock();
                let device_ids_json = serde_json::to_string(&device_ids).unwrap_or_default();
                conn.execute(
                    "INSERT INTO inspection_batches (name, status, triggered_by, device_ids, created_at, updated_at) \
                     VALUES (?1, 'pending', 'scheduled', ?2, ?3, ?3)",
                    rusqlite::params![format!("手动触发-{}", now), device_ids_json, now],
                ).map_err(|e| format!("创建批次失败: {}", e))?;
                conn.last_insert_rowid()
            };

            // 异步执行巡检
            let db = state.db.clone();
            tokio::spawn(async move {
                if let Err(e) = crate::commands::inspections::run_batch_internal(db, batch_id).await {
                    tracing::error!("手动触发巡检任务执行失败: {}", e);
                }
            });
        }
        "periodic_report" => {
            let config: serde_json::Value = serde_json::from_str(&config_json)
                .map_err(|e| format!("解析任务配置失败: {}", e))?;

            let report_type = config.get("report_type")
                .and_then(|v| v.as_str())
                .unwrap_or("monthly")
                .to_string();

            // 计算上一个周期
            let (period_start, period_end) = calculate_previous_period(&report_type);

            let db = state.db.clone();
            tokio::spawn(async move {
                if let Err(e) = crate::commands::periodic_reports::generate_periodic_report_internal(
                    &db,
                    report_type,
                    period_start,
                    period_end,
                    None,
                ).await {
                    tracing::error!("手动触发周期报告生成失败: {}", e);
                }
            });
        }
        _ => {
            warn!("未知任务类型: task_id={}, type={}", task_id, task_type);
            return Err(format!("未知任务类型: {}", task_type));
        }
    }

    info!("定时任务触发完成: task_id={}, type={}", task_id, task_type);
    Ok(())
}

// ============================================================
// 辅助函数
// ============================================================

/// 根据 cron 表达式计算下次执行时间
fn calculate_next_run_from_cron(cron_expr: &str) -> Option<String> {
    // 简化的 cron 解析：支持 "分 时 日 月 周" 格式
    // 这里只做基本的时间计算，复杂场景可以用 cron crate
    let parts: Vec<&str> = cron_expr.split_whitespace().collect();
    if parts.len() < 5 {
        warn!("Cron 表达式格式错误: {}", cron_expr);
        return None;
    }

    let now = chrono::Local::now();

    // 简化处理：假设每天执行
    let next = now + chrono::Duration::days(1);
    Some(next.format("%Y-%m-%d %H:%M:%S").to_string())
}

/// 计算上一个周期的时间范围
fn calculate_previous_period(report_type: &str) -> (String, String) {
    let now = chrono::Local::now().naive_local().date();
    match report_type {
        "weekly" => {
            let start = now - chrono::Duration::days(7);
            (start.format("%Y-%m-%d").to_string(), now.format("%Y-%m-%d").to_string())
        }
        "monthly" => {
            let start = now - chrono::Duration::days(30);
            (start.format("%Y-%m-%d").to_string(), now.format("%Y-%m-%d").to_string())
        }
        "quarterly" => {
            let start = now - chrono::Duration::days(90);
            (start.format("%Y-%m-%d").to_string(), now.format("%Y-%m-%d").to_string())
        }
        "yearly" => {
            let start = now - chrono::Duration::days(365);
            (start.format("%Y-%m-%d").to_string(), now.format("%Y-%m-%d").to_string())
        }
        _ => {
            let start = now - chrono::Duration::days(30);
            (start.format("%Y-%m-%d").to_string(), now.format("%Y-%m-%d").to_string())
        }
    }
}
