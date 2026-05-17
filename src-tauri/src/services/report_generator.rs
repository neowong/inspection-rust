/// 巡检报告生成服务
///
/// 对应 Python: backend/app/services/report_generator.py
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// 服务器/系统类厂商（使用 Linux shell 提示符）
const SERVER_VENDORS: &[&str] = &["ubuntu", "centos", "rhel", "debian", "linux", "openeuler", "redhat", "fedora", "suse"];

/// 数据库客户端默认提示符
fn db_prompt(db_type: &str) -> &str {
    match db_type {
        "mysql" => "mysql> ",
        "postgresql" => "db=# ",
        "oracle" => "SQL> ",
        _ => "",
    }
}

/// SQL 命令关键字
const SQL_STARTS: &[&str] = &["SELECT", "SHOW", "DESCRIBE", "DESC", "INSERT", "UPDATE",
    "DELETE", "CREATE", "ALTER", "DROP", "USE", "GRANT", "REVOKE", "SET",
    "FLUSH", "EXPLAIN", "WITH", "CALL", "EXEC", "MERGE"];

fn is_sql_command(cmd: &str) -> bool {
    let first = cmd.trim().split_whitespace().next().unwrap_or("").to_uppercase();
    SQL_STARTS.contains(&first.as_str())
}

fn is_server_vendor(vendor: &str) -> bool {
    SERVER_VENDORS.contains(&vendor.to_lowercase().as_str())
}

/// 从命令输出中提取 hostname
pub fn parse_hostname(command_outputs: &HashMap<String, String>) -> String {
    // 1. Linux hostname 命令
    for (cmd_key, output) in command_outputs {
        if cmd_key == "hostname" {
            for line in output.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.contains(' ') && line.len() < 100 {
                    return line.to_string();
                }
            }
        }
        // 网络设备: sysname / hostname in config
        if cmd_key.contains("sysname") || cmd_key.contains("hostname") {
            for line in output.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("sysname ") {
                    return rest.trim().to_string();
                }
                if let Some(rest) = line.strip_prefix("hostname ") {
                    return rest.trim().to_string();
                }
                let words: Vec<&str> = line.split_whitespace().collect();
                if words.len() == 1 && words[0].chars().all(|c| c.is_ascii()) {
                    return words[0].to_string();
                }
            }
        }
    }

    // 2. 从提示符提取: <hostname> 或 [hostname] 或 user@host
    for output in command_outputs.values() {
        for line in output.lines() {
            let line = line.trim();
            if let Some(inner) = line.strip_prefix('<').and_then(|s| s.strip_suffix('>')) {
                if !inner.is_empty() && !inner.chars().all(|c| c.is_digit(10)) { return inner.to_string(); }
            }
            if let Some(inner) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                let name = inner.split('(').next().unwrap_or(inner);
                if !name.is_empty() && !name.chars().all(|c| c.is_digit(10)) { return name.to_string(); }
            }
            if let Some(at_pos) = line.find('@') {
                let before = &line[..at_pos];
                let after_host = &line[at_pos+1..];
                if let Some(colon) = after_host.find(':') { return after_host[..colon].to_string(); }
                if let Some(space) = after_host.find(|c: char| c == '$' || c == '#') {
                    return after_host[..space].to_string();
                }
                if after_host.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '.') {
                    return after_host.to_string();
                }
            }
        }
    }
    String::new()
}

/// 提取 OS 版本
pub fn parse_os_release(command_outputs: &HashMap<String, String>) -> String {
    for (cmd_key, output) in command_outputs {
        if cmd_key.contains("os-release") || cmd_key.contains("redhat-release") {
            for line in output.lines() {
                if let Some(val) = line.strip_prefix("PRETTY_NAME=") {
                    return val.trim().trim_matches('"').to_string();
                }
            }
            let first = output.lines().next().unwrap_or("").trim();
            if !first.is_empty() && !first.starts_with("cat:") { return first[..std::cmp::min(50, first.len())].to_string(); }
        }
    }
    String::new()
}

/// 提取内核版本
pub fn parse_kernel(command_outputs: &HashMap<String, String>) -> String {
    for (cmd_key, output) in command_outputs {
        if cmd_key.contains("uname") {
            let parts: Vec<&str> = output.trim().split_whitespace().collect();
            if parts.len() >= 3 { return parts[2].to_string(); }
            return output.trim()[..std::cmp::min(60, output.trim().len())].to_string();
        }
    }
    String::new()
}

