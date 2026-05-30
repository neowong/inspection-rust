use crate::db::models::{InspectionBatch, InspectionRecord, Device, now_str, BATCH_COLUMNS, RECORD_COLUMNS, DEVICE_COLUMNS, batch_from_row, record_from_row, device_from_row};

const REPORT_HTML: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<title>{{report_title}}</title>
<style>
  @page { size: A4 portrait; margin: 20mm; }
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: "宋体", "SimSun", "SimSun-ExtB", "NSimSun", serif;
    font-size: 11pt; color: #000; background: #e8e8e8;
    padding: 24px; display: flex; justify-content: center; line-height: 1.5;
  }
  .page-wrapper {
    background: #fff; max-width: 794px; width: 100%;
    padding: 20mm; box-shadow: 0 2px 12px rgba(0,0,0,0.12);
  }
  @media screen { .page-wrapper { margin: 0 auto; } }
  .report-title {
    text-align: center; font-size: 18pt; font-weight: bold;
    margin-bottom: 6mm; letter-spacing: 2pt;
  }
  .report-meta { text-align: center; font-size: 9pt; color: #000; margin-bottom: 10mm; }
  .device-title { font-size: 14pt; font-weight: bold; margin: 8mm 0 4mm; padding-bottom: 1mm; }
  .section-subtitle { font-size: 12pt; font-weight: bold; margin: 5mm 0 3mm; }
  table.info { width: 100%; border-collapse: collapse; margin-bottom: 5mm; font-size: 11pt; }
  table.info td {
    border: 1pt solid #333; padding: 2mm 3mm; line-height: 1.4;
    text-align: center; white-space: nowrap;
  }
  table.info td.label { font-weight: bold; width: 14%; text-align: left; white-space: nowrap; }
  table.result { width: 100%; border-collapse: collapse; font-size: 10.5pt; table-layout: auto; }
  table.result th {
    border: 1pt solid #333; padding: 1.5mm 2mm; font-weight: bold;
    text-align: center; background: #f5f5f5; color: #000;
  }
  table.result td {
    border: 1pt solid #333; padding: 1.5mm 2mm;
    vertical-align: middle; text-align: center; line-height: 1.4;
  }
  table.result td.num { text-align: center; vertical-align: middle; width: 40px; }
  table.result td.item { width: 80px; font-weight: bold; text-align: center; vertical-align: middle; white-space: nowrap; }
  table.result td.detail {
    font-size: 9pt; font-family: "Consolas", "Courier New", "SimSun", monospace;
    white-space: pre-wrap; word-break: break-all; text-align: left; vertical-align: top; width: auto;
  }
  table.result td.verdict { font-size: 9pt; text-align: left; vertical-align: middle; white-space: normal; word-break: break-all; }
  table.result td.summary {
    padding: 2mm 3mm; font-size: 10.5pt; line-height: 1.6;
    text-align: left; vertical-align: top; border-top: 1.5pt solid #333;
  }
  .device-section { page-break-after: always; page-break-inside: avoid; }
  @media print {
    body { background: #fff; padding: 0; display: block; }
    .page-wrapper { max-width: none; padding: 0; box-shadow: none; margin: 0; }
    table.result th { -webkit-print-color-adjust: exact; print-color-adjust: exact; }
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
  <h3 class="section-subtitle">巡检记录</h3>
  <table class="result">
    <thead>
      <tr><th>序号</th><th>巡检项目</th><th>巡检内容</th><th>评判结论</th></tr>
    </thead>
    <tbody>
      {{inspection_rows}}
    </tbody>
  </table>
</div>"#;

/// Build an HTML inspection report for a completed batch.
/// Returns the full HTML string.
pub fn build_report_html(conn: &rusqlite::Connection, batch_id: i64) -> Result<String, String> {
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

    // 5. Assemble
    let html = REPORT_HTML
        .replace("{{report_title}}", &report_title)
        .replace("{{report_meta}}", &report_meta)
        .replace("{{device_sections}}", &device_sections);

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
        let detail = format!(
            "<span class=\"cmd\">&lt;{}&gt; {}</span>\n{}",
            html_escape(&cmd_label),
            html_escape(&cmd_label),
            html_escape(output),
        );

        let verdict_html = if let Some(jdg) = judgments.get(cmd) {
            let status = jdg.get("status").and_then(|v| v.as_str()).unwrap_or("");
            let finding = jdg.get("finding").and_then(|v| v.as_str()).unwrap_or("");
            let suggestion = jdg.get("suggestion").and_then(|v| v.as_str()).unwrap_or("");

            let mut v = format!(
                "<span class=\"verdict-line\">{}：{}</span>",
                html_escape(status),
                html_escape(finding),
            );
            if !suggestion.is_empty() {
                v.push_str(&format!(
                    "<span class=\"verdict-line\">建议: {}</span>",
                    html_escape(suggestion),
                ));
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
