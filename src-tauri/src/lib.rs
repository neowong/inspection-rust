pub mod commands;
pub mod db;
pub mod services;

use parking_lot::Mutex;
use rusqlite::Connection;
use std::sync::Arc;

/// 全局数据目录，由 `run()` 初始化一次，供 reports.rs / crypto.rs 等模块使用。
pub static APP_DATA_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    /// 批次取消标志注册表：batch_id → AtomicBool。
    /// 与 DB 锁分开，避免 cancel 查询和 DB 操作互相阻塞。
    /// 使用 parking_lot::Mutex（与 db 一致），避免 std::sync::Mutex 的中毒风险：
    /// 持锁 panic 会中毒 std Mutex，导致后续所有 stop/run/pause 链式失败。
    pub batch_cancels:
        Arc<Mutex<std::collections::HashMap<i64, Arc<std::sync::atomic::AtomicBool>>>>,
}

impl AppState {
    pub fn new(db_path: &str) -> Self {
        let mut conn = Connection::open(db_path).expect("Failed to open database");
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .expect("Failed to set PRAGMAs");
        db::migrations::run_migrations(&mut conn).expect("Failed to run migrations");
        if let Err(e) = db::seed_data::seed_command_pool(&mut conn) {
            tracing::warn!("命令池种子数据写入失败（可忽略）: {}", e);
        }
        Self {
            db: Arc::new(Mutex::new(conn)),
            batch_cancels: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }
}


#[cfg(target_os = "windows")]
fn show_webview2_error_and_exit() {
    extern "system" {
        fn MessageBoxW(hWnd: *const core::ffi::c_void, lpText: *const u16, lpCaption: *const u16, uType: u32) -> i32;
    }
    let log_path = std::env::temp_dir().join("inspection-debug.log");
    let msg_text = format!(
        "本程序需要 Microsoft Edge WebView2 Runtime 才能运行。\n\
         \n\
         自动安装失败（需要互联网连接）。\n\
         \n\
         解决方案（任选其一）：\n\
         1. 手动下载离线安装器（~170MB）：\n\
            https://go.microsoft.com/fwlink/p/?LinkId=2124703\n\
            放到程序同目录，下次启动会自动检测安装\n\
         \n\
         2. 确保本机可访问互联网后重新启动程序\n\
         \n\
         调试日志: {}",
        log_path.display()
    );
    let msg: Vec<u16> = msg_text.encode_utf16().chain(std::iter::once(0)).collect();
    let title: Vec<u16> = "AI巡检助手 - 缺少 WebView2".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { MessageBoxW(std::ptr::null(), msg.as_ptr(), title.as_ptr(), 0x10); }
    std::process::exit(1);
}

#[cfg(target_os = "windows")]
fn check_registry_guid(guid: &str) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    for root in [r"HKLM\SOFTWARE", r"HKLM\SOFTWARE\WOW6432Node", r"HKCU\SOFTWARE"] {
        let key = format!(r"{}\Microsoft\EdgeUpdate\Clients\{}", root, guid);
        if let Ok(o) = std::process::Command::new("reg").args(["query", &key, "/v", "pv"])
            .creation_flags(CREATE_NO_WINDOW).output()
        {
            if o.status.success() && String::from_utf8_lossy(&o.stdout).contains("pv") {
                return true;
            }
        }
    }
    false
}

