use tauri::State;

use crate::AppState;
use crate::db::models::SystemSettings;

/// 获取系统设置
#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<SystemSettings, String> {
    let conn = state.db.lock();

    crate::db::query::query_one(
        &conn,
        "SELECT id, report_max_output_lines FROM system_settings WHERE id = 1",
        &[],
        |row| {
            Ok(SystemSettings {
                id: row.get(0)?,
                report_max_output_lines: row.get(1)?,
            })
        },
    )?
    .ok_or_else(|| "系统设置未初始化".to_string())
}

/// 更新系统设置
#[tauri::command]
pub fn update_settings(report_max_output_lines: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock();

    conn.execute(
        "UPDATE system_settings SET report_max_output_lines = ?1 WHERE id = 1",
        rusqlite::params![report_max_output_lines],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}
