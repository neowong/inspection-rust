// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 安装 panic hook，将 panic 信息写到 startup.log 以便排查闪退
    std::panic::set_hook(Box::new(|info| {
        let msg = info.payload_as_str().unwrap_or("未知 panic");
        let location = info.location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "未知位置".to_string());
        let backtrace = std::backtrace::Backtrace::force_capture();
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let log_line = format!("[{}] PANIC: {} @ {}\n{}\n", ts, msg, location, backtrace);

        // 写到 startup.log（与 lib.rs 相同的路径策略）
        let log_path = inspection_rust_lib::startup_log_path();
        let _ = std::fs::OpenOptions::new()
            .create(true).append(true).open(&log_path)
            .and_then(|mut f| { use std::io::Write; f.write_all(log_line.as_bytes()) });
    }));

    inspection_rust_lib::run();
}
