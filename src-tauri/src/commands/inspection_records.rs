use tauri::State;
use crate::AppState;

#[tauri::command]
pub fn delete_record(record_id: i64, state: State<AppState>) -> Result<(), String> {
    let db = state.db.lock();
    let report_path: Option<String> = db.query_row("SELECT report_path FROM inspection_records WHERE id=?1", rusqlite::params![record_id], |r| r.get(0)).ok().flatten();
    if let Some(ref path) = report_path { std::fs::remove_file(path).ok(); }
    db.execute("DELETE FROM inspection_records WHERE id=?1", rusqlite::params![record_id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn batch_delete_records(ids: Vec<i64>, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    for rid in &ids {
        let report_path: Option<String> = db.query_row("SELECT report_path FROM inspection_records WHERE id=?1", rusqlite::params![rid], |r| r.get(0)).ok().flatten();
        if let Some(ref path) = report_path { std::fs::remove_file(path).ok(); }
        db.execute("DELETE FROM inspection_records WHERE id=?1", rusqlite::params![rid]).ok();
    }
    Ok(serde_json::json!({"success": true, "deleted": ids.len()}))
}

#[tauri::command]
pub fn analyze_record(record_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let exists: bool = db.query_row("SELECT COUNT(*)>0 FROM inspection_records WHERE id=?1", rusqlite::params![record_id], |r| r.get(0)).unwrap_or(false);
    if !exists { return Ok(serde_json::json!({"success": false, "message": "记录不存在"})); }
    db.execute("UPDATE inspection_records SET ai_status='processing' WHERE id=?1", rusqlite::params![record_id]).ok();
    Ok(serde_json::json!({"success": true, "message": "AI分析已启动", "record_id": record_id}))
}

#[tauri::command]
pub fn analyze_batch(batch_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let mut stmt = db.prepare("SELECT id FROM inspection_records WHERE batch_id=?1 AND status='completed' AND ai_status IN ('pending','failed')").map_err(|e| e.to_string())?;
    let record_ids: Vec<i64> = stmt.query_map(rusqlite::params![batch_id], |r| r.get(0)).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    if record_ids.is_empty() { return Ok(serde_json::json!({"success": false, "message": "没有待分析或分析失败的巡检记录"})); }
    for rid in &record_ids { db.execute("UPDATE inspection_records SET ai_status='processing' WHERE id=?1", rusqlite::params![rid]).ok(); }
    Ok(serde_json::json!({"success": true, "message": format!("已启动 {} 条记录的AI分析", record_ids.len()), "count": record_ids.len()}))
}

#[tauri::command]
pub fn generate_report(record_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let exists: bool = db.query_row("SELECT COUNT(*)>0 FROM inspection_records WHERE id=?1", rusqlite::params![record_id], |r| r.get(0)).unwrap_or(false);
    if !exists { return Ok(serde_json::json!({"success": false, "message": "记录不存在"})); }
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let path = format!("data/reports/batch/record{}_{}.docx", record_id, ts);
    db.execute("UPDATE inspection_records SET report_path=?1 WHERE id=?2", rusqlite::params![path, record_id]).ok();
    Ok(serde_json::json!({"success": true, "report_path": path}))
}

#[tauri::command]
pub fn generate_batch_reports(batch_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let mut stmt = db.prepare("SELECT id FROM inspection_records WHERE batch_id=?1 AND ai_status='completed'").map_err(|e| e.to_string())?;
    let record_ids: Vec<i64> = { let mapped = stmt.query_map(rusqlite::params![batch_id], |r| r.get(0)).map_err(|e| e.to_string())?; mapped.filter_map(|r| r.ok()).collect() };
    let ids = if record_ids.is_empty() {
        let mut stmt2 = db.prepare("SELECT id FROM inspection_records WHERE batch_id=?1").map_err(|e| e.to_string())?;
        let mapped = stmt2.query_map(rusqlite::params![batch_id], |r| r.get(0)).map_err(|e| e.to_string())?;
        mapped.filter_map(|r| r.ok()).collect()
    } else { record_ids };
    if ids.is_empty() { return Ok(serde_json::json!({"success": false, "message": "没有可生成报告的记录"})); }

    let batch_dir = format!("data/reports/batch{}", batch_id);
    std::fs::create_dir_all(&batch_dir).ok();
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let merged_path = format!("{}/batch{}_综合巡检报告_{}.docx", batch_dir, batch_id, ts);
    for rid in &ids {
        let path = format!("{}/record{}_{}.docx", batch_dir, rid, ts);
        db.execute("UPDATE inspection_records SET report_path=?1 WHERE id=?2", rusqlite::params![path, rid]).ok();
    }
    Ok(serde_json::json!({"success": true, "count": ids.len(), "merged_path": merged_path}))
}

#[tauri::command]
pub fn download_report(record_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let path: Option<String> = db.query_row("SELECT report_path FROM inspection_records WHERE id=?1", rusqlite::params![record_id], |r| r.get(0)).ok().flatten();
    match path {
        Some(ref p) if std::path::Path::new(p).exists() => Ok(serde_json::json!({"success": true, "path": p})),
        _ => Ok(serde_json::json!({"success": false, "message": "报告不存在"})),
    }
}

#[tauri::command]
pub fn download_batch_report(batch_id: i64, _state: State<AppState>) -> Result<serde_json::Value, String> {
    let batch_dir = format!("data/reports/batch{}", batch_id);
    let dir = std::path::Path::new(&batch_dir);
    if !dir.is_dir() { return Ok(serde_json::json!({"success": false, "message": "报告不存在"})); }
    let mut files: Vec<_> = std::fs::read_dir(dir).map_err(|e| e.to_string())?
        .filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().contains("综合巡检报告") && e.file_name().to_string_lossy().ends_with(".docx")).collect();
    files.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    files.reverse();
    if let Some(f) = files.first() { Ok(serde_json::json!({"success": true, "path": f.path().to_string_lossy().to_string()})) }
    else { Ok(serde_json::json!({"success": false, "message": "综合报告不存在，请先生成"})) }
}

#[tauri::command]
pub fn preview_template_context(record_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let record: Option<(String, Option<String>)> = db.query_row("SELECT command_outputs, summary_judgment FROM inspection_records WHERE id=?1", rusqlite::params![record_id], |r| Ok((r.get(0)?, r.get(1)?))).ok();
    let Some((outputs, summary)) = record else { return Ok(serde_json::json!({"success": false, "message": "记录不存在"})); };
    Ok(serde_json::json!({"success": true, "record_id": record_id, "context": {"command_outputs": outputs, "summary_judgment": summary.unwrap_or_default()}}))
}

#[tauri::command]
pub fn get_active_ai_config(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let cfg: Option<(String, String, String, Option<String>)> = db.query_row("SELECT name, provider, model_id, base_url FROM ai_model_configs WHERE is_active=1", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).ok();
    match cfg {
        Some((name, provider, model_id, base_url)) => Ok(serde_json::json!({"active": true, "name": name, "provider": provider, "model_id": model_id, "base_url": base_url})),
        None => Ok(serde_json::json!({"active": false, "provider": null, "model_id": null})),
    }
}
