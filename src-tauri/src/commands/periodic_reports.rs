use std::collections::HashMap;
use std::path::PathBuf;

use chrono::Datelike;
use tauri::State;
use tracing::{info, warn, debug};

use crate::db::models::{
    now_str, periodic_report_from_row, PeriodicReport,
    PERIODIC_REPORT_COLUMNS,
};
use crate::AppState;

// ============================================================
// Helpers
// ============================================================

fn app_data_dir() -> PathBuf {
    crate::APP_DATA_DIR
        .get()
        .cloned()
        .unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("inspection-rust")
        })
}

fn ensure_periodic_reports_dir() -> Result<PathBuf, String> {
    let dir = app_data_dir().join("data").join("periodic_reports");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建周期报告目录失败: {}", e))?;
    Ok(dir)
}

/// 根据报告类型和当前日期，计算建议的周期范围（内部辅助函数）
fn calculate_period_range(report_type: &str) -> (String, String) {
    let now = chrono::Local::now().naive_local().date();
    match report_type {
        "weekly" => {
            // 本周一到今天
            let weekday = now.weekday().num_days_from_monday();
            let start = now - chrono::Duration::days(weekday as i64);
            (start.format("%Y-%m-%d").to_string(), now.format("%Y-%m-%d").to_string())
        }
        "monthly" => {
            // 本月 1 号到今天
            let start = now.with_day(1).unwrap_or(now);
            (start.format("%Y-%m-%d").to_string(), now.format("%Y-%m-%d").to_string())
        }
        "quarterly" => {
            // 本季度第一天到今天
            let month = now.month();
            let quarter_start_month = match month {
                1..=3 => 1,
                4..=6 => 4,
                7..=9 => 7,
                _ => 10,
            };
            let start = now.with_month(quarter_start_month).unwrap_or(now).with_day(1).unwrap_or(now);
            (start.format("%Y-%m-%d").to_string(), now.format("%Y-%m-%d").to_string())
        }
        "yearly" => {
            // 本年 1 月 1 日到今天
            let start = now.with_month(1).unwrap_or(now).with_day(1).unwrap_or(now);
            (start.format("%Y-%m-%d").to_string(), now.format("%Y-%m-%d").to_string())
        }
        _ => {
            let start = now - chrono::Duration::days(30);
            (start.format("%Y-%m-%d").to_string(), now.format("%Y-%m-%d").to_string())
        }
    }
}

// ============================================================
// 内部函数（供 scheduler 调用）
// ============================================================