/// 提取 CPU 规格
pub fn parse_cpu_cores(command_outputs: &HashMap<String, String>) -> String {
    for (cmd_key, output) in command_outputs {
        if cmd_key.contains("lscpu") {
            for line in output.lines() {
                let s = line.trim();
                if let Some(rest) = s.strip_prefix("CPU(s):").or_else(|| s.strip_prefix("CPU：")) {
                    if let Ok(n) = rest.trim().parse::<i32>() { return format!("{}C", n); }
                }
            }
        }
    }
    String::new()
}

/// 提取内存规格
pub fn parse_mem_total(command_outputs: &HashMap<String, String>) -> String {
    for (cmd_key, output) in command_outputs {
        if cmd_key.contains("free") && cmd_key.contains("-h") {
            for line in output.lines() {
                let s = line.trim();
                if s.starts_with("Mem:") || s.contains("内存") || s.starts_with("Mem") {
                    for word in s.split_whitespace() {
                        let num_str = word.trim_end_matches(|c: char| !c.is_ascii_digit() && c != '.');
                        if !num_str.is_empty() {
                            if let Ok(n) = num_str.parse::<f64>() {
                                let unit = word[num_str.len()..].trim().to_uppercase();
                                if unit.starts_with('G') { return format!("{}G", round_mem_gb(n as i32)); }
                                if unit.starts_with('M') {
                                    let gb = (n / 1024.0).round();
                                    if gb >= 1.0 { return format!("{}G", round_mem_gb(gb as i32)); }
                                    return format!("{}M", n as i32);
                                }
                                return word.to_string();
                            }
                        }
                    }
                }
            }
        }
    }
    String::new()
}

fn round_mem_gb(gb: i32) -> i32 {
    if gb % 2 == 1 { gb + 1 } else { gb }
}

/// 提取出厂日期
pub fn parse_manufacturing_date(command_outputs: &HashMap<String, String>) -> String {
    for (cmd_key, output) in command_outputs {
        if cmd_key.to_lowercase().contains("manuinfo") || cmd_key.to_lowercase().contains("inventory") {
            for line in output.lines() {
                let lower = line.to_lowercase();
                if lower.contains("date") && (lower.contains("manufact") || lower.contains("生产") || lower.contains("mfg")) {
                    if let Some((_, val)) = line.split_once(':') { return val.trim().to_string(); }
                }
                if lower.contains("manufact") || lower.contains("mfg_date") {
                    if let Some((_, val)) = line.split_once(':') { return val.trim().to_string(); }
                }
            }
        }
    }
    String::new()
}

/// 提取设备型号
pub fn parse_device_model(command_outputs: &HashMap<String, String>) -> String {
    for line in command_outputs.get("display device manuinfo").unwrap_or(&String::new()).lines() {
        if let Some(val) = line.strip_prefix("DEVICE_NAME") {
            return val.split_once(':').map(|(_, v)| v.trim()).unwrap_or(line).to_string();
        }
    }
    String::new()
}

/// 提取设备序列号
pub fn parse_device_sn(command_outputs: &HashMap<String, String>) -> String {
    // 1. display device manuinfo
    if let Some(manuinfo) = command_outputs.get("display device manuinfo") {
        for line in manuinfo.lines() {
            if let Some(val) = line.trim().strip_prefix("DEVICE_SERIAL_NUMBER") {
                return val.split_once(':').map(|(_, v)| v.trim()).unwrap_or("").to_string();
            }
        }
    }

    // 2. display esn (华为)
    if let Some(esn) = command_outputs.get("display esn") {
        for line in esn.lines() {
            let upper = line.to_uppercase();
            if upper.contains("SN") || upper.contains("ESN") {
                if let Some((_, val)) = line.split_once(':') { return val.trim().to_string(); }
            }
        }
    }

    // 3. show inventory
    if let Some(inv) = command_outputs.get("show inventory") {
        for line in inv.lines() {
            if let Some(rest) = line.to_lowercase().strip_prefix("sn:") {
                return rest.trim().to_string();
            }
        }
    }

    String::new()
}

