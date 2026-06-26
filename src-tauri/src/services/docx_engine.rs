use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use docx_rs::{
    AlignmentType, BorderType, BreakType, Docx, Footer, Header, HeightRule, LineSpacing, NumPages,
    PageMargin, PageNum, Paragraph, ParagraphBorder, ParagraphBorderPosition, ParagraphBorders,
    Run, RunFonts, Shading, Table, TableBorder, TableBorderPosition, TableBorders, TableCell,
    TableCellMargins, TableLayoutType, TableOfContents, TableRow, VAlignType, WidthType,
};

use super::json_util::{parse_json_map, parse_json_object};
use super::report_config::{ReportTemplateConfig, TableColumn};
use crate::db::models::{Device, InspectionRecord};

#[derive(Debug, Clone)]
pub struct ReportCoverContext {
    pub project_name: String,
    pub inspection_date: String,
    pub inspector: String,
}

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
    cover: &ReportCoverContext,
) -> Result<(), String> {
    tracing::info!("DOCX 报告生成开始: device={}, output={}", device.name, output_path.display());
    let start = std::time::Instant::now();
    // 单设备报告：不包含封面和目录
    let docx = build_record_docx(config, device, record, cmd_descs, false, cover);
    let result = write_docx(docx, output_path);
    let latency = start.elapsed().as_millis();
    match &result {
        Ok(()) => tracing::info!("DOCX 报告生成完成: device={}, output={}, latency={}ms", device.name, output_path.display(), latency),
        Err(e) => tracing::warn!("DOCX 报告生成失败: device={}, latency={}ms, error={}", device.name, latency, e),
    }
    result
}

/// 批次 → 合并到一个 docx，每台设备从新页开始
///
/// `configs` 与 `items` 一一对应，每台设备使用各自厂商匹配的报告模板配置，
/// 避免多厂商批次中其余设备套用首台设备的模板。封面/目录使用首份配置（项目级）。
pub fn generate_combined_docx(
    configs: &[ReportTemplateConfig],
    items: &[(Device, InspectionRecord)],
    cmd_descs: &HashMap<String, String>,
    output_path: &Path,
    cover: &ReportCoverContext,
) -> Result<(), String> {
    if items.is_empty() {
        return Err("没有可用的巡检记录".to_string());
    }
    tracing::info!("DOCX 合并报告生成开始: devices={}, output={}", items.len(), output_path.display());
    let start = std::time::Instant::now();

    // 封面配置：优先使用第一个非空 cover title 的模板（用户定制过的），
    // 找不到则回退到 configs[0]
    let project_config = configs
        .iter()
        .find(|c| !c.cover.title.is_empty())
        .unwrap_or(&configs[0]);
    // 封面
    let mut docx = init_docx_with_vars(project_config, "", &cover.project_name);
    docx = build_cover(docx, project_config, None, cover);

    // 每台设备从新页开始
    for (index, (device, record)) in items.iter().enumerate() {
        let cfg = configs.get(index).unwrap_or(project_config);
        docx = page_break(docx);
        docx = device_heading(
            docx,
            device,
            index + 1,
            cfg.cover.primary_color.trim_start_matches('#'),
        );
        docx = append_record_body(docx, cfg, device, record, cmd_descs);
    }

    // 目录在所有设备之后生成（启用时）
    if project_config.cover.include_toc {
        docx = page_break(docx);
        docx = build_device_catalog(docx, project_config, items);
    }

    let result = write_docx(docx, output_path);
    let latency = start.elapsed().as_millis();
    match &result {
        Ok(()) => tracing::info!("DOCX 合并报告生成完成: devices={}, output={}, latency={}ms", items.len(), output_path.display(), latency),
        Err(e) => tracing::warn!("DOCX 合并报告生成失败: devices={}, latency={}ms, error={}", items.len(), latency, e),
    }
    result
}

