/// 生成默认 docx 巡检报告模板
/// 运行: cargo run --bin create_default_template
use docx_rs::*;

fn main() {
    let doc = Docx::new()
        // 标题
        .add_paragraph(
            Paragraph::new()
                .add_run(Run::new().add_text("{{company}} 巡检报告").size(36).bold())
                .align(AlignmentType::Center)
        )
        .add_paragraph(
            Paragraph::new()
                .add_run(Run::new().add_text("生成时间: {{report_date}}").size(20))
                .align(AlignmentType::Center)
        )
        .add_paragraph(Paragraph::new()) // 空行
        // 静态设备信息表（2列 key-value）
        .add_table(
            Table::new(vec![
                // 第1行：设备名称 | IP 地址
                TableRow::new(vec![
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("设备名称").bold()))
                        .width(1500, WidthType::Dxa),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("{{device_name}}"))),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("IP 地址").bold()))
                        .width(1500, WidthType::Dxa),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("{{ip}}"))),
                ]),
                // 第2行：厂商 | 型号
                TableRow::new(vec![
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("厂商").bold())),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("{{vendor}}"))),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("型号").bold())),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("{{model}}"))),
                ]),
                // 第3行：SN | 出厂日期
                TableRow::new(vec![
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("SN").bold())),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("{{sn}}"))),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("出厂日期").bold())),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("{{mfg_date}}"))),
                ]),
            ])
        )
        .add_paragraph(Paragraph::new()) // 空行
        // 动态巡检结果表（4列，含模板行）
        .add_paragraph(
            Paragraph::new()
                .add_run(Run::new().add_text("巡检结果").size(24).bold())
        )
        .add_table(
            Table::new(vec![
                // 表头行
                TableRow::new(vec![
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("序号").bold()))
                        .width(800, WidthType::Dxa),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("巡检项目").bold()))
                        .width(2500, WidthType::Dxa),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("巡检结果").bold()))
                        .width(4000, WidthType::Dxa),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("评判结论").bold()))
                        .width(3000, WidthType::Dxa),
                ]),
                // 模板行（程序会检测 {{seq}} 并克隆此行）
                TableRow::new(vec![
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("{{seq}}"))),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("{{cmd}}"))),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("{{output}}"))),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("{{judgment}}"))),
                ]),
            ])
        )
        .add_paragraph(Paragraph::new()) // 空行
        // 综合评判
        .add_paragraph(
            Paragraph::new()
                .add_run(Run::new().add_text("综合评判").size(24).bold())
        )
        .add_paragraph(
            Paragraph::new()
                .add_run(Run::new().add_text("{{summary}}"))
        );

    // 保存
    let output_path = "data/default_template.docx";
    std::fs::create_dir_all("data").unwrap();
    let file = std::fs::File::create(output_path).unwrap();
    doc.build().pack(file).unwrap();
    println!("默认模板已生成: {}", output_path);
}
