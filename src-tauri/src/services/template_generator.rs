use tracing::info;

/// Priority score for a command category (lower = higher priority).
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

/// Generate a template config by selecting commands from the pool
/// for a given vendor, optionally filtered by model and device_type.
///
/// The returned JSON has the structure:
/// `{"command_ids": [1, 2, 3, ...], "name": "{vendor}-巡检模板"}`
pub fn generate_template(
    db: &rusqlite::Connection,
    vendor: &str,
    model: Option<&str>,
    device_type: Option<&str>,
) -> Result<serde_json::Value, String> {
    // Build base query
    let mut sql = String::from(
        "SELECT id, command, description, category FROM command_pool WHERE vendor = ?1",
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(vendor.to_string())];

    if let Some(dt) = device_type {
        if !dt.is_empty() {
            sql.push_str(" AND (device_type = ?2 OR device_type IS NULL)");
            params.push(Box::new(dt.to_string()));
        }
    }

    sql.push_str(" ORDER BY category, id");

    info!(
        "Generating template for vendor={}, model={:?}, device_type={:?}",
        vendor, model, device_type
    );

    // Prepare and execute query
    let mut stmt = db
        .prepare(&sql)
        .map_err(|e| format!("查询命令池失败: {}", e))?;

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
            let id: i64 = row.get(0)?;
            let cmd: String = row.get(1)?;
            let desc: Option<String> = row.get(2)?;
            let category: Option<String> = row.get(3)?;
            Ok((id, cmd, desc, category))
        })
        .map_err(|e| format!("读取命令池失败: {}", e))?;

    // Collect and sort commands
    let mut commands: Vec<(i64, String, Option<String>, Option<String>, i32)> = Vec::new();

    for row in rows {
        let (id, cmd, desc, category) = row.map_err(|e| format!("读取行失败: {}", e))?;
        let priority = category
            .as_deref()
            .map(category_priority)
            .unwrap_or(9);
        commands.push((id, cmd, desc, category, priority));
    }

    // Sort by category priority, then by id
    commands.sort_by(|a, b| a.4.cmp(&b.4).then(a.0.cmp(&b.0)));

    let command_ids: Vec<i64> = commands.iter().map(|c| c.0).collect();
    let template_name = format!("{}-巡检模板", vendor);

    info!(
        "Generated template '{}' with {} commands for vendor '{}'",
        template_name,
        command_ids.len(),
        vendor
    );

    Ok(serde_json::json!({
        "command_ids": command_ids,
        "name": template_name
    }))
}