#[cfg(target_os = "windows")]
fn is_webview2_installed() -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // 1. 检查独立安装的 WebView2 Runtime（注册表）
    let guid = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    for root in [r"HKLM\SOFTWARE", r"HKLM\SOFTWARE\WOW6432Node"] {
        let key = format!(r"{}\Microsoft\EdgeUpdate\Clients\{}", root, guid);
        if let Ok(o) = std::process::Command::new("reg").args(["query", &key, "/v", "pv"])
            .creation_flags(CREATE_NO_WINDOW).output()
        {
            if o.status.success() && String::from_utf8_lossy(&o.stdout).contains("pv") {
                return true;
            }
        }
    }

    // 2. 检查 Edge 附带的 WebView2（注册表，不同 GUID）
    for edge_guid in [
        "{F3C4FE00-EFD5-403D-956B-27C74A676A66}", // Edge WebView2 (per-machine)
        "{A1C8A206-5A2E-4E56-B231-D486B80023D1}", // Edge WebView2 (per-user)
    ] {
        for root in [r"HKLM\SOFTWARE", r"HKLM\SOFTWARE\WOW6432Node", r"HKCU\SOFTWARE"] {
            let key = format!(r"{}\Microsoft\EdgeUpdate\Clients\{}", root, edge_guid);
            if let Ok(o) = std::process::Command::new("reg").args(["query", &key, "/v", "pv"])
                .creation_flags(CREATE_NO_WINDOW).output()
            {
                if o.status.success() && String::from_utf8_lossy(&o.stdout).contains("pv") {
                    return true;
                }
            }
        }
    }

    // 3. 文件系统回退：检查常见安装路径
    let paths = [
        r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application",
        r"C:\Program Files\Microsoft\EdgeWebView\Application",
        r"C:\Windows\System32\Microsoft-Edge-WebView",
    ];
    for p in &paths {
        if std::path::Path::new(p).exists() {
            return true;
        }
    }

    // 4. 最后尝试：直接加载 WebView2 loader DLL
    let loader_paths = [
        r"C:\Windows\System32\WebView2Loader.dll",
        r"C:\Windows\SysWOW64\WebView2Loader.dll",
    ];
    for p in &loader_paths {
        if std::path::Path::new(p).exists() {
            return true;
        }
    }

    false
}

/// 启动日志：优先写到 exe 同目录，若无权限则写到 %LOCALAPPDATA%\inspection-rust\startup.log
pub fn startup_log_path() -> std::path::PathBuf {
    // 优先尝试 exe 目录（便携模式 / 有写权限时）
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
        let test_file = exe_dir.join("startup.log");
        // 测试能否写入
        if std::fs::OpenOptions::new().create(true).append(true).open(&test_file).is_ok() {
            return test_file;
        }
    }
    // 回退到 %LOCALAPPDATA%\inspection-rust\
    let fallback = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("inspection-rust");
    std::fs::create_dir_all(&fallback).ok();
    fallback.join("startup.log")
}

fn startup_log(msg: &str) {
    let log_path = startup_log_path();
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let line = format!("[{}] {}\n", ts, msg);
    let _ = std::fs::OpenOptions::new()
        .create(true).append(true).open(&log_path)
        .and_then(|mut f| { use std::io::Write; f.write_all(line.as_bytes()) });
}

