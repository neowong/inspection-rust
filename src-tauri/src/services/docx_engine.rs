use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use docx_rs::{
    AlignmentType, BorderType, Docx, Footer, Header, HeightRule, LineSpacing, NumPages, PageNum,
    Paragraph, Run, RunFonts, Shading, Table, TableBorder, TableBorderPosition, TableBorders,
    TableCell, TableCellMargins, TableLayoutType, TableRow, VAlignType, WidthType,
};

use crate::db::models::{Device, InspectionRecord};
use super::json_util::{parse_json_map, parse_json_object};
use super::report_config::{ReportTemplateConfig, TableColumn};

// ----------------------------------------------------------------
// 公共入口
// ----------------------------------------------------------------

/// 单设备 → 单 docx
pub fn generate_record_docx(
    config: &ReportTemplateConfig,
    device: &Device,
    record: &InspectionRecord,
    cmd_descs: &HashMap<String, String>,
    output_path: &Path,
) -> Result<(), String> {
    let docx = build_record_docx(config, device, record, cmd_descs, true);
    write_docx(docx, output_path)
}

/// 批次 → 合并到一个 docx，每台设备从新页开始
pub fn generate_combined_docx(
    config: &ReportTemplateConfig,
    items: &[(Device, InspectionRecord)],
    cmd_descs: &HashMap<String, String>,
    output_path: &Path,
) -> Result<(), String> {
    if items.is_empty() {
        return Err("没有可用的巡检记录".to_string());
    }

    // 用第一台设备的上下文初始化页眉页脚（变量已替换为真实值）
    let mut docx = init_docx(config, &items[0].0);
    docx = build_cover(docx, config, &items[0].0);
    docx = append_record_body(docx, config, &items[0].0, &items[0].1, cmd_descs);

    for (device, record) in items.iter().skip(1) {
        // 强制分页
        docx = docx.add_paragraph(
            Paragraph::new().page_break_before(true).add_run(Run::new().add_text("")),
        );
        // 简单分隔标题
        docx = docx.add_paragraph(
            Paragraph::new()
                .align(AlignmentType::Center)
                .add_run(
                    Run::new()
                        .add_text(format!("{} ({})", device.name, device.ip))
                        .bold()
                        .size(32)
                        .color(config.cover.primary_color.trim_start_matches('#')),
                ),
        );
        docx = docx.add_paragraph(Paragraph::new());
        docx = append_record_body(docx, config, device, record, cmd_descs);
    }
    write_docx(docx, output_path)
}

/// 批次 → 每台一份 docx 打包成 zip
pub fn generate_zip_bundle(
    config: &ReportTemplateConfig,
    items: &[(Device, InspectionRecord)],
    cmd_descs: &HashMap<String, String>,
    output_path: &Path,
) -> Result<(), String> {
    if items.is_empty() {
        return Err("没有可用的巡检记录".to_string());
    }

    if let Some(parent) = output_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("创建输出目录失败: {}", e))?;
        }
    }

    let zip_file = fs::File::create(output_path)
        .map_err(|e| format!("创建 zip 文件失败: {}", e))?;
    let mut zw = zip::ZipWriter::new(zip_file);
    let opts: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut name_counts: HashMap<String, i32> = HashMap::new();
    for (device, record) in items {
        let docx = build_record_docx(config, device, record, cmd_descs, true);
        let buf = pack_docx_to_bytes(docx)?;

        let base = sanitize_filename(&format!(
            "{}-巡检报告",
            if device.name.is_empty() { format!("device-{}", device.id) } else { device.name.clone() }
        ));
        let count = name_counts.entry(base.clone()).or_insert(0);
        *count += 1;
        let filename = if *count == 1 {
            format!("{}.docx", base)
        } else {
            format!("{}-{}.docx", base, count)
        };

        zw.start_file(&filename, opts)
            .map_err(|e| format!("zip 写入条目失败: {}", e))?;
        zw.write_all(&buf)
            .map_err(|e| format!("zip 写入数据失败: {}", e))?;
    }
    zw.finish().map_err(|e| format!("zip 收尾失败: {}", e))?;
    Ok(())
}

