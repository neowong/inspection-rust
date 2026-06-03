pub mod db;
pub mod commands;
pub mod services;

use std::sync::Arc;
use parking_lot::Mutex;
use rusqlite::Connection;
use tauri::Manager;

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
}

/// Cached result of the last background AI health check.
static LAST_AI_HEALTH: std::sync::Mutex<Option<services::ai_health::AiHealthResult>> =
    std::sync::Mutex::new(None);

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
fn ensure_webview2_runtime() {
    // Check if WebView2 Runtime is already installed
    if is_webview2_installed() {
        return;
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    tracing::info!("WebView2 Runtime 未检测到，开始自动安装...");

    // Extract the bootstrapper from the binary
    let setup_path = exe_dir.join("MicrosoftEdgeWebview2Setup.exe");
    if let Err(e) = std::fs::write(&setup_path, include_bytes!("../MicrosoftEdgeWebview2Setup.exe")) {
        tracing::warn!("无法写入 WebView2 安装程序: {}", e);
        return;
    }

    // Run silent install (idempotent — exits fast if already installed)
    match std::process::Command::new(&setup_path)
        .args(["/silent", "/install"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            match child.wait() {
                Ok(status) if status.success() => {
                    tracing::info!("WebView2 Runtime 安装成功");
                }
                Ok(_) => {
                    tracing::warn!("WebView2 Runtime 安装器返回非零退出码，继续启动...");
                }
                Err(e) => {
                    tracing::warn!("WebView2 Runtime 安装等待失败: {}，继续启动...", e);
                }
            }
        }
        Err(e) => {
            tracing::warn!("无法启动 WebView2 安装程序: {}，请手动从 https://go.microsoft.com/fwlink/p/?LinkId=2124703 下载安装", e);
        }
    }

    // Clean up the bootstrapper file
    let _ = std::fs::remove_file(&setup_path);
}

#[cfg(target_os = "windows")]
fn is_webview2_installed() -> bool {
    // Try to check via the registry (HKLM)
    let output = std::process::Command::new("reg")
        .args(["query", r"HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}", "/v", "pv"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout.contains("pv")
        }
        _ => {
            // Fallback: check if any WebView2 runtime DLL exists in system32
            let sys32 = std::path::Path::new(r"C:\Windows\System32");
            // WebView2 Runtime installs edgeupdate and related files
            // The simplest check: look for the WebView2 loader in common locations
            sys32.join("Microsoft-Edge-WebView").exists()
                || std::path::Path::new(r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application").exists()
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn ensure_webview2_runtime() {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    ensure_webview2_runtime();

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    // Load optional config file (inspection.toml next to exe → portable mode)
    let config = load_config(&exe_dir);

    // Determine data & log directories
    let app_data_dir = config
        .get("data_dir")
        .and_then(|v| v.as_str())
        .map(|p| resolve_path(&exe_dir, p))
        .unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("inspection-rust")
        });

    std::fs::create_dir_all(&app_data_dir).ok();

    // Logging: stdout + rolling daily file
    let log_dir = config
        .get("log_dir")
        .and_then(|v| v.as_str())
        .map(|p| resolve_path(&exe_dir, p))
        .unwrap_or_else(|| exe_dir.join("logs"));
    std::fs::create_dir_all(&log_dir).ok();
    let file_appender = tracing_appender::rolling::daily(&log_dir, "inspection.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_writer(non_blocking)
        .init();

    // Keep the guard alive so logs are flushed on exit
    std::mem::forget(_guard);

    tracing::info!("数据目录: {}", app_data_dir.display());
    tracing::info!("日志目录: {}", log_dir.display());

    let db_path = app_data_dir.join("inspection.db");
    let state = AppState::new(db_path.to_str().unwrap());

    // Create data directories
    let data_dir = app_data_dir.join("data");
    for sub in &["reports", "report_templates", "uploads", "logs"] {
        std::fs::create_dir_all(data_dir.join(sub)).ok();
    }

    // Background task: auto-detect device status every 5 minutes (blocking TCP, parallel via std::thread::scope)
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
        .setup(|app| {
            // Background task: AI health check every 5 minutes (async reqwest, needs tokio)
            let ai_db = app.state::<AppState>().db.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(5 * 60)).await;
                    poll_ai_health(&ai_db).await;
                }
            });
            Ok(())
        })
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
            commands::devices::detect_device_model,
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
            commands::reports::render_template_preview_with_record,
            commands::reports::list_recent_records,
            commands::reports::generate_docx_report,
            commands::reports::delete_record_report,
            commands::reports::read_report_content,
            commands::reports::export_batch_csv,
            commands::reports::analyze_record_logs,
            commands::reports::analyze_batch_logs,
            commands::reports::parse_log_text,
            commands::reports::generate_html_report,
            commands::reports::open_in_browser,
            // Tools
            commands::tools::scan_live_hosts,
            commands::tools::scan_ports,
            commands::tools::scan_udp_ports,
            commands::tools::check_web_urls,
            commands::tools::snmp_get,
            commands::tools::snmp_v3_get,
            commands::tools::check_zabbix_agent,
            // Settings
            commands::settings::get_settings,
            commands::settings::update_settings,
            // Stats & Health
            get_stats,
            health_check,
            check_ai_health,
            get_ai_health_status,
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

/// Background poller: TCP-connect each device's SSH port in parallel and update status.
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
    let online_count = std::sync::atomic::AtomicU32::new(0);
    let offline_count = std::sync::atomic::AtomicU32::new(0);

    std::thread::scope(|s| {
        for (id, ip, port) in &devices {
            let db = Arc::clone(db);
            let now = now.clone();
            let online_ref = &online_count;
            let offline_ref = &offline_count;
            s.spawn(move || {
                let new_status = match ip.parse::<std::net::IpAddr>() {
                    Ok(ip_addr) => {
                        match std::net::TcpStream::connect_timeout(
                            &std::net::SocketAddr::new(ip_addr, *port as u16),
                            std::time::Duration::from_secs(5),
                        ) {
                            Ok(_) => {
                                online_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                "online"
                            }
                            Err(_) => {
                                offline_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                "offline"
                            }
                        }
                    }
                    Err(_) => {
                        offline_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        "offline"
                    }
                };

                if let Some(conn) = db.try_lock() {
                    let _ = conn.execute(
                        "UPDATE devices SET status = ?1, last_checked_at = ?2, updated_at = ?3 WHERE id = ?4",
                        rusqlite::params![new_status, now, now, id],
                    );
                }
            });
        }
    });

    tracing::info!(
        "后台设备检测完成: {} 在线, {} 离线",
        online_count.load(std::sync::atomic::Ordering::Relaxed),
        offline_count.load(std::sync::atomic::Ordering::Relaxed)
    );
}

