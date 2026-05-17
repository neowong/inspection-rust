use tauri::State;
use crate::AppState;

#[tauri::command]
pub fn export_scripts(device_ids: Vec<i64>, format: Option<String>, state: State<AppState>) -> Result<serde_json::Value, String> {
    let fmt = format.unwrap_or_else(|| "text".into());
    let db = state.db.lock();

    let placeholders: Vec<String> = device_ids.iter().map(|_| "?".into()).collect();
    let sql = format!(
        "SELECT d.name, d.ip, d.vendor, d.device_type, t.config FROM devices d LEFT JOIN inspection_templates t ON d.template_id=t.id WHERE d.id IN ({})",
        placeholders.join(",")
    );

    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let params: Vec<&dyn rusqlite::types::ToSql> = device_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
    let devices: Vec<(String, String, String, String, Option<String>)> = stmt.query_map(
        params.as_slice(),
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let mut entries = Vec::new();
    let mut total_cmds = 0i64;

    for (name, ip, vendor, dtype, config) in &devices {
        let mut commands = Vec::new();
        if let Some(cfg_str) = config {
            if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(cfg_str) {
                if let Some(cmd_ids) = cfg.get("command_ids").and_then(|v| v.as_array()) {
                    let cids: Vec<i64> = cmd_ids.iter().filter_map(|v| v.as_i64()).collect();
                    if !cids.is_empty() {
                        let cph: Vec<String> = cids.iter().map(|_| "?".into()).collect();
                        let cmd_sql = format!("SELECT command FROM command_pool WHERE id IN ({})", cph.join(","));
                        let mut cmd_stmt = db.prepare(&cmd_sql).map_err(|e| e.to_string())?;
                        let cp: Vec<&dyn rusqlite::types::ToSql> = cids.iter().map(|c| c as &dyn rusqlite::types::ToSql).collect();
                        commands = cmd_stmt.query_map(cp.as_slice(), |r| r.get::<_, String>(0))
                            .map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
                    }
                }
            }
        }
        total_cmds += commands.len() as i64;
        entries.push(serde_json::json!({"device": name, "ip": ip, "vendor": vendor, "type": dtype, "commands": commands}));
    }

    let ts = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let content = match fmt.as_str() {
        "json" => serde_json::to_string_pretty(&serde_json::json!({
            "export_time": ts, "device_count": entries.len(), "command_count": total_cmds, "devices": entries,
        })).unwrap_or_default(),
        "csv" => {
            let mut s = String::from("设备,IP,厂商,命令\n");
            for e in &entries {
                let cmds = e["commands"].as_array();
                if let Some(cs) = cmds {
                    for c in cs { s.push_str(&format!("{},{},{},{}\n", e["device"], e["ip"], e["vendor"], c)); }
                }
            }
            s
        }
        "yaml" => serde_yaml::to_string(&serde_json::json!({
            "export_time": ts, "device_count": entries.len(), "command_count": total_cmds, "devices": entries,
        })).unwrap_or_else(|_| serde_json::to_string_pretty(&entries).unwrap_or_default()),
        _ => {
            let mut s = format!("离线巡检命令清单\n导出时间: {}\n设备数量: {}\n\n", ts, entries.len());
            for e in &entries {
                s.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
                s.push_str(&format!("设备: {} | IP: {} | 厂商: {}\n", e["device"], e["ip"], e["vendor"]));
                s.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
                if let Some(cmds) = e["commands"].as_array() {
                    for (idx, c) in cmds.iter().enumerate() { s.push_str(&format!("  {}. {}\n", idx+1, c)); }
                }
                s.push('\n');
            }
            s
        }
    };

    Ok(serde_json::json!({"success": true, "content": content, "format": fmt, "device_count": entries.len(), "command_count": total_cmds}))
}

#[tauri::command]
pub fn parse_upload_file(content: String, filename: String, _state: State<AppState>) -> Result<serde_json::Value, String> {
    let upload_dir = std::path::PathBuf::from("data/uploads");
    std::fs::create_dir_all(&upload_dir).ok();
    let file_path = upload_dir.join(&filename);
    std::fs::write(&file_path, &content).map_err(|e| e.to_string())?;

    let parsed: Vec<serde_json::Value> = if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
        if let Some(devices) = val.get("devices").and_then(|v| v.as_array()) { devices.clone() }
        else if let Some(arr) = val.as_array() { arr.clone() }
        else { vec![val] }
    } else if content.contains("====") {
        content.split("====").filter_map(|part| {
            let part = part.trim();
            if part.is_empty() || part.starts_with("Device:") { None }
            else { Some(serde_json::json!({"ip": part.lines().next().unwrap_or("unknown").trim(), "command_outputs": {}})) }
        }).collect()
    } else { vec![] };

    let devices_list: Vec<serde_json::Value> = parsed.iter().enumerate().map(|(idx, d)| serde_json::json!({
        "index": idx, "ip": d.get("ip").unwrap_or(&serde_json::json!("unknown")),
        "command_count": d.get("command_outputs").and_then(|v| v.as_object()).map(|o| o.len()).unwrap_or(0),
        "sample_commands": [],
    })).collect();

    Ok(serde_json::json!({
        "success": true, "filename": filename, "file_path": file_path.to_string_lossy(),
        "devices": devices_list, "device_count": devices_list.len(), "raw_devices_data": parsed,
    }))
}