#[cfg(target_os = "windows")]
fn ensure_webview2_runtime_with_log() {
    startup_log("检查 WebView2 Runtime...");
    startup_log(&format!("  Edge 注册表 (独立 GUID): {}", check_registry_guid("{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}")));
    startup_log(&format!("  Edge WebView2 路径存在: {}", std::path::Path::new(r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application").exists()));
    startup_log(&format!("  WebView2Loader.dll 存在: {}", std::path::Path::new(r"C:\Windows\System32\WebView2Loader.dll").exists()));

    if is_webview2_installed() {
        startup_log("WebView2 已安装");
        return;
    }
    startup_log("WebView2 未安装，尝试自动安装...");

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    // 离线优先：检查 exe 目录是否已有独立安装器（用户手动下载的 ~170MB 离线版）
    // 文件名参考 Microsoft 官方: MicrosoftEdgeWebView2RuntimeInstallerX64.exe
    let offline_installer = exe_dir.join("MicrosoftEdgeWebView2RuntimeInstallerX64.exe");
    let setup_path: std::path::PathBuf;
    let is_offline: bool;

    if offline_installer.exists() {
        startup_log("检测到离线安装器，使用离线安装（无需联网）");
        setup_path = offline_installer;
        is_offline = true;
    } else {
        // 回退：释放嵌入的 ~1.6MB 在线 Bootstrapper 到 TEMP
        startup_log("未检测到离线安装器，使用嵌入 Bootstrapper（需要联网）");
        is_offline = false;
        let temp_setup = std::env::temp_dir().join("inspection_webview2_setup.exe");
        if let Err(e) = std::fs::write(&temp_setup, include_bytes!("../MicrosoftEdgeWebview2Setup.exe")) {
            startup_log(&format!("释放 Bootstrapper 到 TEMP 失败: {}", e));
            show_webview2_error_and_exit();
            return;
        }
        setup_path = temp_setup;
        startup_log("Bootstrapper 已释放到 TEMP");
    }

    startup_log("开始静默安装 WebView2...");
    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    // 离线独立安装器用 /silent /install，在线 Bootstrapper 只用 /install
    let args: &[&str] = if is_offline {
        &["/silent", "/install"]
    } else {
        &["/install"]
    };

    let install_ok = match std::process::Command::new(&setup_path)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x08000000)  // CREATE_NO_WINDOW
        .spawn()
    {
        Ok(mut child) => match child.wait() {
            Ok(status) => {
                startup_log(&format!("安装器退出码: {}", status.code().unwrap_or(-1)));
                status.success()
            }
            Err(e) => {
                startup_log(&format!("等待安装器失败: {}", e));
                false
            }
        },
        Err(e) => {
            startup_log(&format!("启动安装器失败: {}", e));
            false
        }
    };

    // 清理：仅删除 TEMP 里的 bootstrapper，离线安装器是用户手动放的，不删
    if !is_offline {
        let _ = std::fs::remove_file(&setup_path);
    }

    if !install_ok || !is_webview2_installed() {
        startup_log("WebView2 安装失败，弹窗退出");
        show_webview2_error_and_exit();
    }

    startup_log("WebView2 安装成功");
}

#[cfg(not(target_os = "windows"))]
fn ensure_webview2_runtime_with_log() {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 超早期调试：写到临时目录
    {
        let temp = std::env::temp_dir().join("inspection-debug.log");
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = std::fs::OpenOptions::new().create(true).append(true).open(&temp)
            .and_then(|mut f| { use std::io::Write; writeln!(f, "[{}] run() 开始", ts) });
    }

    startup_log("=== 程序启动 ===");

    // 调试：记录到临时文件
    let debug_log = |msg: &str| {
        let temp = std::env::temp_dir().join("inspection-debug.log");
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = std::fs::OpenOptions::new().create(true).append(true).open(&temp)
            .and_then(|mut f| { use std::io::Write; writeln!(f, "[{}] {}", ts, msg) });
    };

    debug_log("开始检查 WebView2...");
    ensure_webview2_runtime_with_log();
    debug_log("WebView2 检查完成，准备加载配置...");
    startup_log("WebView2 检查通过，继续启动...");

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    debug_log(&format!("exe_dir: {}", exe_dir.display()));

    // Load optional config file (inspection.toml next to exe → portable mode)
    let config = load_config(&exe_dir);
    debug_log("配置加载完成");

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
    debug_log(&format!("数据目录: {}", app_data_dir.display()));

    // 初始化全局数据目录，供其他模块读取
    let _ = APP_DATA_DIR.set(app_data_dir.clone());

    // Logging: stdout + rolling daily file
    let log_dir = config
        .get("log_dir")
        .and_then(|v| v.as_str())
        .map(|p| resolve_path(&exe_dir, p))
        .unwrap_or_else(|| app_data_dir.join("logs"));
    std::fs::create_dir_all(&log_dir).ok();
    let file_appender = tracing_appender::rolling::daily(&log_dir, "inspection.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // 同时输出到 stdout（控制台/终端）和文件（rolling daily）
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info".into());
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true);
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);
    tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer)
        .init();

    // Keep the guard alive so logs are flushed on exit
    std::mem::forget(_guard);
    debug_log("日志系统初始化完成");

    tracing::info!("数据目录: {}", app_data_dir.display());
    tracing::info!("日志目录: {}", log_dir.display());

    startup_log(&format!("数据目录: {}", app_data_dir.display()));
    startup_log(&format!("日志目录: {}", log_dir.display()));

    let db_path = app_data_dir.join("inspection.db");
    debug_log(&format!("数据库路径: {}", db_path.display()));
    startup_log("初始化数据库...");
    let state = AppState::new(
        db_path
            .to_str()
            .unwrap_or_else(|| {
                // 路径含非 UTF-8 字符时回退到临时目录
                tracing::error!("数据库路径无法转换为 UTF-8: {}", db_path.display());
                panic!("数据库路径包含无效字符，请检查系统用户名是否为纯 ASCII");
            }),
    );
    startup_log("数据库初始化完成");
    debug_log("数据库初始化完成");
    debug_log(&format!("DB 连接测试: 设备数量 = {}", {
        let conn = state.db.lock();
        conn.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get::<_, i64>(0)).unwrap_or(-1)
    }));

    // Create data directories
    let data_dir = app_data_dir.join("data");
    for sub in &["reports", "report_templates", "uploads", "logs"] {
        std::fs::create_dir_all(data_dir.join(sub)).ok();
    }
    debug_log("数据子目录创建完成");

    // Background task: 差异化轮询设备状态
    // - 离线设备 90s 探测一次（尽快发现恢复）
    // - 全量设备 5min 探测一次（覆盖在线设备的掉线检测）
    // 两套节奏独立调度，互不阻塞
    let bg_db = state.db.clone();
    let bg_db3 = state.db.clone();
    let bg_db_startup = state.db.clone();
    std::thread::spawn(move || {
        // 离线设备快轮询：每 90s 跑一次（只探测当前离线的设备，尽快发现恢复）
        loop {
            std::thread::sleep(std::time::Duration::from_secs(90));
            poll_offline_devices(&bg_db3);
        }
    });
    std::thread::spawn(move || {
        // 全量轮询：每 5min 一次（覆盖在线设备掉线 + 离线设备补静态信息）
        loop {
            std::thread::sleep(std::time::Duration::from_secs(5 * 60));
            poll_device_statuses(&bg_db);
        }
    });
    // 启动后立即触发一次所有缺静态信息设备的检测（server + database + 网络设备）
    // detect_static_info_if_missing 内部按已有信息/凭据判断是否真正执行，故全量遍历安全
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3)); // 等 DB 初始化完成
        let device_ids: Vec<i64> = {
            if let Some(conn) = bg_db_startup.try_lock() {
                let stmt = conn
                    .prepare("SELECT id FROM devices ORDER BY id")
                    .ok();
                stmt.and_then(|mut s| {
                    s.query_map([], |row| row.get::<_, i64>(0))
                        .ok()
                        .map(|rows| rows.filter_map(|r| r.ok()).collect())
                })
                .unwrap_or_default()
            } else {
                vec![]
            }
        };
        tracing::info!("[startup] 启动静态信息检测，共 {} 台设备", device_ids.len());
        for id in device_ids {
            commands::devices::detect_static_info_if_missing(id, &bg_db_startup);
        }
        tracing::info!("[startup] 启动静态信息检测完成");
    });
    debug_log("后台检测线程已启动");

    startup_log("注册插件和命令...");
    debug_log("准备创建 Tauri Builder...");
    debug_log("创建 Tauri Builder...");
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .setup(|_app| {
            startup_log("Tauri setup 完成，窗口即将显示");
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
            commands::devices::detect_device_model,
            commands::devices::detect_device_model_by_id,
            // Templates
            commands::templates::list_templates,
            commands::templates::create_template,
            commands::templates::update_template,
            commands::templates::delete_template,
            // Command Pool
            commands::templates::list_commands,
            commands::templates::create_command,
            commands::templates::update_command,
            commands::templates::delete_command,
            // Batches (inspections)
            commands::inspections::list_batches,
            commands::inspections::create_batch,
            commands::inspections::get_batch,
            commands::inspections::run_batch,
            commands::inspections::pause_batch,
            commands::inspections::stop_batch,
            commands::inspections::restart_batch,
            commands::inspections::restart_and_run_batch,
            commands::inspections::retry_device,
            commands::inspections::delete_batch,
            // Reports & AI
            commands::reports::get_record,
            commands::reports::analyze_record,
            commands::reports::analyze_batch,
            commands::reports::download_report,
            commands::reports::save_generated_file,
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
            commands::reports::delete_report_template,
            commands::reports::generate_docx_report,
            commands::reports::generate_batch_docx_zip,
            commands::reports::generate_batch_docx_combined,
            commands::reports::delete_record_report,
            commands::reports::open_reports_dir,
            commands::reports::analyze_record_logs,
            commands::reports::parse_log_text,
            // Tools
            commands::tools::scan_live_hosts,
            commands::tools::scan_ports,
            commands::tools::scan_udp_ports,
            commands::tools::check_web_urls,
            commands::tools::snmp_get,
            commands::tools::snmp_v3_get,
            commands::tools::check_zabbix_agent,
            // Stats
            get_stats,
        ])
        .run(tauri::generate_context!())
        .map_err(|e| {
            let err_msg = format!("Tauri 启动失败: {}", e);
            startup_log(&err_msg);
            debug_log(&err_msg);
            tracing::error!("{}", err_msg);
            // 在 Windows 无控制台时，panic hook 的 MessageBox 会显示此错误
            panic!("{}", err_msg);
        })
        .expect("error while running tauri application");
}