// ----------------------------------------------------------------
// docx 构建主流程
// ----------------------------------------------------------------

fn build_record_docx(
    config: &ReportTemplateConfig,
    device: &Device,
    record: &InspectionRecord,
    cmd_descs: &HashMap<String, String>,
    with_cover: bool,
) -> Docx {
    let mut docx = init_docx(config, device);
    if with_cover {
        docx = build_cover(docx, config, device);
    }
    append_record_body(docx, config, device, record, cmd_descs)
}

fn init_docx(config: &ReportTemplateConfig, device: &Device) -> Docx {
    let mut docx = Docx::new();
    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("vendor", device.vendor.clone());
    vars.insert("device_name", device.name.clone());

    let header = replace_simple_vars(&config.header, &vars);
    if !header.trim().is_empty() {
        docx = docx.header(build_header(&header));
    }
    let footer = replace_simple_vars(&config.footer, &vars);
    if !footer.trim().is_empty() {
        docx = docx.footer(build_footer(&footer));
    }
    docx
}

fn build_header(template: &str) -> Header {
    Header::new()
        .add_paragraph(build_running_paragraph(template))
        .add_table(horizontal_rule_table(TableBorderPosition::Bottom))
}

fn build_footer(template: &str) -> Footer {
    Footer::new()
        .add_table(horizontal_rule_table(TableBorderPosition::Top))
        .add_paragraph(build_running_paragraph(template))
}

fn horizontal_rule_table(position: TableBorderPosition) -> Table {
    let mut borders = TableBorders::with_empty().clear_all();
    borders = borders.set(
        TableBorder::new(position)
            .border_type(BorderType::Single)
            .size(6)
            .color("808080"),
    );
    Table::new(vec![TableRow::new(vec![
        TableCell::new()
            .clear_all_border()
            .add_paragraph(Paragraph::new().line_spacing(LineSpacing::new().before(0).after(0))),
    ])])
    .layout(TableLayoutType::Fixed)
    .set_grid(vec![9072])
    .width(9072, WidthType::Dxa)
    .set_borders(borders)
    .margins(TableCellMargins::new().margin(0, 0, 0, 0))
}

/// 把含 `{{page}} {{total}}` 的字符串构造成段落（用于页眉/页脚）
fn build_running_paragraph(template: &str) -> Paragraph {
    let mut paragraph = Paragraph::new().align(AlignmentType::Center);
    let mut buf = String::new();
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' && chars.peek() == Some(&'{') {
            chars.next();
            let mut tag = String::new();
            while let Some(&c) = chars.peek() {
                if c == '}' {
                    chars.next();
                    if chars.peek() == Some(&'}') {
                        chars.next();
                        break;
                    } else {
                        tag.push('}');
                    }
                } else {
                    tag.push(c);
                    chars.next();
                }
            }
            if !buf.is_empty() {
                paragraph = paragraph.add_run(Run::new().add_text(buf.clone()).size(18).fonts(zh_fonts()));
                buf.clear();
            }
            match tag.trim() {
                "page" => {
                    paragraph = paragraph.add_page_num(PageNum::new());
                }
                "total" => {
                    paragraph = paragraph.add_num_pages(NumPages::new());
                }
                other => {
                    // 未替换的占位原样输出
                    paragraph = paragraph.add_run(
                        Run::new().add_text(format!("{{{{{}}}}}", other)).size(18).fonts(zh_fonts()),
                    );
                }
            }
        } else {
            buf.push(ch);
        }
    }
    if !buf.is_empty() {
        paragraph = paragraph.add_run(Run::new().add_text(buf).size(18).fonts(zh_fonts()));
    }
    paragraph
}

fn replace_simple_vars(template: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = template.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{{{}}}}}", k), v);
    }
    out
}

// ----------------------------------------------------------------
// 封面
// ----------------------------------------------------------------

