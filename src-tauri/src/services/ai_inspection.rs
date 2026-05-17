/// AI 巡检评判服务
///
/// 对应 Python: backend/app/services/ai_inspection.py
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

pub const SYSTEM_PROMPT: &str = r#"你是一位专业的 IT 运维巡检工程师，负责分析设备巡检命令输出，判断设备运行状态是否正常。

你可能面对的设备类型包括：网络设备（交换机、路由器、防火墙）、Linux 服务器、数据库、安全设备等。
请根据实际命令内容自动判断设备类型和巡检上下文，不要假设一定是网络设备。

对于每台设备，你会收到一组命令及其输出。你的任务是：
1. 对收到的**每一条命令**逐条进行评判，不允许跳过任何命令
2. 每条命令给出状态判定（ok=正常 / info=注意 / warning=警告 / critical=严重）
3. 给出简要的整体总结

请用 JSON 格式返回分析结果：
{
  "summary": "整体状态总结，一句话",
  "overall": "ok/info/warning/critical",
  "items": [
    {
      "command": "命令名称",
      "title": "巡检项目简短概括，≤6个字",
      "status": "ok/info/warning/critical",
      "finding": "判断结论（不超过12字，正常写'正常'）",
      "suggestion": "建议或改进措施（正常时为空，异常时不超过15字）"
    }
  ]
}

评判原则：
- title 和 finding 必须简短精炼
- 正常：finding 写 "XX正常" 即可，不写 suggestion
- 异常：finding 简明点出问题（≤12字），suggestion 给出简明建议（≤15字）
- 手误命令：status=info，finding="命令未执行成功"
- 电源状态：DC 的 Fault/Absent 通常正常（该槽位未安装电源模块）
- 磁盘使用率超过 90% 为 warning，超过 95% 为 critical
- 负载 average 超过 CPU 核心数为 warning，超过 2 倍为 critical
- 只返回 JSON，不要有其他内容。

历史对比原则：
- 如果提供了历史数据，对比本次与历史变化趋势
- 在 summary 和 finding 中体现变化
- 趋势判断优先级：持续异常 > 新出现异常 > 持续正常 > 已改善
- 多次异常应提升严重度"#;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JudgmentItem {
    pub command: String,
    pub title: String,
    pub status: String,
    pub finding: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnalysisResult {
    pub summary: String,
    pub overall: String,
    pub items: Vec<JudgmentItem>,
}

/// 提取 AI 返回文本中的 JSON（兼容 markdown 代码栅栏）
pub fn extract_json(text: &str) -> Result<serde_json::Value, String> {
    let text = text.trim();
    // 尝试提取 ```json ... ``` 代码块
    if let Some(start) = text.find("```json") {
        let inner = &text[start + 7..];
        if let Some(end) = inner.find("```") {
            return serde_json::from_str(&inner[..end].trim()).map_err(|e| format!("JSON parse: {}", e));
        }
    }
    if let Some(start) = text.find("```") {
        let inner = &text[start + 3..];
        if let Some(end) = inner.find("```") {
            return serde_json::from_str(&inner[..end].trim()).map_err(|e| format!("JSON parse: {}", e));
        }
    }
    // 尝试提取最外层的 { ... }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return serde_json::from_str(&text[start..=end]).map_err(|e| format!("JSON parse: {}", e));
        }
    }
    Err("无法从 AI 返回中提取 JSON".into())
}

/// 构建 AI 分析用的提示词
pub fn build_analysis_prompt(
    device_name: &str,
    vendor: &str,
    command_outputs: &HashMap<String, String>,
    historical_context: Option<&str>,
) -> String {
    let mut outputs_text = Vec::new();
    for (cmd, output) in command_outputs {
        let lines: Vec<&str> = output.trim().split('\n').take(30).collect();
        let truncated = lines.join("\n");
        outputs_text.push(format!("命令: {}\n输出:\n{}", cmd, truncated));
    }

    let header = if !device_name.is_empty() && device_name != "unknown" {
        format!("设备名称: {}\n设备厂商: {}", device_name, vendor)
    } else {
        "（设备信息不完整，请从命令输出中推断设备类型）".to_string()
    };

    let historical_section = match historical_context {
        Some(ctx) if !ctx.is_empty() => format!("\n\n=== 历史巡检评判记录（供对比参考） ===\n{}", ctx),
        _ => String::new(),
    };

    format!(
        "{}{}\n\n=== 巡检命令输出 ===\n{}\n\n=== 分析要求 ===\n请对以上 {} 条命令逐条分析，必须包含每一条命令的评判结果。",
        header, historical_section,
        outputs_text.join("\n\n"),
        command_outputs.len(),
    )
}

