use tauri::State;
use crate::AppState;
use crate::db::models::{InspectionTemplate, TemplateCreate, TemplateUpdate};

fn template_from_row(row: &rusqlite::Row) -> rusqlite::Result<InspectionTemplate> {
    Ok(InspectionTemplate {
        id: row.get(0)?, name: row.get(1)?, vendor: row.get(2)?, model: row.get(3)?,
        device_type: row.get(4)?, template_type: row.get(5)?, config: row.get(6)?,
        description: row.get(7)?, report_template_id: row.get(8)?,
        created_at: row.get(9)?, updated_at: row.get(10)?,
    })
}

#[tauri::command]
pub fn list_templates(vendor: Option<String>, state: State<AppState>) -> Result<Vec<serde_json::Value>, String> {
    let db = state.db.lock();
    let sql = if vendor.is_some() { "SELECT * FROM inspection_templates WHERE vendor = ?1 ORDER BY created_at DESC" }
              else { "SELECT * FROM inspection_templates ORDER BY created_at DESC" };
    let params: Vec<Box<dyn rusqlite::types::ToSql>> = vendor.map(|v| Box::new(v) as Box<dyn rusqlite::types::ToSql>).into_iter().collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = db.prepare(sql).map_err(|e| e.to_string())?;
    let templates: Vec<InspectionTemplate> = stmt.query_map(param_refs.as_slice(), template_from_row)
        .map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let mut result = Vec::new();
    for t in &templates {
        let device_count: i64 = db.query_row("SELECT COUNT(*) FROM devices WHERE template_id=?1", rusqlite::params![t.id], |r| r.get(0)).unwrap_or(0);
        let config_val: serde_json::Value = t.config.as_deref().and_then(|c| serde_json::from_str(c).ok()).unwrap_or(serde_json::json!({}));
        result.push(serde_json::json!({
            "id": t.id, "name": t.name, "vendor": t.vendor, "model": t.model, "device_type": t.device_type,
            "type": t.template_type, "config": config_val, "description": t.description,
            "report_template_id": t.report_template_id, "created_at": t.created_at, "updated_at": t.updated_at,
            "device_count": device_count,
        }));
    }
    Ok(result)
}

#[tauri::command]
pub fn get_template(template_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let t = db.query_row("SELECT * FROM inspection_templates WHERE id=?1", rusqlite::params![template_id], template_from_row)
        .map_err(|_| "模板不存在".to_string())?;
    let device_count: i64 = db.query_row("SELECT COUNT(*) FROM devices WHERE template_id=?1", rusqlite::params![template_id], |r| r.get(0)).unwrap_or(0);
    let config_val: serde_json::Value = t.config.as_deref().and_then(|c| serde_json::from_str(c).ok()).unwrap_or(serde_json::json!({}));
    Ok(serde_json::json!({
        "id": t.id, "name": t.name, "vendor": t.vendor, "model": t.model, "device_type": t.device_type,
        "type": t.template_type, "config": config_val, "description": t.description,
        "report_template_id": t.report_template_id, "created_at": t.created_at, "updated_at": t.updated_at,
        "device_count": device_count,
    }))
}

#[tauri::command]
pub fn create_template(data: TemplateCreate, state: State<AppState>) -> Result<serde_json::Value, String> {
    let id = {
        let db = state.db.lock();
        let exists: bool = db.query_row("SELECT COUNT(*) > 0 FROM inspection_templates WHERE name=?1", rusqlite::params![data.name], |r| r.get(0)).unwrap_or(false);
        if exists { return Err(format!("模板名称「{}」已存在", data.name)); }
        let config_str = data.config.as_ref().map(|c| c.to_string());
        let cmd_ids: Vec<i64> = data.config.as_ref().and_then(|c| c.get("command_ids")).and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_i64()).collect()).unwrap_or_default();
        if !cmd_ids.is_empty() {
            for cid in cmd_ids {
                let exists: bool = db.query_row("SELECT COUNT(*)>0 FROM command_pool WHERE id=?1", rusqlite::params![cid], |r| r.get(0)).unwrap_or(false);
                if !exists { return Err(format!("命令 ID {} 不存在", cid)); }
            }
        }
        db.execute(
            "INSERT INTO inspection_templates (name,vendor,model,device_type,template_type,config,description,report_template_id) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            rusqlite::params![data.name, data.vendor, data.model, data.device_type, data.template_type.unwrap_or_else(|| "ssh".into()), config_str, data.description, data.report_template_id],
        ).map_err(|e| format!("创建模板失败: {}", e))?;
        db.last_insert_rowid()
    };
    get_template(id, state)
}

