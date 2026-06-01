use std::collections::HashMap;
use serde::Serialize;

use crate::db::models::{Device, InspectionRecord};

/// Variable metadata exposed to the frontend for the variable picker UI.
#[derive(Debug, Serialize, Clone)]
pub struct VariableDef {
    pub name: String,
    pub category: String,
    pub description: String,
    pub example: String,
}

/// Returns all available template variables with Chinese descriptions.
pub fn get_variable_definitions() -> Vec<VariableDef> {
    vec![
        // --- 设备信息 ---
        VariableDef {
            name: "device_name".into(),
            category: "设备信息".into(),
            description: "设备名称".into(),
            example: "核心交换机-01".into(),
        },
        VariableDef {
            name: "device_ip".into(),
            category: "设备信息".into(),
            description: "管理 IP 地址".into(),
            example: "192.168.1.1".into(),
        },
        VariableDef {
            name: "vendor".into(),
            category: "设备信息".into(),
            description: "设备厂商".into(),
            example: "H3C".into(),
        },
        VariableDef {
            name: "model".into(),
            category: "设备信息".into(),
            description: "设备型号".into(),
            example: "S5130-54C-HI".into(),
        },
        VariableDef {
            name: "sn".into(),
            category: "设备信息".into(),
            description: "序列号（从命令输出提取）".into(),
            example: "210235A1B2C3".into(),
        },
        VariableDef {
            name: "hostname".into(),
            category: "设备信息".into(),
            description: "主机名".into(),
            example: "Core-SW-01".into(),
        },
        VariableDef {
            name: "os_release".into(),
            category: "设备信息".into(),
            description: "操作系统版本".into(),
            example: "V7.1.070, Release 6329".into(),
        },
        VariableDef {
            name: "kernel".into(),
            category: "设备信息".into(),
            description: "内核版本".into(),
            example: "Linux 4.4.1".into(),
        },
        VariableDef {
            name: "cpu_cores".into(),
            category: "设备信息".into(),
            description: "CPU 核心数".into(),
            example: "4".into(),
        },
        VariableDef {
            name: "mem_total".into(),
            category: "设备信息".into(),
            description: "内存总量".into(),
            example: "2048 MB".into(),
        },
        VariableDef {
            name: "manufacturing_date".into(),
            category: "设备信息".into(),
            description: "生产日期".into(),
            example: "2023-05-10".into(),
        },
        // --- 命令输出 ---
        VariableDef {
            name: "command_outputs".into(),
            category: "命令输出".into(),
            description: "所有命令输出的完整 JSON（不推荐直接使用）".into(),
            example: r#"{"display version": "H3C Comware..."}"#.into(),
        },
        VariableDef {
            name: "command_outputs.<命令名>".into(),
            category: "命令输出".into(),
            description: "指定命令的原始输出，例如 {{command_outputs.display version}}".into(),
            example: "H3C Comware Software, Version 7.1.070...".into(),
        },
        // --- 逐项判断 ---
        VariableDef {
            name: "command".into(),
            category: "逐项判断 ({{#each}} 块内)".into(),
            description: "当前命令名称（仅用于 {{#each command_judgments}} 块内）".into(),
            example: "display version".into(),
        },
        VariableDef {
            name: "status".into(),
            category: "逐项判断 ({{#each}} 块内)".into(),
            description: "当前命令的判断状态：正常/异常/警告（仅用于 {{#each}} 块内）".into(),
            example: "正常".into(),
        },
        VariableDef {
            name: "finding".into(),
            category: "逐项判断 ({{#each}} 块内)".into(),
            description: "当前命令的分析发现（仅用于 {{#each}} 块内）".into(),
            example: "软件版本为推荐版本".into(),
        },
        VariableDef {
            name: "suggestion".into(),
            category: "逐项判断 ({{#each}} 块内)".into(),
            description: "当前命令的处理建议（仅用于 {{#each}} 块内）".into(),
            example: "建议升级到最新版本".into(),
        },
        VariableDef {
            name: "output".into(),
            category: "逐项判断 ({{#each}} 块内)".into(),
            description: "当前命令的原始输出（仅用于 {{#each}} 块内）".into(),
            example: "H3C Comware Software...".into(),
        },
        // --- AI 分析 ---
        VariableDef {
            name: "ai_analysis".into(),
            category: "AI 分析".into(),
            description: "AI 详细分析文本".into(),
            example: "设备运行状态良好，各项指标正常...".into(),
        },
        VariableDef {
            name: "ai_suggestions".into(),
            category: "AI 分析".into(),
            description: "AI 综合建议".into(),
            example: "建议关注链路使用率变化趋势".into(),
        },
        VariableDef {
            name: "ai_result".into(),
            category: "AI 分析".into(),
            description: "AI 原始分析结果 JSON".into(),
            example: r#"{"summary":"...","items":[...]}"#.into(),
        },
        VariableDef {
            name: "summary".into(),
            category: "AI 分析".into(),
            description: "综合判断结果（summary_judgment）".into(),
            example: "normal".into(),
        },
        // --- 报告元信息 ---
        VariableDef {
            name: "report_timestamp".into(),
            category: "报告元信息".into(),
            description: "报告生成时间".into(),
            example: "2026-05-31 14:30:00".into(),
        },
        VariableDef {
            name: "generated_at".into(),
            category: "报告元信息".into(),
            description: "报告生成时间（同 report_timestamp）".into(),
            example: "2026-05-31 14:30:00".into(),
        },
    ]
}

