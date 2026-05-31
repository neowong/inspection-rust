use crate::db::models::{InspectionBatch, InspectionRecord, Device, now_str, BATCH_COLUMNS, RECORD_COLUMNS, DEVICE_COLUMNS, REPORT_TEMPLATE_COLUMNS, batch_from_row, record_from_row, device_from_row, report_template_from_row};
use super::template_engine;
use std::collections::HashMap;

const REPORT_HTML: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<title>{{report_title}}</title>
<style>
  @page { size: A4 portrait; margin: 15mm; }
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: "微软雅黑", "Microsoft YaHei", "宋体", "SimSun", sans-serif;
    font-size: 10pt; color: #222; background: #e8e8e8;
    padding: 16px; display: flex; justify-content: center; line-height: 1.5;
  }
  .page-wrapper {
    background: #fff; max-width: 794px; width: 100%;
    padding: 15mm 18mm; box-shadow: 0 2px 12px rgba(0,0,0,0.12);
  }
  @media screen { .page-wrapper { margin: 0 auto; } }
  .report-title {
    text-align: center; font-size: 16pt; font-weight: bold;
    margin-bottom: 4mm; letter-spacing: 1pt; border-bottom: 2pt solid #333; padding-bottom: 3mm;
  }
  .report-meta { text-align: center; font-size: 8pt; color: #555; margin-bottom: 6mm; }
  .device-title { font-size: 13pt; font-weight: bold; margin: 6mm 0 3mm; padding-bottom: 1mm; border-bottom: 1pt solid #999; }
  .section-subtitle { font-size: 11pt; font-weight: bold; margin: 4mm 0 2mm; color: #333; }
  table.info { width: 100%; border-collapse: collapse; margin-bottom: 4mm; font-size: 9pt; table-layout: fixed; }
  table.info td {
    border: 0.5pt solid #999; padding: 1mm 2mm; line-height: 1.4;
    text-align: center; word-break: break-all;
  }
  table.info td.label {
    font-weight: bold; width: 15%; text-align: right; background: #f9f9f9;
    white-space: nowrap; padding-right: 2mm;
  }
  table.result { width: 100%; border-collapse: collapse; font-size: 9pt; table-layout: fixed; margin-bottom: 3mm; }
  table.result th {
    border: 0.5pt solid #999; padding: 1mm 1.5mm; font-weight: bold;
    text-align: center; background: #f0f0f0; color: #333; font-size: 8.5pt;
  }
  table.result td {
    border: 0.5pt solid #999; padding: 1mm 1.5mm;
    vertical-align: middle; text-align: center; line-height: 1.4; font-size: 8.5pt;
  }
  table.result td.num { text-align: center; vertical-align: middle; width: 5%; }
  table.result td.item { width: 18%; font-weight: bold; text-align: center; vertical-align: middle; overflow-wrap: break-word; word-break: break-all; }
  table.result td.detail {
    font-size: 8pt; font-family: "Consolas", "Courier New", monospace;
    white-space: pre-wrap; word-break: break-all; text-align: left; vertical-align: top; width: 45%;
    max-height: 180px; overflow: hidden;
  }
  table.result td.verdict { font-size: 8pt; text-align: left; vertical-align: middle; white-space: normal; word-break: break-all; width: 32%; }
  table.result td.summary {
    padding: 1.5mm 2mm; font-size: 9pt; line-height: 1.5;
    text-align: left; vertical-align: top; border-top: 1pt solid #333; background: #fafafa;
  }
  .verdict-status { display: block; font-weight: bold; }
  .verdict-suggestion { display: block; font-size: 7.5pt; color: #555; margin-top: 1mm; }
  .cmd { display: none; }
  .device-section { page-break-after: always; page-break-inside: avoid; }
  @media print {
    body { background: #fff; padding: 0; display: block; }
    .page-wrapper { max-width: none; padding: 0; box-shadow: none; margin: 0; }
    table.result th { -webkit-print-color-adjust: exact; print-color-adjust: exact; }
    table.info td.label { -webkit-print-color-adjust: exact; print-color-adjust: exact; }
    thead { display: table-header-group; }
    .device-section { page-break-after: always; }
  }
</style>
</head>
<body>
<div class="page-wrapper">
<h1 class="report-title">{{report_title}}</h1>
<p class="report-meta">{{report_meta}}</p>
{{device_sections}}
</div>
</body>
</html>"#;

const DEVICE_SECTION: &str = r#"<div class="device-section">
  <h2 class="device-title">{{device_name}}</h2>
  <h3 class="section-subtitle">基本信息</h3>
  <table class="info">
    {{info_rows}}
  </table>
  <h3 class="section-subtitle">巡检结果</h3>
  <table class="result">
    <thead>
      <tr><th>序号</th><th>巡检项目</th><th>巡检结果</th><th>评判结论</th></tr>
    </thead>
    <tbody>
      {{inspection_rows}}
    </tbody>
  </table>
</div>"#;

/// Build an HTML inspection report for a completed batch.
/// If template_id is provided and valid, uses that template for rendering.
/// Returns the full HTML string.
pub fn build_report_html(conn: &rusqlite::Connection, batch_id: i64, template_id: Option<i64>) -> Result<String, String> {
    // 1. Get batch
    let batch_sql = format!("SELECT {} FROM inspection_batches WHERE id = ?1", BATCH_COLUMNS);
    let batch: InspectionBatch = crate::db::query::query_one(
        conn, &batch_sql, rusqlite::params![batch_id], batch_from_row,
    )?
    .ok_or_else(|| format!("巡检批次 ID {} 不存在", batch_id))?;

    // 2. Get completed records
    let records_sql = format!(
        "SELECT {} FROM inspection_records WHERE batch_id = ?1 AND status = 'completed' ORDER BY id",
        RECORD_COLUMNS
    );
    let records: Vec<InspectionRecord> = crate::db::query::query_all(
        conn, &records_sql, rusqlite::params![batch_id], record_from_row,
    )?;

    if records.is_empty() {
        return Err("批次中无已完成记录".to_string());
    }

    // 3. Build device sections
    let mut device_sections = String::new();
    for record in &records {
        // Get device
        let device_sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
        let device = crate::db::query::query_one(
            conn, &device_sql, rusqlite::params![record.device_id], device_from_row,
        )?
        .unwrap_or_else(|| Device {
            id: record.device_id,
            name: "未知设备".into(),
            ip: "".into(),
            device_type: "".into(),
            vendor: "".into(),
            model: None,
            ssh_username: None,
            ssh_password_encrypted: None,
            ssh_port: 22,
            template_id: None,
            status: "unknown".into(),
            last_checked_at: None,
            created_at: "".into(),
            updated_at: "".into(),
        });

        let section = build_device_section(&device, record)?;
        device_sections.push_str(&section);
    }

    // 4. Build metadata
    let batch_name = batch.name.unwrap_or_else(|| format!("批次 #{}", batch_id));
    let report_title = format!("{} 综合巡检报告", batch_name);
    let report_meta = format!(
        "巡检批次: {} &emsp;|&emsp; 生成时间: {} &emsp;|&emsp; 设备数: {}",
        batch_name,
        now_str(),
        records.len(),
    );

    // 5. Load template if specified
    let template_content: Option<(String, String)> = if let Some(tid) = template_id {
        let rt_sql = format!("SELECT {} FROM report_templates WHERE id = ?1", REPORT_TEMPLATE_COLUMNS);
        crate::db::query::query_one(conn, &rt_sql, rusqlite::params![tid], report_template_from_row)
            .ok()
            .flatten()
            .filter(|t| !t.content.is_empty())
            .map(|t| (t.content, t.format))
    } else {
        None
    };

    // 6. Assemble: use template if available, otherwise use hardcoded HTML
    let html = if let Some((content, format)) = template_content {
        let mut ctx: HashMap<String, serde_json::Value> = HashMap::new();
        ctx.insert("report_title".into(), serde_json::Value::String(report_title));
        ctx.insert("report_meta".into(), serde_json::Value::String(report_meta));
        ctx.insert("device_sections".into(), serde_json::Value::String(device_sections));
        template_engine::render_template(&content, &ctx, &format)
    } else {
        REPORT_HTML
            .replace("{{report_title}}", &report_title)
            .replace("{{report_meta}}", &report_meta)
            .replace("{{device_sections}}", &device_sections)
    };

    Ok(html)
}

fn build_device_section(device: &Device, record: &InspectionRecord) -> Result<String, String> {
    // Basic info table rows
    let model = device.model.as_deref().unwrap_or("-");
    let vendor = &device.vendor;

    let info_rows = format!(
        r#"    <tr><td class="label">设备名称</td><td>{name}</td><td class="label">设备型号</td><td>{model}</td></tr>
    <tr><td class="label">IP 地址</td><td>{ip}</td><td class="label">设备 SN</td><td>{sn}</td></tr>
    <tr><td class="label">出厂日期</td><td>{mfg_date}</td><td class="label">厂商</td><td>{vendor}</td></tr>"#,
        name = html_escape(&device.name),
        model = html_escape(model),
        ip = html_escape(&device.ip),
        sn = html_escape(&extract_sn(record)),
        mfg_date = html_escape(&extract_mfg_date(record)),
        vendor = html_escape(vendor),
    );

    // Inspection rows
    let inspection_rows = build_inspection_rows(record);

    let section = DEVICE_SECTION
        .replace("{{device_name}}", &html_escape(&device.name))
        .replace("{{info_rows}}", &info_rows)
        .replace("{{inspection_rows}}", &inspection_rows);

    Ok(section)
}

fn build_inspection_rows(record: &InspectionRecord) -> String {
    let mut rows = String::new();

    // Parse command outputs and judgments
    let outputs = parse_json_map(&record.command_outputs);
    let judgments = parse_json_object(&record.command_judgments);

    let mut seq = 0u32;
    for (cmd, output) in &outputs {
        seq += 1;
        let cmd_label = cmd.clone();
        let mut detail_parts = Vec::new();
        if let Some(jdg) = judgments.get(cmd) {
            let finding = jdg.get("finding").and_then(|v| v.as_str()).unwrap_or("");
            if !finding.is_empty() {
                detail_parts.push(html_escape(finding));
            }
        }
        // Include trimmed output as detail (limit to 15 lines / 600 chars)
        let lines: Vec<&str> = output.lines().collect();
        let trimmed = if lines.len() > 15 {
            format!("{}...\n[共 {} 行，已截断]", &lines[..15].join("\n"), lines.len())
        } else if output.len() > 600 {
            format!("{}...", &output[..600])
        } else {
            output.clone()
        };
        detail_parts.push(html_escape(&trimmed));
        let detail = detail_parts.join("<br>");

        let verdict_html = if let Some(jdg) = judgments.get(cmd) {
            let status = jdg.get("status").and_then(|v| v.as_str()).unwrap_or("");
            let suggestion = jdg.get("suggestion").and_then(|v| v.as_str()).unwrap_or("");
            let mut v = format!("<span class=\"verdict-status\">{}</span>", html_escape(status));
            if !suggestion.is_empty() {
                v.push_str(&format!("<br><span class=\"verdict-line\">建议：{}</span>", html_escape(suggestion)));
            }
            v
        } else {
            "-".to_string()
        };

        rows.push_str(&format!(
            r#"      <tr>
        <td class="num">{seq}</td>
        <td class="item">{label}</td>
        <td class="detail">{detail}</td>
        <td class="verdict">{verdict}</td>
      </tr>
"#,
            seq = seq,
            label = html_escape(&cmd_label),
            detail = detail,
            verdict = verdict_html,
        ));
    }

    // Summary row
    let summary = record
        .ai_analysis
        .as_deref()
        .unwrap_or("暂无总结");
    let overall = record
        .summary_judgment
        .as_deref()
        .unwrap_or("");

    let summary_text = if overall.is_empty() {
        format!("<strong>总结：</strong>{}", html_escape(summary))
    } else {
        format!(
            "<strong>总结：</strong>{}<br>历史趋势：{}",
            html_escape(summary),
            html_escape(overall),
        )
    };

    rows.push_str(&format!(
        r#"      <tr>
        <td colspan="4" class="summary">{summary}</td>
      </tr>
"#,
        summary = summary_text,
    ));

    rows
}

/// Try to extract SN from command outputs (look for display device output).
fn extract_sn(record: &InspectionRecord) -> String {
    let outputs = parse_json_map(&record.command_outputs);
    for (cmd, output) in &outputs {
        let cmd_lower = cmd.to_lowercase();
        if cmd_lower.contains("display device") || cmd_lower.contains("dev") {
            // Look for SN pattern in output
            for line in output.lines() {
                let trimmed = line.trim();
                if trimmed.contains("SN:") || trimmed.contains("Serial") {
                    return trimmed.to_string();
                }
            }
        }
    }
    "-".to_string()
}

/// Try to extract manufacturing date from command outputs.
fn extract_mfg_date(record: &InspectionRecord) -> String {
    let outputs = parse_json_map(&record.command_outputs);
    for (cmd, output) in &outputs {
        let cmd_lower = cmd.to_lowercase();
        if cmd_lower.contains("display device") || cmd_lower.contains("manufacture") {
            for line in output.lines() {
                let trimmed = line.trim();
                if trimmed.contains("MANU") || trimmed.contains("Date") || trimmed.contains("manufactured") {
                    return trimmed.to_string();
                }
            }
        }
    }
    "-".to_string()
}

// --- Helpers ---

fn parse_json_map(json_str: &Option<String>) -> std::collections::HashMap<String, String> {
    let empty = "{}".to_string();
    let val: serde_json::Value =
        serde_json::from_str(json_str.as_deref().unwrap_or(&empty)).unwrap_or_default();
    let mut map = std::collections::HashMap::new();
    if let Some(obj) = val.as_object() {
        for (k, v) in obj {
            let s = v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string());
            map.insert(k.clone(), s);
        }
    }
    map
}

fn parse_json_object(json_str: &Option<String>) -> serde_json::Map<String, serde_json::Value> {
    let empty = "{}".to_string();
    let val: serde_json::Value =
        serde_json::from_str(json_str.as_deref().unwrap_or(&empty)).unwrap_or_default();
    val.as_object().cloned().unwrap_or_default()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