#[tauri::command]
pub fn import_with_mapping(data: serde_json::Value, state: State<AppState>) -> Result<serde_json::Value, String> {
    let device_mapping = data.get("device_mapping").and_then(|v| v.as_array()).ok_or("请提供设备映射")?;
    let raw_devices = data.get("raw_devices_data").and_then(|v| v.as_array()).ok_or("请提供解析数据")?;
    let batch_name = data.get("batch_name").and_then(|v| v.as_str()).unwrap_or("离线巡检");
    let filename = data.get("filename").and_then(|v| v.as_str()).unwrap_or("unknown");
    let existing_batch_id = data.get("batch_id").and_then(|v| v.as_i64());

    let db = state.db.lock();
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let mut idx_to_device_id = std::collections::HashMap::new();
    for m in device_mapping {
        if let (Some(idx), Some(dev_id)) = (m.get("index").and_then(|v| v.as_i64()), m.get("device_id").and_then(|v| v.as_i64())) {
            idx_to_device_id.insert(idx, dev_id);
        }
    }

    let batch_id = if let Some(bid) = existing_batch_id {
        db.execute("UPDATE inspection_batches SET status='completed', completed_at=?1 WHERE id=?2", rusqlite::params![now, bid]).ok();
        bid
    } else {
        let all_ids: Vec<i64> = idx_to_device_id.values().copied().collect();
        db.execute(
            "INSERT INTO inspection_batches (name,mode,status,triggered_by,device_ids,started_at,completed_at) VALUES (?1,'offline','completed','manual',?2,?3,?3)",
            rusqlite::params![batch_name, serde_json::to_string(&all_ids).unwrap_or_default(), now],
        ).map_err(|e| e.to_string())?;
        db.last_insert_rowid()
    };

    let mut record_count = 0i64;
    for (idx, d) in raw_devices.iter().enumerate() {
        let device_id = idx_to_device_id.get(&(idx as i64)).copied().unwrap_or(0);
        if device_id == 0 { continue; }
        let outputs = d.get("command_outputs").map(|o| o.to_string()).unwrap_or_else(|| "{}".into());
        db.execute(
            "INSERT INTO inspection_records (batch_id,device_id,status,upload_source,command_outputs,completed_at) VALUES (?1,?2,'completed','offline',?3,?4)",
            rusqlite::params![batch_id, device_id, outputs, now],
        ).ok();
        record_count += 1;
    }

    let raw_str = serde_json::to_string(&raw_devices).unwrap_or_default();
    db.execute(
        "INSERT INTO offline_log_imports (filename,file_path,mode,parsed_devices,batch_id) VALUES (?1,?2,'upload',?3,?4)",
        rusqlite::params![filename, data.get("file_path").and_then(|v| v.as_str()).unwrap_or(""), raw_str, batch_id],
    ).ok();

    Ok(serde_json::json!({"success": true, "batch_id": batch_id, "record_count": record_count, "batch_name": batch_name}))
}