/// 聚合巡检统计数据
pub fn aggregate_inspection_stats(
    conn: &rusqlite::Connection,
    period_start: &str,
    period_end: &str,
    device_ids: &[i64],
) -> Result<serde_json::Value, String> {
    debug!("聚合巡检统计: start={}, end={}, devices={}", period_start, period_end, device_ids.len());

    // 查询时间范围内的巡检记录
    let mut sql = String::from(
        "SELECT ir.device_id, ir.summary_judgment, ir.ai_analysis, ir.command_judgments, \
                ir.completed_at, d.name, d.ip, d.vendor \
         FROM inspection_records ir \
         JOIN devices d ON ir.device_id = d.id \
         WHERE ir.status = 'completed' \
           AND ir.completed_at >= ?1 \
           AND ir.completed_at < ?2"
    );

    if !device_ids.is_empty() {
        let placeholders: Vec<String> = device_ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 3)).collect();
        sql.push_str(&format!(" AND ir.device_id IN ({})", placeholders.join(",")));
    }

    let mut stmt = conn.prepare(&sql).map_err(|e| format!("准备查询失败: {}", e))?;

    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![
        Box::new(period_start.to_string()),
        Box::new(format!("{} 23:59:59", period_end)),
    ];
    for &did in device_ids {
        params.push(Box::new(did));
    }
    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok((
            row.get::<_, i64>(0)?,           // device_id
            row.get::<_, Option<String>>(1)?, // summary_judgment
            row.get::<_, Option<String>>(2)?, // ai_analysis
            row.get::<_, Option<String>>(3)?, // command_judgments
            row.get::<_, Option<String>>(4)?, // completed_at
            row.get::<_, String>(5)?,         // device name
            row.get::<_, String>(6)?,         // ip
            row.get::<_, String>(7)?,         // vendor
        ))
    }).map_err(|e| format!("查询巡检记录失败: {}", e))?;

    let mut device_stats: HashMap<i64, DeviceStats> = HashMap::new();
    let mut daily_trend: HashMap<String, StatusCounts> = HashMap::new();
    let mut total_inspections = 0;
    let mut overall_counts = StatusCounts::default();

    for row in rows.flatten() {
        let (device_id, judgment, _ai_analysis, _cmd_judgments, completed_at, name, ip, vendor) = row;
        total_inspections += 1;

        let status = judgment.as_deref().unwrap_or("unknown");
        let status_str = match status {
            "ok" | "normal" => "ok",
            "warning" | "warn" => "warning",
            "critical" | "error" | "serious" => "critical",
            _ => "ok",
        };

        // 总体统计
        overall_counts.increment(status_str);

        // 每日趋势
        if let Some(ref date_str) = completed_at {
            let date = &date_str[..10.min(date_str.len())];
            daily_trend.entry(date.to_string()).or_default().increment(status_str);
        }

        // 每设备统计
        let entry = device_stats.entry(device_id).or_insert_with(|| DeviceStats {
            device_id,
            name: name.clone(),
            ip: ip.clone(),
            vendor: vendor.clone(),
            inspection_count: 0,
            status_counts: StatusCounts::default(),
            top_issues: Vec::new(),
        });
        entry.inspection_count += 1;
        entry.status_counts.increment(status_str);
    }

    // 构建设备列表
    let per_device: Vec<serde_json::Value> = device_stats.values().map(|ds| {
        let health_score = if ds.inspection_count > 0 {
            (ds.status_counts.ok as f64 / ds.inspection_count as f64 * 100.0).round() as i64
        } else {
            0
        };
        serde_json::json!({
            "device_id": ds.device_id,
            "device_name": ds.name,
            "ip": ds.ip,
            "vendor": ds.vendor,
            "inspection_count": ds.inspection_count,
            "status_counts": {
                "ok": ds.status_counts.ok,
                "warning": ds.status_counts.warning,
                "critical": ds.status_counts.critical,
            },
            "health_score": health_score,
            "top_issues": ds.top_issues,
        })
    }).collect();

    // 构建每日趋势
    let mut daily: Vec<serde_json::Value> = daily_trend.iter().map(|(date, counts)| {
        serde_json::json!({
            "date": date,
            "ok": counts.ok,
            "warning": counts.warning,
            "critical": counts.critical,
        })
    }).collect();
    daily.sort_by(|a, b| {
        a.get("date").and_then(|v| v.as_str()).unwrap_or("")
            .cmp(b.get("date").and_then(|v| v.as_str()).unwrap_or(""))
    });

    // 告警排行
    let mut alert_ranking: Vec<serde_json::Value> = device_stats.values()
        .map(|ds| {
            serde_json::json!({
                "device_name": ds.name,
                "alert_count": ds.status_counts.warning + ds.status_counts.critical,
            })
        })
        .collect();
    alert_ranking.sort_by(|a, b| {
        let a_count = a.get("alert_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let b_count = b.get("alert_count").and_then(|v| v.as_i64()).unwrap_or(0);
        b_count.cmp(&a_count)
    });

    // 最佳/最差设备
    let best = per_device.iter()
        .max_by_key(|d| d.get("health_score").and_then(|v| v.as_i64()).unwrap_or(0))
        .and_then(|d| d.get("device_name").and_then(|v| v.as_str()).map(String::from));
    let worst = per_device.iter()
        .min_by_key(|d| d.get("health_score").and_then(|v| v.as_i64()).unwrap_or(100))
        .and_then(|d| d.get("device_name").and_then(|v| v.as_str()).map(String::from));

    let health_score = if total_inspections > 0 {
        (overall_counts.ok as f64 / total_inspections as f64 * 100.0).round() as i64
    } else {
        0
    };

    debug!("聚合结果: total_inspections={}, total_devices={}, health_score={}", total_inspections, device_stats.len(), health_score);

    Ok(serde_json::json!({
        "overview": {
            "total_inspections": total_inspections,
            "total_devices": device_stats.len(),
            "status_counts": {
                "ok": overall_counts.ok,
                "warning": overall_counts.warning,
                "critical": overall_counts.critical,
            },
            "health_score": health_score,
        },
        "per_device": per_device,
        "daily_trend": daily,
        "comparison": {
            "alert_ranking": alert_ranking,
            "best_device": best,
            "worst_device": worst,
        }
    }))
}

