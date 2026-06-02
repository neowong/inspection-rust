use std::collections::HashMap;
use regex::Regex;
use serde::Deserialize;
use super::html_util::html_escape;

/// A section in the visual template config.
#[derive(Debug, Deserialize, Clone)]
pub struct TemplateSection {
    #[serde(rename = "type")]
    pub section_type: String,
    pub enabled: bool,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TemplateConfig {
    pub sections: Vec<TemplateSection>,
}

/// Render a template from its config_json and mode.
/// - mode='visual' with valid config_json → section-based rendering
/// - mode='advanced' or no config → raw content rendering
pub fn render_template_from_config(
    config_json: &str,
    content: &str,
    mode: &str,
    ctx: &HashMap<String, serde_json::Value>,
    format: &str,
) -> String {
    if mode == "visual" && !config_json.is_empty() {
        if let Ok(config) = serde_json::from_str::<TemplateConfig>(config_json) {
            return render_sections(&config, ctx, format);
        }
    }

    // Fallback: render raw content
    if !content.is_empty() {
        return render_template(content, ctx, format);
    }

    // Ultimate fallback: built-in minimal template
    render_default_sections(ctx, format)
}

/// Wrap HTML output with custom CSS and page header/footer.
/// Sanitizes custom_css to prevent `</style>` injection.
pub fn wrap_html_output(
    body: &str,
    custom_css: &str,
    page_header: &str,
    page_footer: &str,
    ctx: &HashMap<String, serde_json::Value>,
) -> String {
    let mut out = String::new();

    if !page_header.is_empty() {
        let rendered = render_template(page_header, ctx, "html");
        out.push_str(&format!("<header class=\"report-header\">{}</header>\n", rendered));
    }

    out.push_str(body);

    if !page_footer.is_empty() {
        let rendered = render_template(page_footer, ctx, "html");
        out.push_str(&format!("<footer class=\"report-footer\">{}</footer>\n", rendered));
    }

    if !custom_css.is_empty() {
        // Sanitize: strip </style> to prevent injection
        let safe_css = custom_css.replace("</style>", "");
        out.push_str(&format!(
            "\n<style>/* custom CSS */\n{}\n</style>",
            safe_css
        ));
    }

    out
}

fn render_sections(config: &TemplateConfig, ctx: &HashMap<String, serde_json::Value>, format: &str) -> String {
    let mut out = String::new();

    for section in &config.sections {
        if !section.enabled {
            continue;
        }
        let rendered = render_section(section, ctx, format);
        if !rendered.is_empty() {
            out.push_str(&rendered);
            out.push('\n');
        }
    }

    out
}

fn render_section(section: &TemplateSection, ctx: &HashMap<String, serde_json::Value>, format: &str) -> String {
    let is_html = format == "html";

    match section.section_type.as_str() {
        "title" => {
            let pattern = section.config.get("title_pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("{{device_name}} 巡检报告");
            let title = render_template(pattern, ctx, format);
            if is_html {
                format!("<h1 class=\"report-title\">{}</h1>\n<p class=\"report-meta\">生成时间: {{report_timestamp}}</p>", html_escape(&title))
            } else {
                format!("# {}\n\n> 生成时间: {{report_timestamp}}\n", title)
            }
        }

        "basic_info" => {
            let fields = get_section_fields(&section.config, &["device_name", "device_ip", "vendor", "model", "sn", "manufacturing_date"]);
            let labels: HashMap<&str, &str> = [
                ("device_name", "设备名称"), ("device_ip", "IP 地址"),
                ("vendor", "厂商"), ("model", "型号"),
                ("sn", "序列号"), ("manufacturing_date", "生产日期"),
                ("hostname", "主机名"), ("os_release", "系统版本"),
                ("kernel", "内核版本"), ("cpu_cores", "CPU 核数"),
                ("mem_total", "内存总量"), ("uptime", "运行时间"),
                ("interface_count", "接口数"), ("vlan_count", "VLAN 数"),
            ].iter().cloned().collect();

            let show_header = section.config.get("show_header")
                .and_then(|v| v.as_bool()).unwrap_or(true);

            if is_html {
                let mut t = String::new();
                if show_header {
                    t.push_str("<h2>基本信息</h2>\n");
                }
                t.push_str("<table class=\"info\">\n");
                for chunk in fields.chunks(2) {
                    t.push_str("<tr>");
                    for f in chunk {
                        let label = labels.get(f.as_str()).copied().unwrap_or(f.as_str());
                        t.push_str(&format!("<td class=\"label\">{}</td><td>{{{{{}}}}}</td>", label, f));
                    }
                    if chunk.len() == 1 { t.push_str("<td></td><td></td>"); }
                    t.push_str("</tr>\n");
                }
                t.push_str("</table>\n");
                t
            } else {
                let mut t = String::new();
                if show_header {
                    t.push_str("## 基本信息\n\n");
                }
                t.push_str("| 项目 | 内容 | 项目 | 内容 |\n|------|------|------|------|\n");
                for chunk in fields.chunks(2) {
                    let mut cells = Vec::new();
                    for f in chunk {
                        let label = labels.get(f.as_str()).copied().unwrap_or(f.as_str());
                        cells.push(label.to_string());
                        cells.push(format!("{{{{{}}}}}", f));
                    }
                    if chunk.len() == 1 {
                        cells.push(String::new());
                        cells.push(String::new());
                    }
                    t.push_str(&format!("| {} |\n", cells.join(" | ")));
                }
                t.push('\n');
                t
            }
        }

        "inspection_results" => {
            let max_lines = section.config.get("max_output_lines")
                .and_then(|v| v.as_u64()).unwrap_or(60) as usize;
            let filter_cats = section.config.get("filter_category")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>());

            if is_html {
                let mut t = "<h2>巡检结果</h2>\n<table class=\"result\">\n".to_string();
                t.push_str("<thead><tr><th>序号</th><th>巡检项目</th><th>巡检结果</th><th>评判结论</th></tr></thead>\n<tbody>\n");
                let cmd_desc_map = ctx.get("command_descriptions").and_then(|v| v.as_object());
                let cmd_cat_map = ctx.get("command_categories").and_then(|v| v.as_object());
                let judgments = ctx.get("command_judgments").and_then(|v| v.as_object());
                let outputs = ctx.get("command_outputs").and_then(|v| v.as_object());

                // 按 command_order 排序遍历，而非 HashMap 随机顺序
                let ordered_cmds: Vec<String> = if let Some(order_arr) = ctx.get("command_order").and_then(|v| v.as_array()) {
                    order_arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
                } else if let Some(jdg) = judgments {
                    jdg.keys().cloned().collect()
                } else {
                    vec![]
                };

                let mut seq = 0u32;
                for cmd in &ordered_cmds {
                    let jdg = judgments.and_then(|j| j.get(cmd));
                    if jdg.is_none() && outputs.and_then(|o| o.get(cmd)).is_none() {
                        continue;
                    }

                    // 按类别过滤
                    if let Some(ref cats) = filter_cats {
                        let cmd_cat = cmd_cat_map.and_then(|m| m.get(cmd)).and_then(|v| v.as_str()).unwrap_or("");
                        if !cats.iter().any(|c| c == cmd_cat) {
                            continue;
                        }
                    }
                    seq += 1;

                    let status = jdg.and_then(|j| j.get("status")).and_then(|v| v.as_str()).unwrap_or("-");
                    let finding = jdg.and_then(|j| j.get("finding")).and_then(|v| v.as_str()).unwrap_or("-");
                    let suggestion = jdg.and_then(|j| j.get("suggestion")).and_then(|v| v.as_str()).unwrap_or("-");
                    let conclusion = if suggestion.is_empty() {
                        format!("{}：{}", html_escape(status), html_escape(finding))
                    } else {
                        format!("{}：{}；建议：{}", html_escape(status), html_escape(finding), html_escape(suggestion))
                    };
                    let display_name = cmd_desc_map
                        .and_then(|m| m.get(cmd))
                        .and_then(|v| v.as_str())
                        .unwrap_or(cmd.as_str());
                    // 巡检结果 = 命令原始输出
                    let raw = outputs.and_then(|o| o.get(cmd)).and_then(|v| v.as_str()).unwrap_or("");
                    let trimmed = trim_output(raw, max_lines);
                    t.push_str(&format!(
                        "<tr><td class=\"num\">{}</td><td class=\"item\">{}</td><td class=\"detail\">{}</td><td class=\"verdict\">{}</td></tr>\n",
                        seq, html_escape(display_name), html_escape(&trimmed), conclusion
                    ));
                }
                // 总结行
                if let Some(summary) = ctx.get("summary_judgment").and_then(|v| v.as_str()) {
                    if !summary.is_empty() {
                        t.push_str(&format!(
                            "<tr><td colspan=\"4\" class=\"summary\"><strong>综合评判：</strong>{}</td></tr>\n",
                            html_escape(summary)
                        ));
                    }
                }
                t.push_str("</tbody>\n</table>\n");
                t
            } else {
                let mut t = "## 巡检结果\n\n".to_string();
                t.push_str("| 序号 | 巡检项目 | 巡检结果 | 评判结论 |\n");
                t.push_str("|------|----------|----------|----------|\n");
                // 使用 each_ordered 命令来按 command_order 排序
                t.push_str("{{#each_ordered command_judgments}}\n");
                t.push_str("| {{_seq}} | {{command}} | {{output}} | {{status}}：{{finding}}；建议：{{suggestion}} |\n");
                t.push_str("{{/each_ordered}}\n");
                t.push('\n');
                t
            }
        }

        "ai_analysis" => {
            if is_html {
                "<h2>AI 分析总结</h2>\n<div class=\"ai-analysis\">{{ai_analysis}}</div>\n".to_string()
            } else {
                "## AI 分析总结\n\n{{ai_analysis}}\n".to_string()
            }
        }

        "overall_assessment" => {
            if is_html {
                "<h2>总体评估</h2>\n<div class=\"overall\"><p><strong>综合判断：</strong>{{summary}}</p>\n<p><strong>建议：</strong>{{ai_suggestions}}</p></div>\n".to_string()
            } else {
                "## 总体评估\n\n**综合判断：** {{summary}}\n\n**建议：** {{ai_suggestions}}\n".to_string()
            }
        }

        "custom_text" => {
            let content = section.config.get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if content.is_empty() {
                return String::new();
            }
            let rendered = render_template(content, ctx, format);
            if is_html {
                format!("<div class=\"custom-text\">{}</div>\n", rendered)
            } else {
                format!("{}\n", rendered)
            }
        }

        "header_footer" => {
            let position = section.config.get("position")
                .and_then(|v| v.as_str())
                .unwrap_or("header");
            let content = section.config.get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if content.is_empty() {
                return String::new();
            }
            let rendered = render_template(content, ctx, format);
            if is_html {
                match position {
                    "footer" => format!("<footer class=\"report-footer\">{}</footer>\n", rendered),
                    _ => format!("<header class=\"report-header\">{}</header>\n", rendered),
                }
            } else {
                match position {
                    "footer" => format!("---\n{}\n", rendered),
                    _ => format!("{}\n\n---\n", rendered),
                }
            }
        }

        "device_summary_table" => {
            let fields = get_section_fields(&section.config, &["device_name", "device_ip", "vendor", "model"]);
            let labels: HashMap<&str, &str> = [
                ("device_name", "设备名称"), ("device_ip", "IP 地址"),
                ("vendor", "厂商"), ("model", "型号"),
                ("sn", "序列号"), ("status", "状态"),
            ].iter().cloned().collect();

            // 批量报告：从 context 中取 devices 数组
            let devices = ctx.get("devices").and_then(|v| v.as_array());
            let Some(devices) = devices else {
                return String::new();
            };
            if devices.is_empty() {
                return String::new();
            }

            if is_html {
                let mut t = "<h2>设备汇总</h2>\n<table class=\"info\">\n<thead><tr>".to_string();
                for f in &fields {
                    let label = labels.get(f.as_str()).copied().unwrap_or(f.as_str());
                    t.push_str(&format!("<th>{}</th>", html_escape(label)));
                }
                t.push_str("</tr></thead>\n<tbody>\n");
                for dev in devices {
                    t.push_str("<tr>");
                    for f in &fields {
                        let val = dev.get(f.as_str())
                            .and_then(|v| v.as_str())
                            .unwrap_or("-");
                        t.push_str(&format!("<td>{}</td>", html_escape(val)));
                    }
                    t.push_str("</tr>\n");
                }
                t.push_str("</tbody>\n</table>\n");
                t
            } else {
                let mut t = "## 设备汇总\n\n".to_string();
                let headers: Vec<String> = fields.iter()
                    .map(|f| labels.get(f.as_str()).copied().unwrap_or(f.as_str()).to_string())
                    .collect();
                t.push_str(&format!("| {} |\n", headers.join(" | ")));
                t.push_str(&format!("|{}|\n", headers.iter().map(|_| "------").collect::<Vec<_>>().join("|")));
                for dev in devices {
                    let row: Vec<String> = fields.iter()
                        .map(|f| dev.get(f.as_str()).and_then(|v| v.as_str()).unwrap_or("-").to_string())
                        .collect();
                    t.push_str(&format!("| {} |\n", row.join(" | ")));
                }
                t.push('\n');
                t
            }
        }

        _ => String::new(),
    }
}

/// Default section set used as ultimate fallback.
fn render_default_sections(ctx: &HashMap<String, serde_json::Value>, format: &str) -> String {
    let config = TemplateConfig {
        sections: vec![
            TemplateSection { section_type: "title".into(), enabled: true, label: "报告标题".into(), config: serde_json::Value::Object(Default::default()) },
            TemplateSection { section_type: "basic_info".into(), enabled: true, label: "基本信息".into(), config: serde_json::Value::Object(Default::default()) },
            TemplateSection { section_type: "inspection_results".into(), enabled: true, label: "巡检结果".into(), config: serde_json::Value::Object(Default::default()) },
            TemplateSection { section_type: "ai_analysis".into(), enabled: true, label: "AI 分析总结".into(), config: serde_json::Value::Object(Default::default()) },
            TemplateSection { section_type: "overall_assessment".into(), enabled: true, label: "总体评估".into(), config: serde_json::Value::Object(Default::default()) },
        ],
    };
    render_sections(&config, ctx, format)
}

fn get_section_fields(config: &serde_json::Value, defaults: &[&str]) -> Vec<String> {
    if let Some(fields) = config.get("fields").and_then(|v| v.as_array()) {
        fields.iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.to_string())
            .collect()
    } else {
        defaults.iter().map(|s| s.to_string()).collect()
    }
}

