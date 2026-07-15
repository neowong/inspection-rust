use std::collections::HashMap;

use tauri::State;

use crate::db::models::{
    device_from_row, now_str, record_from_row, report_template_from_row, AiModelConfig,
    InspectionRecord, ReportTemplate, DEVICE_COLUMNS, RECORD_COLUMNS, REPORT_TEMPLATE_COLUMNS,
};
use crate::services::crypto::CryptoService;
use crate::services::docx_engine::ReportCoverContext;
use crate::services::report_config::ReportTemplateConfig;
use crate::services::{ai_inspection, report_config};
use crate::AppState;

// ============================================================
// Helpers
// ============================================================

fn app_data_dir() -> std::path::PathBuf {
    crate::APP_DATA_DIR
        .get()
        .cloned()
        .unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("inspection-rust")
        })
}

fn ensure_reports_dir() -> Result<std::path::PathBuf, String> {
    let dir = app_data_dir().join("data").join("reports");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建报告目录失败: {}", e))?;
    Ok(dir)
}

/// 安全检查：路径是否在 reports 目录内（canonicalize 后比较，防穿越/符号链接）。
/// 返回规范化后的绝对路径，校验失败返回 Err。
pub(crate) fn safe_report_path(path: &str) -> Result<std::path::PathBuf, String> {
    let reports_dir = ensure_reports_dir()?;
    let canonical_reports = reports_dir.canonicalize()
        .map_err(|e| format!("报告目录无法规范化: {}", e))?;
    let p = std::path::PathBuf::from(path);
    let canonical = p.canonicalize().map_err(|_| format!("报告文件不存在: {}", path))?;
    if !canonical.starts_with(&canonical_reports) {
        return Err(format!("不允许访问 reports 目录外的文件: {}", path));
    }
    Ok(canonical)
}

/// 删除单个报告文件：仅在 reports 目录内才删，失败只 warn 不影响调用方。
pub(crate) fn safe_remove_report(path: &str) {
    match safe_report_path(path) {
        Ok(canonical) => {
            if let Err(e) = std::fs::remove_file(&canonical) {
                tracing::warn!("[safe_remove_report] 删除失败 {}: {}", canonical.display(), e);
            }
        }
        Err(e) => tracing::warn!("[safe_remove_report] 可疑路径被阻止: {}", e),
    }
}

fn report_date(record: &InspectionRecord) -> String {
    record
        .completed_at
        .as_deref()
        .or(record.started_at.as_deref())
        .and_then(|s| s.get(..10))
        .unwrap_or("")
        .to_string()
}

