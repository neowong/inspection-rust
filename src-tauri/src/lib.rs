pub mod commands;
pub mod db;
pub mod services;

use parking_lot::Mutex;
use rusqlite::Connection;
use std::sync::Arc;
use tauri::Manager;

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
    /// 离线 IP 归属地库（ip2region.xdb），setup 时加载，None 表示未加载
    pub ip_db: Arc<parking_lot::RwLock<Option<Arc<Vec<u8>>>>>,
}

impl AppState {
    pub fn new(db_path: &str) -> Self {
        let mut conn = Connection::open(db_path).expect("Failed to open database");
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .expect("Failed to set PRAGMAs");
        db::migrations::run_migrations(&mut conn).expect("Failed to run migrations");
        // 启动时清理：上次意外退出卡在 processing 的记录重置为 failed
        if let Err(e) = conn.execute(
            "UPDATE inspection_records SET ai_status = 'failed', error_message = '应用意外退出导致分析中断', updated_at = datetime('now') WHERE ai_status = 'processing'",
            [],
        ) {
            tracing::warn!("清理卡住的处理中记录失败（可忽略）: {}", e);
        }
        if let Err(e) = db::seed_data::seed_command_pool(&mut conn) {
            tracing::warn!("命令池种子数据写入失败（可忽略）: {}", e);
        }
        Self {
            db: Arc::new(Mutex::new(conn)),
            batch_cancels: Arc::new(Mutex::new(std::collections::HashMap::new())),
            ip_db: Arc::new(parking_lot::RwLock::new(None)),
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

/// 启动日志路径：优先 exe_dir/logs/，fallback 到 %LOCALAPPDATA%\ai-inspection\
pub fn startup_log_path() -> std::path::PathBuf {
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
        let log_dir = exe_dir.join("logs");
        if std::fs::create_dir_all(&log_dir).is_ok() {
            let test_file = log_dir.join("startup.log");
            if std::fs::OpenOptions::new().create(true).append(true).open(&test_file).is_ok() {
                return test_file;
            }
        }
    }
    let fallback = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("ai-inspection");
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

    // 调试日志：优先 exe_dir/logs/，fallback 到 %TEMP%
    let debug_log = |msg: &str| {
        let log_file = if let Some(exe_dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
            let log_dir = exe_dir.join("logs");
            if std::fs::create_dir_all(&log_dir).is_ok() {
                log_dir.join("debug.log")
            } else {
                std::env::temp_dir().join("ai-inspection-debug.log")
            }
        } else {
            std::env::temp_dir().join("ai-inspection-debug.log")
        };
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = std::fs::OpenOptions::new().create(true).append(true).open(&log_file)
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

    // Windows 日志行尾修复：tracing 写 \n，记事本需要 \r\n
    #[cfg(windows)]
    struct CrlfWriter<W: std::io::Write>(W);
    #[cfg(windows)]
    impl<W: std::io::Write> std::io::Write for CrlfWriter<W> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let mut written = 0;
            for &b in buf {
                if b == b'\n' {
                    self.0.write_all(b"\r\n")?;
                } else if b != b'\r' {
                    self.0.write_all(&[b])?;
                }
                written += 1;
            }
            Ok(written)
        }
        fn flush(&mut self) -> std::io::Result<()> { self.0.flush() }
    }
    #[cfg(windows)]
    struct CrlfMakeWriter<M>(M);
    #[cfg(windows)]
    impl<'a, M: tracing_subscriber::fmt::MakeWriter<'a>> tracing_subscriber::fmt::MakeWriter<'a> for CrlfMakeWriter<M> {
        type Writer = CrlfWriter<M::Writer>;
        fn make_writer(&'a self) -> Self::Writer { CrlfWriter(self.0.make_writer()) }
    }

    // Logging: stdout + rolling daily file
    // 优先 exe_dir/logs/（与二进制同目录），不可写时 fallback 到 app_data_dir/logs/
    let preferred_log_dir = exe_dir.join("logs");
    let log_dir = config
        .get("log_dir")
        .and_then(|v| v.as_str())
        .map(|p| resolve_path(&exe_dir, p))
        .unwrap_or_else(|| {
            // 尝试在 exe 目录创建 logs/，失败则 fallback
            if std::fs::create_dir_all(&preferred_log_dir).is_ok()
                && preferred_log_dir.metadata().map(|m| !m.permissions().readonly()).unwrap_or(false)
            {
                preferred_log_dir
            } else {
                app_data_dir.join("logs")
            }
        });
    std::fs::create_dir_all(&log_dir).ok();
    tracing::info!("日志目录: {}", log_dir.display());
    let file_appender = tracing_appender::rolling::daily(&log_dir, "ai-inspection.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // 同时输出到 stdout（控制台/终端）和文件（rolling daily）
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info".into());
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true);
    // Windows 日志文件行尾修复：tracing 默认写 \n，Windows 记事本需要 \r\n
    #[cfg(windows)]
    let file_writer = CrlfMakeWriter(non_blocking);
    #[cfg(not(windows))]
    let file_writer = non_blocking;
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_writer)
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
    let db_path_str = match db_path.to_str() {
        Some(s) => s,
        None => {
            // 路径含非 UTF-8 字符时回退到系统临时目录（跨平台兼容）
            tracing::error!("数据库路径无法转换为 UTF-8: {}，回退到临时目录", db_path.display());
            let temp_db = std::env::temp_dir().join("ai-inspection-inspection.db");
            &*Box::leak(temp_db.to_string_lossy().into_owned().into_boxed_str())
        }
    };
    let state = AppState::new(db_path_str);
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