#[derive(Default)]
struct StatusCounts {
    ok: i64,
    warning: i64,
    critical: i64,
}

impl StatusCounts {
    fn increment(&mut self, status: &str) {
        match status {
            "ok" => self.ok += 1,
            "warning" => self.warning += 1,
            "critical" => self.critical += 1,
            _ => self.ok += 1,
        }
    }
}

struct DeviceStats {
    device_id: i64,
    name: String,
    ip: String,
    vendor: String,
    inspection_count: i64,
    status_counts: StatusCounts,
    top_issues: Vec<String>,
}

/// 构建 AI 提示词（供后续 AI 集成使用）
#[allow(dead_code)]
fn build_periodic_ai_prompt(report_type: &str, period_start: &str, period_end: &str, stats: &serde_json::Value) -> String {
    let report_type_cn = match report_type {
        "weekly" => "周报",
        "monthly" => "月报",
        "quarterly" => "季报",
        "yearly" => "年报",
        _ => "周期报告",
    };

    let overview = stats.get("overview").unwrap_or(&serde_json::Value::Null);
    let status_counts = overview.get("status_counts").unwrap_or(&serde_json::Value::Null);

    format!(
        "你是一位资深运维分析师。请根据以下周期巡检统计数据，生成专业的运维分析报告。\n\n\
         【报告周期】{} ({} ~ {})\n\
         【设备数量】{} 台\n\
         【巡检次数】{} 次\n\n\
         【状态统计】\n\
         正常: {} 次\n\
         警告: {} 次\n\
         严重: {} 次\n\
         健康分数: {}/100\n\n\
         【设备详情】\n{}\n\n\
         【告警排行】\n{}\n\n\
         请输出：\n\
         1. 【趋势分析】2-3 段文字，分析本周期设备健康趋势\n\
         2. 【运维建议】3-5 条具体可执行的运维建议\n\
         3. 【风险预警】需要重点关注的设备和潜在风险",
        report_type_cn,
        period_start,
        period_end,
        overview.get("total_devices").and_then(|v| v.as_i64()).unwrap_or(0),
        overview.get("total_inspections").and_then(|v| v.as_i64()).unwrap_or(0),
        status_counts.get("ok").and_then(|v| v.as_i64()).unwrap_or(0),
        status_counts.get("warning").and_then(|v| v.as_i64()).unwrap_or(0),
        status_counts.get("critical").and_then(|v| v.as_i64()).unwrap_or(0),
        overview.get("health_score").and_then(|v| v.as_i64()).unwrap_or(0),
        format_device_details(stats),
        format_alert_ranking(stats),
    )
}

#[allow(dead_code)]
fn format_device_details(stats: &serde_json::Value) -> String {
    let devices = match stats.get("per_device").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return "无设备数据".to_string(),
    };

    devices.iter().map(|d| {
        format!("- {} ({} {}): 巡检{}次, 正常{} 警告{} 严重{}, 健康分{}",
            d.get("device_name").and_then(|v| v.as_str()).unwrap_or("?"),
            d.get("vendor").and_then(|v| v.as_str()).unwrap_or("?"),
            d.get("ip").and_then(|v| v.as_str()).unwrap_or("?"),
            d.get("inspection_count").and_then(|v| v.as_i64()).unwrap_or(0),
            d.get("status_counts").and_then(|v| v.get("ok")).and_then(|v| v.as_i64()).unwrap_or(0),
            d.get("status_counts").and_then(|v| v.get("warning")).and_then(|v| v.as_i64()).unwrap_or(0),
            d.get("status_counts").and_then(|v| v.get("critical")).and_then(|v| v.as_i64()).unwrap_or(0),
            d.get("health_score").and_then(|v| v.as_i64()).unwrap_or(0),
        )
    }).collect::<Vec<_>>().join("\n")
}