/// Build the full template context HashMap from a device and inspection record.
/// This is used both for live rendering and for preview (with sample data).
pub fn build_template_context(
    conn: &rusqlite::Connection,
    device: &Device,
    record: &InspectionRecord,
) -> Result<HashMap<String, serde_json::Value>, String> {
    // Load command descriptions for friendly labels
    let cmd_descs = crate::services::report_builder::load_command_descriptions(conn);
    let cmd_descs_val: serde_json::Map<String, serde_json::Value> = cmd_descs
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
        .collect();
    let mut ctx: HashMap<String, serde_json::Value> = HashMap::new();

    // Device info
    ctx.insert("device_name".into(), serde_json::Value::String(device.name.clone()));
    ctx.insert("device_ip".into(), serde_json::Value::String(device.ip.clone()));
    ctx.insert("vendor".into(), serde_json::Value::String(device.vendor.clone()));
    if let Some(ref model) = device.model {
        ctx.insert("model".into(), serde_json::Value::String(model.clone()));
    } else {
        ctx.insert("model".into(), serde_json::Value::String(String::new()));
    }

    // Try to extract device details from command outputs
    let outputs_map = parse_outputs_map(&record.command_outputs);

    ctx.insert("sn".into(), serde_json::Value::String(extract_detail(&outputs_map, &["display device", "dev"], &["SN:", "Serial", "SN"])));
    ctx.insert("hostname".into(), serde_json::Value::String(extract_detail(&outputs_map, &["display current-configuration", "hostname"], &["sysname ", "hostname "])));

    let os_release = outputs_map.iter().find(|(cmd, _)| {
        let cl = cmd.to_lowercase();
        cl.contains("display version") || cl.contains("show version")
    }).map(|(_, out)| {
        // Take first meaningful line of version output
        out.lines().next().unwrap_or("").to_string()
    }).unwrap_or_default();
    ctx.insert("os_release".into(), serde_json::Value::String(os_release));

    ctx.insert("kernel".into(), serde_json::Value::String(extract_detail(&outputs_map, &["display version"], &["Linux", "kernel"])));
    ctx.insert("cpu_cores".into(), serde_json::Value::String(extract_detail(&outputs_map, &["display cpu", "display cpu-usage"], &["CPU", "cores", "Core"])));
    ctx.insert("mem_total".into(), serde_json::Value::String(extract_detail(&outputs_map, &["display memory", "display memory-usage"], &["Total", "Memory"])));
    ctx.insert("manufacturing_date".into(), serde_json::Value::String(extract_detail(&outputs_map, &["display device", "manufacture"], &["MANU", "Date", "manufactured"])));

    // Command outputs as full JSON value
    if let Some(ref outputs_str) = record.command_outputs {
        if let Ok(outputs_val) = serde_json::from_str::<serde_json::Value>(outputs_str) {
            ctx.insert("command_outputs".into(), outputs_val);
        }
    }

    // Command judgments
    if let Some(ref judgments_str) = record.command_judgments {
        if let Ok(judgments_val) = serde_json::from_str::<serde_json::Value>(judgments_str) {
            ctx.insert("command_judgments".into(), judgments_val);
        }
    }

    // AI analysis
    let summary = record.summary_judgment.clone().unwrap_or_default();
    ctx.insert("summary".into(), serde_json::Value::String(summary));

    let ai_analysis = record.ai_analysis.clone().unwrap_or_default();
    ctx.insert("ai_analysis".into(), serde_json::Value::String(ai_analysis));

    let ai_suggestions = record.ai_suggestions.clone().unwrap_or_default();
    ctx.insert("ai_suggestions".into(), serde_json::Value::String(ai_suggestions));

    let ai_result = record.ai_result.clone().unwrap_or_default();
    ctx.insert("ai_result".into(), serde_json::Value::String(ai_result));

    // Metadata
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    ctx.insert("report_timestamp".into(), serde_json::Value::String(now.clone()));
    ctx.insert("generated_at".into(), serde_json::Value::String(now));

    // Command descriptions for friendly labels
    ctx.insert("command_descriptions".into(), serde_json::Value::Object(cmd_descs_val));

    Ok(ctx)
}