    // Background task: 全量设备 60s 轮询一次
    let bg_db = state.db.clone();
    let bg_db_startup = state.db.clone();
    std::thread::spawn(move || {
        // 首次延迟 10s 等窗口加载完成，之后每 60s 一次
        std::thread::sleep(std::time::Duration::from_secs(10));
        loop {
            poll_device_statuses(&bg_db);
            std::thread::sleep(std::time::Duration::from_secs(60));
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
        .setup(|app| {
            startup_log("Tauri setup 完成，窗口即将显示");
            // 加载离线 IP 归属地库（与二进制同目录的 ip2region_v4.xdb）
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()));
            let xdb_path = exe_dir.as_ref().map(|d| d.join("ip2region_v4.xdb"));
            match xdb_path.as_deref() {
                Some(path) if path.exists() => {
                    match crate::services::ip_location::load_xdb(path) {
                        Ok(data) => {
                            let state = app.state::<AppState>();
                            *state.ip_db.write() = Some(Arc::new(data));
                            tracing::info!("ip2region_v4.xdb 已加载: {}", path.display());
                        }
                        Err(e) => {
                            tracing::warn!("ip2region_v4.xdb 加载失败: {}", e);
                        }
                    }
                }
                _ => {
                    tracing::info!(
                        "ip2region_v4.xdb 未找到（路由跟踪归属地不可用）。请下载放到程序同目录: {}",
                        exe_dir.map(|d| d.display().to_string()).unwrap_or_default()
                    );
                }
            }
            // 非 Linux: 窗口 visible:true 会在 WebView 就绪前闪白，
            // 这里立即 hide 再等所有内容加载后 show 消除闪烁。
            // Linux: visible:true 保持，hide+show 会导致关闭按钮失效。
            #[cfg(not(target_os = "linux"))]
            if let Some(window) = app.get_webview_window("main") {
                window.hide().ok();
                window.show().ok();
            }
            Ok(())
        })
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            // Devices
            commands::devices::list_devices,
            commands::devices::create_device,
            commands::devices::update_device,
            commands::devices::delete_device,
            commands::devices::batch_delete_devices,
            commands::devices::export_devices_csv,
            commands::devices::import_devices_csv,
            commands::devices::check_device_status,
            commands::devices::check_all_devices_status,
            commands::devices::detect_device_model_by_id,
            // Templates
            commands::templates::list_templates,
            commands::templates::create_template,
            commands::templates::update_template,
            commands::templates::check_template_devices,
            commands::templates::delete_template,
            commands::templates::batch_delete_templates,
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
            commands::inspections::batch_delete_batches,
            commands::inspections::delete_batch_reports,
            // Reports & AI
            commands::reports::get_record,
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
            commands::ai_config::test_ai_config,
            // Report Templates
            commands::reports::list_report_templates,
            commands::reports::create_report_template,
            commands::reports::update_report_template,
            commands::reports::delete_report_template,
            commands::reports::analyze_record,
            commands::reports::generate_docx_report,
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
            commands::tools::get_app_version,
            commands::tools::get_os_info,
            commands::tools::has_ip_db,
            commands::tools::download_ip_db,
            commands::tools::check_update,
            commands::tools::submit_feedback,
            commands::tools::trace_route,
            commands::tools::start_tftp_server,
            commands::tools::stop_tftp_server,
            commands::tools::start_syslog_server,
            commands::tools::stop_syslog_server,
            // Stats
            get_stats,
            // Chat
            chat_with_ai,
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
    get_stats_inner(&state)
}

/// 构建工具定义列表
fn build_tools() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "scan_live_hosts",
                "description": "扫描指定网段的存活主机（ICMP ping + TCP fallback）",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "subnet": {"type": "string", "description": "CIDR 网段，如 192.168.1.0/24"},
                        "timeout_ms": {"type": "number", "description": "每台主机超时毫秒", "default": 3000}
                    },
                    "required": ["subnet"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "list_devices",
                "description": "查询设备列表，支持按类型、状态、厂商筛选",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "device_type": {"type": "string", "description": "设备类型过滤，如 switch,router（逗号分隔），other 表示其他"},
                        "status": {"type": "string", "description": "设备状态过滤，如 online,offline"},
                        "vendor": {"type": "string", "description": "厂商过滤"}
                    }
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "get_stats",
                "description": "获取系统统计概览（设备数量、在线率、任务状态、报告数等）",
                "parameters": {"type": "object", "properties": {}}
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "check_device_status",
                "description": "检测指定设备的在线状态",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "device_id": {"type": "integer", "description": "设备 ID"}
                    },
                    "required": ["device_id"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "check_all_devices_status",
                "description": "批量检测所有设备的在线状态",
                "parameters": {"type": "object", "properties": {}}
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "update_device",
                "description": "修改设备信息（名称、IP、类型、厂商等），需先查询设备 ID",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "device_id": {"type": "integer", "description": "设备 ID（必需）"},
                        "name": {"type": "string", "description": "新设备名称"},
                        "ip": {"type": "string", "description": "新 IP 地址"},
                        "device_type": {"type": "string", "description": "设备类型 switch/router/firewall/loadbalancer/server/database"},
                        "vendor": {"type": "string", "description": "厂商"}
                    },
                    "required": ["device_id"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "create_device",
                "description": "添加新设备",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "description": "设备名称"},
                        "ip": {"type": "string", "description": "IP 地址"},
                        "device_type": {"type": "string", "description": "设备类型"},
                        "vendor": {"type": "string", "description": "厂商"}
                    },
                    "required": ["name", "ip", "device_type", "vendor"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "list_templates",
                "description": "查询巡检模板列表",
                "parameters": {"type": "object", "properties": {}}
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "list_batches",
                "description": "查询巡检任务列表，支持按状态筛选",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "status": {"type": "string", "description": "任务状态过滤，如 pending,running,completed"}
                    }
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "update_template",
                "description": "修改巡检模板（名称、厂商、设备类型等），需先查询模板 ID",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "template_id": {"type": "integer", "description": "模板 ID（必需）"},
                        "name": {"type": "string", "description": "新模板名称"},
                        "vendor": {"type": "string", "description": "厂商"},
                        "device_type": {"type": "string", "description": "设备类型"},
                        "description": {"type": "string", "description": "描述"}
                    },
                    "required": ["template_id"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "run_batch",
                "description": "执行巡检任务，对批次中所有设备发起 SSH 巡检",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "batch_id": {"type": "integer", "description": "批次 ID（必需）"}
                    },
                    "required": ["batch_id"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "analyze_batch",
                "description": "AI 分析巡检结果，生成评判结论",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "batch_id": {"type": "integer", "description": "批次 ID（必需）"},
                        "force": {"type": "boolean", "description": "强制重新分析，覆盖已有结果"}
                    },
                    "required": ["batch_id"]
                }
            }
        }),
    ]
}