#[tauri::command]
pub fn update_template(template_id: i64, data: TemplateUpdate, state: State<AppState>) -> Result<serde_json::Value, String> {
    {
        let db = state.db.lock();
        let t = db.query_row("SELECT * FROM inspection_templates WHERE id=?1", rusqlite::params![template_id], template_from_row)
            .map_err(|_| "模板不存在".to_string())?;
        if let Some(ref name) = data.name {
            let exists: bool = db.query_row("SELECT COUNT(*)>0 FROM inspection_templates WHERE name=?1 AND id!=?2", rusqlite::params![name, template_id], |r| r.get(0)).unwrap_or(false);
            if exists { return Err(format!("模板名称「{}」已存在", name)); }
        }
        let config_str = data.config.as_ref().map(|c| c.to_string()).or(t.config);
        db.execute(
            "UPDATE inspection_templates SET name=?1,vendor=?2,model=?3,device_type=?4,template_type=?5,config=?6,description=?7,report_template_id=?8,updated_at=datetime('now') WHERE id=?9",
            rusqlite::params![data.name.unwrap_or(t.name), data.vendor.unwrap_or(t.vendor), data.model.or(t.model), data.device_type.or(t.device_type), data.template_type.unwrap_or(t.template_type), config_str, data.description.or(t.description), data.report_template_id.or(t.report_template_id), template_id],
        ).map_err(|e| format!("更新模板失败: {}", e))?;
    }
    get_template(template_id, state)
}

#[tauri::command]
pub fn delete_template(template_id: i64, state: State<AppState>) -> Result<(), String> {
    let db = state.db.lock();
    let t = db.query_row("SELECT name FROM inspection_templates WHERE id=?1", rusqlite::params![template_id], |r| r.get::<_, String>(0))
        .map_err(|_| "模板不存在".to_string())?;
    let mut stmt = db.prepare("SELECT name FROM devices WHERE template_id=?1").map_err(|e| e.to_string())?;
    let device_names: Vec<String> = stmt.query_map(rusqlite::params![template_id], |r| r.get(0))
        .map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    if !device_names.is_empty() {
        return Err(format!("该模板被 {} 台设备引用（{}），请先解除关联", device_names.len(), device_names.join("、")));
    }
    db.execute("DELETE FROM inspection_templates WHERE id=?1", rusqlite::params![template_id]).map_err(|e| e.to_string())?;
    tracing::info!("删除模板: {}", t);
    Ok(())
}

#[tauri::command]
pub fn batch_delete_templates(ids: Vec<i64>, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let mut deleted = 0i64;
    let mut skipped: Vec<String> = Vec::new();
    for tid in ids {
        let tpl: Option<String> = db.query_row("SELECT name FROM inspection_templates WHERE id=?1", rusqlite::params![tid], |r| r.get(0)).ok();
        let Some(name) = tpl else { continue };
        let count: i64 = db.query_row("SELECT COUNT(*) FROM devices WHERE template_id=?1", rusqlite::params![tid], |r| r.get(0)).unwrap_or(0);
        if count > 0 {
            let mut stmt = db.prepare("SELECT name FROM devices WHERE template_id=?1").map_err(|e| e.to_string())?;
            let devs: Vec<String> = stmt.query_map(rusqlite::params![tid], |r| r.get(0)).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
            skipped.push(format!("{}({})", name, devs.join("、")));
        } else { db.execute("DELETE FROM inspection_templates WHERE id=?1", rusqlite::params![tid]).ok(); deleted += 1; }
    }
    Ok(serde_json::json!({"success": true, "deleted": deleted, "skipped": skipped}))
}

#[tauri::command]
pub fn auto_generate_template(vendor: String, model: Option<String>, device_type: Option<String>, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let mut stmt = db.prepare("SELECT id, command, description, category FROM command_pool WHERE vendor=?1 ORDER BY category").map_err(|e| e.to_string())?;
    let cmds: Vec<(i64, String, Option<String>, Option<String>)> = stmt.query_map(rusqlite::params![vendor], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    if cmds.is_empty() { return Ok(serde_json::json!({"error": "命令库中没有该厂商的命令，请先添加命令或配置 AI 后重试"})); }

    let ids: Vec<i64> = cmds.iter().map(|c| c.0).collect();
    let details: Vec<serde_json::Value> = cmds.iter().map(|c| serde_json::json!({"id": c.0, "command": c.1, "description": c.2, "category": c.3})).collect();
    let name = format!("{} {} {} 巡检模板", vendor, model.as_deref().unwrap_or(""), device_type.as_deref().unwrap_or(""));
    Ok(serde_json::json!({"suggested_name": name.trim(), "config": {"command_ids": ids}, "commands_detail": details, "source": "command_pool", "command_count": ids.len()}))
}

#[tauri::command]
pub fn generate_report_template(template_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let t = db.query_row("SELECT * FROM inspection_templates WHERE id=?1", rusqlite::params![template_id], template_from_row)
        .map_err(|_| "巡检模板不存在".to_string())?;
    let config: serde_json::Value = t.config.as_deref().and_then(|c| serde_json::from_str(c).ok()).unwrap_or(serde_json::json!({}));
    let cmd_ids: Vec<i64> = config.get("command_ids").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_i64()).collect()).unwrap_or_default();
    if cmd_ids.is_empty() { return Err("该巡检模板未包含任何命令".into()); }

    let rpt_name = format!("{}-报告模板", t.name);
    let file_path = format!("data/report_templates/{}.docx", rpt_name);
    db.execute("INSERT INTO report_templates (name, vendor, file_path) VALUES (?1,?2,?3)", rusqlite::params![rpt_name, t.vendor, file_path])
        .map_err(|e| e.to_string())?;
    let id = db.last_insert_rowid();
    Ok(serde_json::json!({"id": id, "name": rpt_name, "file_path": file_path}))
}
