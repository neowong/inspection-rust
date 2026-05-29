use std::collections::HashMap;

use chrono::Utc;

/// Helper to extract a string value from the context map, returning a default if missing.
fn ctx_str(ctx: &HashMap<String, serde_json::Value>, key: &str, default: &str) -> String {
    ctx.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| default.to_string())
}

/// Helper to extract an optional string value from the context map.
fn ctx_opt_str(ctx: &HashMap<String, serde_json::Value>, key: &str) -> String {
    ctx.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "-".to_string())
}

/// Build a Markdown report string from device inspection context.
///
/// The `ctx` map may contain the following keys:
/// - `device_name` (str)
/// - `device_ip` (str)
/// - `vendor` (str)
/// - `model` (str)
/// - `sn` (str)
/// - `hostname` (str)
/// - `os_release` (str)
/// - `kernel` (str)
/// - `cpu_cores` (str)
/// - `mem_total` (str)
/// - `manufacturing_date` (str)
/// - `summary` (str)
/// - `command_outputs` (map of command name to output string)
/// - `command_judgments` (map of command name to judgment JSON object)
pub fn build_markdown(ctx: &HashMap<String, serde_json::Value>) -> String {
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let device_name = ctx_str(ctx, "device_name", "未知设备");

    let mut md = String::new();
    md.push_str(&format!("# {} 巡检报告\n\n", device_name));
    md.push_str(&format!("> 生成时间: {}\n\n", timestamp));

    // --- 基本信息 ---
    md.push_str("## 基本信息\n\n");
    md.push_str("| 项目 | 内容 |\n");
    md.push_str("|------|------|\n");
    md.push_str(&format!("| 设备名称 | {} |\n", ctx_opt_str(ctx, "device_name")));
    md.push_str(&format!("| IP 地址 | {} |\n", ctx_opt_str(ctx, "device_ip")));
    md.push_str(&format!("| 厂商 | {} |\n", ctx_opt_str(ctx, "vendor")));
    md.push_str(&format!("| 型号 | {} |\n", ctx_opt_str(ctx, "model")));
    md.push_str(&format!("| 序列号 | {} |\n", ctx_opt_str(ctx, "sn")));
    md.push_str(&format!("| 主机名 | {} |\n", ctx_opt_str(ctx, "hostname")));
    md.push_str(&format!("| 操作系统 | {} |\n", ctx_opt_str(ctx, "os_release")));
    md.push_str(&format!("| 内核 | {} |\n", ctx_opt_str(ctx, "kernel")));
    md.push_str(&format!("| CPU 核心数 | {} |\n", ctx_opt_str(ctx, "cpu_cores")));
    md.push_str(&format!("| 内存总量 | {} |\n", ctx_opt_str(ctx, "mem_total")));
    md.push_str(&format!("| 生产日期 | {} |\n", ctx_opt_str(ctx, "manufacturing_date")));
    md.push('\n');

    // --- 巡检结果 ---
    md.push_str("## 巡检结果\n\n");

    if let Some(Some(judgments)) = ctx
        .get("command_judgments")
        .map(|v| v.as_object())
    {
        let outputs = ctx
            .get("command_outputs")
            .and_then(|v| v.as_object())
            .map(|m| {
                let mut h = HashMap::new();
                for (k, v) in m {
                    h.insert(k.clone(), v.as_str().unwrap_or("").to_string());
                }
                h
            })
            .unwrap_or_default();

        for (cmd, jdg) in judgments {
            let status = jdg
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let finding = jdg
                .get("finding")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let suggestion = jdg
                .get("suggestion")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            md.push_str(&format!("### {}\n\n", cmd));
            md.push_str(&format!("- 状态: {}\n", status));
            md.push_str(&format!("- 结果: {}\n", finding));
            md.push_str(&format!("- 建议: {}\n", suggestion));

            // Include trimmed raw output
            let raw_output = outputs.get(cmd).map(|s| s.as_str()).unwrap_or("");
            let trimmed = if raw_output.len() > 500 {
                format!("{}...\n[输出已截断，共 {} 字节]", &raw_output[..500], raw_output.len())
            } else {
                raw_output.to_string()
            };
            md.push_str("- 原始输出:\n```\n");
            md.push_str(&trimmed);
            md.push_str("\n```\n\n");
        }
    } else {
        md.push_str("_无巡检判定数据_\n\n");
    }

    // --- 总结 ---
    md.push_str("## 总结\n\n");
    let summary = ctx_str(ctx, "summary", "无总结");
    md.push_str(&summary);
    md.push('\n');

    md
}