/// 查询激活的 AI 配置并解密 API Key，返回 (config, api_key, base_url)
fn get_active_ai_config(
    conn: &rusqlite::Connection,
) -> Result<
    Option<(crate::db::models::AiModelConfig, String, String)>,
    String,
> {
    use crate::db::models::AiModelConfig;
    use crate::services::crypto::CryptoService;

    let config: Option<AiModelConfig> = crate::db::query::query_one(
        conn,
        "SELECT id, name, provider, model_id, api_key_encrypted, base_url, \
         is_active, created_at, updated_at \
         FROM ai_model_configs WHERE is_active = 1 LIMIT 1",
        &[],
        |row| {
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
        },
    )?;

    match config {
        Some(c) => {
            let api_key = CryptoService::decrypt(&c.api_key_encrypted)?;
            let base_url = c.base_url.clone().unwrap_or_default();
            Ok(Some((c, api_key, base_url)))
        }
        None => Ok(None),
    }
}

/// Background task: check the active AI config's API health.
async fn poll_ai_health(db: &Arc<parking_lot::Mutex<rusqlite::Connection>>) {
    let config_data = {
        let conn = match db.try_lock() {
            Some(c) => c,
            None => {
                tracing::warn!("AI 健康检查跳过: DB 被锁定");
                return;
            }
        };
        match get_active_ai_config(&conn) {
            Ok(Some(d)) => Some(d),
            Ok(None) => {
                tracing::debug!("AI 健康检查跳过: 无激活的 AI 配置");
                if let Ok(mut cache) = LAST_AI_HEALTH.lock() {
                    *cache = None;
                }
                return;
            }
            Err(e) => {
                tracing::warn!("AI 健康检查跳过: {}", e);
                return;
            }
        }
    };

    if let Some((config, api_key, base_url)) = config_data {
        let result = services::ai_health::check_ai_health(
            &config.provider,
            &api_key,
            &config.model_id,
            &base_url,
        )
        .await;

        if let Ok(mut cache) = LAST_AI_HEALTH.lock() {
            *cache = Some(result);
        }
    }
}

