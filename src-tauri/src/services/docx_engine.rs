use std::collections::HashMap;
use std::fs;
use docx_rs::*;

use crate::db::models::{Device, InspectionRecord};

/// 用模板 docx 生成巡检报告
pub fn generate_docx_report(
    template_path: &str,
    output_path: &str,
    device: &Device,
    record: &InspectionRecord,
    cmd_descs: &HashMap<String, String>,
) -> Result<(), String> {
    // 1. 读取模板文件
    let template_bytes = fs::read(template_path)
        .map_err(|e| format!("读取模板文件失败: {}", e))?;

    let mut docx = read_docx(&template_bytes)
        .map_err(|e| format!("解析 docx 模板失败: {:?}", e))?;

    // 2. 构建变量映射
    let variables = build_variables(device, record, cmd_descs);

    // 3. 解析巡检数据（用于动态行）
    let inspection_rows = build_inspection_rows(record, cmd_descs);

    // 4. 遍历文档，替换变量 + 处理动态表格行
    let mut new_children: Vec<DocumentChild> = Vec::new();
    let children = std::mem::take(&mut docx.document.children);

    for child in children {
        match child {
            DocumentChild::Table(table) => {
                let mut table = *table;
                // 检查表格是否包含动态行模板（含 {{seq}} 的行）
                let has_dynamic = table.rows.iter().any(|row| {
                    row_contains_text(row, "{{seq}}")
                });

                if has_dynamic && !inspection_rows.is_empty() {
                    // 找到模板行并克隆
                    let mut new_rows: Vec<TableChild> = Vec::new();
                    for row in &table.rows {
                        if row_contains_text(row, "{{seq}}") {
                            // 这是模板行，为每条巡检数据克隆一行
                            for (i, insp) in inspection_rows.iter().enumerate() {
                                let mut cloned = row.clone();
                                replace_in_row(&mut cloned, &[
                                    ("{{seq}}", &(i + 1).to_string()),
                                    ("{{cmd}}", &insp.cmd),
                                    ("{{output}}", &insp.output),
                                    ("{{judgment}}", &insp.judgment),
                                ]);
                                new_rows.push(cloned);
                            }
                        } else {
                            // 普通行，只做变量替换
                            let mut cloned = row.clone();
                            replace_in_row(&mut cloned, &variables_to_pairs(&variables));
                            new_rows.push(cloned);
                        }
                    }
                    table.rows = new_rows;
                    new_children.push(DocumentChild::Table(Box::new(table)));
                } else {
                    // 普通表格，只做变量替换
                    replace_in_table(&mut table, &variables_to_pairs(&variables));
                    new_children.push(DocumentChild::Table(Box::new(table)));
                }
            }
            DocumentChild::Paragraph(p) => {
                let mut p = *p;
                replace_in_paragraph(&mut p, &variables_to_pairs(&variables));
                new_children.push(DocumentChild::Paragraph(Box::new(p)));
            }
            _ => {
                new_children.push(child);
            }
        }
    }

    docx.document.children = new_children;

    // 6. 生成输出文件
    let xml_docx = docx.build();
    let output_file = fs::File::create(output_path)
        .map_err(|e| format!("创建输出文件失败: {}", e))?;
    xml_docx.pack(output_file)
        .map_err(|e| format!("写入 docx 文件失败: {:?}", e))?;

    Ok(())
}

// ============================
// 变量构建
// ============================

struct InspectionRow {
    cmd: String,
    output: String,
    judgment: String,
}