fn build_cover(mut docx: Docx, config: &ReportTemplateConfig, device: &Device) -> Docx {
    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("vendor", device.vendor.clone());
    vars.insert("device_name", device.name.clone());

    let title = replace_simple_vars(&config.cover.title, &vars);
    let color = config.cover.primary_color.trim_start_matches('#').to_string();

    for _ in 0..3 { docx = docx.add_paragraph(Paragraph::new()); }
    docx = docx.add_paragraph(
        Paragraph::new()
            .align(AlignmentType::Center)
            .add_run(Run::new().add_text(title).bold().size(56).color(&color).fonts(zh_fonts())),
    );

    if !config.cover.subtitle.is_empty() {
        let subtitle = replace_simple_vars(&config.cover.subtitle, &vars);
        docx = docx.add_paragraph(Paragraph::new());
        docx = docx.add_paragraph(
            Paragraph::new()
                .align(AlignmentType::Center)
                .add_run(Run::new().add_text(subtitle).size(32).color("595959").fonts(zh_fonts())),
        );
    }

    for _ in 0..6 { docx = docx.add_paragraph(Paragraph::new()); }
    docx = docx.add_paragraph(
        Paragraph::new()
            .align(AlignmentType::Center)
            .add_run(Run::new().add_text(format!("设备：{}", device.name)).size(28).fonts(zh_fonts())),
    );
    docx = docx.add_paragraph(
        Paragraph::new()
            .align(AlignmentType::Center)
            .add_run(
                Run::new()
                    .add_text(format!(
                        "生成日期：{}",
                        chrono::Local::now().format("%Y-%m-%d %H:%M")
                    ))
                    .size(24)
                    .color("595959")
                    .fonts(zh_fonts()),
            ),
    );

    docx.add_paragraph(
        Paragraph::new().page_break_before(true).add_run(Run::new().add_text("")),
    )
}

// ----------------------------------------------------------------
// 主体：设备信息 + 命令表 + 总结
// ----------------------------------------------------------------

fn append_record_body(
    mut docx: Docx,
    config: &ReportTemplateConfig,
    device: &Device,
    record: &InspectionRecord,
    cmd_descs: &HashMap<String, String>,
) -> Docx {
    let color = config.cover.primary_color.trim_start_matches('#').to_string();

    if config.device_info.enabled {
        docx = section_heading(docx, "设备信息", &color);
        docx = build_device_info(docx, config, device, record);
        docx = docx.add_paragraph(Paragraph::new());
    }

    docx = section_heading(docx, "巡检明细", &color);
    docx = build_command_table(docx, config, device, record, cmd_descs);

    if config.summary.enabled {
        docx = docx.add_paragraph(Paragraph::new());
        docx = section_heading(docx, &config.summary.title, &color);
        docx = build_summary(docx, config, record, cmd_descs);
    }
    docx
}

fn section_heading(docx: Docx, text: &str, _color: &str) -> Docx {
    // 章节标题：仿模板 — 黑字加粗 + 仿宋 + 段前段后留白
    docx.add_paragraph(
        Paragraph::new()
            .line_spacing(LineSpacing::new().before(200).after(120))
            .add_run(
                Run::new()
                    .add_text(text)
                    .bold()
                    .size(28)
                    .fonts(zh_fonts()),
            ),
    )
}

// ----------------------------------------------------------------
// 表格样式辅助：参考 H3C 模板
//   - 外框 sz=6（0.75pt），内线 sz=4（0.5pt），黑色
//   - 单元格左右内边距 108 dxa，上下 0
//   - 行高 atLeast 626 dxa（≈ 1.1cm）
// ----------------------------------------------------------------

const BORDER_OUTER: usize = 6;
const BORDER_INNER: usize = 4;
const ROW_HEIGHT: u32 = 626;

/// 中文仿宋 + 西文 Ubuntu，匹配模板字体
fn zh_fonts() -> RunFonts {
    RunFonts::new()
        .ascii("Ubuntu")
        .hi_ansi("Ubuntu")
        .east_asia("仿宋")
        .cs("Ubuntu")
}

