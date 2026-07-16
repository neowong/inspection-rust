use serde::{Deserialize, Serialize};

// ============================
// 设备 (Devices)
// ============================

/// 设备 - 数据库读取模型
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Device {
    pub id: i64,
    pub name: String,
    pub ip: String,
    pub device_type: String,
    pub vendor: String,
    pub model: Option<String>,
    pub ssh_username: Option<String>,
    #[serde(skip_serializing)]
    pub ssh_password_encrypted: Option<String>,
    pub ssh_port: i64,
    pub template_id: Option<i64>,
    pub status: String,
    pub last_checked_at: Option<String>,
    pub serial_number: Option<String>,
    pub manufacturing_date: Option<String>,
    pub sysname: Option<String>,
    pub cpu_cores: Option<i64>,
    pub memory_gb: Option<f64>,
    pub auth_status: Option<String>,
    pub auth_message: Option<String>,
    pub deployment: Option<String>,
    pub db_version: Option<String>,
    pub instance_name: Option<String>,
    pub db_username: Option<String>,
    #[serde(skip_serializing)]
    pub db_password_encrypted: Option<String>,
    pub db_port: Option<i64>,
    pub kernel_version: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 创建设备 - 前端请求 DTO
#[derive(Debug, Deserialize)]
pub struct DeviceCreate {
    pub name: String,
    pub ip: String,
    pub device_type: String,
    pub vendor: String,
    pub model: Option<String>,
    pub ssh_username: Option<String>,
    pub ssh_password_encrypted: Option<String>,
    pub ssh_port: Option<i64>,
    pub template_id: Option<i64>,
    pub status: Option<String>,
    pub last_checked_at: Option<String>,
    pub serial_number: Option<String>,
    pub manufacturing_date: Option<String>,
    pub sysname: Option<String>,
    pub cpu_cores: Option<i64>,
    pub memory_gb: Option<f64>,
    pub deployment: Option<String>,
    pub db_version: Option<String>,
    pub instance_name: Option<String>,
    pub db_username: Option<String>,
    pub db_password_encrypted: Option<String>,
    pub db_port: Option<i64>,
    pub kernel_version: Option<String>,
}

/// 更新设备 - 前端请求 DTO（全部可选）
#[derive(Debug, Deserialize)]
pub struct DeviceUpdate {
    pub name: Option<String>,
    pub ip: Option<String>,
    pub device_type: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub ssh_username: Option<String>,
    pub ssh_password_encrypted: Option<String>,
    pub ssh_port: Option<i64>,
    pub template_id: Option<i64>,
    pub status: Option<String>,
    pub last_checked_at: Option<String>,
    pub serial_number: Option<String>,
    pub manufacturing_date: Option<String>,
    pub sysname: Option<String>,
    pub cpu_cores: Option<i64>,
    pub memory_gb: Option<f64>,
    pub deployment: Option<String>,
    pub db_version: Option<String>,
    pub instance_name: Option<String>,
    pub db_username: Option<String>,
    pub db_password_encrypted: Option<String>,
    pub db_port: Option<i64>,
    pub kernel_version: Option<String>,
}

// ============================
// 设备状态日志 (Device Status Logs)
// ============================
// 注：device_status_logs 表由 lib.rs 后台轮询写入，从不读出为 struct，故无对应模型。

// ============================
// 巡检模板 (Inspection Templates)
// ============================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InspectionTemplate {
    pub id: i64,
    pub name: String,
    pub vendor: String,
    pub model: Option<String>,
    pub device_type: Option<String>,
    pub config: Option<String>,
    pub description: Option<String>,
    pub report_template_id: Option<i64>,
    pub template_type: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct TemplateCreate {
    pub name: String,
    pub vendor: String,
    pub model: Option<String>,
    pub device_type: Option<String>,
    pub config: Option<String>,
    pub description: Option<String>,
    pub report_template_id: Option<i64>,
    pub template_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TemplateUpdate {
    pub name: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub device_type: Option<String>,
    pub config: Option<String>,
    pub description: Option<String>,
    pub report_template_id: Option<i64>,
    pub template_type: Option<String>,
}

// ============================
// 命令库 (Command Pool)
// ============================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommandPool {
    pub id: i64,
    pub vendor: String,
    pub command: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub model: Option<String>,
    pub needs_root: bool,
    pub expectation: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CommandCreate {
    pub vendor: String,
    pub command: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub model: Option<String>,
    pub needs_root: Option<bool>,
    pub expectation: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CommandUpdate {
    pub vendor: Option<String>,
    pub command: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub model: Option<String>,
    pub needs_root: Option<bool>,
    pub expectation: Option<String>,
}

// ============================
// 巡检批次 (Inspection Batches)
// ============================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InspectionBatch {
    pub id: i64,
    pub name: Option<String>,
    pub status: String,
    pub triggered_by: Option<String>,
    pub device_ids: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub combined_report_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct BatchCreate {
    pub name: Option<String>,
    pub status: Option<String>,
    pub triggered_by: Option<String>,
    pub device_ids: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

// ============================
// 巡检记录 (Inspection Records)
// ============================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InspectionRecord {
    pub id: i64,
    pub batch_id: i64,
    pub device_id: i64,
    pub status: String,
    pub error_message: Option<String>,
    pub command_outputs: Option<String>,
    pub static_info: Option<String>,
    pub ai_status: String,
    pub ai_result: Option<String>,
    pub ai_analysis: Option<String>,
    pub ai_suggestions: Option<String>,
    pub command_judgments: Option<String>,
    pub summary_judgment: Option<String>,
    pub report_path: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ============================
// AI 模型配置 (AI Model Configs)
// ============================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AiModelConfig {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub model_id: String,
    #[serde(skip_serializing)]
    pub api_key_encrypted: String,
    pub base_url: Option<String>,
    pub is_active: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct AiConfigCreate {
    pub name: String,
    pub provider: String,
    pub model_id: String,
    pub api_key_encrypted: String,
    pub base_url: Option<String>,
    pub is_active: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AiConfigUpdate {
    pub name: Option<String>,
    pub provider: Option<String>,
    pub model_id: Option<String>,
    pub api_key_encrypted: Option<String>,
    pub base_url: Option<String>,
    pub is_active: Option<i64>,
}

// ============================
// 报告模板 (Report Templates)
// ============================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReportTemplate {
    pub id: i64,
    pub name: String,
    pub vendor: Option<String>,
    pub is_default: i64,
    pub description: String,
    pub config_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ReportTemplateCreate {
    pub name: String,
    pub vendor: Option<String>,
    pub description: Option<String>,
    pub config_json: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReportTemplateUpdate {
    pub name: Option<String>,
    pub vendor: Option<String>,
    pub is_default: Option<i64>,
    pub description: Option<String>,
    pub config_json: Option<String>,
}

// ============================
// 周期报告 (Periodic Reports)
// ============================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PeriodicReport {
    pub id: i64,
    pub report_type: String,  // weekly|monthly|quarterly|yearly
    pub period_start: String,
    pub period_end: String,
    pub status: String,       // pending|generating|completed|failed
    pub device_ids: String,   // JSON array
    pub report_path: Option<String>,
    pub ai_summary: Option<String>,
    pub stats_json: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct PeriodicReportCreate {
    pub report_type: String,
    pub period_start: String,
    pub period_end: String,
    pub device_ids: Option<Vec<i64>>,
}

// ============================
// 定时任务 (Scheduled Tasks)
// ============================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScheduledTask {
    pub id: i64,
    pub name: String,
    pub task_type: String,    // inspection|periodic_report
    pub cron_expr: String,
    pub enabled: i64,         // 0|1
    pub config_json: String,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub run_count: i64,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ScheduledTaskCreate {
    pub name: String,
    pub task_type: String,
    pub cron_expr: String,
    pub enabled: Option<bool>,
    pub config_json: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ScheduledTaskUpdate {
    pub name: Option<String>,
    pub cron_expr: Option<String>,
    pub enabled: Option<bool>,
    pub config_json: Option<String>,
}

// ============================
// 巡检指标快照 (Inspection Metrics)
// ============================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InspectionMetric {
    pub id: i64,
    pub record_id: i64,
    pub device_id: i64,
    pub batch_id: i64,
    pub inspected_at: String,
    pub overall_status: String,
    pub ai_summary: Option<String>,
    pub metrics_json: String,   // JSON: {"cpu_usage": 45.0, "memory_usage": 78.0, ...}
    pub alerts_json: String,    // JSON: [{command, status, finding}]
    pub created_at: String,
}

// ============================
// 公共工具函数
// ============================

/// 返回当前时间戳字符串
pub fn now_str() -> String {
    chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

// ============================
// SQL 列定义常量
// ============================

pub const DEVICE_COLUMNS: &str =
    "id, name, ip, device_type, vendor, model, ssh_username, ssh_password_encrypted, \
     ssh_port, template_id, status, last_checked_at, serial_number, manufacturing_date, sysname, \
     cpu_cores, memory_gb, auth_status, auth_message, deployment, db_version, instance_name, db_username, db_password_encrypted, db_port, kernel_version, created_at, updated_at";

pub const TEMPLATE_COLUMNS: &str =
    "id, name, vendor, model, device_type, config, description, report_template_id, template_type, \
     created_at, updated_at";

pub const COMMAND_COLUMNS: &str =
    "id, vendor, command, description, category, model, needs_root, expectation, created_at, updated_at";

pub const BATCH_COLUMNS: &str =
    "id, name, status, triggered_by, device_ids, started_at, completed_at, combined_report_path, created_at, updated_at";

pub const RECORD_COLUMNS: &str =
    "id, batch_id, device_id, status, error_message, command_outputs, static_info, ai_status, ai_result, \
     ai_analysis, ai_suggestions, command_judgments, summary_judgment, report_path, \
     started_at, completed_at, created_at, updated_at";

// Lightweight columns for batch listing — excludes heavy fields (command_outputs, ai_result, etc.)
pub const RECORD_SUMMARY_COLUMNS: &str =
    "id, batch_id, device_id, status, error_message, ai_status, report_path, \
     started_at, completed_at, created_at, updated_at";

pub const REPORT_TEMPLATE_COLUMNS: &str =
    "id, name, vendor, is_default, description, config_json, created_at, updated_at";

pub const PERIODIC_REPORT_COLUMNS: &str =
    "id, report_type, period_start, period_end, status, device_ids, report_path, \
     ai_summary, stats_json, error_message, created_at, updated_at";

pub const SCHEDULED_TASK_COLUMNS: &str =
    "id, name, task_type, cron_expr, enabled, config_json, last_run_at, next_run_at, \
     run_count, last_error, created_at, updated_at";

pub const INSPECTION_METRIC_COLUMNS: &str =
    "id, record_id, device_id, batch_id, inspected_at, overall_status, ai_summary, \
     metrics_json, alerts_json, created_at";

// ============================
// 行映射函数（统一去重）
// ============================

pub fn device_from_row(row: &rusqlite::Row) -> rusqlite::Result<Device> {
    Ok(Device {
        id: row.get(0)?,
        name: row.get(1)?,
        ip: row.get(2)?,
        device_type: row.get(3)?,
        vendor: row.get(4)?,
        model: row.get(5)?,
        ssh_username: row.get(6)?,
        ssh_password_encrypted: row.get(7)?,
        ssh_port: row.get(8)?,
        template_id: row.get(9)?,
        status: row.get(10)?,
        last_checked_at: row.get(11)?,
        serial_number: row.get(12)?,
        manufacturing_date: row.get(13)?,
        sysname: row.get(14)?,
        cpu_cores: row.get(15)?,
        memory_gb: row.get(16)?,
        auth_status: row.get(17)?,
        auth_message: row.get(18)?,
        deployment: row.get(19)?,
        db_version: row.get(20)?,
        instance_name: row.get(21)?,
        db_username: row.get(22)?,
        db_password_encrypted: row.get(23)?,
        db_port: row.get(24)?,
        kernel_version: row.get(25)?,
        created_at: row.get(26)?,
        updated_at: row.get(27)?,
    })
}

pub fn template_from_row(row: &rusqlite::Row) -> rusqlite::Result<InspectionTemplate> {
    Ok(InspectionTemplate {
        id: row.get(0)?,
        name: row.get(1)?,
        vendor: row.get(2)?,
        model: row.get(3)?,
        device_type: row.get(4)?,
        config: row.get(5)?,
        description: row.get(6)?,
        report_template_id: row.get(7)?,
        template_type: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

pub fn command_from_row(row: &rusqlite::Row) -> rusqlite::Result<CommandPool> {
    Ok(CommandPool {
        id: row.get(0)?,
        vendor: row.get(1)?,
        command: row.get(2)?,
        description: row.get(3)?,
        category: row.get(4)?,
        model: row.get(5)?,
        needs_root: row.get::<_, i64>(6).unwrap_or(0) != 0,
        expectation: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

pub fn batch_from_row(row: &rusqlite::Row) -> rusqlite::Result<InspectionBatch> {
    Ok(InspectionBatch {
        id: row.get(0)?,
        name: row.get(1)?,
        status: row.get(2)?,
        triggered_by: row.get(3)?,
        device_ids: row.get(4)?,
        started_at: row.get(5)?,
        completed_at: row.get(6)?,
        combined_report_path: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

pub fn record_from_row(row: &rusqlite::Row) -> rusqlite::Result<InspectionRecord> {
    Ok(InspectionRecord {
        id: row.get(0)?,
        batch_id: row.get(1)?,
        device_id: row.get(2)?,
        status: row.get(3)?,
        error_message: row.get(4)?,
        command_outputs: row.get(5)?,
        static_info: row.get(6)?,
        ai_status: row.get(7)?,
        ai_result: row.get(8)?,
        ai_analysis: row.get(9)?,
        ai_suggestions: row.get(10)?,
        command_judgments: row.get(11)?,
        summary_judgment: row.get(12)?,
        report_path: row.get(13)?,
        started_at: row.get(14)?,
        completed_at: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

/// Lightweight row mapper for summary columns — excludes heavy JSON fields
pub fn record_summary_from_row(row: &rusqlite::Row) -> rusqlite::Result<InspectionRecord> {
    Ok(InspectionRecord {
        id: row.get(0)?,
        batch_id: row.get(1)?,
        device_id: row.get(2)?,
        status: row.get(3)?,
        error_message: row.get(4)?,
        command_outputs: None,
        static_info: None,
        ai_status: row.get(5)?,
        ai_result: None,
        ai_analysis: None,
        ai_suggestions: None,
        command_judgments: None,
        summary_judgment: None,
        report_path: row.get(6)?,
        started_at: row.get(7)?,
        completed_at: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

pub fn report_template_from_row(row: &rusqlite::Row) -> rusqlite::Result<ReportTemplate> {
    Ok(ReportTemplate {
        id: row.get(0)?,
        name: row.get(1)?,
        vendor: row.get(2)?,
        is_default: row.get(3)?,
        description: row.get(4)?,
        config_json: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

pub fn periodic_report_from_row(row: &rusqlite::Row) -> rusqlite::Result<PeriodicReport> {
    Ok(PeriodicReport {
        id: row.get(0)?,
        report_type: row.get(1)?,
        period_start: row.get(2)?,
        period_end: row.get(3)?,
        status: row.get(4)?,
        device_ids: row.get(5)?,
        report_path: row.get(6)?,
        ai_summary: row.get(7)?,
        stats_json: row.get(8)?,
        error_message: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

pub fn scheduled_task_from_row(row: &rusqlite::Row) -> rusqlite::Result<ScheduledTask> {
    Ok(ScheduledTask {
        id: row.get(0)?,
        name: row.get(1)?,
        task_type: row.get(2)?,
        cron_expr: row.get(3)?,
        enabled: row.get(4)?,
        config_json: row.get(5)?,
        last_run_at: row.get(6)?,
        next_run_at: row.get(7)?,
        run_count: row.get(8)?,
        last_error: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

pub fn inspection_metric_from_row(row: &rusqlite::Row) -> rusqlite::Result<InspectionMetric> {
    Ok(InspectionMetric {
        id: row.get(0)?,
        record_id: row.get(1)?,
        device_id: row.get(2)?,
        batch_id: row.get(3)?,
        inspected_at: row.get(4)?,
        overall_status: row.get(5)?,
        ai_summary: row.get(6)?,
        metrics_json: row.get(7)?,
        alerts_json: row.get(8)?,
        created_at: row.get(9)?,
    })
}