#[tauri::command]
fn get_stats(state: tauri::State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    // 合并为单次查询，减少锁内 prepare/往返开销
    let (device_count, online_count, offline_count, template_count, command_count,
         batch_count, pending_batch_count, completed_batch_count,
         network_device_count, security_device_count, server_count, database_count, report_count) = db
        .query_row(
            "SELECT \
                (SELECT COUNT(*) FROM devices), \
                (SELECT COUNT(*) FROM devices WHERE status='online'), \
                (SELECT COUNT(*) FROM devices WHERE status='offline'), \
                (SELECT COUNT(*) FROM inspection_templates), \
                (SELECT COUNT(*) FROM command_pool), \
                (SELECT COUNT(*) FROM inspection_batches), \
                (SELECT COUNT(*) FROM inspection_batches WHERE status='pending'), \
                (SELECT COUNT(*) FROM inspection_batches WHERE status='completed'), \
                (SELECT COUNT(*) FROM devices WHERE device_type IN ('switch','router')), \
                (SELECT COUNT(*) FROM devices WHERE device_type IN ('firewall','loadbalancer')), \
                (SELECT COUNT(*) FROM devices WHERE device_type = 'server'), \
                (SELECT COUNT(*) FROM devices WHERE device_type = 'database'), \
                ((SELECT COUNT(*) FROM inspection_records WHERE report_path IS NOT NULL AND report_path != '') \
               + (SELECT COUNT(*) FROM inspection_batches WHERE combined_report_path IS NOT NULL AND combined_report_path != ''))",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, i64>(8)?,
                    r.get::<_, i64>(9)?,
                    r.get::<_, i64>(10)?,
                    r.get::<_, i64>(11)?,
                    r.get::<_, i64>(12)?,
                ))
            },
        )
        .unwrap_or((0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0));

    Ok(serde_json::json!({
        "device_count": device_count,
        "online_device_count": online_count,
        "offline_device_count": offline_count,
        "template_count": template_count,
        "command_count": command_count,
        "batch_count": batch_count,
        "pending_batch_count": pending_batch_count,
        "completed_batch_count": completed_batch_count,
        "network_device_count": network_device_count,
        "security_device_count": security_device_count,
        "server_count": server_count,
        "database_count": database_count,
        "report_count": report_count,
    }))
}

