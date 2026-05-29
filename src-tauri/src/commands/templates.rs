use tauri::State;
use rusqlite::types::ToSql;

use crate::AppState;
use crate::db::models::{
    CommandCreate, CommandPool, CommandUpdate, InspectionTemplate, TemplateCreate, TemplateUpdate,
};
use crate::services::template_generator;

// ============================================================
// Constants
// ============================================================

const TEMPLATE_COLUMNS: &str =
    "id, name, vendor, model, device_type, config, description, report_template_id, template_type, created_at, updated_at";
const COMMAND_COLUMNS: &str = "id, vendor, command, description, category, model, created_at, updated_at";

// ============================================================
// Helpers
// ============================================================

fn template_from_row(row: &rusqlite::Row) -> rusqlite::Result<InspectionTemplate> {
    Ok(InspectionTemplate {
        id: row.get(0)?,
        name: row.get(1)?,
        vendor: row.get(2)?,
        model: row.get(3)?,
        device_type: row.get(4)?,
        config: row.get(5)?,
        description: row.get(6)?,
        report_template_id: row.get(7)?,
        template_type: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn command_from_row(row: &rusqlite::Row) -> rusqlite::Result<CommandPool> {
    Ok(CommandPool {
        id: row.get(0)?,
        vendor: row.get(1)?,
        command: row.get(2)?,
        description: row.get(3)?,
        category: row.get(4)?,
        model: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

// ============================================================
// Template Query Commands
// ============================================================

/// 获取巡检模板列表，支持按厂商筛选，包含关联的设备数量
#[tauri::command]
pub fn list_templates(
    vendor: Option<String>,
    state: State<AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = state.db.lock();

    let mut sql = String::from(
        "SELECT t.id, t.name, t.vendor, t.model, t.device_type, t.config, t.description, \
         t.report_template_id, t.created_at, t.updated_at, COUNT(d.id) as device_count \
         FROM inspection_templates t \
         LEFT JOIN devices d ON d.template_id = t.id",
    );
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(ref v) = vendor {
        sql.push_str(" WHERE t.vendor = ?");
        params.push(Box::new(v.clone()));
    }

    sql.push_str(" GROUP BY t.id ORDER BY t.created_at DESC");

    let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
            let id: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let vendor: String = row.get(2)?;
            let model: Option<String> = row.get(3)?;
            let device_type: Option<String> = row.get(4)?;
            let config: Option<String> = row.get(5)?;
            let description: Option<String> = row.get(6)?;
            let report_template_id: Option<i64> = row.get(7)?;
            let created_at: String = row.get(8)?;
            let updated_at: String = row.get(9)?;
            let device_count: i64 = row.get(10)?;
            let parsed_config: Option<serde_json::Value> = config
                .as_deref()
                .and_then(|c| serde_json::from_str(c).ok());
            Ok(serde_json::json!({
                "id": id,
                "name": name,
                "vendor": vendor,
                "model": model,
                "device_type": device_type,
                "config": parsed_config,
                "description": description,
                "report_template_id": report_template_id,
                "created_at": created_at,
                "updated_at": updated_at,
                "device_count": device_count,
            }))
        })
        .map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row.map_err(|e| e.to_string())?);
    }
    Ok(results)
}

/// 获取单个巡检模板详情
#[tauri::command]
pub fn get_template(
    template_id: i64,
    state: State<AppState>,
) -> Result<InspectionTemplate, String> {
    let conn = state.db.lock();

    let sql = format!("SELECT {} FROM inspection_templates WHERE id = ?1", TEMPLATE_COLUMNS);
    crate::db::query::query_one(&conn, &sql, rusqlite::params![template_id], template_from_row)?
        .ok_or_else(|| format!("巡检模板 ID {} 不存在", template_id))
}

/// 创建巡检模板
#[tauri::command]
pub fn create_template(
    data: TemplateCreate,
    state: State<AppState>,
) -> Result<InspectionTemplate, String> {
    let conn = state.db.lock();

    let template_type = data.template_type.as_deref().unwrap_or("ssh");

    conn.execute(
        "INSERT INTO inspection_templates (name, vendor, model, device_type, config, description, \
         report_template_id, template_type) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            data.name,
            data.vendor,
            data.model,
            data.device_type,
            data.config,
            data.description,
            data.report_template_id,
            template_type,
        ],
    )
    .map_err(|e| e.to_string())?;

    let last_id = conn.last_insert_rowid();

    let sql = format!("SELECT {} FROM inspection_templates WHERE id = ?1", TEMPLATE_COLUMNS);
    crate::db::query::query_one(&conn, &sql, rusqlite::params![last_id], template_from_row)?
        .ok_or_else(|| "创建巡检模板后查询失败".to_string())
}

/// 更新巡检模板信息（动态字段，仅提供需更新的字段）
#[tauri::command]
pub fn update_template(
    template_id: i64,
    data: TemplateUpdate,
    state: State<AppState>,
) -> Result<InspectionTemplate, String> {
    let conn = state.db.lock();

    // 验证模板存在
    let sql = format!("SELECT {} FROM inspection_templates WHERE id = ?1", TEMPLATE_COLUMNS);
    let existing = crate::db::query::query_one(
        &conn,
        &sql,
        rusqlite::params![template_id],
        template_from_row,
    )?
    .ok_or_else(|| format!("巡检模板 ID {} 不存在", template_id))?;

    // 构建动态 UPDATE
    let mut set_parts: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    let mut idx = 1i32;

    macro_rules! push_field {
        ($field:ident, $col:expr) => {
            if let Some(ref val) = data.$field {
                set_parts.push(format!("{} = ?{}", $col, idx));
                params.push(Box::new(val.clone()));
                idx += 1;
            }
        };
    }

    push_field!(name, "name");
    push_field!(vendor, "vendor");
    push_field!(model, "model");
    push_field!(device_type, "device_type");
    push_field!(config, "config");
    push_field!(description, "description");
    push_field!(report_template_id, "report_template_id");
    push_field!(template_type, "template_type");

    if set_parts.is_empty() {
        return Ok(existing);
    }

    set_parts.push("updated_at = datetime('now')".to_string());

    let update_sql = format!(
        "UPDATE inspection_templates SET {} WHERE id = ?{}",
        set_parts.join(", "),
        idx
    );
    params.push(Box::new(template_id));

    let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&update_sql, param_refs.as_slice())
        .map_err(|e| e.to_string())?;

    // 返回更新后的模板
    let query_sql = format!("SELECT {} FROM inspection_templates WHERE id = ?1", TEMPLATE_COLUMNS);
    crate::db::query::query_one(
        &conn,
        &query_sql,
        rusqlite::params![template_id],
        template_from_row,
    )?
    .ok_or_else(|| format!("更新后查询巡检模板 ID {} 失败", template_id))
}

