// 生产环境隐藏控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // ⚠️ panic hook 必须是第一个操作！
    // 下面任何代码崩溃都会触发它弹 MessageBox + 写日志。
    // 如果 hook 装得晚、前面代码崩了，默认 handler 在 windows_subsystem="windows"
    // + panic="abort" 下无声消失——无日志无弹窗，用户什么都看不到。
    std::panic::set_hook(Box::new(|info| {
        let msg = info.payload_as_str().unwrap_or("未知 panic");
        let location = info.location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "未知位置".to_string());
        let backtrace = std::backtrace::Backtrace::force_capture();
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let log_line = format!("[{}] PANIC: {} @ {}\n{}\n", ts, msg, location, backtrace);

        // 写入 debug 日志
        let temp_log = std::env::temp_dir().join("inspection-debug.log");
        let _ = std::fs::OpenOptions::new().create(true).append(true).open(&temp_log)
            .and_then(|mut f| { use std::io::Write; f.write_all(log_line.as_bytes()) });

        // 写入 startup.log
        let log_path = inspection_rust_lib::startup_log_path();
        let _ = std::fs::OpenOptions::new()
            .create(true).append(true).open(&log_path)
            .and_then(|mut f| { use std::io::Write; f.write_all(log_line.as_bytes()) });

        // 弹错误对话框（Windows 上无声 crash 用户完全不知道）
        #[cfg(target_os = "windows")]
        {
            extern "system" {
                fn MessageBoxW(hWnd: *const core::ffi::c_void, lpText: *const u16, lpCaption: *const u16, uType: u32) -> i32;
            }
            let dialog_text = format!(
                "程序发生致命错误:\n\n{}\n\n位置: {}\n\n详细日志已写入:\n{}\n{}",
                msg, location, temp_log.display(), log_path.display()
            );
            let text_utf16: Vec<u16> = dialog_text.encode_utf16().chain(std::iter::once(0)).collect();
            let title: Vec<u16> = "AI巡检助手 - 致命错误".encode_utf16().chain(std::iter::once(0)).collect();
            unsafe { MessageBoxW(std::ptr::null(), text_utf16.as_ptr(), title.as_ptr(), 0x10); }
        }
    }));

    // 早期日志：确认 main() 已执行（panic hook 已就位）
    let temp_log_path = std::env::temp_dir().join("inspection-debug.log");
    let _ = std::fs::write(&temp_log_path, ""); // 清理旧日志
    let _ = std::fs::OpenOptions::new().create(true).append(true).open(&temp_log_path)
        .and_then(|mut f| { use std::io::Write; let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S"); writeln!(f, "[{}] main() 开始执行", ts) });

    // 主动禁用 WebView2 GPU 加速，走纯软件渲染（WARP）。
    // 精简版/无显卡/驱动缺失的 Windows 上，硬件 D3D11/DWM 合成会崩溃。
    // 巡检工具是静态页面，软件渲染性能完全足够，兼容性远高于硬件加速。
    std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", "--disable-gpu");

    // panic = "abort" 下 catch_unwind 无效，直接调用 run()
    // panic hook 已在上方安装，会将 panic 信息写入日志文件
    inspection_rust_lib::run();
}
