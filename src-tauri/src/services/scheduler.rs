use std::sync::Arc;
use parking_lot::Mutex;
use rusqlite::Connection;
use chrono::Local;
use tracing::{info, debug};

/// 定时任务调度器
/// 每分钟检查一次 due 的任务，执行对应的巡检或报告生成
pub fn start_scheduler(db: Arc<Mutex<Connection>>) {
    std::thread::spawn(move || {
        tracing::info!("定时任务调度器已启动");
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
            if let Err(e) = check_and_run_due_tasks(&db) {
                tracing::error!("定时任务调度器出错: {}", e);
            }
        }
    });
}

/// 检查并执行到期的任务
fn check_and_run_due_tasks(db: &Arc<Mutex<Connection>>) -> Result<(), String> {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // 查找所有启用且到期的任务
    let due_tasks: Vec<(i64, String, String, String)> = {
        let conn = db.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, task_type, config_json FROM scheduled_tasks \
             WHERE enabled = 1 AND (next_run_at IS NULL OR next_run_at <= ?1)"
        ).map_err(|e| format!("准备查询失败: {}", e))?;

        let rows = stmt.query_map(rusqlite::params![now], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        }).map_err(|e| format!("查询到期任务失败: {}", e))?;

        rows.filter_map(|r| r.ok()).collect()
    };

    if due_tasks.is_empty() {
        debug!("无到期任务，跳过");
        return Ok(());
    }
    info!("发现 {} 个到期任务", due_tasks.len());

    for (task_id, task_name, task_type, config_json) in due_tasks {
        tracing::info!("执行定时任务: {} (ID={}, type={})", task_name, task_id, task_type);

        // 更新 last_run_at 和 run_count
        {
            let conn = db.lock();
            let _ = conn.execute(
                "UPDATE scheduled_tasks SET last_run_at = ?1, run_count = run_count + 1, \
                 updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, task_id],
            );
        }

        // 根据任务类型执行
        let result = match task_type.as_str() {
            "inspection" => run_inspection_task(db, &config_json),
            "periodic_report" => run_periodic_report_task(db, &config_json),
            _ => Err(format!("未知任务类型: {}", task_type)),
        };

        // 更新下次执行时间和错误信息
        let next_run = calculate_next_run(&task_type);
        let (error_msg, next_run_at) = match result {
            Ok(_) => (None, next_run),
            Err(e) => {
                tracing::error!("定时任务 {} 执行失败: {}", task_name, e);
                (Some(e), next_run)
            }
        };

        {
            let conn = db.lock();
            let _ = conn.execute(
                "UPDATE scheduled_tasks SET next_run_at = ?1, last_error = ?2, updated_at = ?3 \
                 WHERE id = ?4",
                rusqlite::params![next_run_at, error_msg, now, task_id],
            );
        }
    }

    Ok(())
}

/// 执行巡检任务
fn run_inspection_task(db: &Arc<Mutex<Connection>>, config_json: &str) -> Result<(), String> {
    let config: serde_json::Value = serde_json::from_str(config_json)
        .map_err(|e| format!("解析任务配置失败: {}", e))?;

    let device_ids: Vec<i64> = config.get("device_ids")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect())
        .unwrap_or_default();

    if device_ids.is_empty() {
        return Err("未指定设备".to_string());
    }
    info!("执行定时巡检: device_count={}", device_ids.len());

    // 创建巡检批次
    let batch_id = {
        let conn = db.lock();
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let device_ids_json = serde_json::to_string(&device_ids).unwrap_or_default();

        conn.execute(
            "INSERT INTO inspection_batches (name, status, triggered_by, device_ids, created_at, updated_at) \
             VALUES (?1, 'pending', 'scheduled', ?2, ?3, ?3)",
            rusqlite::params![format!("定时任务-{}", now), device_ids_json, now],
        ).map_err(|e| format!("创建批次失败: {}", e))?;

        conn.last_insert_rowid()
    };
    info!("巡检批次已创建: batch_id={}", batch_id);

    // 异步执行巡检（在后台线程中）
    let db_clone = db.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Err(e) = crate::commands::inspections::run_batch_internal(db_clone, batch_id).await {
                tracing::error!("定时巡检任务执行失败: {}", e);
            }
        });
    });

    Ok(())
}

/// 执行周期报告生成任务
fn run_periodic_report_task(db: &Arc<Mutex<Connection>>, config_json: &str) -> Result<(), String> {
    let config: serde_json::Value = serde_json::from_str(config_json)
        .map_err(|e| format!("解析任务配置失败: {}", e))?;

    let report_type = config.get("report_type")
        .and_then(|v| v.as_str())
        .unwrap_or("monthly")
        .to_string();

    // 计算上一个周期的时间范围
    let (period_start, period_end) = calculate_previous_period(&report_type);
    info!("执行周期报告任务: type={}, period={}~{}", report_type, period_start, period_end);

    // 调用周期报告生成
    let db_clone = db.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Err(e) = crate::commands::periodic_reports::generate_periodic_report_internal(
                &db_clone,
                report_type,
                period_start,
                period_end,
                None,
            ).await {
                tracing::error!("定时周期报告生成失败: {}", e);
            }
        });
    });

    Ok(())
}

/// 计算下一个执行时间（简化版，基于任务类型）
fn calculate_next_run(task_type: &str) -> Option<String> {
    let now = Local::now();
    let next = match task_type {
        "inspection" => now + chrono::Duration::days(1),
        "periodic_report" => now + chrono::Duration::days(1),
        _ => return None,
    };
    Some(next.format("%Y-%m-%d %H:%M:%S").to_string())
}

/// 计算上一个周期的时间范围
fn calculate_previous_period(report_type: &str) -> (String, String) {
    let now = Local::now();
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
