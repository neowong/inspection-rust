use tauri::State;
use crate::AppState;

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let max_lines: i64 = db.query_row("SELECT report_max_output_lines FROM system_settings WHERE id=1", [], |r| r.get(0)).unwrap_or(100);
    Ok(serde_json::json!({"report_max_output_lines": max_lines}))
}

#[tauri::command]
pub fn update_settings(report_max_output_lines: Option<i64>, state: State<AppState>) -> Result<serde_json::Value, String> {
    if let Some(lines) = report_max_output_lines {
        let lines = lines.max(1).min(10000);
        let db = state.db.lock();
        db.execute("UPDATE system_settings SET report_max_output_lines=?1 WHERE id=1", rusqlite::params![lines]).ok();
    }
    let result = get_settings(state);
    result
}

#[tauri::command]
pub fn get_report_info_fields(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let max_lines: i64 = db.query_row("SELECT report_max_output_lines FROM system_settings WHERE id=1", [], |r| r.get(0)).unwrap_or(100);
    drop(db);
    Ok(serde_json::json!({
        "report_max_output_lines": max_lines,
        "device_fields": {
            "交换机": ["设备名称","设备型号","IP地址","设备SN","出厂日期"],
            "服务器": ["设备名称","IP地址","OS版本","内核版本","CPU规格","内存规格"],
            "_default": ["设备名称","IP地址"],
        }
    }))
}

#[tauri::command]
pub fn update_report_info_fields(data: serde_json::Value, state: State<AppState>) -> Result<serde_json::Value, String> {
    if let Some(lines) = data.get("report_max_output_lines").and_then(|v| v.as_i64()) {
        let db = state.db.lock();
        db.execute("UPDATE system_settings SET report_max_output_lines=?1 WHERE id=1", rusqlite::params![lines.max(1).min(10000)]).ok();
        drop(db);
    }
    get_report_info_fields(state)
}