fn trim_output(output: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() > max_lines {
        format!("{}...\n[输出已截断，共 {} 行]", &lines[..max_lines].join("\n"), lines.len())
    } else {
        output.to_string()
    }
}

/// Render a template string by replacing {{variable}}, {{#each}}, and {{#if}} placeholders.
pub fn render_template(template: &str, ctx: &HashMap<String, serde_json::Value>, format: &str) -> String {
    let mut result = template.to_string();

    // Step 1: Process {{#each KEY}}...{{/each}} blocks
    let each_re = Regex::new(r"\{\{#each\s+(\S+)\}\}([\s\S]*?)\{\{/each\}\}").unwrap();
    result = each_re.replace_all(&result, |caps: &regex::Captures| {
        let key = &caps[1];
        let inner = &caps[2];
        let items = ctx.get(key);

        match items {
            Some(serde_json::Value::Object(map)) => {
                let mut out = String::new();
                let mut seq = 0u32;
                let cmd_desc_map = ctx.get("command_descriptions").and_then(|v| v.as_object());
                for (cmd_name, jdg) in map {
                    seq += 1;
                    let mut rendered = inner.to_string();
                    rendered = rendered.replace("{{_seq}}", &seq.to_string());
                    let display_name = cmd_desc_map
                        .and_then(|m| m.get(cmd_name))
                        .and_then(|v| v.as_str())
                        .unwrap_or(cmd_name);
                    rendered = rendered.replace("{{command}}", display_name);

                    let status = jdg.get("status").and_then(|v| v.as_str()).unwrap_or("-");
                    let finding = jdg.get("finding").and_then(|v| v.as_str()).unwrap_or("-");
                    let suggestion = jdg.get("suggestion").and_then(|v| v.as_str()).unwrap_or("-");

                    rendered = rendered.replace("{{status}}", status);
                    rendered = rendered.replace("{{finding}}", finding);
                    rendered = rendered.replace("{{suggestion}}", suggestion);

                    let output = ctx.get("command_outputs")
                        .and_then(|co| co.as_object())
                        .and_then(|co| co.get(cmd_name))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    rendered = rendered.replace("{{output}}", output);

                    out.push_str(&rendered);
                }
                out
            }
            Some(serde_json::Value::Array(arr)) => {
                let mut out = String::new();
                let mut seq = 0u32;
                for item in arr {
                    seq += 1;
                    let mut rendered = inner.to_string();
                    rendered = rendered.replace("{{_seq}}", &seq.to_string());
                    if let Some(obj) = item.as_object() {
                        for (k, v) in obj {
                            let placeholder = format!("{{{{{}}}}}", k);
                            let val = v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string());
                            rendered = rendered.replace(&placeholder, &val);
                        }
                    }
                    out.push_str(&rendered);
                }
                out
            }
            _ => {
                String::new()
            }
        }
    }).to_string();

    // Step 1b: Process {{#each_ordered KEY}}...{{/each_ordered}} blocks
    // Same as each but uses command_order array for iteration order
    let each_ordered_re = Regex::new(r"\{\{#each_ordered\s+(\S+)\}\}([\s\S]*?)\{\{/each_ordered\}\}").unwrap();
    result = each_ordered_re.replace_all(&result, |caps: &regex::Captures| {
        let key = &caps[1];
        let inner = &caps[2];

        let order = ctx.get("command_order").and_then(|v| v.as_array());
        let items = ctx.get(key).and_then(|v| v.as_object());
        let cmd_desc_map = ctx.get("command_descriptions").and_then(|v| v.as_object());
        let outputs = ctx.get("command_outputs").and_then(|v| v.as_object());

        let ordered_keys: Vec<String> = if let Some(arr) = order {
            arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
        } else if let Some(map) = items {
            map.keys().cloned().collect()
        } else {
            return String::new();
        };

        let mut out = String::new();
        let mut seq = 0u32;
        for cmd_name in &ordered_keys {
            let jdg = items.and_then(|m| m.get(cmd_name));
            if jdg.is_none() && outputs.and_then(|o| o.get(cmd_name)).is_none() {
                continue;
            }
            seq += 1;
            let mut rendered = inner.to_string();
            rendered = rendered.replace("{{_seq}}", &seq.to_string());
            let display_name = cmd_desc_map
                .and_then(|m| m.get(cmd_name))
                .and_then(|v| v.as_str())
                .unwrap_or(cmd_name);
            rendered = rendered.replace("{{command}}", display_name);

            let status = jdg.and_then(|j| j.get("status")).and_then(|v| v.as_str()).unwrap_or("-");
            let finding = jdg.and_then(|j| j.get("finding")).and_then(|v| v.as_str()).unwrap_or("-");
            let suggestion = jdg.and_then(|j| j.get("suggestion")).and_then(|v| v.as_str()).unwrap_or("-");

            rendered = rendered.replace("{{status}}", status);
            rendered = rendered.replace("{{finding}}", finding);
            rendered = rendered.replace("{{suggestion}}", suggestion);

            let output = outputs
                .and_then(|co| co.get(cmd_name))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            rendered = rendered.replace("{{output}}", output);

            out.push_str(&rendered);
        }
        out
    }).to_string();

    // Step 2: Process {{#if KEY}}...{{/if}} blocks (with optional {{else}})
    let if_re = Regex::new(r"\{\{#if\s+(\S+)\}\}([\s\S]*?)\{\{/if\}\}").unwrap();
    result = if_re.replace_all(&result, |caps: &regex::Captures| {
        let key = &caps[1];
        let body = &caps[2];
        let val = resolve_path(ctx, key);
        let is_truthy = !val.is_empty() && val != "-" && val != "null";

        // Check for {{else}} inside the body
        if let Some(else_pos) = body.find("{{else}}") {
            let if_part = &body[..else_pos];
            let else_part = &body[else_pos + 8..]; // len of "{{else}}" = 8
            if is_truthy { if_part.to_string() } else { else_part.to_string() }
        } else {
            if is_truthy { body.to_string() } else { String::new() }
        }
    }).to_string();

    let is_html = format == "html";

    // Step 3: Process remaining {{variable}} and {{nested.path}} placeholders
    let var_re = Regex::new(r"\{\{([^}]+)\}\}").unwrap();
    let final_result = var_re.replace_all(&result, |caps: &regex::Captures| {
        let path = caps[1].trim();
        let val = resolve_path(ctx, path);

        if is_html {
            html_escape(&val)
        } else {
            val
        }
    });

    final_result.to_string()
}

