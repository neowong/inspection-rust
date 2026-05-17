use tauri::State;
use crate::AppState;
use crate::db::models::ReportTemplate;

fn rt_from_row(row: &rusqlite::Row) -> rusqlite::Result<ReportTemplate> {
    Ok(ReportTemplate {
        id: row.get(0)?, name: row.get(1)?, vendor: row.get(2)?,
        file_path: row.get(3)?, created_at: row.get(4)?, updated_at: row.get(5)?,
    })
}

#[tauri::command]
pub fn list_report_templates(state: State<AppState>) -> Result<Vec<ReportTemplate>, String> {
    let db = state.db.lock();
    let mut stmt = db.prepare("SELECT * FROM report_templates ORDER BY created_at DESC").map_err(|e| e.to_string())?;
    let rows: Vec<ReportTemplate> = stmt.query_map([], rt_from_row).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn upload_template(file_path: String, name: String, vendor: Option<String>, state: State<AppState>) -> Result<ReportTemplate, String> {
    let db = state.db.lock();
    db.execute(
        "INSERT INTO report_templates (name, vendor, file_path) VALUES (?1,?2,?3)",
        rusqlite::params![name, vendor, file_path],
    ).map_err(|e| e.to_string())?;
    let id = db.last_insert_rowid();
    db.query_row("SELECT * FROM report_templates WHERE id=?1", rusqlite::params![id], rt_from_row).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn download_template(template_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let path: String = db.query_row("SELECT file_path FROM report_templates WHERE id=?1", rusqlite::params![template_id], |r| r.get(0))
        .map_err(|_| "模板不存在".to_string())?;
    if !std::path::Path::new(&path).exists() { return Err("模板文件不存在".into()); }
    Ok(serde_json::json!({"success": true, "path": path}))
}

#[tauri::command]
pub fn preview_template(template_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let (name, path): (String, String) = db.query_row(
        "SELECT name, file_path FROM report_templates WHERE id=?1", rusqlite::params![template_id], |r| Ok((r.get(0)?, r.get(1)?))
    ).map_err(|_| "模板不存在".to_string())?;
    if !std::path::Path::new(&path).exists() { return Err("模板文件不存在".into()); }
    Ok(serde_json::json!({"name": name, "path": path}))
}

#[tauri::command]
pub fn delete_report_template(template_id: i64, state: State<AppState>) -> Result<(), String> {
    let db = state.db.lock();
    let path: Option<String> = db.query_row("SELECT file_path FROM report_templates WHERE id=?1", rusqlite::params![template_id], |r| r.get(0)).ok();
    drop(db);
    if let Some(ref p) = path { std::fs::remove_file(p).ok(); }
    let db2 = state.db.lock();
    db2.execute("DELETE FROM report_templates WHERE id=?1", rusqlite::params![template_id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn batch_delete_report_templates(ids: Vec<i64>, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let mut deleted = 0i64;
    for tid in ids {
        let path: Option<String> = db.query_row("SELECT file_path FROM report_templates WHERE id=?1", rusqlite::params![tid], |r| r.get(0)).ok();
        if let Some(ref p) = path { std::fs::remove_file(p).ok(); }
        db.execute("DELETE FROM report_templates WHERE id=?1", rusqlite::params![tid]).ok();
        deleted += 1;
    }
    Ok(serde_json::json!({"success": true, "deleted": deleted}))
}
