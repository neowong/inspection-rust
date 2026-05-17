/// 自动生成巡检模板 — 根据厂商/型号推荐巡检命令
///
/// 对应 Python: backend/app/services/template_generator.py
use std::collections::HashMap;
use tracing::info;

/// 类别优先级排序
fn category_priority(category: &str) -> i32 {
    match category.to_lowercase().as_str() {
        "version" => 1,
        "clock" => 2,
        "disk" | "storage" => 3,
        "cpu" => 4,
        "memory" => 5,
        "hardware" | "power" | "fan" | "env" | "temperature" | "module" | "stack" => 6,
        "interface" => 7,
        "protocol" | "ntp" | "log" | "vlan" | "arp" | "mac" | "stp" => 8,
        _ => 9,
    }
}

/// 根据厂商从命令库选择命令并生成模板
pub fn generate_template(
    db: &rusqlite::Connection,
    vendor: &str,
    model: Option<&str>,
    device_type: Option<&str>,
) -> Result<serde_json::Value, String> {
    // Query matching commands
    let mut stmt = db.prepare(
        "SELECT id, command, description, category FROM command_pool WHERE vendor=?1"
    ).map_err(|e| e.to_string())?;

    let mut cmds: Vec<(i64, String, Option<String>, Option<String>)> = stmt.query_map(
        rusqlite::params![vendor],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    if cmds.is_empty() {
        return Ok(serde_json::json!({
            "error": "命令库中没有该厂商的命令，请先添加命令或配置 AI 后重试"
        }));
    }

    // Sort by category priority
    cmds.sort_by_key(|(_, _, _, cat)| category_priority(cat.as_deref().unwrap_or("general")));

    let ids: Vec<i64> = cmds.iter().map(|c| c.0).collect();
    let details: Vec<serde_json::Value> = cmds.iter().map(|c| serde_json::json!({
        "id": c.0, "command": c.1, "description": c.2, "category": c.3,
    })).collect();

    let name = build_template_name(vendor, model, device_type);

    Ok(serde_json::json!({
        "suggested_name": name,
        "config": {"command_ids": ids},
        "commands_detail": details,
        "source": "command_pool",
        "command_count": ids.len(),
        "message": format!("从命令库中找到 {} 条 {} 命令，已按优先级排序", ids.len(), vendor),
    }))
}

/// 构建建议的模板名称
pub fn build_template_name(vendor: &str, model: Option<&str>, device_type: Option<&str>) -> String {
    let parts: Vec<&str> = [Some(vendor), model, device_type].iter()
        .filter_map(|&s| s)
        .collect();
    format!("{} 巡检模板", parts.join(" "))
}

/// AI 推荐补充命令（在已有命令基础上调用 LLM 推荐 3-5 条）
pub async fn ai_recommend_commands(
    vendor: &str,
    model: Option<&str>,
    device_type: Option<&str>,
    existing_commands: &[(String, Option<String>)],
    api_key: &str,
    base_url: &str,
    ai_model: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let existing_list: String = existing_commands.iter()
        .map(|(cmd, desc)| format!("- {}: {}", cmd, desc.as_deref().unwrap_or("")))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "你是一位网络设备巡检专家。请为以下设备推荐补充的巡检命令。\n\n\
         设备厂商: {}\n设备型号: {}\n设备类型: {}\n已有命令:\n{}\n\n\
         请推荐 3-5 条补充的巡检命令，每条包含命令文本和描述，返回 JSON 格式:\n\
         {{\"commands\": [{{\"command\": \"display xxx\", \"description\": \"查看xxx\", \"category\": \"分类\"}}]}}\n\n\
         类别可选: version/clock/disk/cpu/memory/hardware/power/fan/env/interface/protocol/ntp/log/vlan/arp/mac/stp/general\n\n只返回 JSON。",
        vendor, model.unwrap_or("不限"), device_type.unwrap_or("交换机"), existing_list
    );

    let client = reqwest::Client::new();
    let resp = client.post(format!("{}/chat/completions", base_url.trim_end_matches('/')))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": ai_model,
            "messages": [
                {"role": "system", "content": "你是网络设备巡检专家。返回 JSON 格式结果。"},
                {"role": "user", "content": prompt},
            ],
            "temperature": 0.3,
        }))
        .timeout(std::time::Duration::from_secs(60))
        .send().await.map_err(|e| format!("API 请求失败: {}", e))?;

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let content = body["choices"][0]["message"]["content"].as_str().unwrap_or("");
    let result = crate::services::ai_inspection::extract_json(content)?;
    let cmds = result.get("commands").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    Ok(cmds)
}

/// 插入或复用命令：同 vendor + 同 command 文本则复用已有 ID，否则新建
pub fn upsert_command(
    db: &rusqlite::Connection,
    vendor: &str,
    command: &str,
    model: Option<&str>,
    description: Option<&str>,
    category: Option<&str>,
) -> Result<(i64, bool), String> {
    let cmd_norm = command.trim();
    let existing: Option<i64> = db.query_row(
        "SELECT id FROM command_pool WHERE vendor=?1 AND command=?2",
        rusqlite::params![vendor, cmd_norm], |r| r.get(0),
    ).ok();

    if let Some(id) = existing {
        return Ok((id, true));
    }

    db.execute(
        "INSERT INTO command_pool (vendor, command, model, description, category) VALUES (?1,?2,?3,?4,?5)",
        rusqlite::params![vendor, cmd_norm, model, description, category.unwrap_or("general")],
    ).map_err(|e| e.to_string())?;

    Ok((db.last_insert_rowid(), false))
}
