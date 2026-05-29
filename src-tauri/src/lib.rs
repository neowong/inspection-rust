pub mod db;
pub mod commands;
pub mod services;

use std::sync::Arc;
use parking_lot::Mutex;
use rusqlite::Connection;

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
}

impl AppState {
    pub fn new(db_path: &str) -> Self {
        let mut conn = Connection::open(db_path).expect("Failed to open database");
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .expect("Failed to set PRAGMAs");
        db::migrations::run_migrations(&conn).expect("Failed to run migrations");
        db::seed_data::seed_command_pool(&mut conn).ok();
        Self { db: Arc::new(Mutex::new(conn)) }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let app_data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("inspection-rust");

    std::fs::create_dir_all(&app_data_dir).ok();
    let db_path = app_data_dir.join("inspection.db");
    let state = AppState::new(db_path.to_str().unwrap());

    // Create data directories
    let data_dir = app_data_dir.join("data");
    for sub in &["reports", "report_templates", "uploads", "logs"] {
        std::fs::create_dir_all(data_dir.join(sub)).ok();
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            // Devices
            commands::devices::list_devices,
            commands::devices::get_device,
            commands::devices::create_device,
            commands::devices::update_device,
            commands::devices::delete_device,
            commands::devices::batch_delete_devices,
            commands::devices::check_device_status,
            commands::devices::check_all_devices_status,
            commands::devices::get_device_status_log,
            // Templates
            commands::templates::list_templates,
            commands::templates::get_template,
            commands::templates::create_template,
            commands::templates::update_template,
            commands::templates::delete_template,
            commands::templates::batch_delete_templates,
            commands::templates::auto_generate_template,
            // Command Pool
            commands::templates::list_vendors,
            commands::templates::list_commands,
            commands::templates::get_command,
            commands::templates::create_command,
            commands::templates::update_command,
            commands::templates::delete_command,
            commands::templates::batch_delete_commands,
            // Batches (inspections)
            commands::inspections::list_batches,
            commands::inspections::create_batch,
            commands::inspections::get_batch,
            commands::inspections::run_batch,
            commands::inspections::pause_batch,
            commands::inspections::stop_batch,
            commands::inspections::restart_batch,
            commands::inspections::retry_device,
            commands::inspections::delete_batch,
            commands::inspections::batch_delete_batches,
            // Records
            commands::inspections::delete_record,
            commands::inspections::batch_delete_records,
            // Reports & AI
            commands::reports::analyze_record,
            commands::reports::analyze_batch,
            commands::reports::generate_report,
            commands::reports::generate_batch_reports,
            commands::reports::download_report,
            commands::reports::download_batch_report,
            commands::reports::preview_template_context,
            commands::reports::get_active_ai_config,
            // AI Config
            commands::ai_config::list_ai_configs,
            commands::ai_config::create_ai_config,
            commands::ai_config::update_ai_config,
            commands::ai_config::delete_ai_config,
            commands::ai_config::activate_ai_config,
            commands::ai_config::deactivate_ai_config,
            // Report Templates
            commands::reports::list_report_templates,
            commands::reports::upload_template,
            commands::reports::download_template,
            commands::reports::preview_template,
            commands::reports::delete_report_template,
            commands::reports::batch_delete_report_templates,
            // Settings
            commands::settings::get_settings,
            commands::settings::update_settings,
            // Stats & Health
            get_stats,
            health_check,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn get_stats(state: tauri::State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let device_count: i64 = db.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get(0)).unwrap_or(0);
    let online_count: i64 = db.query_row("SELECT COUNT(*) FROM devices WHERE status='online'", [], |r| r.get(0)).unwrap_or(0);
    let offline_count: i64 = db.query_row("SELECT COUNT(*) FROM devices WHERE status='offline'", [], |r| r.get(0)).unwrap_or(0);
    let template_count: i64 = db.query_row("SELECT COUNT(*) FROM inspection_templates", [], |r| r.get(0)).unwrap_or(0);
    let command_count: i64 = db.query_row("SELECT COUNT(*) FROM command_pool", [], |r| r.get(0)).unwrap_or(0);
    let batch_count: i64 = db.query_row("SELECT COUNT(*) FROM inspection_batches", [], |r| r.get(0)).unwrap_or(0);
    let pending_batch_count: i64 = db.query_row("SELECT COUNT(*) FROM inspection_batches WHERE status='pending'", [], |r| r.get(0)).unwrap_or(0);
    let completed_batch_count: i64 = db.query_row("SELECT COUNT(*) FROM inspection_batches WHERE status='completed'", [], |r| r.get(0)).unwrap_or(0);

    Ok(serde_json::json!({
        "device_count": device_count,
        "online_device_count": online_count,
        "offline_device_count": offline_count,
        "template_count": template_count,
        "command_count": command_count,
        "batch_count": batch_count,
        "pending_batch_count": pending_batch_count,
        "completed_batch_count": completed_batch_count,
    }))
}

#[tauri::command]
fn health_check() -> serde_json::Value {
    serde_json::json!({"status": "ok", "version": "3.0.0"})
}