/// 批次 → 每台一份 docx 打包成 zip
///
/// `configs` 与 `items` 一一对应，每台设备使用各自厂商匹配的报告模板配置。
pub fn generate_zip_bundle(
    configs: &[ReportTemplateConfig],
    items: &[(Device, InspectionRecord)],
    cmd_descs: &HashMap<String, String>,
    output_path: &Path,
) -> Result<(), String> {
    if items.is_empty() {
        return Err("没有可用的巡检记录".to_string());
    }
    tracing::info!("DOCX ZIP 报告生成开始: devices={}, output={}", items.len(), output_path.display());
    let start = std::time::Instant::now();

    if let Some(parent) = output_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("创建输出目录失败: {}", e))?;
        }
    }

    let zip_file =
        fs::File::create(output_path).map_err(|e| format!("创建 zip 文件失败: {}", e))?;
    let mut zw = zip::ZipWriter::new(zip_file);
    let opts: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let fallback = &configs[0];
    let mut name_counts: HashMap<String, i32> = HashMap::new();
    for (index, (device, record)) in items.iter().enumerate() {
        let cfg = configs.get(index).unwrap_or(fallback);
        let cover = single_device_cover_context(device, record);
        let docx = build_record_docx(cfg, device, record, cmd_descs, false, &cover);
        let buf = pack_docx_to_bytes(docx)?;

        let base = sanitize_filename(&format!(
            "{}-巡检报告",
            if device.name.is_empty() {
                format!("device-{}", device.id)
            } else {
                device.name.clone()
            }
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
    let latency = start.elapsed().as_millis();
    tracing::info!("DOCX ZIP 报告生成完成: devices={}, output={}, latency={}ms", items.len(), output_path.display(), latency);
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
    cover: &ReportCoverContext,
) -> Docx {
    let mut docx = init_docx(config, device);
    if with_cover {
        docx = build_cover(docx, config, Some(device), cover);
        docx = page_break(docx);
    }
    append_record_body(docx, config, device, record, cmd_descs)
}

fn init_docx(config: &ReportTemplateConfig, device: &Device) -> Docx {
    init_docx_with_vars(config, &device.vendor, &device.name)
}

fn init_docx_with_vars(config: &ReportTemplateConfig, vendor: &str, device_name: &str) -> Docx {
    let mut docx = Docx::new();

    // A4 默认 11906 twips 宽，左右 1701 边距 → 内容仅 8504 twips。
    // 报告内表格统一用 9072 twips 宽，需要把页边距收窄到 1417 (≈1") 才能装下，
    // 否则表格和目录右侧会溢出页面。
    docx = docx.page_margin(
        PageMargin::new()
            .top(1440)
            .bottom(1440)
            .left(1417)
            .right(1417),
    );

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("vendor", vendor.to_string());
    vars.insert("device_name", device_name.to_string());

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
    Header::new().add_paragraph(paragraph_with_line(
        build_running_paragraph(template),
        ParagraphBorderPosition::Bottom,
        LineSpacing::new().before(0).after(80),
    ))
}

fn build_footer(template: &str) -> Footer {
    Footer::new().add_paragraph(paragraph_with_line(
        build_running_paragraph(template),
        ParagraphBorderPosition::Top,
        LineSpacing::new().before(80).after(0),
    ))
}

fn paragraph_with_line(
    mut paragraph: Paragraph,
    position: ParagraphBorderPosition,
    spacing: LineSpacing,
) -> Paragraph {
    paragraph.property = paragraph
        .property
        .line_spacing(spacing)
        .set_borders(ParagraphBorders::with_empty().set(line_border(position)));
    paragraph
}

fn line_border(position: ParagraphBorderPosition) -> ParagraphBorder {
    ParagraphBorder::new(position)
        .val(BorderType::Single)
        .size(6)
        .space(4)
        .color("808080")
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
                paragraph =
                    paragraph.add_run(Run::new().add_text(buf.clone()).size(18).fonts(zh_fonts()));
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
                        Run::new()
                            .add_text(format!("{{{{{}}}}}", other))
                            .size(18)
                            .fonts(zh_fonts()),
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

fn build_cover(
    mut docx: Docx,
    config: &ReportTemplateConfig,
    _device: Option<&Device>,
    cover: &ReportCoverContext,
) -> Docx {
    let color = config
        .cover
        .primary_color
        .trim_start_matches('#')
        .to_string();

    let title = if config.cover.title.trim().is_empty() {
        format!("{} 巡检报告", cover.project_name)
    } else {
        let mut t = config.cover.title.clone();
        t = t.replace("{{vendor}}", &cover.inspector); // fallback
        t
    };

    for _ in 0..5 {
        docx = docx.add_paragraph(Paragraph::new());
    }
    docx = docx.add_paragraph(
        Paragraph::new().align(AlignmentType::Center).add_run(
            Run::new()
                .add_text(&title)
                .bold()
                .size(48)
                .color(&color)
                .fonts(zh_fonts()),
        ),
    );

    if !config.cover.subtitle.trim().is_empty() {
        docx = docx.add_paragraph(Paragraph::new());
        docx = docx.add_paragraph(
            Paragraph::new().align(AlignmentType::Center).add_run(
                Run::new()
                    .add_text(&config.cover.subtitle)
                    .size(28)
                    .color("666666")
                    .fonts(zh_fonts()),
            ),
        );
    }

    for _ in 0..6 {
        docx = docx.add_paragraph(Paragraph::new());
    }
    docx = docx.add_paragraph(cover_info_line("巡检日期", &cover.inspection_date));
    docx = docx.add_paragraph(cover_info_line("巡检人员", &cover.inspector));

    docx
}

fn page_break(docx: Docx) -> Docx {
    docx.add_paragraph(Paragraph::new().add_run(Run::new().add_break(BreakType::Page)))
}

fn cover_info_line(label: &str, value: &str) -> Paragraph {
    Paragraph::new().align(AlignmentType::Center).add_run(
        Run::new()
            .add_text(format!("{}：{}", label, value))
            .size(26)
            .fonts(zh_fonts()),
    )
}

fn build_device_catalog(
    mut docx: Docx,
    config: &ReportTemplateConfig,
    items: &[(Device, InspectionRecord)],
) -> Docx {
    let color = config.cover.primary_color.trim_start_matches('#');
    docx = docx.add_paragraph(
        Paragraph::new().align(AlignmentType::Center).add_run(
            Run::new()
                .add_text("设备目录")
                .bold()
                .size(36)
                .color(color)
                .fonts(zh_fonts()),
        ),
    );
    docx = docx.add_paragraph(Paragraph::new());

    if items.is_empty() {
        return docx;
    }

    // 构建手动设备目录表（序号 + 设备名 + 厂商 + IP）
    let cell = |text: &str, bold: bool| -> TableCell {
        let mut run = Run::new().add_text(text).size(22).fonts(zh_fonts());
        if bold { run = run.bold(); }
        TableCell::new().add_paragraph(Paragraph::new().add_run(run))
    };
    let header_cell = |text: &str| -> TableCell {
        cell(text, true).shading(Shading::new().fill("E8E8E8"))
    };
    let mut rows: Vec<TableRow> = Vec::new();
    rows.push(TableRow::new(vec![header_cell("序号"), header_cell("设备名称"), header_cell("厂商"), header_cell("IP 地址")]));
    for (index, (device, _record)) in items.iter().enumerate() {
        rows.push(TableRow::new(vec![
            cell(&format!("{}", index + 1), false),
            cell(&device.name, false),
            cell(&device.vendor, false),
            cell(&device.ip, false),
        ]));
    }

    docx = docx.add_table(
        Table::new(rows)
            .set_grid(vec![800, 3000, 2000, 2000])
    );

    docx
}

fn device_heading(docx: Docx, device: &Device, index: usize, color: &str) -> Docx {
    // 用 Heading1 标题样式（inject_toc_styles 注入定义，带 outlineLvl=0）：
    // - WPS 目录按标题样式收集，pStyle="Heading1" 才会被 TOC \o "1-2" 收为第1级
    // - 导航窗格也显示为顶层大纲项
    // 视觉样式（颜色/字号）由 Run 属性覆盖样式默认值
    docx.add_paragraph(
        Paragraph::new()
            .style("Heading1")
            .line_spacing(LineSpacing::new().before(240).after(160))
            .add_run(
                Run::new()
                    .add_text(format!("{}. {}", index, device.name))
                    .bold()
                    .size(32)
                    .color(color)
                    .fonts(zh_fonts()),
            ),
    )
}

fn single_device_cover_context(device: &Device, record: &InspectionRecord) -> ReportCoverContext {
    ReportCoverContext {
        project_name: device.name.clone(),
        inspection_date: inspection_date(record),
        inspector: "运维人员".to_string(),
    }
}

fn inspection_date(record: &InspectionRecord) -> String {
    record
        .completed_at
        .as_deref()
        .or(record.started_at.as_deref())
        .and_then(|s| s.get(..10))
        .unwrap_or("")
        .to_string()
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
    let color = config
        .cover
        .primary_color
        .trim_start_matches('#')
        .to_string();

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
    // 用 Heading2 标题样式（带 outlineLvl=1）：导航窗格第2层 + TOC \o "1-2" 收为目录第2级
    docx.add_paragraph(
        Paragraph::new()
            .style("Heading2")
            .line_spacing(LineSpacing::new().before(200).after(120))
            .add_run(Run::new().add_text(text).bold().size(28).fonts(zh_fonts())),
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
    let visible: Vec<&_> = config
        .device_info
        .fields
        .iter()
        .filter(|f| f.visible)
        .collect();
    if visible.is_empty() {
        return docx;
    }

    let inspect_time = record
        .completed_at
        .clone()
        .or_else(|| record.started_at.clone())
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());

    let value_for = |key: &str| -> String {
        match key {
            "name" => device.name.clone(),
            "ip" => device.ip.clone(),
            "vendor" => device.vendor.clone(),
            "model" => device.model.clone().unwrap_or_default(),
            "sn" => device.serial_number.clone().unwrap_or_default(),
            "mfg_date" => device.manufacturing_date.clone().unwrap_or_default(),
            "inspect_time" => inspect_time.clone(),
            "sysname" | "hostname" => device.sysname.clone().unwrap_or_default(),
            // 发行版：服务器用 model 字段（detect 把 OS PRETTY_NAME 写入 model）；
            // 网络设备无该字段，留空
            "os_release" => device.model.clone().unwrap_or_default(),
            "cpu_cores" => device
                .cpu_cores
                .map(|n| n.to_string())
                .unwrap_or_default(),
            "memory_gb" => device
                .memory_gb
                .map(|n| {
                    if (n - n.trunc()).abs() < f64::EPSILON {
                        format!("{} GB", n as i64)
                    } else {
                        format!("{:.1} GB", n)
                    }
                })
                .unwrap_or_default(),
            "kernel_version" => device.kernel_version.clone().unwrap_or_default(),
            _ => String::new(),
        }
    };

    let total_dxa: usize = 9072;

    if config.device_info.layout == "table" {
        // 横向表格：第一行全标签，第二行全值
        let col_count = visible.len();
        let col_w = total_dxa / col_count.max(1);
        let label_row = body_row(
            visible
                .iter()
                .map(|f| {
                    TableCell::new()
                        .shading(Shading::new().fill("F2F2F2"))
                        .vertical_align(VAlignType::Center)
                        .add_paragraph(
                            Paragraph::new()
                                .align(AlignmentType::Center)
                                .line_spacing(LineSpacing::new().before(0).after(0))
                                .add_run(
                                    Run::new()
                                        .add_text(f.label.clone())
                                        .bold()
                                        .size(21)
                                        .fonts(zh_fonts()),
                                ),
                        )
                        .width(col_w, WidthType::Dxa)
                })
                .collect(),
        );
        let value_row = body_row(
            visible
                .iter()
                .map(|f| {
                    TableCell::new()
                        .vertical_align(VAlignType::Center)
                        .add_paragraph(
                            Paragraph::new()
                                .align(AlignmentType::Center)
                                .line_spacing(LineSpacing::new().before(0).after(0))
                                .add_run(
                                    Run::new()
                                        .add_text(value_for(&f.key))
                                        .size(21)
                                        .fonts(zh_fonts()),
                                ),
                        )
                        .width(col_w, WidthType::Dxa)
                })
                .collect(),
        );
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
                        .add_run(
                            Run::new()
                                .add_text(text.to_string())
                                .bold()
                                .size(21)
                                .fonts(zh_fonts()),
                        ),
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
                        .add_run(
                            Run::new()
                                .add_text(text.to_string())
                                .size(21)
                                .fonts(zh_fonts()),
                        ),
                )
        };

        let mut rows: Vec<TableRow> = Vec::new();
        let iter = visible.chunks(2);
        for pair in iter {
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
    let columns: Vec<&TableColumn> = config
        .command_table
        .columns
        .iter()
        .filter(|c| c.visible)
        .collect();
    if columns.is_empty() {
        return docx;
    }

    let total_dxa: usize = 9072;
    let total_w: i32 = columns.iter().map(|c| c.width.max(1)).sum();
    let widths: Vec<usize> = columns
        .iter()
        .map(|c| ((c.width.max(1) as f64 / total_w as f64) * total_dxa as f64) as usize)
        .collect();

    // 表头：白底加粗黑字（仿模板）
    let header_cells: Vec<TableCell> = columns
        .iter()
        .enumerate()
        .map(|(i, col)| {
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
        })
        .collect();

    let mut rows = vec![header_row(header_cells)];

    // command_outputs 包含该设备模板里的全部巡检命令输出（静态信息采集已独立于模板运行）
    let outputs: Vec<(String, String)> = parse_json_map(&record.command_outputs)
        .into_iter()
        .collect();
    let judgments = parse_json_object(&record.command_judgments);
    let max_lines = config.command_table.output_max_lines;
    let prompt = device_prompt(device, record);

    for (idx, (cmd, output)) in outputs.iter().enumerate() {
        let item = cmd_descs.get(cmd).cloned().unwrap_or_else(|| cmd.clone());
        // SSH 输出里有些设备会回显命令本身；报告已补 `<sysname>cmd`，所以先去掉裸命令回显行
        // 及带提示符前缀的回显行（如 `<sysname>display version`），避免报告出现重复命令行。
        let without_echo = strip_command_echo(output, cmd, &prompt);
        let truncated = truncate_output(&without_echo, max_lines);
        // 还原终端真实输出：第一行是 <hostname>cmd，后面是设备返回内容
        let output_with_prompt = format!("{}{}\n{}", prompt, cmd, truncated);

        let (status, finding, suggestion) = judgments
            .get(cmd)
            .map(|jdg| {
                (
                    jdg.get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    jdg.get("finding")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    jdg.get("suggestion")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                )
            })
            .unwrap_or_default();
        let judgment_text = combine_judgment(&status, &finding, &suggestion);

        let cells: Vec<TableCell> = columns
            .iter()
            .enumerate()
            .map(|(ci, col)| {
                let (text, mono, fill) = match col.key.as_str() {
                    "seq" => ((idx + 1).to_string(), false, None),
                    "item" => (item.clone(), false, None),
                    "output" => (output_with_prompt.clone(), true, None),
                    "ai_judgment" => (judgment_text.clone(), false, status_fill(&status)),
                    _ => (String::new(), false, None),
                };
                // output 列：左对齐 + 顶部对齐 + 等宽字体，保留原样格式
                // 其他列：水平居中 + 垂直居中
                let is_output = col.key == "output";
                let valign = if is_output {
                    VAlignType::Top
                } else {
                    VAlignType::Center
                };
                let mut cell = TableCell::new()
                    .vertical_align(valign)
                    .width(widths[ci], WidthType::Dxa);
                if let Some(c) = fill {
                    cell = cell.shading(Shading::new().fill(c));
                }
                let safe = if text.is_empty() {
                    String::from(" ")
                } else {
                    text
                };
                for line in safe.split('\n') {
                    let mut run = Run::new().add_text(line).size(21);
                    if mono {
                        run = run.fonts(RunFonts::new().ascii("Consolas").east_asia("仿宋"));
                    } else {
                        run = run.fonts(zh_fonts());
                    }
                    let mut p =
                        Paragraph::new().line_spacing(LineSpacing::new().before(0).after(0));
                    if !is_output {
                        p = p.align(AlignmentType::Center);
                    }
                    p = p.add_run(run);
                    cell = cell.add_paragraph(p);
                }
                cell
            })
            .collect();

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
    let judgments = parse_json_object(&record.command_judgments);

    if !summary_text.is_empty() {
        let overall_label = match summary_text.as_str() {
            "ok" => "整体状态：正常",
            "info" => "整体状态：提示",
            "warning" => "整体状态：警告",
            "critical" => "整体状态：严重",
            _ => "整体状态：—",
        };
        let overall_color = match summary_text.as_str() {
            "ok" => "385723",
            "info" => "1F4E79",
            "warning" => "806000",
            "critical" => "843C0C",
            _ => "595959",
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
    } else if analysis.is_empty() && judgments.is_empty() {
        docx = docx.add_paragraph(
            Paragraph::new()
                .line_spacing(LineSpacing::new().before(80).after(80))
                .add_run(Run::new().add_text("评判结论：").bold().size(24).fonts(zh_fonts())),
        );
    }

    if !analysis.is_empty() {
        for line in analysis.lines() {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .line_spacing(LineSpacing::new().line(360))
                    .add_run(
                        Run::new()
                            .add_text(line.to_string())
                            .size(21)
                            .fonts(zh_fonts()),
                    ),
            );
        }
    }

    if !config.summary.show_problem_table {
        return docx;
    }

    let mut problems: Vec<(String, String, String, String)> = Vec::new();
    for (cmd, jdg) in &judgments {
        let status = jdg.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if status == "warning" || status == "critical" {
            let item = cmd_descs.get(cmd).cloned().unwrap_or_else(|| cmd.clone());
            let finding = jdg
                .get("finding")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let suggestion = jdg
                .get("suggestion")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            problems.push((status.to_string(), item, finding, suggestion));
        }
    }
    if problems.is_empty() {
        if !judgments.is_empty() {
            docx = docx.add_paragraph(Paragraph::new());
            docx = docx.add_paragraph(
                Paragraph::new().add_run(
                    Run::new()
                        .add_text("未发现需关注的问题项。")
                        .size(21)
                        .color("595959")
                        .fonts(zh_fonts()),
                ),
            );
        }
        return docx;
    }

    docx = docx.add_paragraph(Paragraph::new());
    docx = docx.add_paragraph(
        Paragraph::new().add_run(
            Run::new()
                .add_text("问题汇总")
                .bold()
                .size(24)
                .fonts(zh_fonts()),
        ),
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
    let safe = if text.is_empty() {
        " ".to_string()
    } else {
        text.to_string()
    };
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

/// 去除 SSH 输出首行的命令回显。
/// 处理两种形态：裸命令回显（`display version`）和带提示符前缀的回显（`<sysname>display version`）。
/// 报告随后会用 `prompt + cmd` 重新生成首行，故此处必须剥离，否则会出现重复命令行。
fn strip_command_echo(output: &str, cmd: &str, prompt: &str) -> String {
    let mut lines = output.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let first_t = first.trim();
    let cmd_t = cmd.trim();
    // 去掉提示符前缀后再比较，兼容 `<sysname>cmd` / `[sysname]cmd` 等形态
    let after_prompt = first_t
        .strip_prefix(prompt.trim())
        .unwrap_or(first_t)
        .trim_start();
    if first_t.eq_ignore_ascii_case(cmd_t) || after_prompt.eq_ignore_ascii_case(cmd_t) {
        lines.collect::<Vec<_>>().join("\n")
    } else {
        output.to_string()
    }
}

fn truncate_output(output: &str, max_lines: i32) -> String {
    if max_lines <= 0 {
        return output.to_string();
    }
    let lines: Vec<&str> = output.lines().collect();
    let max = max_lines as usize;
    if lines.len() <= max {
        return output.to_string();
    }
    format!(
        "{}\n…[共 {} 行，已截断]",
        lines[..max].join("\n"),
        lines.len()
    )
}

fn status_label(status: &str) -> String {
    match status {
        "ok" => "正常".into(),
        "info" => "提示".into(),
        "warning" => "注意".into(),
        "critical" => "严重".into(),
        "" => String::new(),
        other => other.to_string(),
    }
}

fn status_fill(status: &str) -> Option<&'static str> {
    match status {
        "ok" => Some("E2F0D9"),
        "info" => Some("DEEBF7"),
        "warning" => Some("FFF2CC"),
        "critical" => Some("FBE5D6"),
        _ => None,
    }
}

/// 整合评判结论：状态 + 发现 + 建议 → 一段文本
/// 格式：[状态]\n发现内容\n建议：xxx
fn combine_judgment(status: &str, finding: &str, suggestion: &str) -> String {
    let status_line = status_label(status);
    let mut parts: Vec<String> = Vec::new();
    if !status_line.is_empty() {
        parts.push(format!("【{}】", status_line));
    }
    if !finding.is_empty() {
        parts.push(finding.to_string());
    }
    if !suggestion.is_empty() {
        parts.push(format!("建议：{}", suggestion));
    }
    parts.join("\n")
}

/// 推断设备 CLI 提示符：优先使用设备表保存的真实 sysname。
/// 取不到时才退回 device.name / device.ip。
fn device_prompt(device: &Device, _record: &InspectionRecord) -> String {
    // 静态信息已改由 devices 表的 detect_static_info_if_missing 写入，record.static_info 恒为 "{}"，
    // 故 sysname 直接取 device.sysname（巡检快照）。
    let hostname = device
        .sysname
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| {
            if !device.name.is_empty() {
                device.name.clone()
            } else {
                device.ip.clone()
            }
        });
    let vendor_lower = device.vendor.to_lowercase();
    // Linux 服务器：按用户名决定 root(#) 或普通用户($)
    let is_linux = matches!(
        vendor_lower.as_str(),
        "linux" | "ubuntu" | "centos" | "rocky" | "debian" | "rhel" | "suse" | "fedora" | "almalinux"
    );
    if is_linux {
        let user = device
            .ssh_username
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("user");
        let symbol = if user == "root" { "#" } else { "$" };
        return format!("[{}@{} ~]{}", user, hostname, symbol);
    }
    match vendor_lower.as_str() {
        "h3c" | "华三" | "huawei" | "华为" => format!("<{}>", hostname),
        "cisco" | "思科" | "ruijie" | "锐捷" | "fortinet" | "fortigate" | "飞塔" => {
            format!("{}#", hostname)
        }
        _ => format!("<{}>", hostname),
    }
}

/// 在 docx-rs 生成的 settings.xml 中注入 <w:updateFields w:val="true"/>，
/// 让 Word/WPS 打开文档时自动更新所有字段（PAGEREF 页码、目录等）。
/// docx-rs 0.4.20 不暴露此设置，只能 pack 前手动改 settings 字节。
fn inject_update_fields(mut xml: docx_rs::XMLDocx) -> docx_rs::XMLDocx {
    let s = String::from_utf8_lossy(&xml.settings).into_owned();
    // 插在 <w:settings ...> 开标签之后第一个子元素之前
    let marker = "<w:defaultTabStop";
    let updated = if s.contains("<w:updateFields") {
        s
    } else if let Some(pos) = s.find(marker) {
        let mut out = String::with_capacity(s.len() + 40);
        out.push_str(&s[..pos]);
        out.push_str("<w:updateFields w:val=\"true\" />");
        out.push_str(&s[pos..]);
        out
    } else {
        s
    };
    xml.settings = updated.into_bytes();
    xml
}

/// 往 styles.xml 注入标题样式(Heading1/Heading2)和目录样式(TOC1/TOC2)定义。
///
/// docx-rs 0.4.20 默认只生成 Normal 样式。WPS 的目录生成依赖"标题1/标题2"内置样式
/// （styleId="1"/"2" 或 "Heading1"/"Heading2"），光设 outlineLvl 不够 —— WPS 的
/// `TOC \o` 开关按标题样式收集，认不到样式就生成空目录。这里注入：
///   - Heading1/Heading2：带 outlineLvl，pStyle 引用即成为大纲项
///   - TOC1/TOC2：带右制表位(pos=8000)+点线引导，控制目录条目宽度不溢出
fn inject_toc_styles(mut xml: docx_rs::XMLDocx) -> docx_rs::XMLDocx {
    let s = String::from_utf8_lossy(&xml.styles).into_owned();
    if s.contains("w:styleId=\"Heading1\"") {
        return xml;
    }
    // 在 </w:styles> 前插入
    let marker = "</w:styles>";
    let Some(pos) = s.find(marker) else {
        return xml;
    };
    let mut out = String::with_capacity(s.len() + 1200);
    out.push_str(&s[..pos]);
    // Heading1：标题1，大纲级别0（导航窗格顶层 + TOC 第1级）
    out.push_str(r#"<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1" /><w:basedOn w:val="Normal" /><w:next w:val="Normal" /><w:qFormat /><w:pPr><w:keepNext /><w:keepLines /><w:spacing w:before="240" w:after="120" /><w:outlineLvl w:val="0" /></w:pPr><w:rPr><w:b /><w:bCs /><w:sz w:val="32" /><w:szCs w:val="32" /></w:rPr></w:style>"#);
    // Heading2：标题2，大纲级别1（导航窗格第2层 + TOC 第2级）
    out.push_str(r#"<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2" /><w:basedOn w:val="Normal" /><w:next w:val="Normal" /><w:qFormat /><w:pPr><w:keepNext /><w:keepLines /><w:spacing w:before="200" w:after="100" /><w:outlineLvl w:val="1" /></w:pPr><w:rPr><w:b /><w:bCs /><w:sz w:val="28" /><w:szCs w:val="28" /></w:rPr></w:style>"#);
    // TOC1：目录第1级条目，右 tab @ 8000 + 点线，outlineLvl=9（条目本身不进导航）
    out.push_str(r#"<w:style w:type="paragraph" w:styleId="TOC1"><w:name w:val="toc 1" /><w:basedOn w:val="Normal" /><w:next w:val="Normal" /><w:qFormat /><w:pPr><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="8000" /></w:tabs><w:spacing w:after="80" /><w:outlineLvl w:val="9" /></w:pPr><w:rPr><w:b /><w:bCs /></w:rPr></w:style>"#);
    // TOC2：目录第2级条目，左缩进 + 右 tab @ 8000 + 点线
    out.push_str(r#"<w:style w:type="paragraph" w:styleId="TOC2"><w:name w:val="toc 2" /><w:basedOn w:val="Normal" /><w:next w:val="Normal" /><w:qFormat /><w:pPr><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="8000" /></w:tabs><w:spacing w:after="60" /><w:ind w:left="420" /><w:outlineLvl w:val="9" /></w:pPr></w:style>"#);
    out.push_str(&s[pos..]);
    xml.styles = out.into_bytes();
    xml
}

fn write_docx(docx: Docx, output_path: &Path) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("创建输出目录失败: {}", e))?;
        }
    }
    let f = fs::File::create(output_path).map_err(|e| format!("创建 docx 文件失败: {}", e))?;
    let xml = inject_toc_styles(docx.build());
    inject_update_fields(xml)
        .pack(f)
        .map_err(|e| format!("写入 docx 失败: {:?}", e))?;
    Ok(())
}

fn pack_docx_to_bytes(docx: Docx) -> Result<Vec<u8>, String> {
    let mut buf: Vec<u8> = Vec::new();
    let cursor = std::io::Cursor::new(&mut buf);
    let xml = inject_toc_styles(docx.build());
    inject_update_fields(xml)
        .pack(cursor)
        .map_err(|e| format!("docx 打包失败: {:?}", e))?;
    Ok(buf)
}

fn sanitize_filename(name: &str) -> String {
    let bad: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    let cleaned: String = name
        .chars()
        .map(|c| if bad.contains(&c) { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').to_string();
    if trimmed.is_empty() {
        "report".into()
    } else {
        trimmed
    }
}



