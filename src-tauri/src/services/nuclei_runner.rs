use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

/// Windows 下隐藏子进程控制台窗口
#[cfg(windows)]
fn hide_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
}
#[cfg(not(windows))]
fn hide_window(_cmd: &mut Command) {}

/// 创建一个隐藏控制台的 nuclei 进程
fn nuclei_cmd() -> Command {
    let mut cmd = std::process::Command::new(nuclei_bin());
    hide_window(&mut cmd);
    cmd
}

const CVE_SERVER: &str = "http://192.168.9.72:18080";

fn app_dir() -> PathBuf {
    crate::APP_DATA_DIR
        .get()
        .map(|p| p.join("tools"))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn nuclei_bin() -> PathBuf {
    #[cfg(windows)] { app_dir().join("nuclei.exe") }
    #[cfg(not(windows))] { app_dir().join("nuclei") }
}

fn templates_dir() -> PathBuf {
    app_dir().join("nuclei-templates")
}

pub fn is_nuclei_ready() -> bool {
    let ready = nuclei_bin().exists();
    tracing::info!("nuclei 状态检查: binary={}, ready={}", nuclei_bin().display(), ready);
    ready
}

pub fn get_nuclei_info() -> serde_json::Value {
    serde_json::json!({
        "installed": is_nuclei_ready(),
        "bin": nuclei_bin().to_string_lossy(),
        "templates": templates_dir().to_string_lossy(),
    })
}

/// 下载并安装 nuclei + 模板
pub async fn download_nuclei(
    progress: Arc<std::sync::Mutex<dyn Fn(u64, u64) + Send>>,
) -> Result<(), String> {
    let dir = app_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .no_proxy()
        .build()
        .map_err(|e| format!("HTTP 客户端: {}", e))?;

    #[cfg(windows)]
    let nuclei_url = format!("{}/api/v1/download/nuclei_windows.zip", CVE_SERVER);
    #[cfg(not(windows))]
    let nuclei_url = format!("{}/api/v1/download/nuclei_linux.zip", CVE_SERVER);

    // 下载 nuclei 二进制
    let bin_name = nuclei_bin().file_name().unwrap_or_default().to_string_lossy().to_string();
    download_zip(&client, &nuclei_url, &dir, std::path::Path::new(&bin_name), &progress).await?;
    // 下载模板
    let tmpl_url = format!("{}/api/v1/download/templates.zip", CVE_SERVER);
    download_zip(&client, &tmpl_url, &dir, "nuclei-templates".as_ref(), &progress).await?;

    // 重命名 nuclei-templates-main → nuclei-templates
    let src = dir.join("nuclei-templates-main");
    if src.exists() {
        let _ = std::fs::remove_dir_all(templates_dir());
        let _ = std::fs::rename(&src, templates_dir());
    }

    // 授权
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(m) = std::fs::metadata(nuclei_bin()) {
            let mut p = m.permissions();
            p.set_mode(0o755);
            let _ = std::fs::set_permissions(nuclei_bin(), p);
        }
    }

    let p = progress.lock().unwrap();
    p(1, 1);
    Ok(())
}

/// 下载 zip 并提取指定文件
async fn download_zip(
    client: &reqwest::Client,
    url: &str,
    dest: &PathBuf,
    target_name: &std::path::Path,
    progress: &Arc<std::sync::Mutex<dyn Fn(u64, u64) + Send>>,
) -> Result<(), String> {
    let resp = client.get(url).send().await
        .map_err(|e| format!("下载失败: {}", e))?;
    let total = resp.content_length().unwrap_or(0);
    let bytes = resp.bytes().await.map_err(|e| format!("读数据失败: {}", e))?;

    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| format!("解析 zip 失败: {}", e))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("读 zip 项失败: {}", e))?;
        let name = entry.name().to_string();
        let out_path = dest.join(&name);

        if entry.is_dir() {
            let _ = std::fs::create_dir_all(&out_path);
            continue;
        }

        // 只提取我们需要的内容
        if name.contains(target_name.to_str().unwrap_or("")) || name.contains("nuclei") {
            if let Some(parent) = out_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut out = std::fs::File::create(&out_path)
                .map_err(|e| format!("创建文件失败 {}: {}", name, e))?;
            std::io::copy(&mut entry, &mut out)
                .map_err(|e| format!("写入文件失败 {}: {}", name, e))?;
        }
    }

    let p = progress.lock().unwrap();
    p(bytes.len() as u64, total);
    Ok(())
}

/// 运行 nuclei 扫描
pub fn scan_target(target: &str, port: u16, service: &str) -> Result<Vec<serde_json::Value>, String> {
    if !is_nuclei_ready() {
        return Err("nuclei 未安装".to_string());
    }

    let tgt = if port > 0 { format!("{}:{}", target, port) } else { target.to_string() };
    let tmpl_path = templates_dir().join("http").join("cves");
    tracing::info!("nuclei 启动扫描: target={}, service={}, templates={}", tgt, service, tmpl_path.display());

    // 检查模板目录是否存在
    if !tmpl_path.exists() {
        tracing::warn!("nuclei 模板目录不存在: {}，尝试 http/cves", tmpl_path.display());
        // 在 v3.11 中 CVE 模板在 http/cves/ 和 network/cves/ 下
        let alt_path = templates_dir().join("http").join("cves");
        let tmpl = if alt_path.exists() { alt_path } else { templates_dir() };
        let cmd_str = format!("{} -u {} -j -timeout 10 -t {}", nuclei_bin().display(), tgt, tmpl.display());
        tracing::info!("nuclei 命令: {}", cmd_str);

        let output = nuclei_cmd()
            .args(["-u", &tgt, "-j", "-timeout", "10", "-t", &tmpl.to_string_lossy()])
            .output()
            .map_err(|e| format!("nuclei 执行失败: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::info!("nuclei 完成: exit={}, stdout_lines={}, stderr={}", output.status, stdout.lines().count(), stderr.trim());

        let mut results = Vec::new();
        for line in stdout.lines() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                results.push(v);
            }
        }
        return Ok(results);
    }

    let cmd_str = format!("{} -u {} -j -timeout 10 -t {}", nuclei_bin().display(), tgt, tmpl_path.display());
    tracing::info!("nuclei 命令: {}", cmd_str);

    let output = nuclei_cmd()
        .args(["-u", &tgt, "-j", "-timeout", "10", "-t", &tmpl_path.to_string_lossy()])
        .output()
        .map_err(|e| format!("nuclei 执行失败: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    tracing::info!("nuclei 完成: exit={}, stdout_lines={}, stderr={}", output.status, stdout.lines().count(), stderr.trim());

    let mut results = Vec::new();
    for line in stdout.lines() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            results.push(v);
        }
    }
    tracing::info!("nuclei 发现 {} 个漏洞", results.len());
    Ok(results)
}