/// 删除巡检模板
#[tauri::command]
pub fn delete_template(template_id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock();

    let affected = conn
        .execute(
            "DELETE FROM inspection_templates WHERE id = ?1",
            rusqlite::params![template_id],
        )
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("巡检模板 ID {} 不存在", template_id));
    }

    Ok(())
}

/// 批量删除巡检模板
#[tauri::command]
pub fn batch_delete_templates(ids: Vec<i64>, state: State<AppState>) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    for id in &ids {
        tx.execute(
            "DELETE FROM inspection_templates WHERE id = ?1",
            rusqlite::params![id],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;

    Ok(())
}

/// 自动生成巡检模板（根据厂商/型号/设备类型，从命令池选取命令）
#[tauri::command]
pub fn auto_generate_template(
    vendor: String,
    model: Option<String>,
    device_type: Option<String>,
    state: State<AppState>,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock();

    let generated = template_generator::generate_template(
        &conn,
        &vendor,
        model.as_deref(),
        device_type.as_deref(),
    )?;

    let template_name = generated["name"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}-巡检模板", vendor));

    // Store command_ids as the template config
    let config_value = serde_json::json!({
        "command_ids": generated["command_ids"],
    });
    let config_str = config_value.to_string();

    conn.execute(
        "INSERT INTO inspection_templates (name, vendor, model, device_type, config, description) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            template_name,
            vendor,
            model,
            device_type,
            Some(config_str),
            None::<String>,
        ],
    )
    .map_err(|e| e.to_string())?;

    let last_id = conn.last_insert_rowid();

    let sql = format!("SELECT {} FROM inspection_templates WHERE id = ?1", TEMPLATE_COLUMNS);
    let template = crate::db::query::query_one(
        &conn,
        &sql,
        rusqlite::params![last_id],
        template_from_row,
    )?
    .ok_or_else(|| "创建自动生成模板后查询失败".to_string())?;

    Ok(serde_json::json!(template))
}

// ============================================================
// Command Pool Query Commands
// ============================================================

/// 获取所有厂商列表（来自命令池）
#[tauri::command]
pub fn list_vendors(state: State<AppState>) -> Result<Vec<String>, String> {
    let conn = state.db.lock();

    let mut stmt = conn
        .prepare("SELECT DISTINCT vendor FROM command_pool ORDER BY vendor")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    let mut vendors = Vec::new();
    for row in rows {
        vendors.push(row.map_err(|e| e.to_string())?);
    }
    Ok(vendors)
}

