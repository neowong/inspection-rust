use std::collections::HashMap;
use regex::Regex;
use serde::Deserialize;

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

fn render_sections(config: &TemplateConfig, ctx: &HashMap<String, serde_json::Value>, format: &str) -> String {
    let mut out = String::new();

    for section in &config.sections {
        if !section.enabled {
            continue;
        }
        let section_md = render_section(section, ctx, format);
        if !section_md.is_empty() {
            out.push_str(&section_md);
            out.push('\n');
        }
    }

    out
}

fn render_section(section: &TemplateSection, ctx: &HashMap<String, serde_json::Value>, format: &str) -> String {
    let is_html = format == "html";

    match section.section_type.as_str() {
        "title" => {
            if is_html {
                format!(
                    "<h1 class=\"report-title\">{{device_name}} 巡检报告</h1>\n<p class=\"report-meta\">生成时间: {{report_timestamp}}</p>"
                )
            } else {
                "# {{device_name}} 巡检报告\n\n> 生成时间: {{report_timestamp}}\n".to_string()
            }
        }

        "basic_info" => {
            let fields = get_section_fields(&section.config, &["device_name", "device_ip", "vendor", "model", "sn", "manufacturing_date"]);
            let labels: HashMap<&str, &str> = [
                ("device_name", "设备名称"), ("device_ip", "IP 地址"),
                ("vendor", "厂商"), ("model", "型号"),
                ("sn", "序列号"), ("manufacturing_date", "生产日期"),
            ].iter().cloned().collect();

            if is_html {
                let mut t = "<h2>基本信息</h2>\n<table class=\"info\">\n".to_string();
                for chunk in fields.chunks(2) {
                    t.push_str("<tr>");
                    for f in chunk {
                        let label = labels.get(f.as_str()).copied().unwrap_or(f.as_str());
                        t.push_str(&format!("<td class=\"label\">{}</td><td>{{{{{}}}}}</td>", label, f));
                    }
                    // Fill empty cells if odd count
                    if chunk.len() == 1 { t.push_str("<td></td><td></td>"); }
                    t.push_str("</tr>\n");
                }
                t.push_str("</table>\n");
                t
            } else {
                let mut t = "## 基本信息\n\n| 项目 | 内容 | 项目 | 内容 |\n|------|------|------|------|\n".to_string();
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
            let show_output = section.config.get("show_output")
                .and_then(|v| v.as_bool()).unwrap_or(true);
            let max_lines = section.config.get("max_output_lines")
                .and_then(|v| v.as_u64()).unwrap_or(60) as usize;

            if is_html {
                let mut t = "<h2>巡检结果</h2>\n<table class=\"result\">\n".to_string();
                t.push_str("<thead><tr><th>序号</th><th>巡检项目</th><th>巡检结果</th><th>评判结论</th>");
                if show_output { t.push_str("<th>原始输出</th>"); }
                t.push_str("</tr></thead>\n<tbody>\n");
                let cmd_desc_map = ctx.get("command_descriptions").and_then(|v| v.as_object());
                if let Some(judgments) = ctx.get("command_judgments").and_then(|v| v.as_object()) {
                    let outputs = ctx.get("command_outputs").and_then(|v| v.as_object());
                    let mut seq = 0u32;
                    for (cmd, jdg) in judgments {
                        seq += 1;
                        let status = jdg.get("status").and_then(|v| v.as_str()).unwrap_or("-");
                        let finding = jdg.get("finding").and_then(|v| v.as_str()).unwrap_or("-");
                        let suggestion = jdg.get("suggestion").and_then(|v| v.as_str()).unwrap_or("-");
                        let conclusion = if suggestion.is_empty() {
                            format!("{}：{}", html_escape_str(status), html_escape_str(finding))
                        } else {
                            format!("{}：{}；建议：{}", html_escape_str(status), html_escape_str(finding), html_escape_str(suggestion))
                        };
                        let display_name = cmd_desc_map
                            .and_then(|m| m.get(cmd))
                            .and_then(|v| v.as_str())
                            .unwrap_or(cmd);
                        t.push_str(&format!("<tr><td class=\"num\">{}</td><td class=\"item\">{}</td><td class=\"detail\">{}</td><td class=\"verdict\">{}</td>",
                            seq, html_escape_str(display_name), html_escape_str(finding), conclusion));
                        if show_output {
                            let raw = outputs.and_then(|o| o.get(cmd)).and_then(|v| v.as_str()).unwrap_or("");
                            let trimmed = trim_output(raw, max_lines);
                            t.push_str(&format!("<td class=\"detail\">{}</td>", html_escape_str(&trimmed)));
                        }
                        t.push_str("</tr>\n");
                    }
                }
                t.push_str("</tbody>\n</table>\n");
                t
            } else {
                let mut t = "## 巡检结果\n\n".to_string();
                t.push_str("| 序号 | 巡检项目 | 巡检结果 | 评判结论 |\n");
                t.push_str("|------|----------|----------|----------|\n");
                t.push_str("{{#each command_judgments}}\n");
                t.push_str("| {{_seq}} | {{command}} | {{finding}} | {{status}}：{{finding}}；建议：{{suggestion}} |\n");
                t.push_str("{{/each}}\n");
                t.push('\n');
                if show_output {
                    t.push_str("### 命令原始输出\n\n");
                    t.push_str("{{#each command_judgments}}\n");
                    t.push_str("**{{command}}**\n```\n{{output}}\n```\n\n");
                    t.push_str("{{/each}}\n");
                }
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

/// Render a template string by replacing {{variable}} and {{#each}}...{{/each}}
/// placeholders with values from the context HashMap.
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

    let html_escape = format == "html";

    // Step 2: Process remaining {{variable}} and {{nested.path}} placeholders
    let var_re = Regex::new(r"\{\{([^}]+)\}\}").unwrap();
    let final_result = var_re.replace_all(&result, |caps: &regex::Captures| {
        let path = caps[1].trim();
        let val = resolve_path(ctx, path);

        if html_escape {
            html_escape_str(&val)
        } else {
            val
        }
    });

    final_result.to_string()
}

/// Resolve a dot-separated path from the context map.
/// e.g., "command_judgments.display version.status" traverses nested objects.
fn resolve_path(ctx: &HashMap<String, serde_json::Value>, path: &str) -> String {
    let parts: Vec<&str> = path.splitn(2, '.').collect();

    if parts.is_empty() {
        return "-".to_string();
    }

    let first = parts[0];
    let value = ctx.get(first);

    match value {
        None => "-".to_string(),
        Some(v) => {
            if parts.len() == 1 {
                // Simple key - return string representation
                v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string())
            } else {
                // Nested path: descend into object
                let rest = parts[1];
                if let Some(obj) = v.as_object() {
                    let nested_val = obj.get(rest);
                    match nested_val {
                        Some(nv) => nv.as_str().map(|s| s.to_string()).unwrap_or_else(|| nv.to_string()),
                        None => "-".to_string(),
                    }
                } else {
                    "-".to_string()
                }
            }
        }
    }
}

fn html_escape_str(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