fn load_cover_context(
    conn: &rusqlite::Connection,
    batch_id: i64,
    fallback_project: &str,
    inspection_date: String,
) -> ReportCoverContext {
    let batch_info = conn
        .query_row(
            "SELECT name, triggered_by FROM inspection_batches WHERE id = ?1",
            rusqlite::params![batch_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .ok();

    let project_name = batch_info
        .as_ref()
        .and_then(|(name, _)| name.as_deref())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(fallback_project)
        .to_string();
    let inspector = batch_info
        .and_then(|(_, triggered_by)| triggered_by)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "运维人员".to_string());

    ReportCoverContext {
        project_name,
        inspection_date,
        inspector,
    }
}

fn parse_command_outputs(json_str: &Option<String>) -> Result<HashMap<String, String>, String> {
    let empty = "{}".to_string();
    let val: serde_json::Value = serde_json::from_str(json_str.as_deref().unwrap_or(&empty))
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

fn safe_filename(s: &str) -> String {
    let bad: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    let cleaned: String = s
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

// ============================================================
// AI Analysis — Inner (async, takes &AppState)
// ============================================================

async fn analyze_record_inner(
    app_state: &AppState,
    record_id: i64,
) -> Result<serde_json::Value, String> {
    let record_id_owned = record_id;
    let (command_outputs_map, device_id) = {
        let conn = app_state.db.lock();
        let sql = format!(
            "SELECT {} FROM inspection_records WHERE id = ?1",
            RECORD_COLUMNS
        );
        let record = crate::db::query::query_one(
            &conn,
            &sql,
            rusqlite::params![record_id],
            record_from_row,
        )?
        .ok_or_else(|| format!("巡检记录 ID {} 不存在", record_id))?;

        let now = now_str();
        conn.execute(
            "UPDATE inspection_records SET ai_status = 'processing', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, record_id],
        )
        .map_err(|e| e.to_string())?;

        let map = parse_command_outputs(&record.command_outputs)?;
        if map.is_empty() {
            return Err("该记录无命令输出，请先完成巡检".to_string());
        }
        (map, record.device_id)
    };

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

    let cmd_keys: Vec<&String> = command_outputs_map.keys().collect();
    tracing::info!(
        "开始 AI 分析 device_id={} record={}, 共 {} 条命令",
        device_id,
        record_id_owned,
        cmd_keys.len()
    );

    // 加载命令的期望描述（AI 评判提示词）
    let expectations: std::collections::HashMap<String, String> = {
        let conn = app_state.db.lock();
        let mut names: Vec<&str> = cmd_keys.iter().map(|s| s.as_str()).collect();
        names.sort();
        let placeholders: Vec<String> = (0..names.len()).map(|i| format!("?{}", i + 1)).collect();
        let sql = format!(
            "SELECT command, COALESCE(expectation, '') FROM command_pool WHERE command IN ({})",
            placeholders.join(",")
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let params: Vec<Box<dyn rusqlite::ToSql>> = names.iter().map(|s| Box::new(s.to_string()) as Box<dyn rusqlite::ToSql>).collect();
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }).map_err(|e| e.to_string())?;
        let mut map = std::collections::HashMap::new();
        for (cmd, exp) in rows.flatten() {
            if !exp.is_empty() {
                map.insert(cmd, exp);
            }
        }
        map
    };

    let analysis = match provider.as_str() {
        "openai" => {
            ai_inspection::analyze_with_openai(&api_key, &model, &base_url, &command_outputs_map, &expectations)
                .await?
        }
        "deepseek" => {
            let deepseek_base = if base_url.is_empty() {
                "https://api.deepseek.com".to_string()
            } else {
                base_url.clone()
            };
            ai_inspection::analyze_with_openai(
                &api_key,
                &model,
                &deepseek_base,
                &command_outputs_map,
                &expectations,
            )
            .await?
        }
        _ => return Err(format!("不支持的 AI 提供商: {}，请选择 OpenAI 兼容 或 DeepSeek", provider)),
    };

    tracing::info!(
        "AI 分析完成 device_id={} record={}: 综合={} 总结={}",
        device_id,
        record_id_owned,
        analysis
            .get("overall")
            .and_then(|v| v.as_str())
            .unwrap_or("?"),
        analysis
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("?"),
    );

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

        let mut judgments = serde_json::Map::new();
        let mut suggestions: Vec<String> = Vec::new();

        if let Some(items_array) = items {
            let cmd_descs = report_config::load_command_descriptions(&conn);
            let original_keys: Vec<&String> = command_outputs_map.keys().collect();
            for item in items_array {
                if let Some(cmd_raw) = item.get("command").and_then(|v| v.as_str()) {
                    let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                    let finding = item.get("finding").and_then(|v| v.as_str()).unwrap_or("");
                    let cmd_label = cmd_descs
                        .get(cmd_raw)
                        .map(|s| s.as_str())
                        .unwrap_or(cmd_raw);
                    tracing::info!(
                        "  AI分析结果 [{}] {} → {} ({})",
                        status,
                        cmd_label,
                        finding,
                        record_id_owned
                    );

                    // 用原始命令 key 做匹配，避免 AI 返回的命令名与存储的 key 不一致
                    let matched_key = original_keys
                        .iter()
                        .find(|k| k.as_str() == cmd_raw)
                        .or_else(|| {
                            let norm = |s: &str| {
                                s.split_whitespace()
                                    .collect::<Vec<_>>()
                                    .join(" ")
                                    .to_lowercase()
                            };
                            let cmd_norm = norm(cmd_raw);
                            original_keys.iter().find(|k| norm(k) == cmd_norm)
                        })
                        .or_else(|| {
                            let cmd_lower = cmd_raw.to_lowercase();
                            original_keys.iter().find(|k| {
                                k.to_lowercase().contains(&cmd_lower)
                                    || cmd_lower.contains(&k.to_lowercase())
                            })
                        })
                        .map(|k| k.to_string());

                    let store_key = matched_key.unwrap_or_else(|| cmd_raw.to_string());

                    let mut jdg = serde_json::Map::new();
                    jdg.insert(
                        "status".to_string(),
                        serde_json::Value::String(status.to_string()),
                    );
                    if !finding.is_empty() {
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
                    judgments.insert(store_key, serde_json::Value::Object(jdg));
                }
            }
        }

        let command_judgments_json = serde_json::to_string(&serde_json::Value::Object(judgments))
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

#[tauri::command]
pub async fn analyze_record(
    record_id: i64,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    match analyze_record_inner(&state, record_id).await {
        Ok(v) => Ok(v),
        Err(e) => {
            tracing::error!("AI 分析失败 record_id={}: {}", record_id, e);
            // analyze_record_inner 已将 ai_status 置为 'processing'，失败时必须回写 'failed'，
            // 否则记录会永久卡在 processing，前端一直显示"分析中"，且不会被 analyze_batch（非 force）重试。
            let now = now_str();
            let conn = state.db.lock();
            if let Err(db_err) = conn.execute(
                "UPDATE inspection_records SET ai_status = 'failed', error_message = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![e, now, record_id],
            ) {
                // 回写失败会导致记录永久卡 processing，必须记录日志便于排查
                tracing::error!("AI 失败后回写 ai_status=failed 失败 record_id={}: {}", record_id, db_err);
            }
            Err(e)
        }
    }
}

/// 分析批次内所有记录的 AI 结果。
/// `force = true` 时重新分析已完成的记录（重新分析全部）。
#[tauri::command]
pub async fn analyze_batch(
    batch_id: i64,
    force: Option<bool>,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let force = force.unwrap_or(false);
    let record_ids: Vec<i64> = {
        let conn = state.db.lock();
        let sql = if force {
            // 强制模式：分析所有有命令输出的记录（包括已完成的）
            format!(
                "SELECT {} FROM inspection_records WHERE batch_id = ?1 \
                 AND command_outputs IS NOT NULL AND command_outputs != '{{}}'",
                RECORD_COLUMNS
            )
        } else {
            // 普通模式：跳过已完成分析的记录，同时跳过无命令输出的记录
            format!(
                "SELECT {} FROM inspection_records WHERE batch_id = ?1 \
                 AND ai_status != 'completed' \
                 AND command_outputs IS NOT NULL AND command_outputs != '{{}}'",
                RECORD_COLUMNS
            )
        };
        let records: Vec<InspectionRecord> =
            crate::db::query::query_all(&conn, &sql, rusqlite::params![batch_id], record_from_row)?;
        records.into_iter().map(|r| r.id).collect()
    };

    if record_ids.is_empty() {
        return Ok(serde_json::json!({
            "total": 0, "completed": 0, "failed": 0,
            "message": "所有记录已完成 AI 分析"
        }));
    }

    let total = record_ids.len();
    // 有限并发：避免大批次一次性发起过多 AI 请求触发 API 限流。
    // AI HTTP 调用在 db 锁外执行，并发主要受益于此；DB 读写仅短暂持锁。
    const AI_CONCURRENCY: usize = 4;
    use futures::stream::StreamExt;
    let state_ref: &AppState = &state;
    let results: Vec<(i64, Result<serde_json::Value, String>)> =
        futures::stream::iter(record_ids.iter().copied())
            .map(|rid| async move { (rid, analyze_record_inner(state_ref, rid).await) })
            .buffer_unordered(AI_CONCURRENCY)
            .collect()
            .await;

    let mut completed = 0u32;
    let mut failed = 0u32;
    let mut errors: Vec<serde_json::Value> = Vec::new();

    for (rid, result) in results {
        match result {
            Ok(_) => {
                completed += 1;
            }
            Err(e) => {
                failed += 1;
                tracing::error!("AI 分析失败 record_id={}: {}", rid, e);
                errors.push(serde_json::json!({"record_id": rid, "error": e}));
                let conn = state.db.lock();
                let now = now_str();
                conn.execute(
                    "UPDATE inspection_records SET ai_status = 'failed', error_message = ?1, updated_at = ?2 WHERE id = ?3",
                    rusqlite::params![e, now, rid],
                ).map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(serde_json::json!({
        "total": total, "completed": completed, "failed": failed, "errors": errors,
    }))
}

// ============================================================
// Record Query
// ============================================================

#[tauri::command]
pub fn get_record(record_id: i64, state: State<AppState>) -> Result<InspectionRecord, String> {
    let conn = state.db.lock();
    let sql = format!(
        "SELECT {} FROM inspection_records WHERE id = ?1",
        RECORD_COLUMNS
    );
    crate::db::query::query_one(&conn, &sql, rusqlite::params![record_id], record_from_row)?
        .ok_or_else(|| format!("巡检记录 ID {} 不存在", record_id))
}

// ============================================================
// Report Template Resolution
// ============================================================

/// 按以下顺序解析使用哪份报告模板配置：
///   override_id → device → inspection_template.report_template_id → 厂商匹配 → is_default → 内置默认
fn resolve_template_config(
    conn: &rusqlite::Connection,
    record: &InspectionRecord,
    override_template_id: Option<i64>,
) -> ReportTemplateConfig {
    let try_load = |tid: i64| -> Option<ReportTemplateConfig> {
        let sql = format!(
            "SELECT {} FROM report_templates WHERE id = ?1",
            REPORT_TEMPLATE_COLUMNS
        );
        let tpl = crate::db::query::query_one(
            conn,
            &sql,
            rusqlite::params![tid],
            report_template_from_row,
        )
        .ok()
        .flatten()?;
        if tpl.config_json.trim().is_empty() {
            None
        } else {
            Some(report_config::parse_config_json(&tpl.config_json))
        }
    };

    if let Some(tid) = override_template_id {
        if let Some(c) = try_load(tid) {
            return c;
        }
    }

    // device → inspection_template → report_template_id
    let device_sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
    let mut vendor_for_match: Option<String> = None;
    if let Ok(Some(device)) = crate::db::query::query_one(
        conn,
        &device_sql,
        rusqlite::params![record.device_id],
        device_from_row,
    ) {
        vendor_for_match = Some(device.vendor.clone());
        if let Some(tid) = device.template_id {
            let tpl_sql = format!(
                "SELECT {} FROM inspection_templates WHERE id = ?1",
                crate::db::models::TEMPLATE_COLUMNS
            );
            if let Ok(Some(tpl)) = crate::db::query::query_one(
                conn,
                &tpl_sql,
                rusqlite::params![tid],
                crate::db::models::template_from_row,
            ) {
                if let Some(rt_id) = tpl.report_template_id {
                    if let Some(c) = try_load(rt_id) {
                        return c;
                    }
                }
            }
        }
    }

    if let Some(vendor) = vendor_for_match {
        if !vendor.is_empty() {
            let vs = format!(
                "SELECT {} FROM report_templates WHERE vendor = ?1 LIMIT 1",
                REPORT_TEMPLATE_COLUMNS
            );
            if let Ok(Some(t)) = crate::db::query::query_one(
                conn,
                &vs,
                rusqlite::params![vendor],
                report_template_from_row,
            ) {
                if !t.config_json.trim().is_empty() {
                    return report_config::parse_config_json(&t.config_json);
                }
            }
        }
    }

    let ds = format!(
        "SELECT {} FROM report_templates WHERE is_default = 1 LIMIT 1",
        REPORT_TEMPLATE_COLUMNS
    );
    if let Ok(Some(t)) = crate::db::query::query_one(conn, &ds, &[], report_template_from_row) {
        if !t.config_json.trim().is_empty() {
            return report_config::parse_config_json(&t.config_json);
        }
    }

    ReportTemplateConfig::default()
}

// ============================================================
// DOCX Report Generation
// ============================================================

/// 生成单条巡检记录的 docx 报告。返回输出文件路径。
#[tauri::command]
pub fn generate_docx_report(
    record_id: i64,
    template_id: Option<i64>,
    state: State<AppState>,
) -> Result<String, String> {
    // 锁内只读取数据，随后释放锁——DOCX 生成（文件 IO + XML 构建）在锁外执行，
    // 避免生成期间阻塞所有 DB 操作。最后短锁写回 report_path。
    let (record, device, cmd_descs, config, cover) = {
        let conn = state.db.lock();

        let rec_sql = format!(
            "SELECT {} FROM inspection_records WHERE id = ?1",
            RECORD_COLUMNS
        );
        let record = crate::db::query::query_one(
            &conn,
            &rec_sql,
            rusqlite::params![record_id],
            record_from_row,
        )?
        .ok_or_else(|| format!("巡检记录 ID {} 不存在", record_id))?;

        let dev_sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
        let device = crate::db::query::query_one(
            &conn,
            &dev_sql,
            rusqlite::params![record.device_id],
            device_from_row,
        )?
        .ok_or_else(|| format!("设备 ID {} 不存在", record.device_id))?;

        let cmd_descs = report_config::load_command_descriptions(&conn);
        let config = resolve_template_config(&conn, &record, template_id);
        let cover = load_cover_context(
            &conn,
            record.batch_id,
            &device.name,
            report_date(&record),
        );
        (record, device, cmd_descs, config, cover)
    }; // 锁释放

    let reports_dir = ensure_reports_dir()?;
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("report_{}_{}.docx", record_id, timestamp);
    let output_path = reports_dir.join(&filename);

    crate::services::docx_engine::generate_record_docx(
        &config,
        &device,
        &record,
        &cmd_descs,
        &output_path,
        &cover,
    )?;

    let output_str = output_path.to_string_lossy().to_string();
    {
        let conn = state.db.lock();
        conn.execute(
            "UPDATE inspection_records SET report_path = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![output_str, now_str(), record_id],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(output_str)
}

/// 收集批次内所有已完成记录及其设备
fn load_batch_items(
    conn: &rusqlite::Connection,
    batch_id: i64,
) -> Result<Vec<(crate::db::models::Device, InspectionRecord)>, String> {
    let sql = format!(
        "SELECT {} FROM inspection_records WHERE batch_id = ?1 AND status IN ('completed', 'partially_completed') ORDER BY id",
        RECORD_COLUMNS
    );
    let records: Vec<InspectionRecord> =
        crate::db::query::query_all(conn, &sql, rusqlite::params![batch_id], record_from_row)?;
    if records.is_empty() {
        return Err("批次中无已完成记录".to_string());
    }

    let mut items = Vec::with_capacity(records.len());
    for record in records {
        let device_sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
        if let Ok(Some(device)) = crate::db::query::query_one(
            conn,
            &device_sql,
            rusqlite::params![record.device_id],
            device_from_row,
        ) {
            items.push((device, record));
        }
    }
    if items.is_empty() {
        return Err("批次中无关联的设备记录".to_string());
    }
    Ok(items)
}

/// 将批次合并为单个 docx，每台设备从新页开始。返回 docx 文件路径。
#[tauri::command]
pub fn generate_batch_docx_combined(
    batch_id: i64,
    template_id: Option<i64>,
    state: State<AppState>,
) -> Result<String, String> {
    // 锁内读数据 → 锁外生成合并 docx → 短锁写回路径
    let (items, cmd_descs, configs, cover) = {
        let conn = state.db.lock();
        let items = load_batch_items(&conn, batch_id)?;
        let cmd_descs = report_config::load_command_descriptions(&conn);
        // 每台设备按其厂商/模板独立解析报告配置，避免多厂商批次套用首台模板
        let configs: Vec<ReportTemplateConfig> = items
            .iter()
            .map(|(_, r)| resolve_template_config(&conn, r, template_id))
            .collect();
        let cover = load_cover_context(&conn, batch_id, "项目", report_date(&items[0].1));
        (items, cmd_descs, configs, cover)
    }; // 锁释放

    let reports_dir = ensure_reports_dir()?;
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("batch_{}_{}.docx", batch_id, timestamp);
    let output_path = reports_dir.join(&filename);

    crate::services::docx_engine::generate_combined_docx(
        &configs,
        &items,
        &cmd_descs,
        &output_path,
        &cover,
    )?;

    let path_str = output_path.to_string_lossy().to_string();
    // 回写路径到批次，后续可随时下载
    {
        let conn = state.db.lock();
        let _ = conn.execute(
            "UPDATE inspection_batches SET combined_report_path = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![path_str, now_str(), batch_id],
        );
    }
    Ok(path_str)
}

// ============================================================
// Download / Delete
// ============================================================

/// 下载单条巡检报告（系统对话框另存）
#[tauri::command]
pub async fn download_report(
    app: tauri::AppHandle,
    record_id: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt;

    let (canonical_report, device_name) = {
        let conn = state.db.lock();
        let sql = format!(
            "SELECT {} FROM inspection_records WHERE id = ?1",
            RECORD_COLUMNS
        );
        let record = crate::db::query::query_one(
            &conn,
            &sql,
            rusqlite::params![record_id],
            record_from_row,
        )?
        .ok_or_else(|| format!("巡检记录 ID {} 不存在", record_id))?;
        let path = record
            .report_path
            .clone()
            .ok_or_else(|| format!("记录 ID {} 尚未生成报告", record_id))?;
        // 安全校验：确保 report_path 在 reports 目录内，防任意文件读
        let canonical_report = safe_report_path(&path)?;
        let dev_sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
        let device = crate::db::query::query_one(
            &conn,
            &dev_sql,
            rusqlite::params![record.device_id],
            device_from_row,
        )?
        .ok_or_else(|| format!("设备 ID {} 不存在", record.device_id))?;
        (canonical_report, device.name)
    };

    let suggested = format!("{}-巡检报告.docx", safe_filename(&device_name));
    let report_path_clone = canonical_report;
    app.dialog()
        .file()
        .add_filter("Word Document", &["docx"])
        .set_file_name(&suggested)
        .save_file(move |file_path| {
            if let Some(save_path) = file_path {
                if let Some(dest) = save_path.as_path().map(|p| p.to_path_buf()) {
                    if let Err(e) = std::fs::copy(&report_path_clone, &dest) {
                        eprintln!("复制报告文件失败: {}", e);
                    }
                }
            }
        });
    Ok(())
}

/// 通用：把已生成的临时文件复制到用户选择的路径
#[tauri::command]
pub async fn save_generated_file(
    app: tauri::AppHandle,
    source_path: String,
    suggested_name: String,
    extension: String,
) -> Result<(), String> {
    // 校验源路径在 reports 目录内（防止路径穿越）
    let src = std::path::PathBuf::from(&source_path);
    let reports_dir = ensure_reports_dir()?;
    let canonical_src = src.canonicalize().map_err(|_| "源文件不存在")?;
    let canonical_reports = reports_dir.canonicalize().unwrap_or_else(|_| reports_dir.clone());
    if !canonical_src.starts_with(&canonical_reports) {
        return Err("不允许复制 reports 目录外的文件".to_string());
    }

    use tauri_plugin_dialog::DialogExt;
    let ext_label = match extension.as_str() {
        "zip" => "Zip Archive",
        "docx" => "Word Document",
        _ => "File",
    };
    let extension_clone = extension.clone();
    // 复制用已校验的 canonical_src，避免校验与复制之间符号链接替换（TOCTOU）
    app.dialog()
        .file()
        .add_filter(ext_label, &[extension_clone.as_str()])
        .set_file_name(&suggested_name)
        .save_file(move |file_path| {
            if let Some(save_path) = file_path {
                if let Some(dest) = save_path.as_path().map(|p| p.to_path_buf()) {
                    if let Err(e) = std::fs::copy(&canonical_src, &dest) {
                        eprintln!("复制生成文件失败: {}", e);
                    }
                }
            }
        });
    Ok(())
}

/// 删除指定记录的报告文件（清空 report_path）
#[tauri::command]
pub fn delete_record_report(record_id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock();
    let record_sql = format!(
        "SELECT {} FROM inspection_records WHERE id = ?1",
        RECORD_COLUMNS
    );
    let record = crate::db::query::query_one(
        &conn,
        &record_sql,
        rusqlite::params![record_id],
        record_from_row,
    )?
    .ok_or_else(|| format!("巡检记录 ID {} 不存在", record_id))?;

    if let Some(ref path) = record.report_path {
        safe_remove_report(path);
    }
    conn.execute(
        "UPDATE inspection_records SET report_path = NULL, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now_str(), record_id],
    )
    .map_err(|e| format!("清除报告记录失败: {}", e))?;
    Ok(())
}

// ============================================================
// Report Template CRUD
// ============================================================

#[tauri::command]
pub fn list_report_templates(state: State<AppState>) -> Result<Vec<ReportTemplate>, String> {
    let conn = state.db.lock();
    let sql = format!(
        "SELECT {} FROM report_templates ORDER BY is_default DESC, created_at DESC",
        REPORT_TEMPLATE_COLUMNS
    );
    crate::db::query::query_all(&conn, &sql, &[], report_template_from_row)
}

#[tauri::command]
pub fn create_report_template(
    data: crate::db::models::ReportTemplateCreate,
    state: State<AppState>,
) -> Result<ReportTemplate, String> {
    let conn = state.db.lock();

    let description = data.description.unwrap_or_default();
    let config_json = data
        .config_json
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(report_config::default_config_json);

    conn.execute(
        "INSERT INTO report_templates (name, vendor, is_default, description, config_json) \
         VALUES (?1, ?2, 0, ?3, ?4)",
        rusqlite::params![data.name, data.vendor, description, config_json],
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
    .ok_or_else(|| "创建报告模板后查询失败".to_string())
}

#[tauri::command]
pub fn update_report_template(
    template_id: i64,
    data: crate::db::models::ReportTemplateUpdate,
    state: State<AppState>,
) -> Result<ReportTemplate, String> {
    let mut conn = state.db.lock();

    let mut sets: Vec<&str> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref name) = data.name {
        sets.push("name = ?");
        params.push(Box::new(name.clone()));
    }
    if let Some(ref vendor) = data.vendor {
        sets.push("vendor = ?");
        params.push(Box::new(vendor.clone()));
    }
    if let Some(is_default) = data.is_default {
        sets.push("is_default = ?");
        params.push(Box::new(is_default));
    }
    if let Some(ref description) = data.description {
        sets.push("description = ?");
        params.push(Box::new(description.clone()));
    }
    if let Some(ref config_json) = data.config_json {
        sets.push("config_json = ?");
        params.push(Box::new(config_json.clone()));
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

    // 用事务包裹整个更新：若同时需要设为默认，清零其他模板和字段更新在同一事务中完成，
    // 避免"先提交清零再更新字段"导致的中间态数据不一致问题（P1: is_default 全被清零）。
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if data.is_default == Some(1) {
        tx.execute(
            "UPDATE report_templates SET is_default = 0, updated_at = ?1 WHERE id != ?2",
            rusqlite::params![now_str(), template_id],
        )
        .map_err(|e| e.to_string())?;
    }

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let affected = tx
        .execute(&sql, param_refs.as_slice())
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("报告模板 ID {} 不存在", template_id));
    }
    tx.commit().map_err(|e| e.to_string())?;

    let q = format!(
        "SELECT {} FROM report_templates WHERE id = ?1",
        REPORT_TEMPLATE_COLUMNS
    );
    crate::db::query::query_one(
        &conn,
        &q,
        rusqlite::params![template_id],
        report_template_from_row,
    )?
    .ok_or_else(|| "更新报告模板后查询失败".to_string())
}

#[tauri::command]
pub fn delete_report_template(template_id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock();

    conn.execute(
        "UPDATE inspection_templates SET report_template_id = NULL WHERE report_template_id = ?1",
        rusqlite::params![template_id],
    )
    .map_err(|e| e.to_string())?;

    let affected = conn
        .execute(
            "DELETE FROM report_templates WHERE id = ?1",
            rusqlite::params![template_id],
        )
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("报告模板 ID {} 不存在", template_id));
    }
    Ok(())
}

// ============================================================
// Log Analysis
// ============================================================

#[tauri::command]
pub fn analyze_record_logs(
    record_id: i64,
    state: State<AppState>,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock();
    let record_sql = format!(
        "SELECT {} FROM inspection_records WHERE id = ?1",
        RECORD_COLUMNS
    );
    let record: InspectionRecord = crate::db::query::query_one(
        &conn,
        &record_sql,
        rusqlite::params![record_id],
        record_from_row,
    )?
    .ok_or_else(|| format!("巡检记录 ID {} 不存在", record_id))?;

    let device_sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
    let device = crate::db::query::query_one(
        &conn,
        &device_sql,
        rusqlite::params![record.device_id],
        device_from_row,
    )?
    .ok_or_else(|| format!("设备 ID {} 不存在", record.device_id))?;

    let outputs = parse_command_outputs(&record.command_outputs).unwrap_or_default();
    let log_patterns = ["logbuffer", "log buffer", "logging", "log"];
    let mut all_logs = String::new();
    for (cmd, output) in &outputs {
        let cmd_lower = cmd.to_lowercase();
        if log_patterns.iter().any(|p| cmd_lower.contains(p)) {
            if !all_logs.is_empty() {
                all_logs.push('\n');
            }
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
        "total": analysis.total, "errors": analysis.errors, "warnings": analysis.warnings,
        "info": analysis.info, "debug": analysis.debug, "entries": analysis.entries,
        "summary": analysis.summary, "device_name": device.name, "device_vendor": device.vendor,
    }))
}

#[tauri::command]
pub fn parse_log_text(text: String, vendor: String) -> Result<serde_json::Value, String> {
    let analysis = crate::services::log_analyzer::parse_logs(&text, &vendor);
    Ok(serde_json::json!({
        "total": analysis.total, "errors": analysis.errors, "warnings": analysis.warnings,
        "info": analysis.info, "debug": analysis.debug, "entries": analysis.entries,
        "summary": analysis.summary,
    }))
}

/// AI 日志分析 - 支持网络/安全/Linux 等多种日志类型
#[tauri::command]
pub async fn analyze_logs_ai(
    _state: State<'_, AppState>,
    text: String,
    log_type: String,
    vendor: String,
    device_type: String,
    ai_config_id: i64,
) -> Result<serde_json::Value, String> {
    // 1. 获取 AI 配置（同步块，释放锁）
    let (api_key, base_url, model_id) = {
        let conn = _state.db.lock();
        let sql = "SELECT api_key_encrypted, base_url, model_id FROM ai_model_configs WHERE id = ?1";
        let (encrypted_key, url, m_id): (String, Option<String>, String) = conn
            .query_row(sql, [ai_config_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .map_err(|_| "AI 配置不存在".to_string())?;
        let key = CryptoService::decrypt(&encrypted_key)
            .map_err(|e| format!("解密 API key 失败: {}", e))?;
        (key, url.unwrap_or_default(), m_id)
    };

    // 2. 构建日志分析 prompt
    let system_prompt = build_log_analysis_prompt(&log_type);

    // 3. 智能预处理日志（去重 → 按优先级保留 → 截断防溢出）
    let max_ai_chars = 6000;
    let (log_content, total_lines, kept_lines, dropped_reason) = smart_truncate_log(&text, max_ai_chars);

    let user_message = if vendor.is_empty() {
        format!(
            "请分析以下{}日志。厂商和设备类型未指定，请根据日志内容自动识别，并在结果中返回 identified_vendor 和 identified_device_type。\n\n--- 日志开始（共 {} 行，显示 {} 行{}）---\n{}\n--- 日志结束 ---",
            log_type, total_lines, kept_lines, dropped_reason, log_content
        )
    } else {
        format!(
            "请分析以下{}日志：\n\n厂商/系统：{}\n设备类型：{}\n\n--- 日志开始（共 {} 行，显示 {} 行{}）---\n{}\n--- 日志结束 ---",
            log_type, vendor, device_type, total_lines, kept_lines, dropped_reason, log_content
        )
    };

    // 4. 调用 AI
    let url = ai_inspection::build_chat_url(&base_url);
    let client = ai_inspection::get_client();

    let body = serde_json::json!({
        "model": model_id,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_message}
        ],
        "temperature": 0.3,
        "max_tokens": 8192
    });

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("AI 请求失败: {}", e))?;

    let status = response.status();
    let response_text = response
        .text()
        .await
        .map_err(|e| format!("读取 AI 响应失败: {}", e))?;

    if !status.is_success() {
        return Err(format!("AI API 错误 ({}): {}", status, response_text));
    }

    let parsed: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("解析 AI 响应 JSON 失败: {}", e))?;

    let content = parsed["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("");

    // 尝试解析 AI 返回的 JSON
    let ai_result = content
        .trim()
        .strip_prefix("```json")
        .or_else(|| content.trim().strip_prefix("```"))
        .map(|s| s.strip_suffix("```").unwrap_or(s))
        .unwrap_or(content.trim());

    match serde_json::from_str::<serde_json::Value>(ai_result) {
        Ok(json) => Ok(serde_json::json!({
            "success": true,
            "summary": json.get("summary").or_else(|| json.get("overall_summary")).and_then(|v| v.as_str()).unwrap_or("分析完成"),
            "overall": json.get("overall").and_then(|v| v.as_str()).unwrap_or("info"),
            "entries": json.get("entries").or_else(|| json.get("items")).cloned().unwrap_or(serde_json::Value::Array(vec![])),
            "advice": json.get("advice").or_else(|| json.get("suggestions")).and_then(|v| v.as_str()).unwrap_or(""),
            "identified_vendor": json.get("identified_vendor").and_then(|v| v.as_str()).unwrap_or(""),
            "identified_device_type": json.get("identified_device_type").and_then(|v| v.as_str()).unwrap_or(""),
            "raw": content,
            "total_lines": total_lines,
            "kept_lines": kept_lines,
        })),
        Err(_) => Ok(serde_json::json!({
            "success": true,
            "summary": "分析完成",
            "overall": "info",
            "entries": [],
            "advice": "",
            "raw": content,
        })),
    }
}

/// 智能日志截断：去重 + 按优先级保留关键行，防止 AI 上下文溢出。
/// 返回 (处理后的文本, 总行数, 保留行数, 丢弃原因说明)。
fn smart_truncate_log(raw: &str, max_chars: usize) -> (String, usize, usize, String) {
    let lines: Vec<&str> = raw.lines().collect();
    let total = lines.len();

    if total == 0 {
        return (String::new(), 0, 0, "".to_string());
    }

    // 1. 去重（连续相同行只保留第一条，但计数保留）
    let mut deduped: Vec<&str> = Vec::new();
    let mut skipped = 0usize;
    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if deduped.last().map_or(false, |last| *last == trimmed) {
            skipped += 1;
            continue;
        }
        deduped.push(trimmed);
    }

    // 如果去重后直接能放下，完美
    let candidate = deduped.join("\n");
    if candidate.len() <= max_chars {
        let reason = if skipped > 0 {
            format!("，已合并 {} 行重复内容", skipped)
        } else {
            String::new()
        };
        return (candidate, total, deduped.len(), reason);
    }

    // 2. 按优先级排序：错误行 > 警告行 > 普通行
    let is_error = |line: &str| {
        let upper = line.to_uppercase();
        upper.contains("ERROR") || upper.contains("CRIT") || upper.contains("EMERG")
            || upper.contains("FATAL") || upper.contains("FAIL") || upper.contains("DENIED")
            || upper.contains("OOM") || upper.contains("PANIC")
    };
    let is_warning = |line: &str| {
        let upper = line.to_uppercase();
        upper.contains("WARNING") || upper.contains("ALERT") || upper.contains("NOTICE")
            || upper.contains("ABNORMAL") || upper.contains("DROPPED") || upper.contains("TIMEOUT")
    };

    // 分割
    let mut errors: Vec<&str> = Vec::new();
    let mut warnings: Vec<&str> = Vec::new();
    let mut normals: Vec<&str> = Vec::new();

    for line in &deduped {
        if is_error(line) {
            errors.push(line);
        } else if is_warning(line) {
            warnings.push(line);
        } else {
            normals.push(line);
        }
    }

    // 3. 按优先级填充，直到 max_chars
    let mut result_lines: Vec<String> = Vec::new();
    let mut remaining = max_chars;

    for batch in [&errors, &warnings, &normals] {
        for line in batch {
            let line_len = line.len() + 1;
            if line_len <= remaining {
                result_lines.push(line.to_string());
                remaining -= line_len;
            } else if remaining > 50 {
                let truncated: String = line.chars().take(remaining.saturating_sub(4)).collect();
                result_lines.push(format!("{}...", truncated.trim()));
                remaining = 0;
                break;
            } else {
                break;
            }
        }
        if remaining < 10 {
            break;
        }
    }

    let kept = result_lines.len();
    let dropped = deduped.len() - kept;
    let result = result_lines.join("\n");

    let reason = if skipped > 0 && dropped > 0 {
        format!("，已合并 {} 行重复 + 丢弃 {} 行低优先级内容", skipped, dropped)
    } else if dropped > 0 {
        format!("，丢弃 {} 行低优先级内容（优先保留错误/警告）", dropped)
    } else if skipped > 0 {
        format!("，已合并 {} 行重复内容", skipped)
    } else {
        String::new()
    };

    (result, total, kept, reason)
}

/// 构建日志分析系统提示词
fn build_log_analysis_prompt(log_type: &str) -> String {
    let base_prompt = r#"你是一位专业的 IT 运维工程师。请分析以下日志内容，识别关键事件、异常和安全隐患。

请按 JSON 格式返回分析结果：
{
  "summary": "整体分析概述，一句话总结",
  "overall": "ok/info/warning/critical",
  "entries": [
    {
      "time": "事件时间（如果日志包含时间戳）",
      "level": "级别（如 ERROR/WARNING/INFO）",
      "source": "来源模块或进程",
      "content": "原始日志内容（保留原文）",
      "analysis": "对此条日志的分析判断（≤20字）",
      "severity": "危害等级（high/medium/low）"
    }
  ],
  "stats": {
    "total": 总条数,
    "errors": 错误数,
    "warnings": 警告数,
    "info": 信息数
  },
  "advice": "运维建议（基于日志分析给出的改进建议，≤100字）"
}

注意：
- 只分析有意义的日志行，忽略空行/分隔行
- severity 标注每条日志的危害程度
- 如果日志数量较多，请合理归类后分析
- 对于高危事件（如登录失败、配置变更、异常复位等）要特别标注"#
;

    let type_prompt = match log_type {
        "network" => r#"
【网络设备日志分析要点】
- 重点关注接口 UP/DOWN、链路振荡、STP 拓扑变更
- CPU 和内存告警、FAN/POWER 硬件故障
- 配置变更、登录认证失败、ACL 拒绝记录
- OSPF/BGP 邻居震荡、路由环路
- 常见厂商格式：H3C(%MMM DD HH:MM:SS:mmm YYYY hostname MODULE/SEV/MNEMONIC: msg), Cisco(*MMM DD HH:MM:SS.mmm: %FACILITY-SEV-MNEMONIC: msg)
"#.to_string(),
        "security" => r#"
【安全设备/安全日志分析要点】
- 重点关注暴力破解、DDoS 攻击、端口扫描等攻击行为
- 防火墙策略命中/拒绝记录、IPS/IDS 告警
- VPN 隧道状态、用户认证失败（多次）
- 恶意软件检测、异常流量、DNS 异常查询
- 0Day/漏洞利用特征、权限提升尝试
- 合规性违规记录
"#.to_string(),
        "linux" => r#"
【Linux 系统日志分析要点】
- 重点关注 SSH 登录记录（成功/失败）、sudo 提权操作
- 系统错误（如 disk I/O error、file system full、OOM killer）
- 服务启动/停止异常（systemd 服务状态变更）
- 内核错误、panic、硬件错误（EDAC、MCE）
- cron 任务执行异常、应用程序崩溃
- 安全相关：fail2ban 封禁、firewalld/iptables 规则变更、SELinux 告警
- 常见文件：/var/log/syslog, /var/log/messages, /var/log/auth.log, /var/log/kern.log
"#.to_string(),
        _ => "".to_string(),
    };

    format!("{}{}", base_prompt, type_prompt)
}

/// 导出日志分析结果到文件（前端调用 save 对话框后写入）
#[tauri::command]
pub fn export_log_analysis(
    save_path: String,
    content: String,
) -> Result<(), String> {
    std::fs::write(&save_path, &content)
        .map_err(|e| format!("写入文件失败: {}", e))
}

/// 用系统文件管理器打开报告目录，方便用户查看历史报告
#[tauri::command]
pub fn open_reports_dir() -> Result<(), String> {
    let dir = crate::APP_DATA_DIR
        .get()
        .ok_or("数据目录未初始化")?
        .join("data")
        .join("reports");
    std::fs::create_dir_all(&dir).ok();
    #[cfg(target_os = "windows")]
    { std::process::Command::new("explorer").arg(&dir).spawn().ok(); }
    #[cfg(target_os = "linux")]
    { std::process::Command::new("xdg-open").arg(&dir).spawn().ok(); }
    #[cfg(target_os = "macos")]
    { std::process::Command::new("open").arg(&dir).spawn().ok(); }
    Ok(())
}