fn build_variables(
    device: &Device,
    record: &InspectionRecord,
    _cmd_descs: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    // 静态设备信息
    vars.insert("{{device_name}}".into(), device.name.clone());
    vars.insert("{{ip}}".into(), device.ip.clone());
    vars.insert("{{vendor}}".into(), device.vendor.clone());
    vars.insert("{{model}}".into(), device.model.clone().unwrap_or_default());
    vars.insert("{{sn}}".into(), device.serial_number.clone().unwrap_or_default());
    vars.insert("{{mfg_date}}".into(), device.manufacturing_date.clone().unwrap_or_default());

    // 报告元信息
    vars.insert("{{report_date}}".into(), chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());

    // AI 综合评判
    vars.insert("{{summary}}".into(), record.summary_judgment.clone().unwrap_or_default());
    vars.insert("{{ai_analysis}}".into(), record.ai_analysis.clone().unwrap_or_default());
    vars.insert("{{ai_suggestions}}".into(), record.ai_suggestions.clone().unwrap_or_default());

    // 如果 SN 为空，尝试从命令输出提取
    if vars.get("{{sn}}").map(|s| s.is_empty()).unwrap_or(true) {
        let sn = extract_sn_from_outputs(record);
        if !sn.is_empty() {
            vars.insert("{{sn}}".into(), sn);
        }
    }

    vars
}

fn variables_to_pairs(vars: &HashMap<String, String>) -> Vec<(&str, &str)> {
    vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
}

/// 构建巡检结果行数据
fn build_inspection_rows(
    record: &InspectionRecord,
    cmd_descs: &HashMap<String, String>,
) -> Vec<InspectionRow> {
    let outputs = parse_json_map(&record.command_outputs);
    let judgments = parse_json_object(&record.command_judgments);

    let mut rows = Vec::new();
    for (cmd, output) in &outputs {
        let cmd_label = cmd_descs.get(cmd).cloned().unwrap_or_else(|| cmd.clone());

        // 截断输出
        let lines: Vec<&str> = output.lines().collect();
        let trimmed = if lines.len() > 30 {
            format!("{}...\n[共 {} 行，已截断]", &lines[..30].join("\n"), lines.len())
        } else if output.len() > 1000 {
            format!("{}...", &output[..1000])
        } else {
            output.clone()
        };

        // AI 评判
        let judgment = if let Some(jdg) = judgments.get(cmd) {
            let status = jdg.get("status").and_then(|v| v.as_str()).unwrap_or("-");
            let finding = jdg.get("finding").and_then(|v| v.as_str()).unwrap_or("-");
            let suggestion = jdg.get("suggestion").and_then(|v| v.as_str()).unwrap_or("");
            if suggestion.is_empty() {
                format!("{}：{}", status, finding)
            } else {
                format!("{}：{}；建议：{}", status, finding, suggestion)
            }
        } else {
            "-".into()
        };

        rows.push(InspectionRow {
            cmd: cmd_label,
            output: trimmed,
            judgment,
        });
    }
    rows
}

// ============================
// 文本替换核心逻辑
// ============================

/// 替换段落中的变量
fn replace_in_paragraph(paragraph: &mut Paragraph, vars: &[(&str, &str)]) {
    // 先合并所有 Run 的文本，检测是否有变量需要替换
    let full_text: String = paragraph.children.iter().filter_map(|c| {
        if let ParagraphChild::Run(r) = c {
            Some(run_text(r))
        } else {
            None
        }
    }).collect();

    let mut needs_replace = false;
    for (key, _) in vars {
        if full_text.contains(key) {
            needs_replace = true;
            break;
        }
    }

    if !needs_replace {
        return;
    }

    // 合并所有文本到第一个 Run，清空其余 Run
    // 这样可以处理 Word 把 {{variable}} 拆成多个 Run 的情况
    let mut merged = String::new();
    let mut first_run_idx = None;
    let mut run_indices = Vec::new();

    for (i, child) in paragraph.children.iter().enumerate() {
        if let ParagraphChild::Run(r) = child {
            merged.push_str(&run_text(r));
            if first_run_idx.is_none() {
                first_run_idx = Some(i);
            }
            run_indices.push(i);
        }
    }

    // 执行替换
    let mut result = merged;
    for (key, val) in vars {
        result = result.replace(key, val);
    }

    // 把替换后的文本放回第一个 Run，清空其余 Run
    if let Some(first_idx) = first_run_idx {
        for &idx in &run_indices {
            if idx == first_idx {
                if let ParagraphChild::Run(ref mut r) = paragraph.children[idx] {
                    set_run_text(r, &result);
                }
            } else {
                if let ParagraphChild::Run(ref mut r) = paragraph.children[idx] {
                    clear_run_text(r);
                }
            }
        }
    }
}

