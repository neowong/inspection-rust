use std::collections::HashMap;
use std::io::Write;

use tauri::State;

use crate::AppState;
use crate::db::models::{
    AiModelConfig, InspectionRecord, ReportTemplate,
    RECORD_COLUMNS, DEVICE_COLUMNS, REPORT_TEMPLATE_COLUMNS,
    record_from_row, device_from_row, report_template_from_row, now_str,
};
use crate::services::crypto::CryptoService;
use crate::services::{ai_inspection, report_generator, template_engine, template_variables};

// ============================================================
// Helpers
// ============================================================

fn ensure_reports_dir() -> Result<std::path::PathBuf, String> {
    let dir = std::path::PathBuf::from("data").join("reports");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建报告目录失败: {}", e))?;
    Ok(dir)
}

fn ensure_report_templates_dir() -> Result<std::path::PathBuf, String> {
    let dir = std::path::PathBuf::from("data").join("report_templates");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建报告模板目录失败: {}", e))?;
    Ok(dir)
}

/// Parse command_outputs JSON string into a HashMap<String, String>.
fn parse_command_outputs(json_str: &Option<String>) -> Result<HashMap<String, String>, String> {
    let empty = "{}".to_string();
    let val: serde_json::Value =
        serde_json::from_str(json_str.as_deref().unwrap_or(&empty))
            .map_err(|e| format!("解析命令输出 JSON 失败: {}", e))?;

    let obj = val
        .as_object()
        .ok_or_else(|| "命令输出 JSON 格式异常：不是对象".to_string())?;

    let mut map = HashMap::new();
    for (k, v) in obj {
        let s = v
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| v.to_string());
        map.insert(k.clone(), s);
    }
    Ok(map)
}

// ============================================================
// AI Analysis — Inner (async, takes &AppState)
// ============================================================