/// Resolve a dot-separated path from the context map.
/// First tries the full path as a literal key (handles keys with spaces/dots like "display version"),
/// then progressively splits from the right for nested access.
fn resolve_path(ctx: &HashMap<String, serde_json::Value>, path: &str) -> String {
    if path.is_empty() {
        return "-".to_string();
    }

    // Try the full path as a literal key first (handles "display version", etc.)
    if let Some(v) = ctx.get(path) {
        return v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string());
    }

    // Split on the first dot for top-level.nested access
    if let Some(dot_pos) = path.find('.') {
        let first = &path[..dot_pos];
        let rest = &path[dot_pos + 1..];

        if let Some(v) = ctx.get(first) {
            return resolve_nested(v, rest);
        }
    }

    "-".to_string()
}

/// Recursively resolve a nested path within a JSON value.
/// Tries the full remaining path as a literal key first, then splits on '.'.
fn resolve_nested(value: &serde_json::Value, path: &str) -> String {
    // Try full path as literal key (for keys like "display version")
    if let Some(obj) = value.as_object() {
        if let Some(v) = obj.get(path) {
            return v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string());
        }
    }

    // Try splitting on the first dot
    if let Some(dot_pos) = path.find('.') {
        let first = &path[..dot_pos];
        let rest = &path[dot_pos + 1..];

        if let Some(obj) = value.as_object() {
            if let Some(v) = obj.get(first) {
                return resolve_nested(v, rest);
            }
        }
    }

    // Try as direct key (no more dots)
    if let Some(obj) = value.as_object() {
        if let Some(v) = obj.get(path) {
            return v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string());
        }
    }

    "-".to_string()
}