fn pretty_borders() -> TableBorders {
    let make = |pos: TableBorderPosition, sz: usize| {
        TableBorder::new(pos)
            .border_type(BorderType::Single)
            .size(sz)
            .color("auto")
    };
    TableBorders::new()
        .set(make(TableBorderPosition::Top, BORDER_OUTER))
        .set(make(TableBorderPosition::Bottom, BORDER_OUTER))
        .set(make(TableBorderPosition::Left, BORDER_OUTER))
        .set(make(TableBorderPosition::Right, BORDER_OUTER))
        .set(make(TableBorderPosition::InsideH, BORDER_INNER))
        .set(make(TableBorderPosition::InsideV, BORDER_INNER))
}

fn pretty_cell_margins() -> TableCellMargins {
    TableCellMargins::new().margin(0, 108, 0, 108)
}

fn header_row(cells: Vec<TableCell>) -> TableRow {
    TableRow::new(cells)
        .row_height(ROW_HEIGHT as f32)
        .height_rule(HeightRule::AtLeast)
        .cant_split()
}

fn body_row(cells: Vec<TableCell>) -> TableRow {
    TableRow::new(cells)
        .row_height(ROW_HEIGHT as f32)
        .height_rule(HeightRule::AtLeast)
}

// ----------------------------------------------------------------
// 设备信息表
// ----------------------------------------------------------------

fn build_device_info(
    docx: Docx,
    config: &ReportTemplateConfig,
    device: &Device,
    record: &InspectionRecord,
) -> Docx {
    let visible: Vec<&_> = config.device_info.fields.iter().filter(|f| f.visible).collect();
    if visible.is_empty() { return docx; }

    let inspect_time = record
        .completed_at
        .clone()
        .or_else(|| record.started_at.clone())
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());

    let value_for = |key: &str| -> String {
        match key {
            "name"         => device.name.clone(),
            "ip"           => device.ip.clone(),
            "vendor"       => device.vendor.clone(),
            "model"        => device.model.clone().unwrap_or_default(),
            "sn"           => device.serial_number.clone().unwrap_or_default(),
            "mfg_date"     => device.manufacturing_date.clone().unwrap_or_default(),
            "inspect_time" => inspect_time.clone(),
            _              => String::new(),
        }
    };

    let total_dxa: usize = 9072;

    if config.device_info.layout == "table" {
        // 横向表格：第一行全标签，第二行全值
        let col_count = visible.len();
        let col_w = total_dxa / col_count.max(1);
        let label_row = body_row(visible.iter().map(|f| {
            TableCell::new()
                .shading(Shading::new().fill("F2F2F2"))
                .vertical_align(VAlignType::Center)
                .add_paragraph(
                    Paragraph::new()
                        .align(AlignmentType::Center)
                        .line_spacing(LineSpacing::new().before(0).after(0))
                        .add_run(Run::new().add_text(f.label.clone()).bold().size(21).fonts(zh_fonts())),
                )
                .width(col_w, WidthType::Dxa)
        }).collect());
        let value_row = body_row(visible.iter().map(|f| {
            TableCell::new()
                .vertical_align(VAlignType::Center)
                .add_paragraph(
                    Paragraph::new()
                        .align(AlignmentType::Center)
                        .line_spacing(LineSpacing::new().before(0).after(0))
                        .add_run(Run::new().add_text(value_for(&f.key)).size(21).fonts(zh_fonts())),
                )
                .width(col_w, WidthType::Dxa)
        }).collect());
        docx.add_table(
            Table::new(vec![label_row, value_row])
                .layout(TableLayoutType::Fixed)
                .set_grid(vec![col_w; col_count])
                .width(total_dxa, WidthType::Dxa)
                .set_borders(pretty_borders())
                .margins(pretty_cell_margins())
                .align(docx_rs::TableAlignmentType::Center),
        )
    } else {
        // two_column 布局 → 仿模板四列两组：标签 | 值 | 标签 | 值
        // 列宽参考 H3C 模板：1831 / 2901 / 1115 / 3049 ≈ 9072 总宽
        let widths = [1831usize, 2901, 1115, 3049];
        let label_a_w = widths[0];
        let value_a_w = widths[1];
        let label_b_w = widths[2];
        let value_b_w = widths[3];

        let mk_label = |text: &str, w: usize| -> TableCell {
            TableCell::new()
                .shading(Shading::new().fill("F2F2F2"))
                .vertical_align(VAlignType::Center)
                .width(w, WidthType::Dxa)
                .add_paragraph(
                    Paragraph::new()
                        .align(AlignmentType::Left)
                        .line_spacing(LineSpacing::new().before(0).after(0))
                        .add_run(Run::new().add_text(text.to_string()).bold().size(21).fonts(zh_fonts())),
                )
        };
        let mk_value = |text: &str, w: usize| -> TableCell {
            TableCell::new()
                .vertical_align(VAlignType::Center)
                .width(w, WidthType::Dxa)
                .add_paragraph(
                    Paragraph::new()
                        .align(AlignmentType::Left)
                        .line_spacing(LineSpacing::new().before(0).after(0))
                        .add_run(Run::new().add_text(text.to_string()).size(21).fonts(zh_fonts())),
                )
        };

        let mut rows: Vec<TableRow> = Vec::new();
        let mut iter = visible.chunks(2);
        while let Some(pair) = iter.next() {
            let f1 = pair[0];
            if pair.len() == 2 {
                let f2 = pair[1];
                rows.push(body_row(vec![
                    mk_label(&f1.label, label_a_w),
                    mk_value(&value_for(&f1.key), value_a_w),
                    mk_label(&f2.label, label_b_w),
                    mk_value(&value_for(&f2.key), value_b_w),
                ]));
            } else {
                // 奇数个字段，最后一行只有左半组，右半组用空格占位
                rows.push(body_row(vec![
                    mk_label(&f1.label, label_a_w),
                    mk_value(&value_for(&f1.key), value_a_w),
                    mk_label("", label_b_w),
                    mk_value("", value_b_w),
                ]));
            }
        }

        docx.add_table(
            Table::new(rows)
                .layout(TableLayoutType::Fixed)
                .set_grid(widths.to_vec())
                .width(total_dxa, WidthType::Dxa)
                .set_borders(pretty_borders())
                .margins(pretty_cell_margins())
                .align(docx_rs::TableAlignmentType::Center),
        )
    }
}

