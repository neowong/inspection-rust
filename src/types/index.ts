export interface Device {
  id: number;
  name: string;
  ip: string;
  device_type: string;
  vendor: string;
  model: string | null;
  ssh_username: string | null;
  ssh_port: number;
  template_id: number | null;
  status: "online" | "offline" | "unknown";
  last_checked_at: string | null;
  serial_number: string | null;
  manufacturing_date: string | null;
  sysname: string | null;
  cpu_cores: number | null;
  memory_gb: number | null;
  /** SSH 账号验证状态：unknown/ok/auth_failed/unreachable/timeout/dns_fail/no_credential/error */
  auth_status: string | null;
  /** 账号验证错误的简短中文消息 */
  auth_message: string | null;
  /** 部署方式：direct/docker/podman */
  deployment: string | null;
  /** 数据库版本（数据库设备专用） */
  db_version: string | null;
  /** 实例名/容器名（容器部署时为容器名） */
  instance_name: string | null;
  /** 数据库用户名 */
  db_username: string | null;
  /** 数据库端口 */
  db_port: number | null;
  /** 内核版本（Linux/数据库设备） */
  kernel_version: string | null;
  created_at: string;
  updated_at: string;
}

export interface TemplateCommandConfig {
  command_id: number;
}

export interface InspectionTemplate {
  id: number;
  name: string;
  vendor: string;
  model: string | null;
  device_type: string | null;
  config: { commands?: TemplateCommandConfig[] };
  description: string | null;
  report_template_id: number | null;
  template_type: string | null;
  device_count: number;
  created_at: string;
  updated_at: string;
}

export interface CommandPool {
  id: number;
  vendor: string;
  command: string;
  description: string | null;
  category: string | null;
  model: string | null;
  needs_root: boolean;
  expectation: string | null;
  created_at: string;
  updated_at: string;
}

type BatchStatusType =
  | "pending" | "running" | "completed" | "failed"
  | "stopped" | "paused" | "waiting" | "in_progress" | "partially_completed";

type RecordStatusType =
  | "pending" | "running" | "completed" | "failed"
  | "stopped" | "skipped";

type AiStatusType =
  | "none" | "pending" | "processing" | "completed" | "failed";

export interface InspectionBatch {
  id: number;
  name: string | null;
  status: BatchStatusType;
  triggered_by: string;
  device_ids: number[];
  started_at: string | null;
  completed_at: string | null;
  combined_report_path: string | null;
  created_at: string;
  records: InspectionRecordSummary[];
}

interface InspectionRecordSummary {
  id: number;
  batch_id: number;
  device_id: number;
  status: RecordStatusType;
  ai_status: AiStatusType;
  report_path: string | null;
  error_message: string | null;
}

export interface InspectionRecord {
  id: number;
  batch_id: number;
  device_id: number;
  status: RecordStatusType;
  command_outputs: string | null;
  ai_status: AiStatusType;
  ai_result: string | null;
  ai_analysis: string | null;
  ai_suggestions: string | null;
  command_judgments: string | null;
  summary_judgment: string | null;
  report_path: string | null;
  error_message: string | null;
  static_info: string | null;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
}

export interface AiModelConfig {
  id: number;
  name: string;
  provider: string;
  model_id: string;
  base_url: string | null;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface ReportTemplate {
  id: number;
  name: string;
  vendor: string | null;
  // 后端 Rust 字段为 i64，序列化为 0/1；前端用 === 1 判断（0 falsy / 1 truthy 碰巧可用但类型应一致）
  is_default: number;
  description: string;
  config_json: string;
  created_at: string;
  updated_at: string;
}

// ----- 报告模板配置（与后端 ReportTemplateConfig 同形）-----

export interface ReportTemplateConfig {
  cover: CoverConfig;
  device_info: DeviceInfoConfig;
  command_table: CommandTableConfig;
  summary: SummaryConfig;
  header: string;
  footer: string;
}

interface CoverConfig {
  title: string;
  subtitle: string;
  logo_path: string;
  primary_color: string;
  include_toc?: boolean;
}

interface DeviceInfoConfig {
  enabled: boolean;
  fields: DeviceField[];
  layout: "two_column" | "table";
}

export interface DeviceField {
  key:
    | "name"
    | "ip"
    | "vendor"
    | "model"
    | "sn"
    | "mfg_date"
    | "inspect_time"
    | "sysname"
    | "hostname"
    | "os_release"
    | "cpu_cores"
    | "memory_gb"
    | "db_version"
    | "instance_name"
    | "kernel_version";
  label: string;
  visible: boolean;
}

interface CommandTableConfig {
  columns: TableColumn[];
  output_max_lines: number;
}

export interface TableColumn {
  key: "seq" | "item" | "output" | "ai_judgment";
  label: string;
  width: number;
  visible: boolean;
}

interface SummaryConfig {
  enabled: boolean;
  title: string;
  show_problem_table: boolean;
}

export const DEFAULT_REPORT_CONFIG: ReportTemplateConfig = {
  cover: {
    title: "{{vendor}} 设备巡检报告",
    subtitle: "运维巡检中心",
    logo_path: "",
    primary_color: "#1F4E79",
  },
  device_info: {
    enabled: true,
    layout: "two_column",
    fields: [
      { key: "name",         label: "设备名称", visible: true },
      { key: "ip",           label: "IP 地址",  visible: true },
      { key: "vendor",       label: "厂商",     visible: true },
      { key: "model",        label: "型号",     visible: true },
      { key: "sysname",      label: "主机名",   visible: false },
      { key: "os_release",   label: "发行版",   visible: false },
      { key: "cpu_cores",    label: "CPU 核心数", visible: false },
      { key: "memory_gb",    label: "内存容量", visible: false },
      { key: "sn",           label: "序列号",   visible: false },
      { key: "mfg_date",     label: "出厂日期",  visible: false },
      { key: "db_version",   label: "数据库版本", visible: false },
      { key: "instance_name",label: "实例名",     visible: false },
      { key: "kernel_version", label: "内核版本", visible: false },
      { key: "inspect_time", label: "巡检时间",   visible: true },
    ],
  },
  command_table: {
    output_max_lines: 15,
    columns: [
      { key: "seq",         label: "序号",     width: 6,  visible: true },
      { key: "item",        label: "项目",     width: 16, visible: true },
      { key: "output",      label: "巡检内容", width: 58, visible: true },
      { key: "ai_judgment", label: "评判结论", width: 20, visible: true },
    ],
  },
  summary: {
    enabled: true,
    title: "巡检总结",
    show_problem_table: true,
  },
  header: "{{vendor}} 巡检报告",
  footer: "第 {{page}} 页 / 共 {{total}} 页",
};

export interface Stats {
  device_count: number;
  online_device_count: number;
  offline_device_count: number;
  template_count: number;
  command_count: number;
  batch_count: number;
  pending_batch_count: number;
  completed_batch_count: number;
  network_device_count: number;
  security_device_count: number;
  server_count: number;
  database_count: number;
  other_device_count: number;
  report_count: number;
}

