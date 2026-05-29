/// Markdown 巡检报告生成服务
use std::collections::HashMap;
use std::path::Path;

/// 构建单设备报告的 Markdown 字符串
pub fn build_markdown(ctx: &HashMap<String, serde_json::Value>) -> String {
    let device_name = ctx.get("device_name").and_then(|v| v.as_str()).unwrap_or("未知设备");
    let device_ip = ctx.get("device_ip").and_then(|v| v.as_str()).unwrap_or("");
    let vendor = ctx.get("device_vendor").and_then(|v| v.as_str()).unwrap_or("-");
    let model = ctx.get("device_model").and_then(|v| v.as_str()).unwrap_or("-");
    let sn = ctx.get("device_sn").and_then(|v| v.as_str()).unwrap_or("-");
    let hostname = ctx.get("device_hostname").and_then(|v| v.as_str()).unwrap_or("-");
    let os_release = ctx.get("os_release").and_then(|v| v.as_str()).unwrap_or("-");
    let kernel = ctx.get("kernel").and_then(|v| v.as_str()).unwrap_or("-");
    let cpu = ctx.get("cpu_cores").and_then(|v| v.as_str()).unwrap_or("-");
    let mem = ctx.get("mem_total").and_then(|v| v.as_str()).unwrap_or("-");
    let mfg_date = ctx.get("manufacturing_date").and_then(|v| v.as_str()).unwrap_or("-");
    let summary = ctx.get("summary").and_then(|v| v.as_str()).unwrap_or("");

    let command_outputs: HashMap<String, String> = ctx.get("command_outputs")
        .and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();

    let command_judgments: HashMap<String, String> = ctx.get("command_judgments")
        .and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();

    let ts = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");

    let mut md = String::new();
    md.push_str(&format!("# {} 巡检报告\n\n", device_name));
    md.push_str(&format!("> 生成时间: {}\n\n", ts));

    // 基本信息
    md.push_str("## 基本信息\n\n");
    md.push_str("| 项目 | 内容 |\n|------|------|\n");
    md.push_str(&format!("| 设备名称 | {} |\n", device_name));
    md.push_str(&format!("| IP 地址 | {} |\n", device_ip));
    md.push_str(&format!("| 厂商 | {} |\n", vendor));
    md.push_str(&format!("| 型号 | {} |\n", model));
    md.push_str(&format!("| 序列号 | {} |\n", sn));
    md.push_str(&format!("| 主机名 | {} |\n", hostname));
    if os_release != "-" { md.push_str(&format!("| OS | {} |\n", os_release)); }
    if kernel != "-" { md.push_str(&format!("| 内核 | {} |\n", kernel)); }
    if cpu != "-" { md.push_str(&format!("| CPU | {} |\n", cpu)); }
    if mem != "-" { md.push_str(&format!("| 内存 | {} |\n", mem)); }
    if mfg_date != "-" { md.push_str(&format!("| 出厂日期 | {} |\n", mfg_date)); }
    md.push_str("\n");

    // 巡检记录
    md.push_str("## 巡检记录\n\n");
    if command_outputs.is_empty() {
        md.push_str("（无命令输出）\n\n");
    } else {
        md.push_str("| 序号 | 巡检项目 | 评判结论 |\n|------|---------|----------|\n");
        for (i, (cmd, output)) in command_outputs.iter().enumerate() {
            let judgment = command_judgments.get(cmd)
                .map(|s| s.split('\x00').next().unwrap_or(""))
                .unwrap_or("");
            let output_short = output.lines().take(3).collect::<Vec<_>>().join("  \n");
            let status_icon = if judgment.contains("[OK]") { "✅" }
                else if judgment.contains("[WARNING]") { "⚠️" }
                else if judgment.contains("[CRITICAL]") { "🔴" }
                else { "ℹ️" };
            md.push_str(&format!("| {} | **{}**<br/>{} | {} {} |\n",
                i + 1, cmd, output_short, status_icon, judgment));
        }
        md.push_str("\n");
    }

    // AI 总结
    if !summary.is_empty() {
        md.push_str("## AI 分析总结\n\n");
        md.push_str(&format!("{}\n", summary));
    }

    md
}

/// 生成报告文件并更新数据库
pub fn generate_report_file(
    db: &rusqlite::Connection,
    record_id: i64,
    output_dir: &Path,
) -> Result<String, String> {
    let record: Option<(i64, String, Option<String>)> = db.query_row(
        "SELECT batch_id, command_outputs, summary_judgment FROM inspection_records WHERE id=?1",
        rusqlite::params![record_id],
        |r| Ok((r.get(0)?, r.get::<_, String>(1)?, r.get(2)?)),
    ).ok();

    let Some((batch_id, cmd_outputs, summary)) = record else {
        return Err("记录不存在".into());
    };

    let device_id: i64 = db.query_row(
        "SELECT device_id FROM inspection_records WHERE id=?1",
        rusqlite::params![record_id], |r| r.get(0)
    ).map_err(|e| e.to_string())?;

    let (name, ip, vendor, model): (String, String, String, Option<String>) = db.query_row(
        "SELECT name, ip, vendor, model FROM devices WHERE id=?1",
        rusqlite::params![device_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    ).map_err(|e| e.to_string())?;

    let outputs: HashMap<String, serde_json::Value> = serde_json::from_str(&cmd_outputs).unwrap_or_default();
    let judgments: HashMap<String, serde_json::Value> = db.query_row(
        "SELECT command_judgments FROM inspection_records WHERE id=?1",
        rusqlite::params![record_id],
        |r| r.get::<_, Option<String>>(0),
    ).ok().flatten().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();

    let mut ctx = HashMap::new();
    ctx.insert("device_name".into(), serde_json::json!(name));
    ctx.insert("device_ip".into(), serde_json::json!(ip));
    ctx.insert("device_vendor".into(), serde_json::json!(vendor));
    ctx.insert("device_model".into(), serde_json::json!(model.unwrap_or_default()));
    ctx.insert("command_outputs".into(), serde_json::to_value(&outputs).unwrap_or_default());
    ctx.insert("command_judgments".into(), serde_json::to_value(&judgments).unwrap_or_default());
    ctx.insert("summary".into(), serde_json::json!(summary.unwrap_or_default()));

    let md = build_markdown(&ctx);

    let batch_dir = output_dir.join(format!("batch{}", batch_id));
    std::fs::create_dir_all(&batch_dir).ok();
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filepath = batch_dir.join(format!("device{}_{}.md", device_id, ts));

    std::fs::write(&filepath, &md).map_err(|e| e.to_string())?;

    let path_str = filepath.to_string_lossy().to_string();
    db.execute("UPDATE inspection_records SET report_path=?1 WHERE id=?2",
        rusqlite::params![path_str, record_id])
        .map_err(|e| e.to_string())?;

    tracing::info!("Markdown 报告已生成: {}", path_str);
    Ok(path_str)
}

/// 读取 Markdown 报告内容
pub fn read_report(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("读取报告失败: {}", e))
}