// ----------------------------------------------------------------
// 巡检明细表
// ----------------------------------------------------------------

fn build_command_table(
    docx: Docx,
    config: &ReportTemplateConfig,
    device: &Device,
    record: &InspectionRecord,
    cmd_descs: &HashMap<String, String>,
) -> Docx {
    let columns: Vec<&TableColumn> =
        config.command_table.columns.iter().filter(|c| c.visible).collect();
    if columns.is_empty() { return docx; }

    let total_dxa: usize = 9072;
    let total_w: i32 = columns.iter().map(|c| c.width.max(1)).sum();
    let widths: Vec<usize> = columns.iter()
        .map(|c| ((c.width.max(1) as f64 / total_w as f64) * total_dxa as f64) as usize)
        .collect();

    // 表头：白底加粗黑字（仿模板）
    let header_cells: Vec<TableCell> = columns.iter().enumerate().map(|(i, col)| {
        TableCell::new()
            .vertical_align(VAlignType::Center)
            .add_paragraph(
                Paragraph::new()
                    .align(AlignmentType::Center)
                    .line_spacing(LineSpacing::new().before(0).after(0))
                    .add_run(
                        Run::new()
                            .add_text(col.label.clone())
                            .bold()
                            .size(21)
                            .fonts(zh_fonts()),
                    ),
            )
            .width(widths[i], WidthType::Dxa)
    }).collect();

    let mut rows = vec![header_row(header_cells)];

    let outputs: Vec<(String, String)> = parse_json_map(&record.command_outputs)
        .into_iter()
        .filter(|(cmd, _)| !is_static_info_command(cmd))
        .collect();
    let judgments = parse_json_object(&record.command_judgments);
    let max_lines = config.command_table.output_max_lines;
    let prompt = device_prompt(device, record);

    for (idx, (cmd, output)) in outputs.iter().enumerate() {
        let item = cmd_descs.get(cmd).cloned().unwrap_or_else(|| cmd.clone());
        // SSH 输出里有些设备会回显命令本身；报告已补 `<sysname>cmd`，所以先去掉裸命令回显行。
        let without_echo = strip_command_echo(output, cmd);
        let truncated = truncate_output(&without_echo, max_lines);
        // 还原终端真实输出：第一行是 <hostname>cmd，后面是设备返回内容
        let output_with_prompt = format!("{}{}\n{}", prompt, cmd, truncated);

        let (status, finding, suggestion) = judgments
            .get(cmd)
            .map(|jdg| (
                jdg.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                jdg.get("finding").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                jdg.get("suggestion").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            ))
            .unwrap_or_default();
        let judgment_text = combine_judgment(&status, &finding, &suggestion);

        let cells: Vec<TableCell> = columns.iter().enumerate().map(|(ci, col)| {
            let (text, mono, fill) = match col.key.as_str() {
                "seq"          => ((idx + 1).to_string(),       false, None),
                "item"         => (item.clone(),                false, None),
                "output"       => (output_with_prompt.clone(),  true,  None),
                "ai_judgment"  => (judgment_text.clone(),       false, status_fill(&status)),
                _              => (String::new(),               false, None),
            };
            // output 列：左对齐 + 顶部对齐 + 等宽字体，保留原样格式
            // 其他列：水平居中 + 垂直居中
            let is_output = col.key == "output";
            let valign = if is_output { VAlignType::Top } else { VAlignType::Center };
            let mut cell = TableCell::new()
                .vertical_align(valign)
                .width(widths[ci], WidthType::Dxa);
            if let Some(c) = fill {
                cell = cell.shading(Shading::new().fill(c));
            }
            let safe = if text.is_empty() { String::from(" ") } else { text };
            for line in safe.split('\n') {
                let mut run = Run::new().add_text(line).size(21);
                if mono {
                    run = run.fonts(RunFonts::new().ascii("Consolas").east_asia("仿宋"));
                } else {
                    run = run.fonts(zh_fonts());
                }
                let mut p = Paragraph::new()
                    .line_spacing(LineSpacing::new().before(0).after(0));
                if !is_output { p = p.align(AlignmentType::Center); }
                p = p.add_run(run);
                cell = cell.add_paragraph(p);
            }
            cell
        }).collect();

        rows.push(body_row(cells));
    }

    docx.add_table(
        Table::new(rows)
            .layout(TableLayoutType::Fixed)
            .set_grid(widths.clone())
            .width(total_dxa, WidthType::Dxa)
            .set_borders(pretty_borders())
            .margins(pretty_cell_margins())
            .align(docx_rs::TableAlignmentType::Center),
    )
}