/// 查询同设备最近 N 次已完成 AI 分析的巡检记录
pub fn fetch_historical_records(
    db: &rusqlite::Connection,
    device_id: i64,
    current_record_id: i64,
    limit: usize,
) -> Vec<HashMap<String, String>> {
    let mut stmt = db.prepare(
        "SELECT created_at, command_judgments, summary_judgment FROM inspection_records
         WHERE device_id=?1 AND ai_status='completed' AND id!=?2
         ORDER BY created_at DESC LIMIT ?3"
    ).ok();
    match stmt {
        None => vec![],
        Some(mut s) => {
            s.query_map(
                rusqlite::params![device_id, current_record_id, limit as i64],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?)),
            ).ok().map(|rows| {
                rows.filter_map(|r| r.ok()).map(|(ts, judgments, summary)| {
                    let mut m = HashMap::new();
                    m.insert("timestamp".into(), ts);
                    m.insert("command_judgments".into(), judgments.unwrap_or_default());
                    m.insert("summary_judgment".into(), summary.unwrap_or_default());
                    m
                }).collect()
            }).unwrap_or_default()
        }
    }
}

/// 格式化历史上下文为紧凑文本
pub fn format_historical_context(
    history: &[HashMap<String, String>],
    current_commands: &[String],
) -> String {
    if history.is_empty() {
        return "（该设备首次巡检，无历史数据可对比）".into();
    }

    let current_set: std::collections::HashSet<&str> = current_commands.iter().map(|s| s.trim()).collect();
    let mut lines = Vec::new();

    for (i, h) in history.iter().enumerate() {
        let ts = h.get("timestamp").map(|s| &s[..std::cmp::min(19, s.len())]).unwrap_or("");
        lines.push(format!("--- 历史巡检 #{}（{}）---", i + 1, ts.replace('T', " ")));

        let judgments = h.get("command_judgments").map(|s| s.as_str()).unwrap_or("{}");
        if let Ok(jval) = serde_json::from_str::<serde_json::Value>(judgments) {
            if let Some(obj) = jval.as_object() {
                for (cmd, judgment) in obj {
                    if !current_set.contains(cmd.trim()) { continue; }
                    let text = judgment.as_str().unwrap_or("");
                    let text = text.split('\x00').next().unwrap_or(text);
                    lines.push(format!("  {}: {}", cmd, text));
                }
            }
        }

        if let Some(summary) = h.get("summary_judgment") {
            if !summary.is_empty() {
                lines.push(format!("  整体总结: {}", summary));
            }
        }
    }

    lines.join("\n")
}

/// 将 AI 结果转为评判字典
pub fn result_to_judgments(result: &serde_json::Value, original_commands: Option<&[String]>) -> (HashMap<String, String>, String) {
    let items = result.get("items").and_then(|v| v.as_array()).map(|a| a.as_slice()).unwrap_or(&[]);
    let mut judgments = HashMap::new();

    let norm_map: HashMap<String, String> = original_commands.map(|cmds| {
        cmds.iter().map(|c| (c.trim().to_string(), c.clone())).collect()
    }).unwrap_or_default();

    for item in items {
        let ai_cmd = item.get("command").and_then(|v| v.as_str()).unwrap_or("");
        let cmd = norm_map.get(ai_cmd.trim()).cloned().unwrap_or_else(|| ai_cmd.to_string());
        let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("info").to_uppercase();
        let finding = item.get("finding").and_then(|v| v.as_str()).unwrap_or("").trim();
        let suggestion = item.get("suggestion").and_then(|v| v.as_str()).unwrap_or("").trim();
        let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("").trim();

        let judgment_text = if suggestion.is_empty() {
            format!("[{}] {}", status, finding)
        } else {
            format!("[{}] {} 建议: {}", status, finding, suggestion)
        };

        let text_with_title = if title.is_empty() {
            judgment_text
        } else {
            format!("{}\x00{}", judgment_text, title)
        };

        judgments.insert(cmd, text_with_title);
    }

    let summary = result.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string();
    (judgments, summary)
}