/// 构建报告模板变量上下文
pub fn build_report_context(
    record: &HashMap<String, String>,
    device: &HashMap<String, String>,
) -> HashMap<String, serde_json::Value> {
    let command_outputs: HashMap<String, serde_json::Value> = record
        .get("command_outputs")
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let command_judgments: HashMap<String, serde_json::Value> = record
        .get("command_judgments")
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let summary = record.get("summary_judgment").cloned().unwrap_or_default();
    let vendor = device.get("vendor").cloned().unwrap_or_default();
    let device_name = device.get("name").cloned().unwrap_or_default();
    let ip = device.get("ip").cloned().unwrap_or_default();

    // Convert to HashMap<String,String> for parsers
    let outputs_str: HashMap<String, String> = command_outputs.iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
        .collect();

    let hostname = parse_hostname(&outputs_str);
    let model = parse_device_model(&outputs_str);
    let sn = parse_device_sn(&outputs_str);

    let mut ctx = HashMap::new();
    ctx.insert("device_name".into(), serde_json::json!(device_name));
    ctx.insert("device_ip".into(), serde_json::json!(ip));
    ctx.insert("device_vendor".into(), serde_json::json!(vendor));
    ctx.insert("device_model".into(), serde_json::json!(model));
    ctx.insert("device_sn".into(), serde_json::json!(sn));
    ctx.insert("device_hostname".into(), serde_json::json!(hostname));
    ctx.insert("summary".into(), serde_json::json!(summary));
    ctx.insert("inspection_result".into(), serde_json::json!(summary));

    // Command outputs and judgments
    ctx.insert("command_outputs".into(), serde_json::to_value(&command_outputs).unwrap_or_default());
    ctx.insert("command_judgments".into(), serde_json::to_value(&command_judgments).unwrap_or_default());

    // Device info fields
    ctx.insert("os_release".into(), serde_json::json!(parse_os_release(&outputs_str)));
    ctx.insert("kernel".into(), serde_json::json!(parse_kernel(&outputs_str)));
    ctx.insert("cpu_cores".into(), serde_json::json!(parse_cpu_cores(&outputs_str)));
    ctx.insert("mem_total".into(), serde_json::json!(parse_mem_total(&outputs_str)));
    ctx.insert("manufacturing_date".into(), serde_json::json!(parse_manufacturing_date(&outputs_str)));

    ctx
}

/// 生成 HTML 格式的巡检报告（可在 Tauri webview 中直接显示）
pub fn generate_html_report(ctx: &HashMap<String, serde_json::Value>) -> String {
    let device_name = ctx.get("device_name").and_then(|v| v.as_str()).unwrap_or("未知设备");
    let device_ip = ctx.get("device_ip").and_then(|v| v.as_str()).unwrap_or("");
    let device_model = ctx.get("device_model").and_then(|v| v.as_str()).unwrap_or("-");
    let device_sn = ctx.get("device_sn").and_then(|v| v.as_str()).unwrap_or("-");
    let os_release = ctx.get("os_release").and_then(|v| v.as_str()).unwrap_or("-");
    let kernel = ctx.get("kernel").and_then(|v| v.as_str()).unwrap_or("-");
    let cpu = ctx.get("cpu_cores").and_then(|v| v.as_str()).unwrap_or("-");
    let mem = ctx.get("mem_total").and_then(|v| v.as_str()).unwrap_or("-");
    let summary = ctx.get("summary").and_then(|v| v.as_str()).unwrap_or("");

    let command_outputs: HashMap<String, serde_json::Value> = ctx.get("command_outputs")
        .and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();

    let command_judgments: HashMap<String, serde_json::Value> = ctx.get("command_judgments")
        .and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();

    let mut html = String::new();
    html.push_str("<!DOCTYPE html><html><head><meta charset='utf-8'>");
    html.push_str("<style>body{font-family:SimSun,serif;font-size:10.5pt;max-width:210mm;margin:0 auto;padding:20px}");
    html.push_str("h1{font-size:16pt;text-align:center}h2{font-size:14pt;margin-top:20px}");
    html.push_str("table{border-collapse:collapse;width:100%;margin:10px 0}");
    html.push_str("td,th{border:1px solid #999;padding:4px 8px;font-size:10pt}");
    html.push_str("th{background:#f0f0f0;font-weight:bold}.normal{color:#008000}.abnormal{color:#cc0000}");
    html.push_str("</style></head><body>");

    html.push_str(&format!("<h1>{} 巡检报告</h1>", device_name));

    // 基本信息表
    html.push_str("<h2>基本信息</h2><table>");
    html.push_str(&format!("<tr><th>设备名称</th><td>{}</td><th>IP地址</th><td>{}</td></tr>", device_name, device_ip));
    html.push_str(&format!("<tr><th>设备型号</th><td>{}</td><th>序列号</th><td>{}</td></tr>", device_model, device_sn));
    if os_release != "-" { html.push_str(&format!("<tr><th>OS版本</th><td>{}</td><th>内核</th><td>{}</td></tr>", os_release, kernel)); }
    if cpu != "-" { html.push_str(&format!("<tr><th>CPU</th><td>{}</td><th>内存</th><td>{}</td></tr>", cpu, mem)); }
    html.push_str("</table>");

    // 巡检记录表
    html.push_str("<h2>巡检记录</h2><table>");
    html.push_str("<tr><th>序号</th><th>项目</th><th>巡检内容</th><th>评判结论</th></tr>");

    let mut idx = 1;
    for (cmd, output) in &command_outputs {
        let output_str = output.as_str().unwrap_or("");
        let judgment = command_judgments.get(cmd).and_then(|v| v.as_str()).unwrap_or("");

        let judgment_text = judgment.split('\x00').next().unwrap_or(judgment);
        let j_class = if judgment_text.contains("[OK]") || judgment_text.contains("正常") {
            "normal"
        } else if judgment_text.contains("[WARNING]") || judgment_text.contains("[CRITICAL]") || judgment_text.contains("异常") {
            "abnormal"
        } else { "" };

        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td><pre style='margin:0;font-size:9pt;white-space:pre-wrap'>{}</pre></td><td class='{}'>{}</td></tr>",
            idx, cmd, html_escape(output_str), j_class, html_escape(judgment_text)
        ));
        idx += 1;
    }

    if !summary.is_empty() {
        html.push_str(&format!("<tr><td colspan='4'><strong>总结：{}</strong></td></tr>", html_escape(summary)));
    }

    html.push_str("</table></body></html>");
    html
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        .replace('"', "&quot;").replace('\'', "&#39;")
}

