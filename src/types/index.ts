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
  created_at: string;
  updated_at: string;
}

export interface InspectionTemplate {
  id: number;
  name: string;
  vendor: string;
  model: string | null;
  device_type: string | null;
  config: { command_ids?: number[] };
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
  created_at: string;
  updated_at: string;
}

export type BatchStatusType =
  | "pending" | "running" | "completed" | "failed"
  | "stopped" | "paused" | "waiting" | "in_progress" | "partially_completed";

export type RecordStatusType =
  | "pending" | "running" | "completed" | "failed"
  | "stopped" | "skipped";

export type AiStatusType =
  | "none" | "pending" | "processing" | "completed" | "failed";

export interface InspectionBatch {
  id: number;
  name: string | null;
  status: BatchStatusType;
  triggered_by: string;
  device_ids: number[];
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
  records: InspectionRecordSummary[];
}

export interface InspectionRecordSummary {
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
  file_path: string;
  content: string;
  format: "markdown" | "html" | "docx";
  is_default: boolean;
  description: string;
  sample_data: string;
  config_json: string;
  mode: "visual" | "advanced";
  custom_css: string;
  page_header: string;
  page_footer: string;
  created_at: string;
  updated_at: string;
}

export interface TemplateSection {
  type: "title" | "basic_info" | "inspection_results" | "ai_analysis" | "overall_assessment" | "custom_text" | "header_footer" | "device_summary_table";
  enabled: boolean;
  label: string;
  config: Record<string, unknown>;
}

export interface TemplateConfig {
  sections: TemplateSection[];
}

export interface Stats {
  device_count: number;
  online_device_count: number;
  offline_device_count: number;
  template_count: number;
  command_count: number;
  batch_count: number;
  pending_batch_count: number;
  completed_batch_count: number;
}