#[allow(dead_code)]
fn format_alert_ranking(stats: &serde_json::Value) -> String {
    let ranking = match stats.get("comparison").and_then(|v| v.get("alert_ranking")).and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return "无告警数据".to_string(),
    };

    ranking.iter().enumerate().map(|(i, r)| {
        format!("{}. {} — {} 次告警",
            i + 1,
            r.get("device_name").and_then(|v| v.as_str()).unwrap_or("?"),
            r.get("alert_count").and_then(|v| v.as_i64()).unwrap_or(0),
        )
    }).collect::<Vec<_>>().join("\n")
}

/// 调用 AI 生成周期总结（供后续 AI 集成使用）
#[allow(dead_code)]
async fn generate_ai_summary(
    app_state: &AppState,
    report_type: &str,
    period_start: &str,
    period_end: &str,
    stats: &serde_json::Value,
) -> Result<String, String> {
    let (provider, model, api_key, base_url) = {
        let conn = app_state.db.lock();
        let config = crate::db::query::query_one(
            &conn,
            "SELECT id, name, provider, model_id, api_key_encrypted, base_url, \
             is_active, created_at, updated_at \
             FROM ai_model_configs WHERE is_active = 1 LIMIT 1",
            &[],
            |row| {
                Ok(crate::db::models::AiModelConfig {
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
        .ok_or_else(|| "未找到激活的 AI 配置".to_string())?;

        let decrypted_key = crate::services::crypto::CryptoService::decrypt(&config.api_key_encrypted)?;
        (
            config.provider,
            config.model_id,
            decrypted_key,
            config.base_url.unwrap_or_default(),
        )
    };

    let prompt = build_periodic_ai_prompt(report_type, period_start, period_end, stats);

    let base_url = if base_url.is_empty() {
        match provider.as_str() {
            "deepseek" => "https://api.deepseek.com".to_string(),
            _ => "https://api.openai.com".to_string(),
        }
    } else {
        base_url
    };

    // 直接调用 AI API
    let url = crate::services::ai_inspection::build_chat_url(&base_url);
    let client = crate::services::ai_inspection::get_client();

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "你是一位资深运维分析师，擅长分析巡检数据并生成专业的运维报告。"},
            {"role": "user", "content": &prompt}
        ],
        "temperature": 0.3,
        "max_tokens": 4096
    });

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("AI 请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("AI API 返回错误 {}: {}", status, text));
    }

    let result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析 AI 响应失败: {}", e))?;

    let content = result
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|msg| msg.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    Ok(content)
}

/// 生成周期报告（内部实现）
pub async fn generate_periodic_report_internal(
    db: &std::sync::Arc<parking_lot::Mutex<rusqlite::Connection>>,
    report_type: String,
    period_start: String,
    period_end: String,
    device_ids: Option<Vec<i64>>,
) -> Result<i64, String> {
    let device_ids = device_ids.unwrap_or_default();
    info!("开始生成周期报告: type={}, period={}~{}, devices={:?}", report_type, period_start, period_end, device_ids);

    // 创建记录
    let report_id = {
        let conn = db.lock();
        let now = now_str();
        let device_ids_json = serde_json::to_string(&device_ids).unwrap_or_default();
        conn.execute(
            "INSERT INTO periodic_reports (report_type, period_start, period_end, status, device_ids, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'generating', ?4, ?5, ?5)",
            rusqlite::params![report_type, period_start, period_end, device_ids_json, now],
        ).map_err(|e| format!("创建周期报告记录失败: {}", e))?;
        conn.last_insert_rowid()
    };
    info!("周期报告记录已创建: report_id={}", report_id);

    // 执行生成
    let result = generate_periodic_report_content(db, report_id, &report_type, &period_start, &period_end, &device_ids).await;

    // 更新状态
    {
        let conn = db.lock();
        let now = now_str();
        match &result {
            Ok((report_path, ai_summary, stats_json)) => {
                info!("周期报告生成成功: report_id={}, path={}", report_id, report_path);
                conn.execute(
                    "UPDATE periodic_reports SET status = 'completed', report_path = ?1, \
                     ai_summary = ?2, stats_json = ?3, updated_at = ?4 WHERE id = ?5",
                    rusqlite::params![report_path, ai_summary, stats_json, now, report_id],
                ).map_err(|e| format!("更新报告状态失败: {}", e))?;
            }
            Err(e) => {
                warn!("周期报告生成失败: report_id={}, error={}", report_id, e);
                conn.execute(
                    "UPDATE periodic_reports SET status = 'failed', error_message = ?1, updated_at = ?2 WHERE id = ?3",
                    rusqlite::params![e, now, report_id],
                ).map_err(|e| format!("更新报告状态失败: {}", e))?;
            }
        }
    }

    result.map(|_| report_id)
}

