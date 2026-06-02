use std::path::PathBuf;

fn main() {
    tauri_build::build();

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