// ----------------------------------------------------------------
// 总结
// ----------------------------------------------------------------

fn build_summary(
    mut docx: Docx,
    config: &ReportTemplateConfig,
    record: &InspectionRecord,
    cmd_descs: &HashMap<String, String>,
) -> Docx {
    let summary_text = record.summary_judgment.clone().unwrap_or_default();
    let analysis = record.ai_analysis.clone().unwrap_or_default();

    let overall_label = match summary_text.as_str() {
        "ok"        => "整体状态：正常",
        "info"      => "整体状态：提示",
        "warning"   => "整体状态：警告",
        "critical"  => "整体状态：严重",
        _           => "整体状态：—",
    };
    let overall_color = match summary_text.as_str() {
        "ok"        => "385723",
        "info"      => "1F4E79",
        "warning"   => "806000",
        "critical"  => "843C0C",
        _           => "595959",
    };
    docx = docx.add_paragraph(
        Paragraph::new()
            .line_spacing(LineSpacing::new().before(80).after(80))
            .add_run(
                Run::new()
                    .add_text(overall_label)
                    .bold()
                    .size(24)
                    .color(overall_color)
                    .fonts(zh_fonts()),
            ),
    );
    if !analysis.is_empty() {
        for line in analysis.lines() {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .line_spacing(LineSpacing::new().line(360))
                    .add_run(Run::new().add_text(line.to_string()).size(21).fonts(zh_fonts())),
            );
        }
    }

    if !config.summary.show_problem_table {
        return docx;
    }

    let judgments = parse_json_object(&record.command_judgments);
    let mut problems: Vec<(String, String, String, String)> = Vec::new();
    for (cmd, jdg) in &judgments {
        let status = jdg.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if status == "warning" || status == "critical" {
            let item = cmd_descs.get(cmd).cloned().unwrap_or_else(|| cmd.clone());
            let finding = jdg.get("finding").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let suggestion = jdg.get("suggestion").and_then(|v| v.as_str()).unwrap_or("").to_string();
            problems.push((status.to_string(), item, finding, suggestion));
        }
    }
    if problems.is_empty() {
        docx = docx.add_paragraph(Paragraph::new());
        docx = docx.add_paragraph(
            Paragraph::new()
                .add_run(Run::new().add_text("未发现需关注的问题项。").size(21).color("595959").fonts(zh_fonts())),
        );
        return docx;
    }

    docx = docx.add_paragraph(Paragraph::new());
    docx = docx.add_paragraph(
        Paragraph::new().add_run(Run::new().add_text("问题汇总").bold().size(24).fonts(zh_fonts())),
    );

    let total_dxa: usize = 9072;
    let widths = vec![1200usize, 2400, 2700, 2772];
    let mut rows = vec![header_row(vec![
        header_cell("状态", widths[0], ""),
        header_cell("巡检项目", widths[1], ""),
        header_cell("发现", widths[2], ""),
        header_cell("建议", widths[3], ""),
    ])];
    for (status, item, finding, suggestion) in problems {
        rows.push(body_row(vec![
            text_cell(&status_label(&status), widths[0], status_fill(&status)),
            text_cell(&item, widths[1], None),
            text_cell(&finding, widths[2], None),
            text_cell(&suggestion, widths[3], None),
        ]));
    }
    docx.add_table(
        Table::new(rows)
            .layout(TableLayoutType::Fixed)
            .set_grid(widths)
            .width(total_dxa, WidthType::Dxa)
            .set_borders(pretty_borders())
            .margins(pretty_cell_margins())
            .align(docx_rs::TableAlignmentType::Center),
    )
}