/// 生成周期报告内容
async fn generate_periodic_report_content(
    db: &std::sync::Arc<parking_lot::Mutex<rusqlite::Connection>>,
    _report_id: i64,
    report_type: &str,
    period_start: &str,
    period_end: &str,
    device_ids: &[i64],
) -> Result<(String, String, String), String> {
    // 1. 聚合统计数据
    info!("聚合巡检数据: period={}~{}", period_start, period_end);
    let stats = {
        let conn = db.lock();
        aggregate_inspection_stats(&conn, period_start, period_end, device_ids)?
    };
    let total_devices = stats.get("overview").and_then(|v| v.get("total_devices")).and_then(|v| v.as_i64()).unwrap_or(0);
    let total_inspections = stats.get("overview").and_then(|v| v.get("total_inspections")).and_then(|v| v.as_i64()).unwrap_or(0);
    let health_score = stats.get("overview").and_then(|v| v.get("health_score")).and_then(|v| v.as_i64()).unwrap_or(0);
    info!("统计聚合完成: devices={}, inspections={}, health_score={}", total_devices, total_inspections, health_score);

    // 2. 调用 AI 生成总结
    // 这里需要 AppState，但内部函数没有直接访问
    // 简化处理：直接使用统计摘要作为 AI 总结
    let ai_summary = format!(
        "周期巡检统计：共 {} 台设备，{} 次巡检。正常 {} 次，警告 {} 次，严重 {} 次。健康分数 {}/100。",
        stats.get("overview").and_then(|v| v.get("total_devices")).and_then(|v| v.as_i64()).unwrap_or(0),
        stats.get("overview").and_then(|v| v.get("total_inspections")).and_then(|v| v.as_i64()).unwrap_or(0),
        stats.get("overview").and_then(|v| v.get("status_counts")).and_then(|v| v.get("ok")).and_then(|v| v.as_i64()).unwrap_or(0),
        stats.get("overview").and_then(|v| v.get("status_counts")).and_then(|v| v.get("warning")).and_then(|v| v.as_i64()).unwrap_or(0),
        stats.get("overview").and_then(|v| v.get("status_counts")).and_then(|v| v.get("critical")).and_then(|v| v.as_i64()).unwrap_or(0),
        stats.get("overview").and_then(|v| v.get("health_score")).and_then(|v| v.as_i64()).unwrap_or(0),
    );

    // 3. 生成 DOCX 报告
    let reports_dir = ensure_periodic_reports_dir()?;
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("periodic_{}_{}_{}.docx", report_type, period_start.replace('-', ""), timestamp);
    let output_path = reports_dir.join(&filename);

    let stats_json = serde_json::to_string(&stats).map_err(|e| format!("序列化统计失败: {}", e))?;

    // 生成 DOCX
    crate::services::docx_engine::generate_periodic_docx(
        report_type,
        period_start,
        period_end,
        &stats,
        &ai_summary,
        &output_path,
    )?;
    info!("周期报告 DOCX 生成完成: path={}", output_path.display());

    let report_path = output_path.to_string_lossy().to_string();
    Ok((report_path, ai_summary, stats_json))
}

// ============================================================
// Tauri Commands
// ============================================================

/// 计算建议的周期范围
#[tauri::command]
pub fn suggest_period_range(report_type: String) -> Result<(String, String), String> {
    Ok(calculate_period_range(&report_type))
}

/// 生成周期报告
#[tauri::command]
pub async fn generate_periodic_report(
    report_type: String,
    period_start: String,
    period_end: String,
    device_ids: Option<Vec<i64>>,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("前端请求生成周期报告: type={}, period={}~{}", report_type, period_start, period_end);
    let report_id = generate_periodic_report_internal(
        &state.db,
        report_type,
        period_start,
        period_end,
        device_ids,
    ).await?;

    let conn = state.db.lock();
    let sql = format!("SELECT {} FROM periodic_reports WHERE id = ?1", PERIODIC_REPORT_COLUMNS);
    let report = crate::db::query::query_one(&conn, &sql, rusqlite::params![report_id], periodic_report_from_row)?
        .ok_or_else(|| format!("周期报告 ID {} 不存在", report_id))?;

    Ok(serde_json::to_value(&report).unwrap_or_default())
}