/// 生成报告文件并更新数据库
pub fn generate_report_file(
    db: &rusqlite::Connection,
    record_id: i64,
    output_dir: &Path,
) -> Result<String, String> {
    // Load record data
    let record: Option<(i64, String, Option<String>, Option<String>)> = db.query_row(
        "SELECT batch_id, command_outputs, command_judgments, summary_judgment FROM inspection_records WHERE id=?1",
        rusqlite::params![record_id],
        |r| Ok((r.get(0)?, r.get::<_, String>(1)?, r.get(2)?, r.get(3)?)),
    ).ok();

    let Some((batch_id, cmd_outputs, _cmd_judgments, _summary)) = record else { return Err("记录不存在".into()); };

    let device_id: i64 = db.query_row(
        "SELECT device_id FROM inspection_records WHERE id=?1", rusqlite::params![record_id], |r| r.get(0)
    ).map_err(|e| e.to_string())?;

    let (name, ip, vendor, model): (String, String, String, Option<String>) = db.query_row(
        "SELECT name, ip, vendor, model FROM devices WHERE id=?1", rusqlite::params![device_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    ).map_err(|e| e.to_string())?;

    // Build context
    let mut record_map = HashMap::new();
    record_map.insert("command_outputs".into(), cmd_outputs);
    if let Some(j) = _cmd_judgments { record_map.insert("command_judgments".into(), j); }
    if let Some(s) = _summary { record_map.insert("summary_judgment".into(), s); }

    let mut device_map = HashMap::new();
    device_map.insert("name".into(), name.clone());
    device_map.insert("ip".into(), ip);
    device_map.insert("vendor".into(), vendor);
    device_map.insert("model".into(), model.unwrap_or_default());

    let ctx = build_report_context(&record_map, &device_map);
    let html = generate_html_report(&ctx);

    // Save HTML report
    let batch_dir = output_dir.join(format!("batch{}", batch_id));
    std::fs::create_dir_all(&batch_dir).ok();
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filepath = batch_dir.join(format!("device{}_{}.html", device_id, ts));

    std::fs::write(&filepath, &html).map_err(|e| e.to_string())?;

    let path_str = filepath.to_string_lossy().to_string();
    db.execute("UPDATE inspection_records SET report_path=?1 WHERE id=?2", rusqlite::params![path_str, record_id])
        .map_err(|e| e.to_string())?;

    info!("报告已生成: {}", path_str);
    Ok(path_str)
}

/// 合并多个 HTML 报告
pub fn merge_reports(report_paths: &[String], output_path: &str) -> Result<String, String> {
    if report_paths.is_empty() { return Err("No reports to merge".into()); }
    if report_paths.len() == 1 {
        std::fs::copy(&report_paths[0], output_path).map_err(|e| e.to_string())?;
        return Ok(output_path.to_string());
    }

    let mut merged = String::new();
    merged.push_str("<!DOCTYPE html><html><head><meta charset='utf-8'>");
    merged.push_str("<title>综合巡检报告</title></head><body>");

    for path in report_paths {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        // Extract body content
        if let Some(body_start) = content.find("<body>") {
            if let Some(body_end) = content.rfind("</body>") {
                merged.push_str(&content[body_start + 6..body_end]);
            } else {
                merged.push_str(&content[body_start + 6..]);
            }
        }
        merged.push_str("<div style='page-break-after:always'></div>\n");
    }

    merged.push_str("</body></html>");
    std::fs::write(output_path, &merged).map_err(|e| e.to_string())?;

    info!("合并报告已生成: {}", output_path);
    Ok(output_path.to_string())
}