/// 格式化结果为简短摘要（供前端查看）
pub fn format_suggestions(result: &serde_json::Value) -> String {
    let items = result.get("items").and_then(|v| v.as_array()).map(|a| a.as_slice()).unwrap_or(&[]);
    if items.is_empty() { return "✅ 未发现异常".into(); }

    let severity_map: HashMap<&str, &str> = [
        ("critical", "🔴"), ("warning", "🟡"), ("info", "🔵"), ("ok", "✅"),
    ].iter().copied().collect();

    let mut lines = Vec::new();
    for item in items {
        let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("info");
        let sev = severity_map.get(status).unwrap_or(&"🔵");
        let cmd = item.get("command").and_then(|v| v.as_str()).unwrap_or("");
        let finding = item.get("finding").and_then(|v| v.as_str()).unwrap_or("");
        lines.push(format!("{} {}: {}", sev, cmd, finding));
        if let Some(sug) = item.get("suggestion").and_then(|v| v.as_str()) {
            if !sug.is_empty() { lines.push(format!("   建议: {}", sug)); }
        }
    }

    let overall = result.get("overall").and_then(|v| v.as_str()).unwrap_or("ok");
    let summary = result.get("summary").and_then(|v| v.as_str()).unwrap_or("");
    format!("[{}] {}\n{}", overall.to_uppercase(), summary, lines.join("\n"))
}

/// 格式化结果为详细分析
pub fn format_analysis(result: &serde_json::Value) -> String {
    let overall = result.get("overall").and_then(|v| v.as_str()).unwrap_or("ok");
    let summary = result.get("summary").and_then(|v| v.as_str()).unwrap_or("");
    let items = result.get("items").and_then(|v| v.as_array()).map(|a| a.as_slice()).unwrap_or(&[]);

    let mut lines = vec![format!("整体评估: {} - {}", overall.to_uppercase(), summary), String::new()];
    if !items.is_empty() {
        lines.push("详细发现:".into());
        for item in items {
            let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("info").to_uppercase();
            let cmd = item.get("command").and_then(|v| v.as_str()).unwrap_or("");
            let finding = item.get("finding").and_then(|v| v.as_str()).unwrap_or("");
            lines.push(format!("  [{}] {}: {}", status, cmd, finding));
            if let Some(sug) = item.get("suggestion").and_then(|v| v.as_str()) {
                if !sug.is_empty() { lines.push(format!("    → {}", sug)); }
            }
        }
    }

    lines.join("\n")
}

/// 调用 OpenAI 兼容 API 进行 AI 分析
pub async fn call_openai_analysis(
    prompt: &str,
    api_key: &str,
    base_url: &str,
    model_id: &str,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let api_base = base_url.trim_end_matches('/');
    let resp = client.post(format!("{}/chat/completions", api_base))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": model_id,
            "messages": [
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": prompt},
            ],
            "temperature": 0.3,
        }))
        .timeout(std::time::Duration::from_secs(180))
        .send().await.map_err(|e| format!("API 请求失败: {}", e))?;

    let body: serde_json::Value = resp.json().await.map_err(|e| format!("JSON 解析失败: {}", e))?;
    let content = body["choices"][0]["message"]["content"]
        .as_str().ok_or("无响应内容")?;

    extract_json(content)
}

/// 调用 Anthropic API 进行 AI 分析
pub async fn call_anthropic_analysis(
    prompt: &str,
    api_key: &str,
    base_url: &str,
    model_id: &str,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let api_base = base_url.trim_end_matches('/');
    let resp = client.post(format!("{}/messages", api_base))
        .header("x-api-key", api_key)
        .header("Content-Type", "application/json")
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": model_id,
            "max_tokens": 2048,
            "system": SYSTEM_PROMPT,
            "messages": [{"role": "user", "content": prompt}],
        }))
        .timeout(std::time::Duration::from_secs(180))
        .send().await.map_err(|e| format!("API 请求失败: {}", e))?;

    let body: serde_json::Value = resp.json().await.map_err(|e| format!("JSON 解析失败: {}", e))?;
    let content = body["content"][0]["text"]
        .as_str().ok_or("无响应内容")?;

    extract_json(content)
}