/// 静态信息检测冷却：同一设备 10 分钟内不重复触发（防止状态抖动反复 SSH）
const STATIC_DETECT_COOLDOWN_SECS: i64 = 600;
/// 进程内记录每设备最近一次静态检测时间戳（epoch 秒），用于冷却去重
static LAST_STATIC_DETECT: std::sync::OnceLock<Mutex<std::collections::HashMap<i64, i64>>> =
    std::sync::OnceLock::new();

fn last_detect_map() -> &'static Mutex<std::collections::HashMap<i64, i64>> {
    LAST_STATIC_DETECT.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

/// 判断设备是否仍在冷却期内（返回 true 表示应跳过本次检测）
fn in_cooldown(device_id: i64, now_epoch: i64) -> bool {
    let map = last_detect_map().lock();
    if let Some(&last) = map.get(&device_id) {
        now_epoch - last < STATIC_DETECT_COOLDOWN_SECS
    } else {
        false
    }
}

/// 记录一次静态检测（更新冷却时间戳）
fn mark_detected(device_id: i64, now_epoch: i64) {
    last_detect_map().lock().insert(device_id, now_epoch);
}

fn now_epoch() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 仅探测当前离线设备的状态（高频快轮询用），上线后触发静态检测（带冷却）
fn poll_offline_devices(db: &Arc<parking_lot::Mutex<rusqlite::Connection>>) {
    // 读取当前离线设备
    let devices: Vec<(i64, String, i64)> = {
        if let Some(conn) = db.try_lock() {
            let mut stmt = match conn.prepare(
                "SELECT id, ip, ssh_port FROM devices WHERE status = 'offline' OR status = 'unknown'",
            ) {
                Ok(s) => s,
                Err(_) => return,
            };
            stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
            })
            .ok()
            .map(|mapped| mapped.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        } else {
            return;
        }
    };
    if devices.is_empty() {
        return;
    }

    let now_str = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let now = now_epoch();
    let recovered: Mutex<Vec<i64>> = Mutex::new(Vec::new());

    std::thread::scope(|s| {
        for (id, ip, port) in &devices {
            let recovered = &recovered;
            let id = *id;
            s.spawn(move || {
                let online = ip.parse::<std::net::IpAddr>()
                    .ok()
                    .zip(u16::try_from(*port).ok().filter(|&p| p > 0))
                    .map(|(ip_addr, p)| {
                        std::net::TcpStream::connect_timeout(
                            &std::net::SocketAddr::new(ip_addr, p),
                            std::time::Duration::from_secs(5),
                        ).is_ok()
                    })
                    .unwrap_or(false);
                if online {
                    recovered.lock().push(id);
                }
            });
        }
    });

    let recovered = recovered.lock();
    if recovered.is_empty() {
        return;
    }
    tracing::info!("[poll-offline] {} 台离线设备已恢复在线", recovered.len());

    // 写回 online 状态 + 状态日志
    {
        let conn = db.lock();
        for id in recovered.iter() {
            let _ = conn.execute(
                "UPDATE devices SET status = 'online', last_checked_at = ?1 WHERE id = ?2",
                rusqlite::params![now_str, id],
            );
            let _ = conn.execute(
                "INSERT INTO device_status_logs (device_id, old_status, new_status, checked_at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![id, "offline", "online", now_str],
            );
        }
    }

    // 恢复在线的设备触发静态检测（带冷却去重）
    for id in recovered.iter() {
        if !in_cooldown(*id, now) {
            mark_detected(*id, now);
            commands::devices::detect_static_info_if_missing(*id, db);
        }
    }
}