/// 获取命令池列表，支持按厂商和分类筛选
#[tauri::command]
pub fn list_commands(
    vendor: Option<String>,
    category: Option<String>,
    state: State<AppState>,
) -> Result<Vec<CommandPool>, String> {
    let conn = state.db.lock();

    let mut sql = format!("SELECT {} FROM command_pool WHERE 1=1", COMMAND_COLUMNS);
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(ref v) = vendor {
        sql.push_str(" AND vendor = ?");
        params.push(Box::new(v.clone()));
    }
    if let Some(ref c) = category {
        sql.push_str(" AND category = ?");
        params.push(Box::new(c.clone()));
    }

    sql.push_str(" ORDER BY category, id");

    let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
    crate::db::query::query_all(&conn, &sql, &param_refs, command_from_row)
}

/// 获取单个命令详情
#[tauri::command]
pub fn get_command(command_id: i64, state: State<AppState>) -> Result<CommandPool, String> {
    let conn = state.db.lock();

    let sql = format!("SELECT {} FROM command_pool WHERE id = ?1", COMMAND_COLUMNS);
    crate::db::query::query_one(&conn, &sql, rusqlite::params![command_id], command_from_row)?
        .ok_or_else(|| format!("命令 ID {} 不存在", command_id))
}

/// 创建命令
#[tauri::command]
pub fn create_command(data: CommandCreate, state: State<AppState>) -> Result<CommandPool, String> {
    let conn = state.db.lock();

    conn.execute(
        "INSERT INTO command_pool (vendor, command, description, category, model) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            data.vendor,
            data.command,
            data.description,
            data.category,
            data.model,
        ],
    )
    .map_err(|e| e.to_string())?;

    let last_id = conn.last_insert_rowid();

    let sql = format!("SELECT {} FROM command_pool WHERE id = ?1", COMMAND_COLUMNS);
    crate::db::query::query_one(&conn, &sql, rusqlite::params![last_id], command_from_row)?
        .ok_or_else(|| "创建命令后查询失败".to_string())
}

/// 更新命令信息（动态字段，仅提供需更新的字段）
#[tauri::command]
pub fn update_command(
    command_id: i64,
    data: CommandUpdate,
    state: State<AppState>,
) -> Result<CommandPool, String> {
    let conn = state.db.lock();

    // 验证命令存在
    let sql = format!("SELECT {} FROM command_pool WHERE id = ?1", COMMAND_COLUMNS);
    let existing = crate::db::query::query_one(
        &conn,
        &sql,
        rusqlite::params![command_id],
        command_from_row,
    )?
    .ok_or_else(|| format!("命令 ID {} 不存在", command_id))?;

    // 构建动态 UPDATE
    let mut set_parts: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    let mut idx = 1i32;

    macro_rules! push_field {
        ($field:ident, $col:expr) => {
            if let Some(ref val) = data.$field {
                set_parts.push(format!("{} = ?{}", $col, idx));
                params.push(Box::new(val.clone()));
                idx += 1;
            }
        };
    }

    push_field!(vendor, "vendor");
    push_field!(command, "command");
    push_field!(description, "description");
    push_field!(category, "category");
    push_field!(model, "model");

    if set_parts.is_empty() {
        return Ok(existing);
    }

    set_parts.push("updated_at = datetime('now')".to_string());

    let update_sql = format!(
        "UPDATE command_pool SET {} WHERE id = ?{}",
        set_parts.join(", "),
        idx
    );
    params.push(Box::new(command_id));

    let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&update_sql, param_refs.as_slice())
        .map_err(|e| e.to_string())?;

    // 返回更新后的命令
    let query_sql = format!("SELECT {} FROM command_pool WHERE id = ?1", COMMAND_COLUMNS);
    crate::db::query::query_one(
        &conn,
        &query_sql,
        rusqlite::params![command_id],
        command_from_row,
    )?
    .ok_or_else(|| format!("更新后查询命令 ID {} 失败", command_id))
}

/// 删除命令
#[tauri::command]
pub fn delete_command(command_id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock();

    let affected = conn
        .execute(
            "DELETE FROM command_pool WHERE id = ?1",
            rusqlite::params![command_id],
        )
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("命令 ID {} 不存在", command_id));
    }

    Ok(())
}

/// 批量删除命令
#[tauri::command]
pub fn batch_delete_commands(ids: Vec<i64>, state: State<AppState>) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    for id in &ids {
        tx.execute(
            "DELETE FROM command_pool WHERE id = ?1",
            rusqlite::params![id],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;

    Ok(())
}
