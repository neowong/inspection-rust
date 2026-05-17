use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Device {
    pub id: i64,
    pub group_name: String,
    pub name: String,
    pub ip: String,
    pub device_type: String,
    pub vendor: String,
    pub model: Option<String>,
    pub inspection_mode: String,
    pub ssh_username: Option<String>,
    #[serde(skip_serializing)]
    pub ssh_password_encrypted: Option<String>,
    pub ssh_port: i64,
    pub web_url: Option<String>,
    pub web_port: Option<i64>,
    pub template_id: Option<i64>,
    pub db_type: Option<String>,
    pub db_port: Option<i64>,
    pub db_username: Option<String>,
    #[serde(skip_serializing)]
    pub db_password_encrypted: Option<String>,
    pub db_os_user: Option<String>,
    pub status: String,
    pub last_checked_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct DeviceCreate {
    pub group: Option<String>,
    pub name: String,
    pub ip: String,
    pub device_type: String,
    pub vendor: String,
    pub model: Option<String>,
    pub inspection_mode: Option<String>,
    pub ssh_username: Option<String>,
    pub ssh_password: Option<String>,
    pub ssh_port: Option<i64>,
    pub template_id: Option<i64>,
    pub db_type: Option<String>,
    pub db_port: Option<i64>,
    pub db_username: Option<String>,
    pub db_password: Option<String>,
    pub db_os_user: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeviceUpdate {
    pub group: Option<String>,
    pub name: Option<String>,
    pub ip: Option<String>,
    pub device_type: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub inspection_mode: Option<String>,
    pub ssh_username: Option<String>,
    pub ssh_password: Option<String>,
    pub ssh_port: Option<i64>,
    pub template_id: Option<i64>,
    pub db_type: Option<String>,
    pub db_port: Option<i64>,
    pub db_username: Option<String>,
    pub db_password: Option<String>,
    pub db_os_user: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InspectionTemplate {
    pub id: i64,
    pub name: String,
    pub vendor: String,
    pub model: Option<String>,
    pub device_type: Option<String>,
    pub template_type: String,
    pub config: Option<String>,
    pub description: Option<String>,
    pub report_template_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct TemplateCreate {
    pub name: String,
    pub vendor: String,
    pub model: Option<String>,
    pub device_type: Option<String>,
    pub template_type: Option<String>,
    pub config: Option<serde_json::Value>,
    pub description: Option<String>,
    pub report_template_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TemplateUpdate {
    pub name: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub device_type: Option<String>,
    pub template_type: Option<String>,
    pub config: Option<serde_json::Value>,
    pub description: Option<String>,
    pub report_template_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommandPool {
    pub id: i64,
    pub vendor: String,
    pub command: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub command_type: String,
    pub model: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CommandCreate {
    pub vendor: String,
    pub command: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub command_type: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CommandUpdate {
    pub vendor: Option<String>,
    pub command: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub command_type: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InspectionBatch {
    pub id: i64,
    pub name: Option<String>,
    pub mode: String,
    pub status: String,
    pub triggered_by: String,
    pub scheduled_task_id: Option<i64>,
    pub device_ids: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct BatchCreate {
    pub name: Option<String>,
    pub mode: Option<String>,
    pub device_ids: Vec<i64>,
    pub auto_start: Option<bool>,
    pub scheduled_task_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InspectionRecord {
    pub id: i64,
    pub batch_id: i64,
    pub device_id: i64,
    pub status: String,
    pub upload_source: String,
    pub error_message: Option<String>,
    pub command_outputs: String,
    pub ai_status: String,
    pub ai_result: Option<String>,
    pub ai_analysis: Option<String>,
    pub ai_suggestions: Option<String>,
    pub command_judgments: Option<String>,
    pub summary_judgment: Option<String>,
    pub report_path: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub timestamp: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScheduledTask {
    pub id: i64,
    pub name: String,
    pub cron_expression: String,
    pub enabled: bool,
    pub device_ids: String,
    pub next_run_at: Option<String>,
    pub last_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct TaskCreate {
    pub name: String,
    pub device_ids: Vec<i64>,
    pub cron_expression: String,
}

#[derive(Debug, Deserialize)]
pub struct TaskUpdate {
    pub name: Option<String>,
    pub device_ids: Option<Vec<i64>>,
    pub cron_expression: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AiModelConfig {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub model_id: String,
    #[serde(skip_serializing)]
    pub api_key_encrypted: String,
    pub base_url: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct AiConfigCreate {
    pub name: String,
    pub provider: String,
    pub model_id: String,
    pub api_key: String,
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AiConfigUpdate {
    pub name: Option<String>,
    pub provider: Option<String>,
    pub model_id: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReportTemplate {
    pub id: i64,
    pub name: String,
    pub vendor: Option<String>,
    pub file_path: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OfflineLogImport {
    pub id: i64,
    pub filename: String,
    pub file_path: String,
    pub mode: String,
    pub parsed_devices: String,
    pub batch_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeviceStatusLog {
    pub id: i64,
    pub device_id: i64,
    pub old_status: Option<String>,
    pub new_status: String,
    pub checked_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemSettings {
    pub id: i64,
    pub report_max_output_lines: i64,
}
