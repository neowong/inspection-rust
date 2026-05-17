use tauri::State;
use crate::AppState;
use crate::db::models::{CommandPool, CommandCreate, CommandUpdate};
use rusqlite::types::ToSql;

fn cmd_from_row(row: &rusqlite::Row) -> rusqlite::Result<CommandPool> {
    Ok(CommandPool {
        id: row.get(0)?, vendor: row.get(1)?, command: row.get(2)?, description: row.get(3)?,
        category: row.get(4)?, command_type: row.get(5)?, model: row.get(6)?,
        created_at: row.get(7)?, updated_at: row.get(8)?,
    })
}

const ALLOWED_VENDORS: &[&str] = &["H3C", "华为", "思科", "深信服", "锐捷", "Linux", "CentOS", "Ubuntu", "openEuler", "MySQL", "PostgreSQL", "Oracle", "其它"];

#[tauri::command]
pub fn list_vendors() -> Result<Vec<String>, String> {
    Ok(ALLOWED_VENDORS.iter().map(|s| s.to_string()).collect())
}

#[tauri::command]
pub fn list_commands(vendor: Option<String>, state: State<AppState>) -> Result<Vec<CommandPool>, String> {
    let db = state.db.lock();
    let (sql, params): (String, Vec<Box<dyn ToSql>>) = if let Some(ref v) = vendor {
        ("SELECT * FROM command_pool WHERE vendor = ?1 ORDER BY vendor, category".into(),
         vec![Box::new(v.clone())])
    } else {
        ("SELECT * FROM command_pool ORDER BY vendor, category".into(), vec![])
    };

    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows: Vec<CommandPool> = stmt.query_map(param_refs.as_slice(), cmd_from_row)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[tauri::command]
pub fn get_command(command_id: i64, state: State<AppState>) -> Result<CommandPool, String> {
    let db = state.db.lock();
    db.query_row("SELECT * FROM command_pool WHERE id=?1", rusqlite::params![command_id], cmd_from_row)
        .map_err(|_| "命令不存在".into())
}

#[tauri::command]
pub fn create_command(data: CommandCreate, state: State<AppState>) -> Result<CommandPool, String> {
    let db = state.db.lock();
    let cmd = data.command.trim().to_string();
    let exists: bool = db.query_row(
        "SELECT COUNT(*) > 0 FROM command_pool WHERE vendor=?1 AND command=?2",
        rusqlite::params![data.vendor, cmd], |r| r.get(0)
    ).unwrap_or(false);
    if exists { return Err("已存在相同厂商和命令的记录".into()); }

    let cat = data.category.clone().unwrap_or_else(|| "general".into());
    let ctype = data.command_type.clone().unwrap_or_else(|| "ssh".into());

    db.execute(
        "INSERT INTO command_pool (vendor, command, description, category, command_type, model)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![data.vendor, cmd, data.description, cat, ctype, data.model],
    ).map_err(|e| e.to_string())?;

    let id = db.last_insert_rowid();
    db.query_row("SELECT * FROM command_pool WHERE id=?1", rusqlite::params![id], cmd_from_row)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_command(command_id: i64, data: CommandUpdate, state: State<AppState>) -> Result<CommandPool, String> {
    let db = state.db.lock();
    let c = db.query_row("SELECT * FROM command_pool WHERE id=?1", rusqlite::params![command_id], cmd_from_row)
        .map_err(|_| "命令不存在".to_string())?;

    let has_vendor = data.vendor.is_some();
    let has_command = data.command.is_some();
    let vendor = data.vendor.unwrap_or(c.vendor);
    let command = data.command.map(|s| s.trim().to_string()).unwrap_or(c.command);
    let description = data.description.or(c.description);
    let category = data.category.or(c.category);
    let command_type = data.command_type.unwrap_or(c.command_type);
    let model = data.model.or(c.model);

    if has_vendor || has_command {
        let exists: bool = db.query_row(
            "SELECT COUNT(*) > 0 FROM command_pool WHERE vendor=?1 AND command=?2 AND id!=?3",
            rusqlite::params![vendor, command, command_id], |r| r.get(0)
        ).unwrap_or(false);
        if exists { return Err("已存在相同厂商和命令的记录".into()); }
    }

    db.execute(
        "UPDATE command_pool SET vendor=?1, command=?2, description=?3, category=?4, command_type=?5, model=?6, updated_at=datetime('now') WHERE id=?7",
        rusqlite::params![vendor, command, description, category, command_type, model, command_id],
    ).map_err(|e| e.to_string())?;

    db.query_row("SELECT * FROM command_pool WHERE id=?1", rusqlite::params![command_id], cmd_from_row)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_command(command_id: i64, state: State<AppState>) -> Result<(), String> {
    let db = state.db.lock();
    let c = db.query_row("SELECT command FROM command_pool WHERE id=?1", rusqlite::params![command_id], |r| r.get::<_, String>(0))
        .map_err(|_| "命令不存在".to_string())?;

    let mut templates_stmt = db.prepare("SELECT name, config FROM inspection_templates").map_err(|e| e.to_string())?;
    let templates: Vec<(String, String)> = templates_stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default())))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut referencing = Vec::new();
    for (tpl_name, config) in &templates {
        if let Ok(cfg_val) = serde_json::from_str::<serde_json::Value>(config) {
            if let Some(ids) = cfg_val.get("command_ids").and_then(|v| v.as_array()) {
                if ids.iter().any(|v| v.as_i64() == Some(command_id)) {
                    referencing.push(tpl_name.clone());
                }
            }
        }
    }

    if !referencing.is_empty() {
        return Err(format!("该命令被模板「{}」引用，请先移除关联", referencing.join("、")));
    }

    db.execute("DELETE FROM command_pool WHERE id=?1", rusqlite::params![command_id]).map_err(|e| e.to_string())?;
    tracing::info!("删除命令: {}", c);
    Ok(())
}

#[tauri::command]
pub fn batch_delete_commands(ids: Vec<i64>, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let mut all_stmt = db.prepare("SELECT name, config FROM inspection_templates").map_err(|e| e.to_string())?;
    let all_templates: Vec<(String, String)> = all_stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default())))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut deleted = 0i64;
    let mut skipped: Vec<String> = Vec::new();
    for cid in ids {
        let cmd: Option<String> = db.query_row("SELECT command FROM command_pool WHERE id=?1", rusqlite::params![cid], |r| r.get(0)).ok();
        let Some(cmd_text) = cmd else { continue };
        let referenced = all_templates.iter().find(|(_name, config)| {
            if let Ok(cfg_val) = serde_json::from_str::<serde_json::Value>(config) {
                cfg_val.get("command_ids").and_then(|v| v.as_array())
                    .map(|a| a.iter().any(|v| v.as_i64() == Some(cid))).unwrap_or(false)
            } else { false }
        });
        if let Some((tpl_name, _)) = referenced {
            skipped.push(format!("{}({})", cmd_text, tpl_name));
        } else {
            db.execute("DELETE FROM command_pool WHERE id=?1", rusqlite::params![cid]).ok();
            deleted += 1;
        }
    }
    Ok(serde_json::json!({"success": true, "deleted": deleted, "skipped": skipped}))
}
