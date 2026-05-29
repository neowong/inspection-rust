use tauri::State;

use crate::AppState;
use crate::db::models::{AiConfigCreate, AiConfigUpdate, AiModelConfig};
use crate::services::crypto::CryptoService;

// ============================================================
// Constants
// ============================================================

const AI_CONFIG_COLUMNS: &str =
    "id, name, provider, model_id, api_key_encrypted, base_url, is_active, created_at, updated_at";

// ============================================================
// Helpers
// ============================================================

fn config_from_row(row: &rusqlite::Row) -> rusqlite::Result<AiModelConfig> {
    Ok(AiModelConfig {
        id: row.get(0)?,
        name: row.get(1)?,
        provider: row.get(2)?,
        model_id: row.get(3)?,
        api_key_encrypted: row.get(4)?,
        base_url: row.get(5)?,
        is_active: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

// ============================================================
// Query Commands
// ============================================================

/// 获取所有 AI 模型配置列表
#[tauri::command]
pub fn list_ai_configs(state: State<AppState>) -> Result<Vec<AiModelConfig>, String> {
    let conn = state.db.lock();

    let sql = format!(
        "SELECT {} FROM ai_model_configs ORDER BY created_at DESC",
        AI_CONFIG_COLUMNS
    );
    crate::db::query::query_all(&conn, &sql, &[], config_from_row)
}

// ============================================================
// Mutate Commands
// ============================================================

/// 创建 AI 模型配置（自动加密 API Key）
#[tauri::command]
pub fn create_ai_config(
    data: AiConfigCreate,
    state: State<AppState>,
) -> Result<AiModelConfig, String> {
    let conn = state.db.lock();

    // Encrypt the API key before storing
    let encrypted_key = CryptoService::encrypt(&data.api_key_encrypted)?;
    let is_active = data.is_active.unwrap_or(0);

    conn.execute(
        "INSERT INTO ai_model_configs (name, provider, model_id, api_key_encrypted, base_url, is_active) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            data.name,
            data.provider,
            data.model_id,
            encrypted_key,
            data.base_url,
            is_active,
        ],
    )
    .map_err(|e| e.to_string())?;

    let last_id = conn.last_insert_rowid();

    let sql = format!(
        "SELECT {} FROM ai_model_configs WHERE id = ?1",
        AI_CONFIG_COLUMNS
    );
    crate::db::query::query_one(&conn, &sql, rusqlite::params![last_id], config_from_row)?
        .ok_or_else(|| "创建 AI 配置后查询失败".to_string())
}

/// 更新 AI 模型配置（如果提供了 API Key，自动重新加密）
#[tauri::command]
pub fn update_ai_config(
    config_id: i64,
    data: AiConfigUpdate,
    state: State<AppState>,
) -> Result<AiModelConfig, String> {
    let conn = state.db.lock();

    // Verify config exists
    let sql = format!(
        "SELECT {} FROM ai_model_configs WHERE id = ?1",
        AI_CONFIG_COLUMNS
    );
    let existing = crate::db::query::query_one(
        &conn,
        &sql,
        rusqlite::params![config_id],
        config_from_row,
    )?
    .ok_or_else(|| format!("AI 配置 ID {} 不存在", config_id))?;

    // Build dynamic UPDATE
    let mut set_parts: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
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
    push_field!(provider, "provider");
    push_field!(model_id, "model_id");
    push_field!(base_url, "base_url");

    // Handle API key encryption
    if let Some(ref api_key) = data.api_key_encrypted {
        if !api_key.is_empty() {
            let encrypted = CryptoService::encrypt(api_key)?;
            set_parts.push(format!("api_key_encrypted = ?{}", idx));
            params.push(Box::new(encrypted));
            idx += 1;
        }
    }

    // Handle is_active
    if let Some(ref is_active) = data.is_active {
        set_parts.push(format!("is_active = ?{}", idx));
        params.push(Box::new(*is_active));
        idx += 1;
    }

    if set_parts.is_empty() {
        return Ok(existing);
    }

    set_parts.push("updated_at = datetime('now')".to_string());

    let update_sql = format!(
        "UPDATE ai_model_configs SET {} WHERE id = ?{}",
        set_parts.join(", "),
        idx
    );
    params.push(Box::new(config_id));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&update_sql, param_refs.as_slice())
        .map_err(|e| e.to_string())?;

    // Return updated config
    let query_sql = format!(
        "SELECT {} FROM ai_model_configs WHERE id = ?1",
        AI_CONFIG_COLUMNS
    );
    crate::db::query::query_one(
        &conn,
        &query_sql,
        rusqlite::params![config_id],
        config_from_row,
    )?
    .ok_or_else(|| format!("更新后查询 AI 配置 ID {} 失败", config_id))
}

/// 删除 AI 模型配置
#[tauri::command]
pub fn delete_ai_config(config_id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock();

    let affected = conn
        .execute(
            "DELETE FROM ai_model_configs WHERE id = ?1",
            rusqlite::params![config_id],
        )
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("AI 配置 ID {} 不存在", config_id));
    }

    Ok(())
}

/// 激活 AI 模型配置（同时反激活其他所有配置）
#[tauri::command]
pub fn activate_ai_config(config_id: i64, state: State<AppState>) -> Result<(), String> {
    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // Deactivate all configs first
    tx.execute(
        "UPDATE ai_model_configs SET is_active = 0, updated_at = datetime('now')",
        [],
    )
    .map_err(|e| e.to_string())?;

    // Activate the target config
    let affected = tx
        .execute(
            "UPDATE ai_model_configs SET is_active = 1, updated_at = datetime('now') WHERE id = ?1",
            rusqlite::params![config_id],
        )
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("AI 配置 ID {} 不存在", config_id));
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 反激活 AI 模型配置
#[tauri::command]
pub fn deactivate_ai_config(config_id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock();

    let affected = conn
        .execute(
            "UPDATE ai_model_configs SET is_active = 0, updated_at = datetime('now') WHERE id = ?1",
            rusqlite::params![config_id],
        )
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("AI 配置 ID {} 不存在", config_id));
    }

    Ok(())
}
