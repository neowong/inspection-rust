use tauri::State;
use crate::AppState;
use crate::db::models::{ScheduledTask, TaskCreate, TaskUpdate};

fn task_from_row(row: &rusqlite::Row) -> rusqlite::Result<ScheduledTask> {
    Ok(ScheduledTask {
        id: row.get(0)?, name: row.get(1)?, cron_expression: row.get(2)?,
        enabled: row.get::<_, i64>(3)? != 0, device_ids: row.get(4)?,
        next_run_at: row.get(5)?, last_run_at: row.get(6)?,
        created_at: row.get(7)?, updated_at: row.get(8)?,
    })
}

#[tauri::command]
pub fn list_tasks(state: State<AppState>) -> Result<Vec<ScheduledTask>, String> {
    let db = state.db.lock();
    let mut stmt = db.prepare("SELECT * FROM scheduled_tasks ORDER BY created_at DESC").map_err(|e| e.to_string())?;
    let rows: Vec<ScheduledTask> = stmt.query_map([], task_from_row).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn create_task(data: TaskCreate, state: State<AppState>) -> Result<ScheduledTask, String> {
    let db = state.db.lock();
    let dev_ids_json = serde_json::to_string(&data.device_ids).unwrap_or_default();
    db.execute("INSERT INTO scheduled_tasks (name,cron_expression,enabled,device_ids) VALUES (?1,?2,1,?3)", rusqlite::params![data.name, data.cron_expression, dev_ids_json]).map_err(|e| e.to_string())?;
    let id = db.last_insert_rowid();
    db.query_row("SELECT * FROM scheduled_tasks WHERE id=?1", rusqlite::params![id], task_from_row).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_task(task_id: i64, state: State<AppState>) -> Result<ScheduledTask, String> {
    let db = state.db.lock();
    db.query_row("SELECT * FROM scheduled_tasks WHERE id=?1", rusqlite::params![task_id], task_from_row).map_err(|_| "定时任务不存在".into())
}

#[tauri::command]
pub fn update_task(task_id: i64, data: TaskUpdate, state: State<AppState>) -> Result<ScheduledTask, String> {
    let db = state.db.lock();
    let t = db.query_row("SELECT * FROM scheduled_tasks WHERE id=?1", rusqlite::params![task_id], task_from_row).map_err(|_| "定时任务不存在".to_string())?;
    let dev_ids = if let Some(ref ids) = data.device_ids { serde_json::to_string(ids).unwrap_or_else(|_| t.device_ids.clone()) } else { t.device_ids };
    db.execute("UPDATE scheduled_tasks SET name=?1,cron_expression=?2,device_ids=?3,updated_at=datetime('now') WHERE id=?4",
        rusqlite::params![data.name.unwrap_or(t.name), data.cron_expression.unwrap_or(t.cron_expression), dev_ids, task_id]).map_err(|e| e.to_string())?;
    db.query_row("SELECT * FROM scheduled_tasks WHERE id=?1", rusqlite::params![task_id], task_from_row).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_task(task_id: i64, state: State<AppState>) -> Result<(), String> {
    let db = state.db.lock();
    db.query_row("SELECT name FROM scheduled_tasks WHERE id=?1", rusqlite::params![task_id], |r| r.get::<_, String>(0)).map_err(|_| "定时任务不存在".to_string())?;
    let mut stmt = db.prepare("SELECT id FROM inspection_batches WHERE scheduled_task_id=?1").map_err(|e| e.to_string())?;
    let batch_ids: Vec<i64> = stmt.query_map(rusqlite::params![task_id], |r| r.get(0)).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    for bid in batch_ids { db.execute("DELETE FROM inspection_records WHERE batch_id=?1", rusqlite::params![bid]).ok(); }
    db.execute("DELETE FROM inspection_batches WHERE scheduled_task_id=?1", rusqlite::params![task_id]).ok();
    db.execute("DELETE FROM scheduled_tasks WHERE id=?1", rusqlite::params![task_id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn batch_delete_tasks(ids: Vec<i64>, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let mut deleted = 0i64;
    for tid in ids {
        let mut stmt = db.prepare("SELECT id FROM inspection_batches WHERE scheduled_task_id=?1").map_err(|e| e.to_string())?;
        let batch_ids: Vec<i64> = stmt.query_map(rusqlite::params![tid], |r| r.get(0)).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
        for bid in batch_ids {
            db.execute("DELETE FROM inspection_records WHERE batch_id=?1", rusqlite::params![bid]).ok();
            db.execute("DELETE FROM inspection_batches WHERE id=?1", rusqlite::params![bid]).ok();
        }
        db.execute("DELETE FROM scheduled_tasks WHERE id=?1", rusqlite::params![tid]).ok();
        deleted += 1;
    }
    Ok(serde_json::json!({"success": true, "deleted": deleted}))
}

#[tauri::command]
pub fn pause_task(task_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let exists: bool = db.query_row("SELECT COUNT(*)>0 FROM scheduled_tasks WHERE id=?1", rusqlite::params![task_id], |r| r.get(0)).unwrap_or(false);
    if !exists { return Err("定时任务不存在".into()); }
    db.execute("UPDATE scheduled_tasks SET enabled=0 WHERE id=?1", rusqlite::params![task_id]).ok();
    Ok(serde_json::json!({"message": "任务已暂停"}))
}

#[tauri::command]
pub fn resume_task(task_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let exists: bool = db.query_row("SELECT COUNT(*)>0 FROM scheduled_tasks WHERE id=?1", rusqlite::params![task_id], |r| r.get(0)).unwrap_or(false);
    if !exists { return Err("定时任务不存在".into()); }
    db.execute("UPDATE scheduled_tasks SET enabled=1 WHERE id=?1", rusqlite::params![task_id]).ok();
    Ok(serde_json::json!({"message": "任务已恢复"}))
}