/// Build a sample/fake context for template preview.
pub fn build_sample_context() -> HashMap<String, serde_json::Value> {
    let mut ctx = HashMap::new();
    ctx.insert("device_name".into(), serde_json::Value::String("示例-核心交换机-01".into()));
    ctx.insert("device_ip".into(), serde_json::Value::String("192.168.1.1".into()));
    ctx.insert("vendor".into(), serde_json::Value::String("H3C".into()));
    ctx.insert("model".into(), serde_json::Value::String("S5130-54C-HI".into()));
    ctx.insert("sn".into(), serde_json::Value::String("210235A1B2C3D4E5".into()));
    ctx.insert("hostname".into(), serde_json::Value::String("Core-SW-01".into()));
    ctx.insert("os_release".into(), serde_json::Value::String("H3C Comware Software, Version 7.1.070, Release 6329".into()));
    ctx.insert("kernel".into(), serde_json::Value::String("Linux 4.4.1".into()));
    ctx.insert("cpu_cores".into(), serde_json::Value::String("4".into()));
    ctx.insert("mem_total".into(), serde_json::Value::String("2048 MB".into()));
    ctx.insert("manufacturing_date".into(), serde_json::Value::String("2023-05-10".into()));

    // Sample command judgments
    let judgments = serde_json::json!({
        "display version": {
            "status": "正常",
            "finding": "软件版本为推荐版本",
            "suggestion": ""
        },
        "display device": {
            "status": "正常",
            "finding": "设备运行时间 365 天",
            "suggestion": ""
        },
        "display cpu-usage": {
            "status": "警告",
            "finding": "CPU 使用率偏高 (78%)",
            "suggestion": "建议关注 CPU 负载趋势，必要时扩容"
        },
        "display memory-usage": {
            "status": "正常",
            "finding": "内存使用率 45%",
            "suggestion": ""
        },
        "display logbuffer": {
            "status": "异常",
            "finding": "发现 3 条 ERROR 级别日志",
            "suggestion": "建议排查错误日志来源，检查链路状态"
        }
    });
    ctx.insert("command_judgments".into(), judgments);

    // Sample command outputs
    let outputs = serde_json::json!({
        "display version": "H3C Comware Software, Version 7.1.070, Release 6329\nCopyright (c) 2004-2023 New H3C Technologies Co., Ltd.",
        "display device": "Slot 1: S5130-54C-HI\nSN: 210235A1B2C3D4E5\nManufactured: 2023-05-10",
        "display cpu-usage": "CPU Usage: 78% (5s), 75% (1min), 72% (5min)",
        "display memory-usage": "Total: 2048 MB\nUsed: 922 MB (45%)\nFree: 1126 MB",
        "display logbuffer": "[2026-05-31 10:00:01] ERROR: Interface GigabitEthernet1/0/24 link down\n[2026-05-31 10:00:05] WARNING: BGP neighbor 10.0.0.1 state changed to Active"
    });
    ctx.insert("command_outputs".into(), outputs);

    ctx.insert("ai_analysis".into(), serde_json::Value::String("设备整体运行状态良好。CPU 使用率偏高需要关注，建议监控趋势并在必要时进行扩容评估。内存和软件版本均处于正常范围。".into()));
    ctx.insert("ai_suggestions".into(), serde_json::Value::String("建议关注 CPU 负载趋势；排查链路异常日志".into()));
    ctx.insert("ai_result".into(), serde_json::Value::String(r#"{"summary":"设备整体正常，CPU偏高需关注","overall":"warning"}"#.into()));
    ctx.insert("summary".into(), serde_json::Value::String("warning".into()));

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    ctx.insert("report_timestamp".into(), serde_json::Value::String(now.clone()));
    ctx.insert("generated_at".into(), serde_json::Value::String(now));

    ctx
}

// --- helpers ---

fn parse_outputs_map(outputs_str: &Option<String>) -> HashMap<String, String> {
    let empty = "{}".to_string();
    let val: serde_json::Value =
        serde_json::from_str(outputs_str.as_deref().unwrap_or(&empty)).unwrap_or_default();
    let mut map = HashMap::new();
    if let Some(obj) = val.as_object() {
        for (k, v) in obj {
            map.insert(k.clone(), v.as_str().unwrap_or("").to_string());
        }
    }
    map
}

fn extract_detail(outputs: &HashMap<String, String>, cmd_keywords: &[&str], line_keywords: &[&str]) -> String {
    for (cmd, output) in outputs {
        let cmd_lower = cmd.to_lowercase();
        if cmd_keywords.iter().any(|k| cmd_lower.contains(k)) {
            for line in output.lines() {
                if line_keywords.iter().any(|k| line.contains(k)) {
                    return line.trim().to_string();
                }
            }
        }
    }
    "-".to_string()
}
