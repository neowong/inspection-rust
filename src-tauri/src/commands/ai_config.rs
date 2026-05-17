use tauri::State;
use crate::AppState;
use crate::db::models::{AiModelConfig, AiConfigCreate, AiConfigUpdate};
use crate::services::crypto::CryptoService;

fn ai_cfg_from_row(row: &rusqlite::Row) -> rusqlite::Result<AiModelConfig> {
    Ok(AiModelConfig {
        id: row.get(0)?, name: row.get(1)?, provider: row.get(2)?, model_id: row.get(3)?,
        api_key_encrypted: row.get(4)?, base_url: row.get(5)?, is_active: row.get::<_, i64>(6)? != 0,
        created_at: row.get(7)?, updated_at: row.get(8)?,
    })
}

#[tauri::command]
pub fn list_ai_configs(state: State<AppState>) -> Result<Vec<AiModelConfig>, String> {
    let db = state.db.lock();
    let mut stmt = db.prepare("SELECT * FROM ai_model_configs ORDER BY is_active DESC, created_at DESC").map_err(|e| e.to_string())?;
    let rows: Vec<AiModelConfig> = stmt.query_map([], ai_cfg_from_row).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn create_ai_config(data: AiConfigCreate, state: State<AppState>) -> Result<AiModelConfig, String> {
    let db = state.db.lock();
    let encrypted = CryptoService::encrypt(&data.api_key)?;
    db.execute("INSERT INTO ai_model_configs (name,provider,model_id,api_key_encrypted,base_url,is_active) VALUES (?1,?2,?3,?4,?5,0)", rusqlite::params![data.name, data.provider, data.model_id, encrypted, data.base_url]).map_err(|e| e.to_string())?;
    let id = db.last_insert_rowid();
    db.query_row("SELECT * FROM ai_model_configs WHERE id=?1", rusqlite::params![id], ai_cfg_from_row).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_ai_config(config_id: i64, data: AiConfigUpdate, state: State<AppState>) -> Result<AiModelConfig, String> {
    let db = state.db.lock();
    let c = db.query_row("SELECT * FROM ai_model_configs WHERE id=?1", rusqlite::params![config_id], ai_cfg_from_row).map_err(|_| "AI 配置不存在".to_string())?;
    if let Some(ref key) = data.api_key { if !key.trim().is_empty() { let enc = CryptoService::encrypt(key)?; db.execute("UPDATE ai_model_configs SET api_key_encrypted=?1 WHERE id=?2", rusqlite::params![enc, config_id]).ok(); } }
    let name = data.name.unwrap_or(c.name);
    let provider = data.provider.unwrap_or(c.provider);
    let model_id = data.model_id.unwrap_or(c.model_id);
    let base_url = data.base_url.or(c.base_url);
    db.execute("UPDATE ai_model_configs SET name=?1,provider=?2,model_id=?3,base_url=?4,updated_at=datetime('now') WHERE id=?5", rusqlite::params![name, provider, model_id, base_url, config_id]).map_err(|e| e.to_string())?;
    db.query_row("SELECT * FROM ai_model_configs WHERE id=?1", rusqlite::params![config_id], ai_cfg_from_row).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_ai_config(config_id: i64, state: State<AppState>) -> Result<(), String> {
    let db = state.db.lock();
    let exists: bool = db.query_row("SELECT COUNT(*)>0 FROM ai_model_configs WHERE id=?1", rusqlite::params![config_id], |r| r.get(0)).unwrap_or(false);
    if !exists { return Err("AI 配置不存在".into()); }
    db.execute("DELETE FROM ai_model_configs WHERE id=?1", rusqlite::params![config_id]).ok();
    Ok(())
}

#[tauri::command]
pub fn activate_ai_config(config_id: i64, state: State<AppState>) -> Result<AiModelConfig, String> {
    let db = state.db.lock();
    let exists: bool = db.query_row("SELECT COUNT(*)>0 FROM ai_model_configs WHERE id=?1", rusqlite::params![config_id], |r| r.get(0)).unwrap_or(false);
    if !exists { return Err("AI 配置不存在".into()); }
    db.execute("UPDATE ai_model_configs SET is_active=0", []).ok();
    db.execute("UPDATE ai_model_configs SET is_active=1 WHERE id=?1", rusqlite::params![config_id]).ok();
    db.query_row("SELECT * FROM ai_model_configs WHERE id=?1", rusqlite::params![config_id], ai_cfg_from_row).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn deactivate_ai_config(config_id: i64, state: State<AppState>) -> Result<AiModelConfig, String> {
    let db = state.db.lock();
    let exists: bool = db.query_row("SELECT COUNT(*)>0 FROM ai_model_configs WHERE id=?1", rusqlite::params![config_id], |r| r.get(0)).unwrap_or(false);
    if !exists { return Err("AI 配置不存在".into()); }
    db.execute("UPDATE ai_model_configs SET is_active=0 WHERE id=?1", rusqlite::params![config_id]).ok();
    db.query_row("SELECT * FROM ai_model_configs WHERE id=?1", rusqlite::params![config_id], ai_cfg_from_row).map_err(|e| e.to_string())
}
