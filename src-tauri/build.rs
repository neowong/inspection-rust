use std::path::PathBuf;

fn get_git_branch() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !branch.is_empty() && branch != "HEAD" { Some(branch) } else { None }
    } else {
        None
    }
}

fn main() {
    tauri_build::build();

    // 自动检测更新渠道：CI 设置了 INSPECTION_CHANNEL 则尊重，
    // 否则按 Git 分支决定（master/main → master，其他 → internal）
    if std::env::var("INSPECTION_CHANNEL").is_err() {
        if let Some(branch) = get_git_branch() {
            let channel = if branch == "master" || branch == "main" { "master" } else { "internal" };
            println!("cargo:rustc-env=INSPECTION_CHANNEL={}", channel);
            println!("cargo:warning=INSPECTION_CHANNEL=auto-detected as '{channel}' from branch '{branch}'");
        } else {
            println!("cargo:rustc-env=INSPECTION_CHANNEL=master");
        }
    }

    // Copy WebView2Loader.dll next to the output binary on Windows.
    // Windows PE loader requires this DLL BEFORE main() runs, so it
    // must be a separate file — embedding won't work.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let dll_src = PathBuf::from("WebView2Loader.dll");
        if dll_src.exists() {
            // Walk from OUT_DIR to the build profile directory
            // OUT_DIR = .../target/<triple>/<profile>/build/<pkg>/out
            let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
            let profile_dir = out
                .parent()  // build/<pkg>/
                .and_then(|p| p.parent())  // build/
                .and_then(|p| p.parent()); // <profile>/
            if let Some(dir) = profile_dir {
                let dll_dst = dir.join("WebView2Loader.dll");
                if let Err(e) = std::fs::copy(&dll_src, &dll_dst) {
                    eprintln!("copy WebView2Loader.dll failed: {}", e);
                } else {
                    println!("cargo:warning=Copied WebView2Loader.dll to {}", dll_dst.display());
                }
            }
        }
    }
}