// ----------------------------------------------------------------
// 辅助
// ----------------------------------------------------------------

fn header_cell(label: &str, width: usize, _primary: &str) -> TableCell {
    // 仿模板：白底加粗黑字
    TableCell::new()
        .vertical_align(VAlignType::Center)
        .add_paragraph(
            Paragraph::new()
                .align(AlignmentType::Center)
                .line_spacing(LineSpacing::new().before(0).after(0))
                .add_run(Run::new().add_text(label).bold().size(21).fonts(zh_fonts())),
        )
        .width(width, WidthType::Dxa)
}

fn text_cell(text: &str, width: usize, fill: Option<&str>) -> TableCell {
    let mut cell = TableCell::new()
        .vertical_align(VAlignType::Center)
        .width(width, WidthType::Dxa);
    if let Some(c) = fill {
        cell = cell.shading(Shading::new().fill(c));
    }
    let safe = if text.is_empty() { " ".to_string() } else { text.to_string() };
    for line in safe.split('\n') {
        cell = cell.add_paragraph(
            Paragraph::new()
                .align(AlignmentType::Center)
                .line_spacing(LineSpacing::new().before(0).after(0))
                .add_run(Run::new().add_text(line).size(21).fonts(zh_fonts())),
        );
    }
    cell
}

fn strip_command_echo(output: &str, cmd: &str) -> String {
    let mut lines = output.lines();
    let Some(first) = lines.next() else { return String::new(); };
    if first.trim().eq_ignore_ascii_case(cmd.trim()) {
        lines.collect::<Vec<_>>().join("\n")
    } else {
        output.to_string()
    }
}

