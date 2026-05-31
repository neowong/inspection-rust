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

#[cfg(target_os = "windows")]
fn extract_webview2_loader() {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let dll_path = exe_dir.join("WebView2Loader.dll");
    if !dll_path.exists() {
        let _ = std::fs::write(&dll_path, include_bytes!("../WebView2Loader.dll"));
    }
}

#[cfg(not(target_os = "windows"))]
fn extract_webview2_loader() {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    extract_webview2_loader();

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

    // Background task: auto-detect device status every 5 minutes
    let bg_db = state.db.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(5 * 60));
            poll_device_statuses(&bg_db);
        }
    });

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
            commands::reports::get_record,
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
            commands::reports::create_report_template,
            commands::reports::update_report_template,
            commands::reports::upload_template,
            commands::reports::download_template,
            commands::reports::preview_template,
            commands::reports::delete_report_template,
            commands::reports::batch_delete_report_templates,
            commands::reports::get_available_variables,
            commands::reports::render_template_preview,
            commands::reports::export_batch_csv,
            commands::reports::analyze_record_logs,
            commands::reports::analyze_batch_logs,
            commands::reports::parse_log_text,
            commands::reports::generate_html_report,
            commands::reports::open_in_browser,
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

/// Background poller: TCP-connect each device's SSH port and update status.
fn poll_device_statuses(db: &Arc<parking_lot::Mutex<rusqlite::Connection>>) {
    let devices: Vec<(i64, String, i64)> = {
        if let Some(conn) = db.try_lock() {
            let mut stmt = match conn.prepare("SELECT id, ip, ssh_port FROM devices") {
                Ok(s) => s,
                Err(_) => return,
            };
            let rows: Vec<_> = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
                })
                .ok()
                .map(|mapped| mapped.filter_map(|r| r.ok()).collect())
                .unwrap_or_default();
            rows
        } else {
            return; // DB locked, skip this round
        }
    };

    if devices.is_empty() { return; }

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut online_count = 0u32;
    let mut offline_count = 0u32;

    for (id, ip, port) in &devices {
        let new_status = match ip.parse::<std::net::IpAddr>() {
            Ok(ip_addr) => {
                match std::net::TcpStream::connect_timeout(
                    &std::net::SocketAddr::new(ip_addr, *port as u16),
                    std::time::Duration::from_secs(5),
                ) {
                    Ok(_) => { online_count += 1; "online" }
                    Err(_) => { offline_count += 1; "offline" }
                }
            }
            Err(_) => { offline_count += 1; "offline" }
        };

        if let Some(conn) = db.try_lock() {
            let _ = conn.execute(
                "UPDATE devices SET status = ?1, last_checked_at = ?2, updated_at = ?3 WHERE id = ?4",
                rusqlite::params![new_status, now, now, id],
            );
        }
    }

    tracing::info!("后台设备检测完成: {} 在线, {} 离线", online_count, offline_count);
}