/// Manually trigger an AI health check for the active config.
#[tauri::command]
async fn check_ai_health(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let (config, api_key, base_url) = {
        let conn = state.db.lock();
        get_active_ai_config(&conn)?
            .ok_or_else(|| "未找到激活的 AI 配置，请先在设置中配置并激活一个 AI 模型".to_string())?
    };

    let result = services::ai_health::check_ai_health(
        &config.provider,
        &api_key,
        &config.model_id,
        &base_url,
    )
    .await;

    if let Ok(mut cache) = LAST_AI_HEALTH.lock() {
        *cache = Some(result.clone());
    }

    Ok(serde_json::to_value(&result).map_err(|e| e.to_string())?)
}

/// Get the last cached AI health check result (from background poller).
#[tauri::command]
fn get_ai_health_status() -> Result<Option<serde_json::Value>, String> {
    let cache = LAST_AI_HEALTH.lock().map_err(|e| e.to_string())?;
    match &*cache {
        Some(result) => Ok(Some(serde_json::to_value(result).map_err(|e| e.to_string())?)),
        None => Ok(None),
    }
}

/// Load optional config from `inspection.toml` next to the exe.
/// If the file doesn't exist or can't be parsed, returns empty map.
///
/// Example `inspection.toml`:
/// ```toml
/// # 数据目录（数据库、报告、模板等），留空则用系统默认目录
/// data_dir = ".\\data"
/// # 日志目录，留空则用 exe 同目录下的 logs/
/// log_dir = ".\\logs"
/// ```
fn load_config(exe_dir: &std::path::Path) -> serde_json::Map<String, serde_json::Value> {
    let config_path = exe_dir.join("inspection.toml");
    if !config_path.exists() {
        return serde_json::Map::new();
    }

    match std::fs::read_to_string(&config_path) {
        Ok(content) => {
            match content.parse::<toml::Table>() {
                Ok(table) => {
                    // Convert toml to serde_json::Value for uniform access
                    let val = toml_to_json(table);
                    val.as_object().cloned().unwrap_or_default()
                }
                Err(e) => {
                    tracing::warn!("配置文件解析失败 {}: {}", config_path.display(), e);
                    serde_json::Map::new()
                }
            }
        }
        Err(e) => {
            tracing::warn!("无法读取配置文件 {}: {}", config_path.display(), e);
            serde_json::Map::new()
        }
    }
}

/// Resolve a path from config: if absolute, use as-is; if relative, resolve against exe_dir.
fn resolve_path(exe_dir: &std::path::Path, path: &str) -> std::path::PathBuf {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        exe_dir.join(p)
    }
}

fn toml_to_json(table: toml::Table) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (k, v) in table {
        map.insert(k, toml_value_to_json(v));
    }
    serde_json::Value::Object(map)
}

fn toml_value_to_json(value: toml::Value) -> serde_json::Value {
    match value {
        toml::Value::String(s) => serde_json::Value::String(s),
        toml::Value::Integer(i) => serde_json::Value::Number((i).into()),
        toml::Value::Float(f) => {
            serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::String(f.to_string()))
        }
        toml::Value::Boolean(b) => serde_json::Value::Bool(b),
        toml::Value::Table(t) => toml_to_json(t),
        toml::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(toml_value_to_json).collect())
        }
        _ => serde_json::Value::Null,
    }
}
