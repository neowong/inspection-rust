use tauri::State;
use crate::AppState;
use crate::db::models::{InspectionBatch, BatchCreate};

fn batch_from_row(row: &rusqlite::Row) -> rusqlite::Result<InspectionBatch> {
    Ok(InspectionBatch {
        id: row.get(0)?, name: row.get(1)?, mode: row.get(2)?, status: row.get(3)?,
        triggered_by: row.get(4)?, scheduled_task_id: row.get(5)?, device_ids: row.get(6)?,
        started_at: row.get(7)?, completed_at: row.get(8)?, created_at: row.get(9)?, updated_at: row.get(10)?,
    })
}

#[tauri::command]
pub fn list_batches(status: Option<String>, state: State<AppState>) -> Result<Vec<serde_json::Value>, String> {
    let db = state.db.lock();
    let sql = if status.is_some() { "SELECT * FROM inspection_batches WHERE status = ?1 ORDER BY created_at DESC LIMIT 50" }
              else { "SELECT * FROM inspection_batches ORDER BY created_at DESC LIMIT 50" };
    let params: Vec<Box<dyn rusqlite::types::ToSql>> = status.map(|s| Box::new(s) as Box<dyn rusqlite::types::ToSql>).into_iter().collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = db.prepare(sql).map_err(|e| e.to_string())?;
    let batches: Vec<InspectionBatch> = stmt.query_map(param_refs.as_slice(), batch_from_row)
        .map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let batch_ids: Vec<i64> = batches.iter().map(|b| b.id).collect();
    let mut all_records: Vec<(i64, i64, i64, String, String, Option<String>, Option<String>)> = Vec::new();
    if !batch_ids.is_empty() {
        let ph: Vec<String> = batch_ids.iter().map(|_| "?".into()).collect();
        let rec_sql = format!("SELECT id,batch_id,device_id,status,ai_status,report_path,error_message FROM inspection_records WHERE batch_id IN ({})", ph.join(","));
        let mut rec_stmt = db.prepare(&rec_sql).map_err(|e| e.to_string())?;
        let rp: Vec<&dyn rusqlite::types::ToSql> = batch_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        all_records = rec_stmt.query_map(rp.as_slice(), |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))
            .map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    }

    let mut result = Vec::new();
    for b in batches {
        let records: Vec<serde_json::Value> = all_records.iter().filter(|r| r.1 == b.id)
            .map(|r| serde_json::json!({"id": r.0, "batch_id": r.1, "device_id": r.2, "status": r.3, "ai_status": r.4, "report_path": r.5, "error_message": r.6})).collect();
        result.push(serde_json::json!({"id": b.id, "name": b.name, "mode": b.mode, "status": b.status, "triggered_by": b.triggered_by, "device_ids": b.device_ids, "started_at": b.started_at, "completed_at": b.completed_at, "created_at": b.created_at, "records": records}));
    }
    Ok(result)
}