/// 执行工具调用并返回 JSON 字符串结果
async fn execute_tool(
    name: &str,
    args: &str,
    state: tauri::State<'_, AppState>,
    app_handle: &tauri::AppHandle,
) -> String {
    tracing::info!("工具调用: name={}, args={}", name, args);
    let result: Result<String, String> = match name {
        "get_stats" => {
            get_stats_inner(&state).map(|v| v.to_string())
        }
        "list_devices" => {
            let parsed: std::collections::HashMap<String, serde_json::Value> =
                serde_json::from_str(args).unwrap_or_default();
            let vendor = parsed.get("vendor").and_then(|v| v.as_str().map(|s| s.to_string()));
            let device_type = parsed.get("device_type").and_then(|v| v.as_str().map(|s| s.to_string()));
            let status = parsed.get("status").and_then(|v| v.as_str().map(|s| s.to_string()));
            match commands::devices::list_devices(vendor, device_type, status, state) {
                Ok(devices) => {
                    let simplified: Vec<serde_json::Value> = devices.into_iter().map(|d| {
                        serde_json::json!({
                            "id": d.id, "name": d.name, "ip": d.ip,
                            "device_type": d.device_type, "vendor": d.vendor, "status": d.status,
                        })
                    }).collect();
                    Ok(serde_json::to_string(&simplified).unwrap_or_default())
                }
                Err(e) => Err(e),
            }
        }
        "check_device_status" => {
            let parsed: std::collections::HashMap<String, serde_json::Value> =
                serde_json::from_str(args).unwrap_or_default();
            let device_id = parsed.get("device_id")
                .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
                .unwrap_or(0);
            match commands::devices::check_device_status(device_id, state.clone()).await {
                Ok(v) => Ok(v.to_string()),
                Err(e) => Err(e),
            }
        }
        "check_all_devices_status" => {
            match commands::devices::check_all_devices_status(state.clone()).await {
                Ok(v) => Ok(v.to_string()),
                Err(e) => Err(e),
            }
        }
        "scan_live_hosts" => {
            let parsed: std::collections::HashMap<String, serde_json::Value> =
                serde_json::from_str(args).unwrap_or_default();
            let subnet = parsed.get("subnet")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let timeout_ms = parsed.get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(3000);
            match commands::tools::scan_live_hosts(
                app_handle.clone(), subnet, timeout_ms,
            ).await {
                Ok(results) => Ok(serde_json::to_string(&results).unwrap_or_default()),
                Err(e) => Err(e),
            }
        }
        "update_device" => {
            let parsed: std::collections::HashMap<String, serde_json::Value> =
                serde_json::from_str(args).unwrap_or_default();
            let device_id = parsed.get("device_id")
                .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
                .unwrap_or(0);
            let data = crate::db::models::DeviceUpdate {
                name: parsed.get("name").and_then(|v| v.as_str().map(|s| s.to_string())),
                ip: parsed.get("ip").and_then(|v| v.as_str().map(|s| s.to_string())),
                device_type: parsed.get("device_type").and_then(|v| v.as_str().map(|s| s.to_string())),
                vendor: parsed.get("vendor").and_then(|v| v.as_str().map(|s| s.to_string())),
                model: None,
                ssh_username: None,
                ssh_password_encrypted: None,
                ssh_port: None,
                template_id: None,
                status: None,
                last_checked_at: None,
                serial_number: None,
                manufacturing_date: None,
                sysname: None,
                cpu_cores: None,
                memory_gb: None,
                deployment: None,
                db_version: None,
                instance_name: None,
                db_username: None,
                db_password_encrypted: None,
                db_port: None,
                kernel_version: None,
            };
            tracing::info!("execute_tool update_device: device_id={}, name={:?}, parsed_args={}", device_id, data.name, args);
            match commands::devices::update_device(device_id, data, state).await {
                Ok(d) => {
                    tracing::info!("execute_tool update_device 成功: id={}, name={}", d.id, d.name);
                    Ok(serde_json::json!({
                        "id": d.id, "name": d.name, "ip": d.ip, "device_type": d.device_type, "vendor": d.vendor, "status": d.status,
                    }).to_string())
                }
                Err(e) => {
                    tracing::error!("execute_tool update_device 失败: {}", e);
                    Err(e)
                }
            }
        }
        "create_device" => {
            let parsed: std::collections::HashMap<String, serde_json::Value> =
                serde_json::from_str(args).unwrap_or_default();
            let name = parsed.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let ip = parsed.get("ip").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let device_type = parsed.get("device_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let vendor = parsed.get("vendor").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let data = crate::db::models::DeviceCreate {
                name, ip, device_type, vendor,
                model: None,
                ssh_username: None,
                ssh_password_encrypted: None,
                ssh_port: None,
                template_id: None,
                status: None,
                last_checked_at: None,
                serial_number: None,
                manufacturing_date: None,
                sysname: None,
                cpu_cores: None,
                memory_gb: None,
                deployment: None,
                db_version: None,
                instance_name: None,
                db_username: None,
                db_password_encrypted: None,
                db_port: None,
                kernel_version: None,
            };
            match commands::devices::create_device(data, state).await {
                Ok(d) => Ok(serde_json::json!({
                    "id": d.id, "name": d.name, "ip": d.ip, "device_type": d.device_type, "vendor": d.vendor, "status": d.status,
                }).to_string()),
                Err(e) => Err(e),
            }
        }
        "list_templates" => {
            match commands::templates::list_templates(None, state) {
                Ok(templates) => {
                    let simplified: Vec<serde_json::Value> = templates.into_iter().map(|t| {
                        serde_json::json!({
                            "id": t.get("id"), "name": t.get("name"), "vendor": t.get("vendor")
                        })
                    }).collect();
                    Ok(serde_json::to_string(&simplified).unwrap_or_default())
                }
                Err(e) => Err(e),
            }
        }
        "update_template" => {
            let parsed: std::collections::HashMap<String, serde_json::Value> =
                serde_json::from_str(args).unwrap_or_default();
            let template_id = parsed.get("template_id")
                .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
                .unwrap_or(0);
            let data = crate::db::models::TemplateUpdate {
                name: parsed.get("name").and_then(|v| v.as_str().map(|s| s.to_string())),
                vendor: parsed.get("vendor").and_then(|v| v.as_str().map(|s| s.to_string())),
                device_type: parsed.get("device_type").and_then(|v| v.as_str().map(|s| s.to_string())),
                description: parsed.get("description").and_then(|v| v.as_str().map(|s| s.to_string())),
                model: None,
                config: None,
                report_template_id: None,
                template_type: None,
            };
            tracing::info!("update_template: id={}, name={:?}", template_id, data.name);
            match commands::templates::update_template(template_id, data, state) {
                Ok(t) => Ok(serde_json::json!({
                    "id": t.id, "name": t.name, "vendor": t.vendor, "device_type": t.device_type
                }).to_string()),
                Err(e) => Err(e),
            }
        }
        "list_batches" => {
            let parsed: std::collections::HashMap<String, serde_json::Value> =
                serde_json::from_str(args).unwrap_or_default();
            let status = parsed.get("status").and_then(|v| v.as_str().map(|s| s.to_string()));
            match commands::inspections::list_batches(status, state) {
                Ok(batches) => Ok(serde_json::to_string(&batches).unwrap_or_default()),
                Err(e) => Err(e),
            }
        }
        "run_batch" => {
            let parsed: std::collections::HashMap<String, serde_json::Value> =
                serde_json::from_str(args).unwrap_or_default();
            let batch_id = parsed.get("batch_id")
                .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
                .unwrap_or(0);
            match commands::inspections::run_batch(batch_id, state.clone()).await {
                Ok(()) => Ok(r#"{"status":"started","message":"巡检任务已启动，正在后台执行"}"#.to_string()),
                Err(e) => Err(e),
            }
        }
        "analyze_batch" => {
            let parsed: std::collections::HashMap<String, serde_json::Value> =
                serde_json::from_str(args).unwrap_or_default();
            let batch_id = parsed.get("batch_id")
                .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
                .unwrap_or(0);
            let force = parsed.get("force").and_then(|v| v.as_bool());
            match commands::reports::analyze_batch(batch_id, force, state.clone()).await {
                Ok(v) => Ok(v.to_string()),
                Err(e) => Err(e),
            }
        }
        _ => Err(format!("未知工具: {}", name)),
    };

    match result {
        Ok(r) => r,
        Err(e) => {
            let err = format!(r#"{{"error": "{}"}}"#, e.replace('"', "\\\""));
            tracing::warn!("工具执行失败: name={}, error={}", name, e);
            err
        }
    }
}

/// get_stats 内部函数（供工具调用和原有 #[tauri::command] 共用）
fn get_stats_inner(state: &tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock();
    let (device_count, online_count, offline_count, template_count, command_count,
         batch_count, pending_batch_count, completed_batch_count,
         network_device_count, security_device_count, server_count, database_count, other_device_count, report_count) = db
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
                (SELECT COUNT(*) FROM devices WHERE device_type NOT IN ('switch','router','firewall','loadbalancer','server','database')), \
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
                    r.get::<_, i64>(13)?,
                ))
            },
        )
        .map_err(|e| format!("统计查询失败: {}", e))?;

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
        "other_device_count": other_device_count,
        "report_count": report_count,
    }))
}

