// 临时启用控制台窗口，排查闪退问题
// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 超早期日志：写到临时目录，确认程序是否能启动
    let temp = std::env::temp_dir().join("inspection-debug.log");
    {
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let msg = format!("[{}] main() 开始执行\n", ts);
        let _ = std::fs::write(&temp, msg);
    }

    // 安装 panic hook，将 panic 信息写到 startup.log 以便排查闪退
    std::panic::set_hook(Box::new(|info| {
        let msg = info.payload_as_str().unwrap_or("未知 panic");
        let location = info.location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "未知位置".to_string());
        let backtrace = std::backtrace::Backtrace::force_capture();
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let log_line = format!("[{}] PANIC: {} @ {}\n{}\n", ts, msg, location, backtrace);

        // 写到多个位置确保能找到
        let temp = std::env::temp_dir().join("inspection-debug.log");
        let _ = std::fs::OpenOptions::new().create(true).append(true).open(&temp)
            .and_then(|mut f| { use std::io::Write; f.write_all(log_line.as_bytes()) });

        // 写到 startup.log（与 lib.rs 相同的路径策略）
        let log_path = inspection_rust_lib::startup_log_path();
        let _ = std::fs::OpenOptions::new()
            .create(true).append(true).open(&log_path)
            .and_then(|mut f| { use std::io::Write; f.write_all(log_line.as_bytes()) });
    }));

    // 用 catch_unwind 捕获任何未处理的错误
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        inspection_rust_lib::run();
    }));

    if let Err(e) = result {
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let msg = format!("[{}] 捕获到错误: {:?}\n", ts, e);
        let _ = std::fs::OpenOptions::new().create(true).append(true).open(&temp)
            .and_then(|mut f| { use std::io::Write; f.write_all(msg.as_bytes()) });

        // 尝试显示错误对话框
        #[cfg(target_os = "windows")]
        {
            extern "system" {
                fn MessageBoxW(hWnd: *const core::ffi::c_void, lpText: *const u16, lpCaption: *const u16, uType: u32) -> i32;
            }
            let err_msg = format!("程序发生错误，请查看日志：\n{}", temp.display());
            let msg_utf16: Vec<u16> = err_msg.encode_utf16().chain(std::iter::once(0)).collect();
            let title: Vec<u16> = "AI巡检助手 - 错误".encode_utf16().chain(std::iter::once(0)).collect();
            unsafe { MessageBoxW(std::ptr::null(), msg_utf16.as_ptr(), title.as_ptr(), 0x10); }
        }
    }
}
