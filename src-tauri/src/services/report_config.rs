use std::collections::HashMap;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// 报告模板的可视化配置（存入 report_templates.config_json）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportTemplateConfig {
    #[serde(default = "default_cover")]
    pub cover: CoverConfig,
    #[serde(default = "default_device_info")]
    pub device_info: DeviceInfoConfig,
    #[serde(default = "default_command_table")]
    pub command_table: CommandTableConfig,
    #[serde(default = "default_summary")]
    pub summary: SummaryConfig,
    #[serde(default = "default_header")]
    pub header: String,
    #[serde(default = "default_footer")]
    pub footer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverConfig {
    pub title: String,
    pub subtitle: String,
    pub logo_path: String,
    pub primary_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfoConfig {
    pub enabled: bool,
    pub fields: Vec<DeviceField>,
    /// "two_column" | "table"
    pub layout: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceField {
    /// name | ip | vendor | model | sn | mfg_date | inspect_time
    pub key: String,
    pub label: String,
    pub visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTableConfig {
    pub columns: Vec<TableColumn>,
    /// 每条命令输出截断行数；0 = 不截断
    pub output_max_lines: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableColumn {
    /// seq | item | output | ai_judgment
    pub key: String,
    pub label: String,
    /// 百分比，所有可见列加起来应等于 100；不严格校验
    pub width: i32,
    pub visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryConfig {
    pub enabled: bool,
    pub title: String,
    pub show_problem_table: bool,
}

// --- 默认值 ---

fn default_cover() -> CoverConfig {
    CoverConfig {
        title: "{{vendor}} 设备巡检报告".into(),
        subtitle: "运维巡检中心".into(),
        logo_path: String::new(),
        primary_color: "#1F4E79".into(),
    }
}

fn default_device_info() -> DeviceInfoConfig {
    DeviceInfoConfig {
        enabled: true,
        layout: "two_column".into(),
        fields: vec![
            DeviceField { key: "name".into(),         label: "设备名称".into(), visible: true },
            DeviceField { key: "ip".into(),           label: "IP 地址".into(),  visible: true },
            DeviceField { key: "vendor".into(),       label: "厂商".into(),     visible: true },
            DeviceField { key: "model".into(),        label: "型号".into(),     visible: true },
            DeviceField { key: "sysname".into(),      label: "主机名".into(),   visible: false },
            DeviceField { key: "os_release".into(),   label: "发行版".into(),   visible: false },
            DeviceField { key: "cpu_cores".into(),    label: "CPU 核心数".into(), visible: false },
            DeviceField { key: "memory_gb".into(),    label: "内存容量".into(), visible: false },
            DeviceField { key: "sn".into(),           label: "序列号".into(),   visible: false },
            DeviceField { key: "mfg_date".into(),     label: "出厂日期".into(), visible: false },
            DeviceField { key: "inspect_time".into(), label: "巡检时间".into(), visible: true },
        ],
    }
}

fn default_command_table() -> CommandTableConfig {
    CommandTableConfig {
        output_max_lines: 15,
        columns: vec![
            TableColumn { key: "seq".into(),         label: "序号".into(),     width: 6,  visible: true },
            TableColumn { key: "item".into(),        label: "项目".into(),     width: 16, visible: true },
            TableColumn { key: "output".into(),      label: "巡检内容".into(), width: 58, visible: true },
            TableColumn { key: "ai_judgment".into(), label: "评判结论".into(), width: 20, visible: true },
        ],
    }
}

fn default_summary() -> SummaryConfig {
    SummaryConfig {
        enabled: true,
        title: "巡检总结".into(),
        show_problem_table: true,
    }
}

fn default_header() -> String { "{{vendor}} 巡检报告".into() }
fn default_footer() -> String { "第 {{page}} 页 / 共 {{total}} 页".into() }

impl Default for ReportTemplateConfig {
    fn default() -> Self {
        Self {
            cover: default_cover(),
            device_info: default_device_info(),
            command_table: default_command_table(),
            summary: default_summary(),
            header: default_header(),
            footer: default_footer(),
        }
    }
}

/// 解析 report_templates.config_json，缺失/损坏时回退默认。
pub fn parse_config_json(s: &str) -> ReportTemplateConfig {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return ReportTemplateConfig::default();
    }
    serde_json::from_str(trimmed).unwrap_or_else(|e| {
        tracing::warn!("报告模板配置 JSON 解析失败: {}，使用默认配置。原始: {}",
            e, trimmed.chars().take(200).collect::<String>());
        ReportTemplateConfig::default()
    })
}

/// 序列化默认配置（用于 DB seed/migration）。
pub fn default_config_json() -> String {
    serde_json::to_string(&ReportTemplateConfig::default()).unwrap_or_default()
}

// ----------------------------------------------------------------
// 命令池描述映射加载（由 docx_engine 与命令层共用）
// ----------------------------------------------------------------

pub fn load_command_descriptions(conn: &Connection) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(mut stmt) = conn.prepare("SELECT command, description FROM command_pool") {
        let rows = stmt.query_map([], |row| {
            let cmd: String = row.get(0)?;
            let desc: Option<String> = row.get(1)?;
            Ok((cmd, desc.unwrap_or_default()))
        });
        if let Ok(rows) = rows {
            for r in rows.flatten() {
                if !r.1.is_empty() {
                    map.insert(r.0, r.1);
                }
            }
        }
    }
    map
}