/// 对话模式：发送消息到 AI 并返回回复，支持 tool calling
#[tauri::command]
async fn chat_with_ai(
    app_handle: tauri::AppHandle,
    config_id: Option<i64>,
    system_prompt: String,
    messages: Vec<serde_json::Value>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    // 获取指定的或激活的 AI 配置
    let (api_key, model, base_url) = {
        let db = state.db.lock();
        if let Some(cid) = config_id {
            db.query_row(
                "SELECT api_key_encrypted, model_id, base_url FROM ai_model_configs WHERE id = ?1",
                [cid],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
            )
            .map_err(|_| format!("未找到 ID 为 {} 的 AI 配置", cid))?
        } else {
            db.query_row(
                "SELECT api_key_encrypted, model_id, base_url FROM ai_model_configs WHERE is_active = 1 LIMIT 1",
                [],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
            )
            .map_err(|_| "未找到激活的 AI 配置，请先在系统设置中配置并激活一个 AI 模型".to_string())?
        }
    };

    let decrypted_key = crate::services::crypto::CryptoService::decrypt(&api_key)
        .map_err(|e| format!("解密 API Key 失败: {}", e))?;

    let url = crate::services::ai_inspection::build_chat_url(&base_url);
    let tools = build_tools();

    // 构建消息数组
    let mut api_messages = vec![serde_json::json!({"role": "system", "content": system_prompt})];
    api_messages.extend(messages);

    let client = crate::services::ai_inspection::get_client();
    let max_rounds = 5;

    // 记录用户自然语言问题
    if let Some(first) = api_messages.iter().find(|m| m["role"] == "user") {
        if let Some(text) = first["content"].as_str() {
            let truncated: String = text.chars().take(200).collect();
            tracing::info!("[AI聊天] 用户问题: {}", truncated);
        }
    }

    for round in 0..max_rounds {
        tracing::info!("[AI聊天] 第 {} 轮请求", round + 1);

        let mut body = serde_json::json!({
            "model": model,
            "messages": api_messages,
            "temperature": 0.7,
            "max_tokens": 4096,
        });
        // 每轮都带上工具定义，允许 AI 在收到 tool 结果后继续调用新工具
        body["tools"] = serde_json::Value::Array(tools.clone());
        body["tool_choice"] = serde_json::json!("auto");

        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", decrypted_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("AI 请求失败: {}", e))?;

        let status = response.status();
        let response_text = response.text().await
            .map_err(|e| format!("读取 AI 响应失败: {}", e))?;

        if !status.is_success() {
            let snippet: String = response_text.chars().take(200).collect();
            return Err(format!("AI 返回错误 ({}): {}", status, snippet));
        }

        let json: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| format!("解析 AI 响应失败: {}", e))?;

        let choice = &json["choices"][0];
        let message = &choice["message"];
        let finish_reason = choice["finish_reason"].as_str().unwrap_or("");
        let has_tool_calls = message.get("tool_calls").is_some();

        tracing::debug!(
            "chat_with_ai 响应: round={}, finish_reason={}, has_tool_calls={}, content={}",
            round + 1, finish_reason, has_tool_calls,
            message["content"].as_str().unwrap_or("").chars().take(50).collect::<String>()
        );

        // 记录 API 响应头几行用于诊断
        if has_tool_calls {
            let tc_count = message["tool_calls"].as_array().map(|a| a.len()).unwrap_or(0);
            tracing::info!("[AI聊天] 返回 tool_calls: count={}", tc_count);
            for (ti, tc) in message["tool_calls"].as_array().unwrap_or(&vec![]).iter().enumerate() {
                tracing::info!("[AI聊天]   tool_call[{}]: name={}, args={}",
                    ti,
                    tc["function"]["name"].as_str().unwrap_or(""),
                    tc["function"]["arguments"].as_str().unwrap_or("")
                );
            }
        } else {
            tracing::info!("[AI聊天] 文本回复: finish_reason={}, text={}", finish_reason,
                message["content"].as_str().unwrap_or("").chars().take(100).collect::<String>()
            );
        }

        if finish_reason == "tool_calls" {
            let tool_calls = message["tool_calls"].as_array()
                .cloned()
                .unwrap_or_default();

            // 添加 assistant 的 tool_calls 消息到历史
            api_messages.push(message.clone());

            for tc in &tool_calls {
                let id = tc["id"].as_str().unwrap_or("");
                let func_name = tc["function"]["name"].as_str().unwrap_or("");
                let func_args = tc["function"]["arguments"].as_str().unwrap_or("");
                let result = execute_tool(func_name, func_args, state.clone(), &app_handle).await;
                api_messages.push(serde_json::json!({
                    "tool_call_id": id,
                    "role": "tool",
                    "content": result,
                }));
            }
            continue;
        }

        // finish_reason: stop → 正常；length → 截断；content_filter → 被过滤
        if finish_reason == "content_filter" {
            return Err("AI 回复被内容过滤拦截，请修改输入后重试".to_string());
        }
        if finish_reason == "length" {
            tracing::warn!("[AI聊天] 响应被截断 (max_tokens 不足)");
        }
        let content = message["content"]
            .as_str()
            .unwrap_or("AI 未返回有效回复")
            .to_string();
        return Ok(content);
    }

    Ok("AI 对话已达到最大轮次，请简化请求或重试".to_string())
}