/// 执行完整的 AI 分析流程
pub async fn run_ai_analysis(
    db: &rusqlite::Connection,
    record_id: i64,
) -> Result<(), String> {
    // Get record data
    let record_data: Option<(i64, Option<String>)> = db.query_row(
        "SELECT device_id, command_outputs FROM inspection_records WHERE id=?1",
        rusqlite::params![record_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).ok();
    let Some((device_id, command_outputs)) = record_data else { return Err("记录不存在".into()); };
    let outputs_str = command_outputs.unwrap_or_else(|| "{}".into());
    let outputs: HashMap<String, String> = serde_json::from_str(&outputs_str).unwrap_or_default();

    // Get device info
    let device_info: Option<(String, String)> = db.query_row(
        "SELECT name, vendor FROM devices WHERE id=?1", rusqlite::params![device_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).ok();
    let (device_name, vendor) = device_info.unwrap_or_else(|| ("unknown".into(), "unknown".into()));

    // Get AI config
    let cfg: Option<(String, String, Option<String>, String)> = db.query_row(
        "SELECT provider, model_id, base_url, api_key_encrypted FROM ai_model_configs WHERE is_active=1",
        [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    ).ok();
    let Some((provider, model_id, base_url, api_key_enc)) = cfg else {
        db.execute("UPDATE inspection_records SET ai_status='failed', ai_result='无AI模型配置' WHERE id=?1", rusqlite::params![record_id]).ok();
        return Err("无AI模型配置".into());
    };

    let api_key = crate::services::crypto::CryptoService::decrypt(&api_key_enc).unwrap_or(api_key_enc);

    // Fetch historical records
    let history = fetch_historical_records(db, device_id, record_id, 5);
    let cmd_keys: Vec<String> = outputs.keys().cloned().collect();
    let hist_ctx = format_historical_context(&history, &cmd_keys);

    // Build prompt
    let prompt = build_analysis_prompt(&device_name, &vendor, &outputs, Some(&hist_ctx));

    // Update status to processing
    db.execute("UPDATE inspection_records SET ai_status='processing' WHERE id=?1", rusqlite::params![record_id])
        .map_err(|e| e.to_string())?;

    // Call AI
    let api_base = base_url.unwrap_or_else(|| match provider.as_str() {
        "openai" => "https://api.openai.com/v1".into(),
        "anthropic" => "https://api.anthropic.com/v1".into(),
        _ => "".into(),
    });

    let result = match provider.as_str() {
        "openai" => call_openai_analysis(&prompt, &api_key, &api_base, &model_id).await,
        "anthropic" => call_anthropic_analysis(&prompt, &api_key, &api_base, &model_id).await,
        p => Err(format!("不支持的 provider: {}", p)),
    };

    match result {
        Ok(ai_json) => {
            let ai_result = format_suggestions(&ai_json);
            let ai_analysis = format_analysis(&ai_json);
            let ai_suggestions = serde_json::to_string(&ai_json).unwrap_or_default();
            let (judgments, summary) = result_to_judgments(&ai_json, Some(&cmd_keys));

            info!("AI分析完成: 记录#{}, device={}", record_id, device_name);

            db.execute(
                "UPDATE inspection_records SET ai_status='completed', ai_result=?1, ai_analysis=?2, ai_suggestions=?3, command_judgments=?4, summary_judgment=?5 WHERE id=?6",
                rusqlite::params![ai_result, ai_analysis, ai_suggestions, serde_json::to_string(&judgments).unwrap_or_default(), summary, record_id],
            ).map_err(|e| e.to_string())?;
        }
        Err(e) => {
            warn!("AI分析失败: 记录#{} - {}", record_id, e);
            db.execute(
                "UPDATE inspection_records SET ai_status='failed', ai_result=?1 WHERE id=?2",
                rusqlite::params![format!("分析失败: {}", e), record_id],
            ).ok();
        }
    }

    Ok(())
}