fn truncate_output(output: &str, max_lines: i32) -> String {
    if max_lines <= 0 { return output.to_string(); }
    let lines: Vec<&str> = output.lines().collect();
    let max = max_lines as usize;
    if lines.len() <= max { return output.to_string(); }
    format!("{}\n…[共 {} 行，已截断]", lines[..max].join("\n"), lines.len())
}

fn status_label(status: &str) -> String {
    match status {
        "ok"        => "正常".into(),
        "info"      => "提示".into(),
        "warning"   => "注意".into(),
        "critical"  => "严重".into(),
        ""          => String::new(),
        other       => other.to_string(),
    }
}

fn status_fill(status: &str) -> Option<&'static str> {
    match status {
        "ok"       => Some("E2F0D9"),
        "info"     => Some("DEEBF7"),
        "warning"  => Some("FFF2CC"),
        "critical" => Some("FBE5D6"),
        _          => None,
    }
}

/// 整合 AI 评判：状态 + 发现 + 建议 → 一段文本
/// 格式：[状态]\n发现内容\n建议：xxx
fn combine_judgment(status: &str, finding: &str, suggestion: &str) -> String {
    let status_line = status_label(status);
    let mut parts: Vec<String> = Vec::new();
    if !status_line.is_empty() {
        parts.push(format!("【{}】", status_line));
    }
    if !finding.is_empty() { parts.push(finding.to_string()); }
    if !suggestion.is_empty() { parts.push(format!("建议：{}", suggestion)); }
    parts.join("\n")
}

/// 推断设备 CLI 提示符：优先使用设备表保存的真实 sysname。
/// 取不到时才退回 device.name / device.ip。
fn device_prompt(device: &Device, record: &InspectionRecord) -> String {
    let record_sysname = parse_static_info(&record.static_info)
        .and_then(|m| m.get("sysname").and_then(|v| v.as_str()).map(|s| s.to_string()));
    let hostname = record_sysname
        .or_else(|| device.sysname.as_ref().filter(|s| !s.trim().is_empty()).cloned())
        .unwrap_or_else(|| if !device.name.is_empty() { device.name.clone() } else { device.ip.clone() });
    match device.vendor.to_lowercase().as_str() {
        "h3c" | "华三" | "huawei" | "华为" => format!("<{}>", hostname),
        "cisco" | "思科" | "ruijie" | "锐捷" => format!("{}>", hostname),
        _ => format!("<{}>", hostname),
    }
}

fn parse_static_info(json: &Option<String>) -> Option<serde_json::Map<String, serde_json::Value>> {
    let s = json.as_deref()?;
    serde_json::from_str::<serde_json::Value>(s).ok()?.as_object().cloned()
}

fn is_static_info_command(cmd: &str) -> bool {
    let c = cmd.to_lowercase().replace(char::is_whitespace, "");
    c == "displaycurrent-configuration|includesysname"
        || (c.contains("current-configuration") && c.contains("includesysname"))
}

fn write_docx(docx: Docx, output_path: &Path) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("创建输出目录失败: {}", e))?;
        }
    }
    let f = fs::File::create(output_path).map_err(|e| format!("创建 docx 文件失败: {}", e))?;
    docx.build()
        .pack(f)
        .map_err(|e| format!("写入 docx 失败: {:?}", e))?;
    Ok(())
}

fn pack_docx_to_bytes(docx: Docx) -> Result<Vec<u8>, String> {
    let mut buf: Vec<u8> = Vec::new();
    let cursor = std::io::Cursor::new(&mut buf);
    docx.build()
        .pack(cursor)
        .map_err(|e| format!("docx 打包失败: {:?}", e))?;
    Ok(buf)
}

fn sanitize_filename(name: &str) -> String {
    let bad: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    let cleaned: String = name.chars().map(|c| if bad.contains(&c) { '_' } else { c }).collect();
    let trimmed = cleaned.trim().trim_matches('.').to_string();
    if trimmed.is_empty() { "report".into() } else { trimmed }
}