/// 列出周期报告
#[tauri::command]
pub fn list_periodic_reports(
    report_type: Option<String>,
    limit: Option<i64>,
    state: State<AppState>,
) -> Result<Vec<PeriodicReport>, String> {
    let conn = state.db.lock();
    let limit = limit.unwrap_or(50);

    let (sql, params): (String, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(ref rt) = report_type {
        (
            format!("SELECT {} FROM periodic_reports WHERE report_type = ?1 ORDER BY created_at DESC LIMIT ?2", PERIODIC_REPORT_COLUMNS),
            vec![Box::new(rt.clone()), Box::new(limit)],
        )
    } else {
        (
            format!("SELECT {} FROM periodic_reports ORDER BY created_at DESC LIMIT ?1", PERIODIC_REPORT_COLUMNS),
            vec![Box::new(limit)],
        )
    };

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    crate::db::query::query_all(&conn, &sql, param_refs.as_slice(), periodic_report_from_row)
}

/// 获取单条周期报告详情
#[tauri::command]
pub fn get_periodic_report(report_id: i64, state: State<AppState>) -> Result<PeriodicReport, String> {
    let conn = state.db.lock();
    let sql = format!("SELECT {} FROM periodic_reports WHERE id = ?1", PERIODIC_REPORT_COLUMNS);
    crate::db::query::query_one(&conn, &sql, rusqlite::params![report_id], periodic_report_from_row)?
        .ok_or_else(|| format!("周期报告 ID {} 不存在", report_id))
}

/// 删除周期报告
#[tauri::command]
pub fn delete_periodic_report(report_id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock();

    // 获取报告路径
    let sql = format!("SELECT {} FROM periodic_reports WHERE id = ?1", PERIODIC_REPORT_COLUMNS);
    let report = crate::db::query::query_one(&conn, &sql, rusqlite::params![report_id], periodic_report_from_row)?
        .ok_or_else(|| format!("周期报告 ID {} 不存在", report_id))?;

    // 删除文件
    if let Some(ref path) = report.report_path {
        info!("周期报告已删除: id={}, path={:?}", report_id, path);
        let _ = std::fs::remove_file(path);
    }

    // 删除记录
    conn.execute("DELETE FROM periodic_reports WHERE id = ?1", rusqlite::params![report_id])
        .map_err(|e| format!("删除周期报告失败: {}", e))?;

    Ok(())
}

/// 下载周期报告
#[tauri::command]
pub async fn download_periodic_report(
    app: tauri::AppHandle,
    report_id: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt;
    info!("下载周期报告: id={}", report_id);

    let (canonical_report, report_name) = {
        let conn = state.db.lock();
        let sql = format!("SELECT {} FROM periodic_reports WHERE id = ?1", PERIODIC_REPORT_COLUMNS);
        let report = crate::db::query::query_one(&conn, &sql, rusqlite::params![report_id], periodic_report_from_row)?
            .ok_or_else(|| format!("周期报告 ID {} 不存在", report_id))?;

        let path = report.report_path.ok_or_else(|| "报告尚未生成".to_string())?;
        let canonical = std::path::PathBuf::from(&path);
        if !canonical.exists() {
            return Err("报告文件不存在".to_string());
        }

        let report_type_cn = match report.report_type.as_str() {
            "weekly" => "周报",
            "monthly" => "月报",
            "quarterly" => "季报",
            "yearly" => "年报",
            _ => "周期报告",
        };
        let name = format!("{}-{}~{}.docx", report_type_cn, report.period_start, report.period_end);
        (canonical, name)
    };

    app.dialog()
        .file()
        .add_filter("Word Document", &["docx"])
        .set_file_name(&report_name)
        .save_file(move |file_path| {
            if let Some(save_path) = file_path {
                if let Some(dest) = save_path.as_path().map(|p| p.to_path_buf()) {
                    if let Err(e) = std::fs::copy(&canonical_report, &dest) {
                        tracing::error!("复制周期报告文件失败: {}", e);
                    }
                }
            }
        });

    Ok(())
}