fn poll_device_statuses(db: &Arc<parking_lot::Mutex<rusqlite::Connection>>) {
    // Phase 1: read id/ip/port + model/sysname/status — model/sysname 用于跳过冗余 SSH，
    // status 用于判断是否变更（仅变更时写状态日志）
    #[allow(clippy::type_complexity)]
    let devices: Vec<(i64, String, i64, String)> = {
        let conn = db.lock();
        let mut stmt = match conn.prepare(
            "SELECT id, ip, ssh_port, status FROM devices",
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
                    row.get::<_, String>(3)?,
                ))
            })
            .ok()
            .into_iter()
            .flat_map(|mapped| mapped.filter_map(|r| r.ok()))
            .collect();
        rows
    };

    if devices.is_empty() {
        return;
    }
    tracing::info!("[poll-full] 全量检测 {} 台设备", devices.len());

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let online_count = std::sync::atomic::AtomicU32::new(0);
    let offline_count = std::sync::atomic::AtomicU32::new(0);
    // 收集 (id, new_status, old_status)，scope 结束后一次性持锁批量更新，避免 try_lock 丢更新
    let results: Mutex<Vec<(i64, String, String)>> = Mutex::new(Vec::new());

    // 分批并发：每批 50 台，避免一次性创建过多 OS 线程
    std::thread::scope(|s| {
        for chunk in devices.chunks(50) {
            for (id, ip, port, old_status) in chunk {
            let online_ref = &online_count;
            let offline_ref = &offline_count;
            let results = &results;
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
            });
        }
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