/// Background poller: TCP-connect each device's SSH port in parallel and update status.
/// After status update, triggers static-info SSH detection for newly-online devices
/// that don't yet have model/sysname (带冷却去重，防止状态抖动反复 SSH).
fn poll_device_statuses(db: &Arc<parking_lot::Mutex<rusqlite::Connection>>) {
    // Phase 1: read id/ip/port + model/sysname/status — model/sysname 用于跳过冗余 SSH，
    // status 用于判断是否变更（仅变更时写状态日志）
    #[allow(clippy::type_complexity)]
    let devices: Vec<(i64, String, i64, Option<String>, Option<String>, String)> = {
        if let Some(conn) = db.try_lock() {
            let mut stmt = match conn.prepare(
                "SELECT id, ip, ssh_port, model, sysname, status FROM devices",
            ) {
                Ok(s) => s,
                Err(_) => return,
            };
            let rows: Vec<_> = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                })
                .ok()
                .map(|mapped| mapped.filter_map(|r| r.ok()).collect())
                .unwrap_or_default();
            rows
        } else {
            return;
        }
    };

    if devices.is_empty() {
        return;
    }

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let online_count = std::sync::atomic::AtomicU32::new(0);
    let offline_count = std::sync::atomic::AtomicU32::new(0);
    // Collect device IDs that came online and need static info detection
    let needs_detect: Mutex<Vec<i64>> = Mutex::new(Vec::new());
    // 收集 (id, new_status, old_status)，scope 结束后一次性持锁批量更新，避免 try_lock 丢更新
    let results: Mutex<Vec<(i64, String, String)>> = Mutex::new(Vec::new());

    std::thread::scope(|s| {
        for (id, ip, port, model, sysname, old_status) in &devices {
            let online_ref = &online_count;
            let offline_ref = &offline_count;
            let needs_detect = &needs_detect;
            let results = &results;
            let has_static = model.as_ref().filter(|s| !s.is_empty()).is_some()
                || sysname.as_ref().filter(|s| !s.is_empty()).is_some();
            let id = *id;
            let old_status = old_status.clone();
            s.spawn(move || {
                let new_status = match ip.parse::<std::net::IpAddr>() {
                    Ok(ip_addr) => {
                        match u16::try_from(*port).ok().filter(|&p| p > 0) {
                            Some(port) => {
                                match std::net::TcpStream::connect_timeout(
                                    &std::net::SocketAddr::new(ip_addr, port),
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
                            None => {
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

                results.lock().push((id, new_status.to_string(), old_status));

                // 在线且无静态信息的设备 → 标记待 SSH 采集
                if new_status == "online" && !has_static {
                    needs_detect.lock().push(id);
                }
            });
        }
    });

    // Phase 1.5: 一次性持锁批量写回状态 + 状态变更日志（持锁时间短，无网络 IO）
    {
        let results = results.lock().clone();
        let conn = db.lock();
        for (id, new_status, old_status) in &results {
            let _ = conn.execute(
                "UPDATE devices SET status = ?1, last_checked_at = ?2 WHERE id = ?3",
                rusqlite::params![new_status, now, id],
            );
            // 仅状态变更时写日志，避免 device_status_logs 无限增长
            if old_status != new_status {
                let _ = conn.execute(
                    "INSERT INTO device_status_logs (device_id, old_status, new_status, checked_at) VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![id, old_status, new_status, now],
                );
            }
        }
    }

    tracing::info!(
        "后台设备检测完成: {} 在线, {} 离线",
        online_count.load(std::sync::atomic::Ordering::Relaxed),
        offline_count.load(std::sync::atomic::Ordering::Relaxed),
    );

    // Phase 2: 对需要采集静态信息的在线设备做后台 SSH 检测（带冷却去重）
    // 每批最多 3 台并发，完成后再取下一批
    let pending = needs_detect.lock();
    if !pending.is_empty() {
        let now = now_epoch();
        let filtered: Vec<i64> = pending
            .iter()
            .filter(|id| !in_cooldown(**id, now))
            .copied()
            .collect();
        let skipped = pending.len() - filtered.len();
        tracing::info!(
            "后台静态信息采集: {} 台需要检测，{} 台冷却中跳过（每批并发 3）",
            filtered.len(), skipped
        );
        for id in &filtered {
            mark_detected(*id, now);
        }
        for chunk in filtered.chunks(3) {
            std::thread::scope(|s| {
                for id in chunk {
                    let db = Arc::clone(db);
                    s.spawn(move || {
                        commands::devices::detect_static_info_if_missing(*id, &db);
                    });
                }
            });
        }
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
        toml::Value::Float(f) => serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::String(f.to_string())),
        toml::Value::Boolean(b) => serde_json::Value::Bool(b),
        toml::Value::Table(t) => toml_to_json(t),
        toml::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(toml_value_to_json).collect())
        }
        _ => serde_json::Value::Null,
    }
}