async fn analyze_record_inner(
    app_state: &AppState,
    record_id: i64,
) -> Result<serde_json::Value, String> {
    // Step 1: Get record and parse command outputs (lock, read, drop)
    let record_id_owned = record_id;
    let (command_outputs_map, device_id) = {
        let conn = app_state.db.lock();
        let sql =
            format!("SELECT {} FROM inspection_records WHERE id = ?1", RECORD_COLUMNS);
        let record = crate::db::query::query_one(
            &conn,
            &sql,
            rusqlite::params![record_id],
            record_from_row,
        )?
        .ok_or_else(|| format!("巡检记录 ID {} 不存在", record_id))?;

        // Mark ai_status as "running"
        let now = now_str();
        conn.execute(
            "UPDATE inspection_records SET ai_status = 'processing', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, record_id],
        )
        .map_err(|e| e.to_string())?;

        let map = parse_command_outputs(&record.command_outputs)?;
        (map, record.device_id)
    };
    let _ = device_id; // device_id isn't used further in this flow

    // Step 2: Get active AI config and decrypt API key (lock, read, drop)
    let (provider, model, api_key, base_url) = {
        let conn = app_state.db.lock();
        let config = crate::db::query::query_one(
            &conn,
            "SELECT id, name, provider, model_id, api_key_encrypted, base_url, \
             is_active, created_at, updated_at \
             FROM ai_model_configs WHERE is_active = 1 LIMIT 1",
            &[],
            |row| {
                Ok(AiModelConfig {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    provider: row.get(2)?,
                    model_id: row.get(3)?,
                    api_key_encrypted: row.get(4)?,
                    base_url: row.get(5)?,
                    is_active: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        )?
        .ok_or_else(|| "未找到激活的 AI 配置，请先在 AI 配置页面设置并激活".to_string())?;

        let decrypted_key = CryptoService::decrypt(&config.api_key_encrypted)?;
        (
            config.provider,
            config.model_id,
            decrypted_key,
            config.base_url.unwrap_or_default(),
        )
    };

    // Step 3: AI analysis (async, no DB lock held)
    let analysis = match provider.as_str() {
        "openai" => {
            ai_inspection::analyze_with_openai(&api_key, &model, &base_url, &command_outputs_map)
                .await?
        }
        "anthropic" => {
            ai_inspection::analyze_with_anthropic(
                &api_key,
                &model,
                &base_url,
                &command_outputs_map,
            )
            .await?
        }
        "deepseek" => {
            // DeepSeek uses OpenAI-compatible API format
            let deepseek_base = if base_url.is_empty() {
                "https://api.deepseek.com".to_string()
            } else {
                base_url.clone()
            };
            ai_inspection::analyze_with_openai(&api_key, &model, &deepseek_base, &command_outputs_map)
                .await?
        }
        _ => return Err(format!("不支持的 AI 提供商: {}", provider)),
    };

    // Step 4: Parse result and update record (lock, update, drop)
    {
        let conn = app_state.db.lock();

        let summary = analysis
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let overall = analysis
            .get("overall")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let items = analysis.get("items").and_then(|v| v.as_array());

        // Build command_judgments map and collect suggestions
        let mut judgments = serde_json::Map::new();
        let mut suggestions = Vec::new();

        if let Some(items_array) = items {
            for item in items_array {
                if let Some(cmd) = item.get("command").and_then(|v| v.as_str()) {
                    let mut jdg = serde_json::Map::new();
                    if let Some(status) = item.get("status").and_then(|v| v.as_str()) {
                        jdg.insert(
                            "status".to_string(),
                            serde_json::Value::String(status.to_string()),
                        );
                    }
                    if let Some(finding) = item.get("finding").and_then(|v| v.as_str()) {
                        jdg.insert(
                            "finding".to_string(),
                            serde_json::Value::String(finding.to_string()),
                        );
                    }
                    if let Some(suggestion) = item.get("suggestion").and_then(|v| v.as_str()) {
                        jdg.insert(
                            "suggestion".to_string(),
                            serde_json::Value::String(suggestion.to_string()),
                        );
                        if !suggestion.is_empty() {
                            suggestions.push(suggestion.to_string());
                        }
                    }
                    judgments.insert(cmd.to_string(), serde_json::Value::Object(jdg));
                }
            }
        }

        let command_judgments_json =
            serde_json::to_string(&serde_json::Value::Object(judgments))
                .map_err(|e| format!("序列化命令判定结果失败: {}", e))?;

        let suggestions_text = if suggestions.is_empty() {
            String::new()
        } else {
            suggestions.join("；")
        };

        let ai_result_str = serde_json::to_string(&analysis)
            .map_err(|e| format!("序列化 AI 分析结果失败: {}", e))?;

        let now = now_str();
        conn.execute(
            "UPDATE inspection_records \
             SET ai_status = 'completed', ai_result = ?1, ai_analysis = ?2, \
                 ai_suggestions = ?3, command_judgments = ?4, summary_judgment = ?5, \
                 updated_at = ?6 \
             WHERE id = ?7",
            rusqlite::params![
                ai_result_str,
                summary,
                suggestions_text,
                command_judgments_json,
                overall,
                now,
                record_id_owned,
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(analysis)
}

// ============================================================
// AI Analysis — Tauri Commands
// ============================================================

/// 对单条巡检记录执行 AI 分析
#[tauri::command]
pub async fn analyze_record(
    record_id: i64,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    analyze_record_inner(&*state, record_id).await
}

/// 对批次内所有未完成 AI 分析的记录执行批量分析
#[tauri::command]
pub async fn analyze_batch(
    batch_id: i64,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    // Get all records in the batch that need AI analysis
    let record_ids: Vec<i64> = {
        let conn = state.db.lock();
        let sql = format!(
            "SELECT {} FROM inspection_records WHERE batch_id = ?1 AND ai_status != 'completed'",
            RECORD_COLUMNS
        );
        let records: Vec<InspectionRecord> =
            crate::db::query::query_all(&conn, &sql, rusqlite::params![batch_id], record_from_row)?;
        records.into_iter().map(|r| r.id).collect()
    };

    if record_ids.is_empty() {
        return Ok(serde_json::json!({
            "total": 0,
            "completed": 0,
            "failed": 0,
            "message": "所有记录已完成 AI 分析"
        }));
    }

    let total = record_ids.len();

    // Run all records concurrently
    let futures: Vec<_> = record_ids
        .iter()
        .map(|rid| async { (*rid, analyze_record_inner(&*state, *rid).await) })
        .collect();
    let results = futures::future::join_all(futures).await;

    let mut completed = 0u32;
    let mut failed = 0u32;
    let mut errors: Vec<serde_json::Value> = Vec::new();

    for (rid, result) in results {
        match result {
            Ok(_result) => {
                completed += 1;
            }
            Err(e) => {
                failed += 1;
                errors.push(serde_json::json!({
                    "record_id": rid,
                    "error": e,
                }));
                // Mark record as failed
                let conn = state.db.lock();
                let now = now_str();
                conn.execute(
                    "UPDATE inspection_records SET ai_status = 'failed', updated_at = ?1 WHERE id = ?2",
                    rusqlite::params![now, rid],
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(serde_json::json!({
        "total": total,
        "completed": completed,
        "failed": failed,
        "errors": errors,
    }))
}

// ============================================================
// Record Query
// ============================================================

/// 获取单条巡检记录的完整详情
#[tauri::command]
pub fn get_record(
    record_id: i64,
    state: State<AppState>,
) -> Result<InspectionRecord, String> {
    let conn = state.db.lock();
    let sql = format!(
        "SELECT {} FROM inspection_records WHERE id = ?1",
        RECORD_COLUMNS
    );
    crate::db::query::query_one(&conn, &sql, rusqlite::params![record_id], record_from_row)?
        .ok_or_else(|| format!("巡检记录 ID {} 不存在", record_id))
}

// ============================================================
// Report Generation — Inner (sync, takes &Connection)
// ============================================================

/// Resolved template data: config_json, content, mode, format
struct ResolvedTemplate {
    config_json: String,
    content: String,
    mode: String,
    format: String,
}

/// Resolve the report template to use for a record.
/// Chain: record -> device -> inspection_template -> report_template_id.
/// Falls back to default template (is_default=1), then to hardcoded builder.
fn resolve_report_template(
    conn: &rusqlite::Connection,
    record: &InspectionRecord,
    override_template_id: Option<i64>,
) -> Result<Option<ResolvedTemplate>, String> {
    let try_load = |tid: i64| -> Option<ResolvedTemplate> {
        let sql = format!("SELECT {} FROM report_templates WHERE id = ?1", REPORT_TEMPLATE_COLUMNS);
        crate::db::query::query_one(conn, &sql, rusqlite::params![tid], report_template_from_row)
            .ok()
            .flatten()
            .filter(|t: &ReportTemplate| !t.config_json.is_empty() || !t.content.is_empty())
            .map(|t| ResolvedTemplate {
                config_json: t.config_json,
                content: t.content,
                mode: t.mode,
                format: t.format,
            })
    };

    // If an explicit override is provided, use that
    if let Some(tid) = override_template_id {
        if let Some(t) = try_load(tid) {
            return Ok(Some(t));
        }
    }

    // Walk the chain: record -> device -> template_id -> inspection_template -> report_template_id
    let device_sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
    if let Ok(Some(device)) = crate::db::query::query_one(
        conn, &device_sql, rusqlite::params![record.device_id], device_from_row,
    ) {
        if let Some(tid) = device.template_id {
            let tpl_sql = format!("SELECT {} FROM inspection_templates WHERE id = ?1", crate::db::models::TEMPLATE_COLUMNS);
            if let Ok(Some(tpl)) = crate::db::query::query_one(
                conn, &tpl_sql, rusqlite::params![tid],
                crate::db::models::template_from_row,
            ) {
                if let Some(rt_id) = tpl.report_template_id {
                    if let Some(t) = try_load(rt_id) {
                        return Ok(Some(t));
                    }
                }
            }
        }
    }

    // Fallback to default template
    let default_sql = format!(
        "SELECT {} FROM report_templates WHERE is_default = 1 LIMIT 1",
        REPORT_TEMPLATE_COLUMNS
    );
    if let Ok(Some(t)) = crate::db::query::query_one(conn, &default_sql, &[], report_template_from_row) {
        if !t.config_json.is_empty() || !t.content.is_empty() {
            return Ok(Some(ResolvedTemplate {
                config_json: t.config_json,
                content: t.content,
                mode: t.mode,
                format: t.format,
            }));
        }
    }

    Ok(None)
}

fn generate_report_inner(
    conn: &rusqlite::Connection,
    record_id: i64,
    template_id: Option<i64>,
) -> Result<String, String> {
    // 1. Get record
    let record_sql =
        format!("SELECT {} FROM inspection_records WHERE id = ?1", RECORD_COLUMNS);
    let record = crate::db::query::query_one(
        conn,
        &record_sql,
        rusqlite::params![record_id],
        record_from_row,
    )?
    .ok_or_else(|| format!("巡检记录 ID {} 不存在", record_id))?;

    // 2. Get associated device
    let device_sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
    let device = crate::db::query::query_one(
        conn,
        &device_sql,
        rusqlite::params![record.device_id],
        device_from_row,
    )?
    .ok_or_else(|| format!("设备 ID {} 不存在", record.device_id))?;

    // 3. Resolve template
    let template = resolve_report_template(conn, &record, template_id)?;

    let (markdown, extension) = if let Some(t) = template {
        let ctx = template_variables::build_template_context(&device, &record);
        let rendered = template_engine::render_template_from_config(
            &t.config_json, &t.content, &t.mode, &ctx, &t.format,
        );
        let ext = if t.format == "html" { "html" } else { "md" };
        (rendered, ext)
    } else {
        // Fallback to hardcoded markdown builder
        let mut ctx: HashMap<String, serde_json::Value> = HashMap::new();
        ctx.insert("device_name".into(), serde_json::Value::String(device.name.clone()));
        ctx.insert("device_ip".into(), serde_json::Value::String(device.ip.clone()));
        ctx.insert("vendor".into(), serde_json::Value::String(device.vendor.clone()));
        if let Some(ref model) = device.model {
            ctx.insert("model".into(), serde_json::Value::String(model.clone()));
        }
        if let Some(ref outputs_str) = record.command_outputs {
            if let Ok(outputs_val) = serde_json::from_str::<serde_json::Value>(outputs_str) {
                ctx.insert("command_outputs".into(), outputs_val);
            }
        }
        if let Some(ref judgments_str) = record.command_judgments {
            if let Ok(judgments_val) = serde_json::from_str::<serde_json::Value>(judgments_str) {
                ctx.insert("command_judgments".into(), judgments_val);
            }
        }
        let summary = record.summary_judgment.clone().unwrap_or_default();
        ctx.insert("summary".into(), serde_json::Value::String(summary));
        (report_generator::build_markdown(&ctx), "md")
    };

    // 5. Save to file
    let reports_dir = ensure_reports_dir()?;
    let now = chrono::Local::now()
        .format("%Y%m%d_%H%M%S")
        .to_string();
    let file_name = format!("report_{}_{}.{}", record_id, now, extension);
    let file_path = reports_dir.join(&file_name);
    std::fs::write(&file_path, &markdown)
        .map_err(|e| format!("保存报告文件失败: {}", e))?;

    let file_path_str = file_path.to_string_lossy().to_string();

    // 6. Update record report_path
    let now_str = now_str();
    conn.execute(
        "UPDATE inspection_records SET report_path = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![file_path_str, now_str, record_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(markdown)
}

// ============================================================
// Report Generation — Tauri Commands
// ============================================================

/// 生成单条巡检记录的报告
#[tauri::command]
pub fn generate_report(
    record_id: i64,
    template_id: Option<i64>,
    state: State<AppState>,
) -> Result<String, String> {
    let conn = state.db.lock();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        generate_report_inner(&conn, record_id, template_id)
    }))
    .map_err(|e| format!("生成报告时发生内部错误: {:?}", e))?
}

/// 生成批次内所有已完成记录的巡检报告
#[tauri::command]
pub fn generate_batch_reports(
    batch_id: i64,
    template_id: Option<i64>,
    state: State<AppState>,
) -> Result<Vec<String>, String> {
    // Get completed record IDs
    let record_ids: Vec<i64> = {
        let conn = state.db.lock();
        let sql = format!(
            "SELECT {} FROM inspection_records WHERE batch_id = ?1 AND status = 'completed' ORDER BY id",
            RECORD_COLUMNS
        );
        let records: Vec<InspectionRecord> =
            crate::db::query::query_all(&conn, &sql, rusqlite::params![batch_id], record_from_row)?;
        records.into_iter().map(|r| r.id).collect()
    };

    if record_ids.is_empty() {
        return Err("批次中无已完成记录".to_string());
    }

    let mut reports = Vec::new();
    for rid in &record_ids {
        let conn = state.db.lock();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generate_report_inner(&conn, *rid, template_id)
        })) {
            Ok(Ok(markdown)) => reports.push(markdown),
            Ok(Err(e)) => {
                eprintln!("生成记录 {} 报告失败: {}", rid, e);
            }
            Err(e) => {
                eprintln!("生成记录 {} 报告时发生内部错误: {:?}", rid, e);
            }
        }
    }

    Ok(reports)
}

/// 下载单条巡检报告（使用系统保存对话框）
#[tauri::command]
pub async fn download_report(
    app: tauri::AppHandle,
    record_id: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt;

    // Get report path and device name
    let (report_path, device_name) = {
        let conn = state.db.lock();
        let record_sql =
            format!("SELECT {} FROM inspection_records WHERE id = ?1", RECORD_COLUMNS);
        let record = crate::db::query::query_one(
            &conn,
            &record_sql,
            rusqlite::params![record_id],
            record_from_row,
        )?
        .ok_or_else(|| format!("巡检记录 ID {} 不存在", record_id))?;

        let path = record
            .report_path
            .clone()
            .ok_or_else(|| format!("记录 ID {} 尚未生成报告，请先生成", record_id))?;

        let device_sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
        let device = crate::db::query::query_one(
            &conn,
            &device_sql,
            rusqlite::params![record.device_id],
            device_from_row,
        )?
        .ok_or_else(|| format!("设备 ID {} 不存在", record.device_id))?;

        (path, device.name)
    };

    // Show save dialog (callback-based, non-blocking)
    let report_path_clone = report_path.clone();
    app.dialog()
        .file()
        .add_filter("Markdown", &["md"])
        .set_file_name(&format!("{}-巡检报告.md", device_name))
        .save_file(move |file_path| {
            if let Some(save_path) = file_path {
                let dest = save_path.as_path().unwrap().to_path_buf();
                if let Err(e) = std::fs::copy(&report_path_clone, &dest) {
                    eprintln!("复制报告文件失败: {}", e);
                }
            }
        });

    Ok(())
}

/// 下载批次内所有已完成记录的巡检报告
#[tauri::command]
pub async fn download_batch_report(
    app: tauri::AppHandle,
    batch_id: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt;

    // Get all completed records with reports in the batch
    let reports_info: Vec<(i64, String, String)> = {
        let conn = state.db.lock();
        let sql = format!(
            "SELECT {} FROM inspection_records WHERE batch_id = ?1 AND status = 'completed' \
             AND report_path IS NOT NULL ORDER BY id",
            RECORD_COLUMNS
        );
        let records: Vec<InspectionRecord> =
            crate::db::query::query_all(&conn, &sql, rusqlite::params![batch_id], record_from_row)?;

        let mut info = Vec::new();
        for record in &records {
            if let Some(ref path) = record.report_path {
                let device_sql =
                    format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
                if let Ok(Some(device)) = crate::db::query::query_one(
                    &conn,
                    &device_sql,
                    rusqlite::params![record.device_id],
                    device_from_row,
                ) {
                    info.push((record.id, path.clone(), device.name));
                }
            }
        }
        info
    };

    if reports_info.is_empty() {
        return Err("批次中无已生成报告的已完成记录".to_string());
    }

    // Show save directory dialog
    let reports_info_clone = reports_info.clone();
    app.dialog()
        .file()
        .set_file_name("巡检报告合集")
        .save_file(move |file_path| {
            if let Some(save_path) = file_path {
                let base_dir = save_path.as_path().unwrap().to_path_buf();
                for (_id, src_path, device_name) in &reports_info_clone {
                    let safe_name = device_name.replace('/', "_");
                    let file_name = format!("{}-巡检报告.md", safe_name);
                    let dest = if base_dir.is_dir() {
                        base_dir.join(&file_name)
                    } else {
                        let parent = base_dir.parent().unwrap_or(&base_dir);
                        parent.join(&file_name)
                    };
                    if let Err(e) = std::fs::copy(src_path, &dest) {
                        eprintln!("复制报告 {} 失败: {}", device_name, e);
                    }
                }
            }
        });

    Ok(())
}

// ============================================================
// Report Template Management
// ============================================================

/// 获取所有报告模板列表
#[tauri::command]
pub fn list_report_templates(state: State<AppState>) -> Result<Vec<ReportTemplate>, String> {
    let conn = state.db.lock();

    let sql = format!(
        "SELECT {} FROM report_templates ORDER BY created_at DESC",
        REPORT_TEMPLATE_COLUMNS
    );
    crate::db::query::query_all(&conn, &sql, &[], report_template_from_row)
}

/// 上传报告模板（复制文件到 data/report_templates/ 目录，并读取内容到 DB）
#[tauri::command]
pub fn upload_template(
    file_path: String,
    name: String,
    vendor: Option<String>,
    state: State<AppState>,
) -> Result<ReportTemplate, String> {
    let conn = state.db.lock();

    // Copy file to report_templates directory
    let templates_dir = ensure_report_templates_dir()?;
    let src = std::path::Path::new(&file_path);
    let ext = src.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("md");
    let format = if ext == "html" { "html" } else { "markdown" };
    let file_name = format!(
        "{}_{}",
        uuid::Uuid::new_v4(),
        src.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "template.md".to_string()),
    );
    let dest = templates_dir.join(&file_name);

    std::fs::copy(src, &dest).map_err(|e| format!("复制模板文件失败: {}", e))?;

    let dest_str = dest.to_string_lossy().to_string();

    // Read file content into DB
    let content = std::fs::read_to_string(&dest).unwrap_or_default();

    // Insert into DB with new columns
    conn.execute(
        "INSERT INTO report_templates (name, vendor, file_path, content, format) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![name, vendor, dest_str, content, format],
    )
    .map_err(|e| e.to_string())?;

    let last_id = conn.last_insert_rowid();

    let sql = format!(
        "SELECT {} FROM report_templates WHERE id = ?1",
        REPORT_TEMPLATE_COLUMNS
    );
    crate::db::query::query_one(
        &conn,
        &sql,
        rusqlite::params![last_id],
        report_template_from_row,
    )?
    .ok_or_else(|| "上传模板后查询失败".to_string())
}

/// 下载报告模板（返回文件内容）
#[tauri::command]
pub fn download_template(template_id: i64, state: State<AppState>) -> Result<String, String> {
    let conn = state.db.lock();

    let sql = format!(
        "SELECT {} FROM report_templates WHERE id = ?1",
        REPORT_TEMPLATE_COLUMNS
    );
    let template = crate::db::query::query_one(
        &conn,
        &sql,
        rusqlite::params![template_id],
        report_template_from_row,
    )?
    .ok_or_else(|| format!("报告模板 ID {} 不存在", template_id))?;

    std::fs::read_to_string(&template.file_path)
        .map_err(|e| format!("读取模板文件失败: {}", e))
}

/// 预览报告模板内容
#[tauri::command]
pub fn preview_template(template_id: i64, state: State<AppState>) -> Result<String, String> {
    let conn = state.db.lock();

    let sql = format!(
        "SELECT {} FROM report_templates WHERE id = ?1",
        REPORT_TEMPLATE_COLUMNS
    );
    let template = crate::db::query::query_one(
        &conn,
        &sql,
        rusqlite::params![template_id],
        report_template_from_row,
    )?
    .ok_or_else(|| format!("报告模板 ID {} 不存在", template_id))?;

    std::fs::read_to_string(&template.file_path)
        .map_err(|e| format!("读取模板文件失败: {}", e))
}

/// 预览报告模板上下文信息（返回元数据字符串）
#[tauri::command]
pub fn preview_template_context(
    template_id: i64,
    state: State<AppState>,
) -> Result<String, String> {
    let conn = state.db.lock();

    let sql = format!(
        "SELECT {} FROM report_templates WHERE id = ?1",
        REPORT_TEMPLATE_COLUMNS
    );
    let template = crate::db::query::query_one(
        &conn,
        &sql,
        rusqlite::params![template_id],
        report_template_from_row,
    )?
    .ok_or_else(|| format!("报告模板 ID {} 不存在", template_id))?;

    Ok(format!(
        "名称: {}\n厂商: {}\n路径: {}\n创建时间: {}\n更新时间: {}",
        template.name,
        template.vendor.unwrap_or_else(|| "通用".to_string()),
        template.file_path,
        template.created_at,
        template.updated_at,
    ))
}

/// 删除报告模板（同时删除文件，并清理关联引用）
#[tauri::command]
pub fn delete_report_template(
    template_id: i64,
    state: State<AppState>,
) -> Result<(), String> {
    let conn = state.db.lock();

    let sql = format!(
        "SELECT {} FROM report_templates WHERE id = ?1",
        REPORT_TEMPLATE_COLUMNS
    );
    let template = crate::db::query::query_one(
        &conn,
        &sql,
        rusqlite::params![template_id],
        report_template_from_row,
    )?
    .ok_or_else(|| format!("报告模板 ID {} 不存在", template_id))?;

    // Clear references in inspection_templates
    conn.execute(
        "UPDATE inspection_templates SET report_template_id = NULL WHERE report_template_id = ?1",
        rusqlite::params![template_id],
    )
    .map_err(|e| e.to_string())?;

    // Delete from DB
    let affected = conn
        .execute(
            "DELETE FROM report_templates WHERE id = ?1",
            rusqlite::params![template_id],
        )
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("报告模板 ID {} 不存在", template_id));
    }

    // Try to delete the file (best-effort)
    if !template.file_path.is_empty() {
        if let Err(e) = std::fs::remove_file(&template.file_path) {
            eprintln!("删除模板文件失败: {}", e);
        }
    }

    Ok(())
}

/// 批量删除报告模板
#[tauri::command]
pub fn batch_delete_report_templates(
    ids: Vec<i64>,
    state: State<AppState>,
) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    for id in &ids {
        // Get file path before deleting
        let sql = format!(
            "SELECT {} FROM report_templates WHERE id = ?1",
            REPORT_TEMPLATE_COLUMNS
        );
        if let Ok(Some(template)) = crate::db::query::query_one(
            &tx,
            &sql,
            rusqlite::params![id],
            report_template_from_row,
        ) {
            tx.execute(
                "DELETE FROM report_templates WHERE id = ?1",
                rusqlite::params![id],
            )
            .map_err(|e| e.to_string())?;

            // Try to delete file (best-effort)
            if let Err(e) = std::fs::remove_file(&template.file_path) {
                eprintln!("删除模板文件失败: {}", e);
            }
        }
    }

    tx.commit().map_err(|e| e.to_string())?;

    Ok(())
}

/// 创建报告模板（内容编辑模式）
#[tauri::command]
pub fn create_report_template(
    data: crate::db::models::ReportTemplateCreate,
    state: State<AppState>,
) -> Result<ReportTemplate, String> {
    let conn = state.db.lock();

    let format = data.format.unwrap_or_else(|| "markdown".to_string());
    let description = data.description.unwrap_or_default();
    let sample_data = data.sample_data.unwrap_or_else(|| "{}".to_string());
    let content = data.content.unwrap_or_default();
    let config_json = data.config_json.unwrap_or_default();
    let mode = data.mode.unwrap_or_else(|| "visual".to_string());

    conn.execute(
        "INSERT INTO report_templates (name, vendor, file_path, content, format, is_default, description, sample_data, config_json, mode) \
         VALUES (?1, ?2, '', ?3, ?4, 0, ?5, ?6, ?7, ?8)",
        rusqlite::params![data.name, data.vendor, content, format, description, sample_data, config_json, mode],
    )
    .map_err(|e| e.to_string())?;

    let last_id = conn.last_insert_rowid();

    let sql = format!(
        "SELECT {} FROM report_templates WHERE id = ?1",
        REPORT_TEMPLATE_COLUMNS
    );
    crate::db::query::query_one(&conn, &sql, rusqlite::params![last_id], report_template_from_row)?
        .ok_or_else(|| "创建报告模板后查询失败".to_string())
}

/// 更新报告模板内容
#[tauri::command]
pub fn update_report_template(
    template_id: i64,
    data: crate::db::models::ReportTemplateUpdate,
    state: State<AppState>,
) -> Result<ReportTemplate, String> {
    let conn = state.db.lock();

    // Build dynamic UPDATE
    let mut sets = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref name) = data.name {
        sets.push("name = ?");
        params.push(Box::new(name.clone()));
    }
    if let Some(ref vendor) = data.vendor {
        sets.push("vendor = ?");
        params.push(Box::new(vendor.clone()));
    }
    if let Some(ref content) = data.content {
        sets.push("content = ?");
        params.push(Box::new(content.clone()));
    }
    if let Some(ref format) = data.format {
        sets.push("format = ?");
        params.push(Box::new(format.clone()));
    }
    if let Some(is_default) = data.is_default {
        sets.push("is_default = ?");
        params.push(Box::new(is_default));
    }
    if let Some(ref description) = data.description {
        sets.push("description = ?");
        params.push(Box::new(description.clone()));
    }
    if let Some(ref sample_data) = data.sample_data {
        sets.push("sample_data = ?");
        params.push(Box::new(sample_data.clone()));
    }
    if let Some(ref config_json) = data.config_json {
        sets.push("config_json = ?");
        params.push(Box::new(config_json.clone()));
    }
    if let Some(ref mode) = data.mode {
        sets.push("mode = ?");
        params.push(Box::new(mode.clone()));
    }

    if sets.is_empty() {
        return Err("未提供任何更新字段".to_string());
    }

    sets.push("updated_at = ?");
    params.push(Box::new(now_str()));

    let set_clause = sets.join(", ");
    let sql = format!(
        "UPDATE report_templates SET {} WHERE id = ?{}",
        set_clause,
        params.len() + 1
    );
    params.push(Box::new(template_id));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let affected = conn.execute(&sql, param_refs.as_slice()).map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("报告模板 ID {} 不存在", template_id));
    }

    let query_sql = format!(
        "SELECT {} FROM report_templates WHERE id = ?1",
        REPORT_TEMPLATE_COLUMNS
    );
    crate::db::query::query_one(&conn, &query_sql, rusqlite::params![template_id], report_template_from_row)?
        .ok_or_else(|| "更新报告模板后查询失败".to_string())
}