/// 替换表格中的变量
fn replace_in_table(table: &mut Table, vars: &[(&str, &str)]) {
    for row in table.rows.iter_mut() {
        replace_in_row(row, vars);
    }
}

/// 替换表格行中的变量
fn replace_in_row(row: &mut TableChild, vars: &[(&str, &str)]) {
    let TableChild::TableRow(ref mut tr) = row;
    for cell in tr.cells.iter_mut() {
        let TableRowChild::TableCell(ref mut tc) = cell;
        for content in tc.children.iter_mut() {
            if let TableCellContent::Paragraph(ref mut p) = content {
                replace_in_paragraph(p, vars);
            }
        }
    }
}

/// 检查表格行是否包含指定文本
fn row_contains_text(row: &TableChild, text: &str) -> bool {
    let TableChild::TableRow(tr) = row;
    for cell in &tr.cells {
        let TableRowChild::TableCell(tc) = cell;
        for content in &tc.children {
            if let TableCellContent::Paragraph(p) = content {
                let full: String = p.children.iter().filter_map(|c| {
                    if let ParagraphChild::Run(r) = c {
                        Some(run_text(r))
                    } else {
                        None
                    }
                }).collect();
                if full.contains(text) {
                    return true;
                }
            }
        }
    }
    false
}

// ============================
// Run 文本操作辅助
// ============================

/// 提取 Run 中的全部文本
fn run_text(run: &Run) -> String {
    run.children.iter().filter_map(|c| {
        if let RunChild::Text(t) = c {
            Some(t.text.clone())
        } else {
            None
        }
    }).collect()
}

/// 设置 Run 的文本，支持多行（用 Break 元素实现换行）
fn set_run_text(run: &mut Run, text: &str) {
    // 清空所有现有子节点
    run.children.clear();

    // 按换行符拆分，插入 Break 元素
    let lines: Vec<&str> = text.split('\n').collect();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            run.children.push(RunChild::Break(Break::new(BreakType::TextWrapping)));
        }
        if !line.is_empty() {
            run.children.push(RunChild::Text(Text::new(line.to_string())));
        }
    }
    // 如果完全为空，放一个空 Text
    if run.children.is_empty() {
        run.children.push(RunChild::Text(Text::new("")));
    }
}

/// 清空 Run 的文本
fn clear_run_text(run: &mut Run) {
    for child in run.children.iter_mut() {
        if let RunChild::Text(ref mut t) = child {
            t.text.clear();
        }
    }
}

// ============================
// JSON 解析辅助
// ============================

fn parse_json_map(json: &Option<String>) -> Vec<(String, String)> {
    let Some(s) = json else { return vec![] };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(s) else { return vec![] };
    match val {
        serde_json::Value::Object(map) => map.into_iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
            .collect(),
        _ => vec![],
    }
}

fn parse_json_object(json: &Option<String>) -> serde_json::Map<String, serde_json::Value> {
    let Some(s) = json else { return serde_json::Map::new(); };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(s) else { return serde_json::Map::new(); };
    match val {
        serde_json::Value::Object(map) => map,
        _ => serde_json::Map::new(),
    }
}

/// 从命令输出中提取 SN
fn extract_sn_from_outputs(record: &InspectionRecord) -> String {
    let outputs = parse_json_map(&record.command_outputs);
    for (cmd, output) in &outputs {
        let cl = cmd.to_lowercase();
        if cl.contains("display device") || cl.contains("show device") {
            for line in output.lines() {
                let ll = line.to_lowercase();
                if ll.contains("sn:") || ll.contains("serial number") || ll.contains("serialnum") {
                    // 提取冒号后面的部分
                    if let Some(pos) = line.find(':') {
                        let val = line[pos + 1..].trim();
                        if !val.is_empty() && val != "-" {
                            return val.to_string();
                        }
                    }
                    // 也尝试空格分隔
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let last = parts[parts.len() - 1];
                        if !last.is_empty() && last != "-" {
                            return last.to_string();
                        }
                    }
                }
            }
        }
    }
    String::new()
}