#[tauri::command]
pub fn create_batch(data: BatchCreate, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let name = data.name.unwrap_or_else(|| chrono::Utc::now().format("巡检_%Y-%m-%d_%H:%M").to_string());
    let device_ids_json = serde_json::to_string(&data.device_ids).unwrap_or_default();
    let mode = data.mode.unwrap_or_else(|| "ssh".into());
    db.execute("INSERT INTO inspection_batches (name,mode,device_ids,scheduled_task_id) VALUES (?1,?2,?3,?4)", rusqlite::params![name, mode, device_ids_json, data.scheduled_task_id]).map_err(|e| e.to_string())?;
    let batch_id = db.last_insert_rowid();
    let mut offline_names: Vec<String> = Vec::new();
    if data.auto_start.unwrap_or(true) && !data.device_ids.is_empty() {
        let ph: Vec<String> = data.device_ids.iter().map(|_| "?".into()).collect();
        let dev_sql = format!("SELECT name FROM devices WHERE id IN ({}) AND status != 'online'", ph.join(","));
        let mut dev_stmt = db.prepare(&dev_sql).map_err(|e| e.to_string())?;
        let rp: Vec<&dyn rusqlite::types::ToSql> = data.device_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let m = dev_stmt.query_map(rp.as_slice(), |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
        offline_names = m.filter_map(|r| r.ok()).collect();
    }
    Ok(serde_json::json!({"success": true, "data": {"id": batch_id, "status": "pending"}, "offline_devices": offline_names}))
}

#[tauri::command]
pub fn get_batch(batch_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let b = db.query_row("SELECT * FROM inspection_batches WHERE id=?1", rusqlite::params![batch_id], batch_from_row).map_err(|_| "批次不存在".to_string())?;
    let mut rec_stmt = db.prepare("SELECT * FROM inspection_records WHERE batch_id=?1 ORDER BY id").map_err(|e| e.to_string())?;
    let records: Vec<serde_json::Value> = rec_stmt.query_map(rusqlite::params![batch_id], |r| Ok(serde_json::json!({
        "id": r.get::<_, i64>(0)?, "device_id": r.get::<_, i64>(2)?, "status": r.get::<_, String>(3)?,
        "error_message": r.get::<_, Option<String>>(5)?, "ai_status": r.get::<_, String>(7)?,
        "report_path": r.get::<_, Option<String>>(13)?, "command_outputs": r.get::<_, String>(6)?,
        "ai_result": r.get::<_, Option<String>>(8)?, "command_judgments": r.get::<_, Option<String>>(11)?,
        "summary_judgment": r.get::<_, Option<String>>(12)?, "upload_source": r.get::<_, String>(4)?,
    }))).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(serde_json::json!({"id": b.id, "name": b.name, "mode": b.mode, "status": b.status, "triggered_by": b.triggered_by, "device_ids": b.device_ids, "started_at": b.started_at, "completed_at": b.completed_at, "records": records}))
}

#[tauri::command]
pub fn run_batch(batch_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let b = db.query_row("SELECT * FROM inspection_batches WHERE id=?1", rusqlite::params![batch_id], batch_from_row).map_err(|_| "批次不存在".to_string())?;
    if b.status == "running" { return Ok(serde_json::json!({"success": false, "message": "批次已在运行中"})); }
    let device_ids: Vec<i64> = serde_json::from_str(&b.device_ids).unwrap_or_default();
    let offline: Vec<String> = if !device_ids.is_empty() {
        let ph: Vec<String> = device_ids.iter().map(|_| "?".into()).collect();
        let sql = format!("SELECT name FROM devices WHERE id IN ({}) AND status != 'online'", ph.join(","));
        let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
        let rp: Vec<&dyn rusqlite::types::ToSql> = device_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        { let m = stmt.query_map(rp.as_slice(), |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?; m.filter_map(|r| r.ok()).collect() }
    } else { Vec::new() };
    db.execute("UPDATE inspection_batches SET status='running', started_at=datetime('now') WHERE id=?1", rusqlite::params![batch_id]).ok();
    Ok(serde_json::json!({"success": true, "message": "巡检已开始", "batch_id": batch_id, "offline_devices": offline}))
}

#[tauri::command]
pub fn pause_batch(batch_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let status: String = db.query_row("SELECT status FROM inspection_batches WHERE id=?1", rusqlite::params![batch_id], |r| r.get(0)).map_err(|_| "批次不存在".to_string())?;
    if status != "running" { return Err("只有运行中的批次才能暂停".into()); }
    db.execute("UPDATE inspection_batches SET status='paused' WHERE id=?1", rusqlite::params![batch_id]).ok();
    Ok(serde_json::json!({"success": true, "status": "paused"}))
}

#[tauri::command]
pub fn stop_batch(batch_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let status: String = db.query_row("SELECT status FROM inspection_batches WHERE id=?1", rusqlite::params![batch_id], |r| r.get(0)).map_err(|_| "批次不存在".to_string())?;
    if status != "running" && status != "paused" { return Err("只有运行中或已暂停的批次才能停止".into()); }
    db.execute("UPDATE inspection_batches SET status='stopped', completed_at=datetime('now') WHERE id=?1", rusqlite::params![batch_id]).ok();
    db.execute("UPDATE inspection_records SET status='stopped' WHERE batch_id=?1 AND status IN ('pending','running')", rusqlite::params![batch_id]).ok();
    Ok(serde_json::json!({"success": true, "status": "stopped"}))
}

#[tauri::command]
pub fn restart_batch(batch_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let b = db.query_row("SELECT * FROM inspection_batches WHERE id=?1", rusqlite::params![batch_id], batch_from_row).map_err(|_| "批次不存在".to_string())?;
    if b.status == "running" { return Err("批次正在运行中".into()); }
    let device_ids: Vec<i64> = serde_json::from_str(&b.device_ids).unwrap_or_default();
    let offline: Vec<String> = if !device_ids.is_empty() {
        let ph: Vec<String> = device_ids.iter().map(|_| "?".into()).collect();
        let sql = format!("SELECT name FROM devices WHERE id IN ({}) AND status != 'online'", ph.join(","));
        let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
        let rp: Vec<&dyn rusqlite::types::ToSql> = device_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        { let m = stmt.query_map(rp.as_slice(), |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?; m.filter_map(|r| r.ok()).collect() }
    } else { Vec::new() };
    db.execute("DELETE FROM inspection_records WHERE batch_id=?1", rusqlite::params![batch_id]).ok();
    db.execute("UPDATE inspection_batches SET status='pending', started_at=NULL, completed_at=NULL WHERE id=?1", rusqlite::params![batch_id]).ok();
    Ok(serde_json::json!({"success": true, "message": "批次已重新启动", "offline_devices": offline}))
}

#[tauri::command]
pub fn retry_device(batch_id: i64, device_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let b = db.query_row("SELECT * FROM inspection_batches WHERE id=?1", rusqlite::params![batch_id], batch_from_row).map_err(|_| "批次不存在".to_string())?;
    let dev_ids: Vec<i64> = serde_json::from_str(&b.device_ids).unwrap_or_default();
    if !dev_ids.contains(&device_id) { return Err("该设备不属于此批次".into()); }
    db.execute("DELETE FROM inspection_records WHERE batch_id=?1 AND device_id=?2", rusqlite::params![batch_id, device_id]).ok();
    Ok(serde_json::json!({"success": true, "message": "单设备重试已启动", "batch_id": batch_id, "device_id": device_id}))
}

#[tauri::command]
pub fn delete_batch(batch_id: i64, permanent: Option<bool>, state: State<AppState>) -> Result<(), String> {
    let db = state.db.lock();
    let b = db.query_row("SELECT * FROM inspection_batches WHERE id=?1", rusqlite::params![batch_id], batch_from_row).map_err(|_| "批次不存在".to_string())?;
    if permanent.unwrap_or(false) || matches!(b.status.as_str(), "pending" | "running" | "paused") {
        db.execute("DELETE FROM inspection_records WHERE batch_id=?1", rusqlite::params![batch_id]).ok();
        db.execute("DELETE FROM inspection_batches WHERE id=?1", rusqlite::params![batch_id]).ok();
    } else {
        db.execute("UPDATE inspection_batches SET status='archived' WHERE id=?1", rusqlite::params![batch_id]).ok();
    }
    Ok(())
}

#[tauri::command]
pub fn batch_delete_batches(ids: Vec<i64>, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let mut deleted = 0;
    for batch_id in ids {
        let status: Option<String> = db.query_row("SELECT status FROM inspection_batches WHERE id=?1", rusqlite::params![batch_id], |r| r.get(0)).ok();
        if let Some(s) = status {
            if matches!(s.as_str(), "pending" | "running" | "paused") {
                db.execute("DELETE FROM inspection_records WHERE batch_id=?1", rusqlite::params![batch_id]).ok();
                db.execute("DELETE FROM inspection_batches WHERE id=?1", rusqlite::params![batch_id]).ok();
            } else { db.execute("UPDATE inspection_batches SET status='archived' WHERE id=?1", rusqlite::params![batch_id]).ok(); }
            deleted += 1;
        }
    }
    Ok(serde_json::json!({"success": true, "deleted": deleted}))
}