/// 获取所有可用的模板变量定义（供前端变量选择器使用）
#[tauri::command]
pub fn get_available_variables() -> Result<Vec<template_variables::VariableDef>, String> {
    Ok(template_variables::get_variable_definitions())
}

/// 预览报告模板（用示例数据渲染）
#[tauri::command]
pub fn render_template_preview(
    template_id: i64,
    state: State<AppState>,
) -> Result<String, String> {
    let conn = state.db.lock();

    let sql = format!(
        "SELECT {} FROM report_templates WHERE id = ?1",
        REPORT_TEMPLATE_COLUMNS
    );
    let template = crate::db::query::query_one(
        &conn,
        &sql,
        rusqlite::params![template_id],
        report_template_from_row,
    )?
    .ok_or_else(|| format!("报告模板 ID {} 不存在", template_id))?;

    let ctx = template_variables::build_sample_context();
    Ok(template_engine::render_template_from_config(
        &template.config_json, &template.content, &template.mode, &ctx, &template.format,
    ))
}

// ============================================================
// AI Config Helper
// ============================================================

/// 获取当前激活的 AI 模型配置
#[tauri::command]
pub fn get_active_ai_config(state: State<AppState>) -> Result<AiModelConfig, String> {
    let conn = state.db.lock();

    crate::db::query::query_one(
        &conn,
        "SELECT id, name, provider, model_id, api_key_encrypted, base_url, \
         is_active, created_at, updated_at \
         FROM ai_model_configs WHERE is_active = 1 LIMIT 1",
        &[],
        |row| {
            Ok(AiModelConfig {
                id: row.get(0)?,
                name: row.get(1)?,
                provider: row.get(2)?,
                model_id: row.get(3)?,
                api_key_encrypted: row.get(4)?,
                base_url: row.get(5)?,
                is_active: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        },
    )?
    .ok_or_else(|| "未找到激活的 AI 配置".to_string())
}

// ============================================================
// Log Analysis
// ============================================================

/// 解析单条巡检记录中的设备日志（display logbuffer 等），返回结构化日志分析。
#[tauri::command]
pub fn analyze_record_logs(record_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock();

    let record_sql = format!("SELECT {} FROM inspection_records WHERE id = ?1", RECORD_COLUMNS);
    let record: InspectionRecord = crate::db::query::query_one(
        &conn, &record_sql, rusqlite::params![record_id], record_from_row,
    )?
    .ok_or_else(|| format!("巡检记录 ID {} 不存在", record_id))?;

    let device_sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
    let device = crate::db::query::query_one(
        &conn, &device_sql, rusqlite::params![record.device_id], device_from_row,
    )?
    .ok_or_else(|| format!("设备 ID {} 不存在", record.device_id))?;

    let outputs = parse_command_outputs(&record.command_outputs).unwrap_or_default();

    // Find log-related commands (display logbuffer, show logging, etc.)
    let log_patterns = ["logbuffer", "log buffer", "logging", "log"];
    let mut all_logs = String::new();
    for (cmd, output) in &outputs {
        let cmd_lower = cmd.to_lowercase();
        if log_patterns.iter().any(|p| cmd_lower.contains(p)) {
            if !all_logs.is_empty() { all_logs.push('\n'); }
            all_logs.push_str(output);
        }
    }

    if all_logs.is_empty() {
        return Ok(serde_json::json!({
            "total": 0, "errors": 0, "warnings": 0, "info": 0, "debug": 0,
            "entries": [], "summary": "未找到设备日志数据。请确保巡检模板包含 display logbuffer 或类似命令。",
            "device_name": device.name,
        }));
    }

    let analysis = crate::services::log_analyzer::parse_logs(&all_logs, &device.vendor);
    Ok(serde_json::json!({
        "total": analysis.total,
        "errors": analysis.errors,
        "warnings": analysis.warnings,
        "info": analysis.info,
        "debug": analysis.debug,
        "entries": analysis.entries,
        "summary": analysis.summary,
        "device_name": device.name,
        "device_vendor": device.vendor,
    }))
}

/// 解析批次中所有记录的设备日志，汇总返回。
#[tauri::command]
pub fn analyze_batch_logs(batch_id: i64, state: State<AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock();

    let records_sql = format!("SELECT {} FROM inspection_records WHERE batch_id = ?1", RECORD_COLUMNS);
    let records: Vec<InspectionRecord> = crate::db::query::query_all(
        &conn, &records_sql, rusqlite::params![batch_id], record_from_row,
    )?;

    let mut all_results = Vec::new();
    let mut total_errors = 0usize;
    let mut total_warnings = 0usize;

    for record in &records {
        let result = analyze_record_logs(record.id, state.clone())?;
        if let Some(e) = result.get("errors").and_then(|v| v.as_u64()) {
            total_errors += e as usize;
        }
        if let Some(w) = result.get("warnings").and_then(|v| v.as_u64()) {
            total_warnings += w as usize;
        }
        all_results.push(result);
    }

    Ok(serde_json::json!({
        "batch_id": batch_id,
        "total_errors": total_errors,
        "total_warnings": total_warnings,
        "devices": all_results,
    }))
}

/// 直接解析用户提供的日志文本（与巡检记录无关），返回结构化分析结果。
#[tauri::command]
pub fn parse_log_text(text: String, vendor: String) -> Result<serde_json::Value, String> {
    let analysis = crate::services::log_analyzer::parse_logs(&text, &vendor);
    Ok(serde_json::json!({
        "total": analysis.total,
        "errors": analysis.errors,
        "warnings": analysis.warnings,
        "info": analysis.info,
        "debug": analysis.debug,
        "entries": analysis.entries,
        "summary": analysis.summary,
    }))
}

// ============================================================
// CSV Export
// ============================================================

/// 将巡检批次的命令输出导出为 CSV 文件，返回文件路径。
/// 若提供 save_path 则保存到指定路径，否则保存到默认 reports 目录。
#[tauri::command]
pub fn export_batch_csv(batch_id: i64, save_path: Option<String>, state: State<AppState>) -> Result<String, String> {
    let conn = state.db.lock();

    let batch_sql = format!("SELECT {} FROM inspection_batches WHERE id = ?1", crate::db::models::BATCH_COLUMNS);
    let batch = crate::db::query::query_one(&conn, &batch_sql, rusqlite::params![batch_id], crate::db::models::batch_from_row)?
        .ok_or_else(|| format!("巡检批次 ID {} 不存在", batch_id))?;

    let records_sql = format!("SELECT {} FROM inspection_records WHERE batch_id = ?1 ORDER BY id", RECORD_COLUMNS);
    let records = crate::db::query::query_all(&conn, &records_sql, rusqlite::params![batch_id], record_from_row)?;

    let filepath = if let Some(ref p) = save_path {
        std::path::PathBuf::from(p)
    } else {
        let dir = ensure_reports_dir()?;
        let safe_name = batch.name.unwrap_or_else(|| format!("batch_{}", batch_id));
        let filename = format!("{}_{}.csv", safe_name.replace('/', "_").replace('\\', "_"), now_str().replace(' ', "_").replace(':', "-"));
        dir.join(&filename)
    };

    let mut w = std::io::BufWriter::new(std::fs::File::create(&filepath).map_err(|e| format!("创建CSV文件失败: {}", e))?);

    // BOM for Excel UTF-8 compatibility
    w.write(b"\xEF\xBB\xBF").map_err(|e| e.to_string())?;

    // Header
    writeln!(w, "设备名称,设备IP,厂商,记录状态,命令,命令输出").map_err(|e| e.to_string())?;

    for record in &records {
        let device_sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
        let device = crate::db::query::query_one(&conn, &device_sql, rusqlite::params![record.device_id], device_from_row)?
            .unwrap_or_else(|| crate::db::models::Device {
                id: record.device_id,
                name: "未知设备".into(),
                ip: "".into(),
                device_type: "".into(),
                vendor: "".into(),
                model: None,
                ssh_username: None,
                ssh_password_encrypted: None,
                ssh_port: 22,
                template_id: None,
                status: "unknown".into(),
                last_checked_at: None,
                created_at: "".into(),
                updated_at: "".into(),
            });

        let outputs = parse_command_outputs(&record.command_outputs).unwrap_or_default();

        if outputs.is_empty() {
            writeln!(w, "{},{},{},{},,", csv_escape(&device.name), csv_escape(&device.ip), csv_escape(&device.vendor), record.status).map_err(|e| e.to_string())?;
        } else {
            for (cmd, output) in &outputs {
                writeln!(w, "{},{},{},{},{},{}",
                    csv_escape(&device.name),
                    csv_escape(&device.ip),
                    csv_escape(&device.vendor),
                    record.status,
                    csv_escape(cmd),
                    csv_escape(output),
                ).map_err(|e| e.to_string())?;
            }
        }
    }

    w.flush().map_err(|e| e.to_string())?;
    let path_str = filepath.to_string_lossy().to_string();
    tracing::info!("CSV 导出完成: {}", path_str);
    Ok(path_str)
}

/// CSV field escaping: wrap in quotes if contains comma/newline/quote.
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('\n') || s.contains('"') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ============================================================
// HTML Report Generation
// ============================================================

/// Generate an HTML inspection report for a batch and return the file path.
#[tauri::command]
pub fn generate_html_report(
    batch_id: i64,
    template_id: Option<i64>,
    state: State<AppState>,
) -> Result<String, String> {
    let conn = state.db.lock();

    let html = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::services::report_builder::build_report_html(&conn, batch_id, template_id)
    }))
    .map_err(|e| format!("生成 HTML 报告时发生内部错误: {:?}", e))??;

    let reports_dir = ensure_reports_dir()?;
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let file_name = format!("batch_{}_report_{}.html", batch_id, timestamp);
    let file_path = reports_dir.join(&file_name);

    std::fs::write(&file_path, &html)
        .map_err(|e| format!("保存 HTML 报告失败: {}", e))?;

    let path_str = file_path.to_string_lossy().to_string();
    tracing::info!("HTML 报告已生成: {}", path_str);
    Ok(path_str)
}

/// Open a file in the system default browser.
#[tauri::command]
pub fn open_in_browser(file_path: String) -> Result<(), String> {
    open::that(&file_path).map_err(|e| format!("打开浏览器失败: {}", e))
}