#[tauri::command]
pub fn upload_result(content: String, filename: String, batch_name: Option<String>, batch_id: Option<i64>, state: State<AppState>) -> Result<serde_json::Value, String> {
    let upload_dir = std::path::PathBuf::from("data/uploads");
    std::fs::create_dir_all(&upload_dir).ok();
    let file_path = upload_dir.join(&filename);
    std::fs::write(&file_path, &content).map_err(|e| e.to_string())?;

    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap_or(serde_json::json!({"devices":[]}));
    let devices = parsed.get("devices").and_then(|v| v.as_array()).cloned().unwrap_or_default();

    let db = state.db.lock();
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let b_id = if let Some(bid) = batch_id {
        db.execute("UPDATE inspection_batches SET status='completed' WHERE id=?1", rusqlite::params![bid]).ok();
        bid
    } else {
        db.execute(
            "INSERT INTO inspection_batches (name,mode,status,triggered_by,device_ids,started_at,completed_at) VALUES (?1,'offline','completed','manual','[]',?2,?2)",
            rusqlite::params![batch_name.unwrap_or_else(|| "离线巡检".into()), now],
        ).map_err(|e| e.to_string())?;
        db.last_insert_rowid()
    };

    let mut record_count = 0i64;
    for d in &devices {
        let ip = d.get("ip").and_then(|v| v.as_str()).unwrap_or("unknown");
        if ip == "unknown" { continue; }
        let dev_id: Option<i64> = db.query_row("SELECT id FROM devices WHERE ip=?1", rusqlite::params![ip], |r| r.get(0)).ok();
        if let Some(did) = dev_id {
            let outputs = d.get("command_outputs").map(|o| o.to_string()).unwrap_or_else(|| "{}".into());
            db.execute(
                "INSERT INTO inspection_records (batch_id,device_id,status,upload_source,command_outputs,completed_at) VALUES (?1,?2,'completed','offline',?3,?4)",
                rusqlite::params![b_id, did, outputs, now],
            ).ok();
            record_count += 1;
        }
    }

    let dev_str = serde_json::to_string(&devices).unwrap_or_default();
    db.execute(
        "INSERT INTO offline_log_imports (filename,file_path,mode,parsed_devices,batch_id) VALUES (?1,?2,'upload',?3,?4)",
        rusqlite::params![filename, file_path.to_string_lossy().to_string(), dev_str, b_id],
    ).ok();

    Ok(serde_json::json!({"success": true, "batch_id": b_id, "record_count": record_count}))
}

#[tauri::command]
pub fn list_imports(state: State<AppState>) -> Result<Vec<serde_json::Value>, String> {
    let db = state.db.lock();
    let mut stmt = db.prepare(
        "SELECT id, filename, mode, parsed_devices, batch_id, created_at FROM offline_log_imports ORDER BY created_at DESC LIMIT 50"
    ).map_err(|e| e.to_string())?;
    let rows: Vec<serde_json::Value> = stmt.query_map([], |r| Ok(serde_json::json!({
        "id": r.get::<_, i64>(0)?, "filename": r.get::<_, String>(1)?, "mode": r.get::<_, String>(2)?,
        "parsed_devices": r.get::<_, String>(3)?, "batch_id": r.get::<_, Option<i64>>(4)?,
        "created_at": r.get::<_, String>(5)?,
    }))).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn delete_import(import_id: i64, state: State<AppState>) -> Result<(), String> {
    let db = state.db.lock();
    let file_path: Option<String> = db.query_row("SELECT file_path FROM offline_log_imports WHERE id=?1", rusqlite::params![import_id], |r| r.get(0)).ok();
    if let Some(ref fp) = file_path { std::fs::remove_file(fp).ok(); }
    db.execute("DELETE FROM offline_log_imports WHERE id=?1", rusqlite::params![import_id]).map_err(|e| e.to_string())?;
    Ok(())
}
