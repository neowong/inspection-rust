export interface Device {
  id: number;
  group: string;
  name: string;
  ip: string;
  device_type: string;
  vendor: string;
  model: string | null;
  inspection_mode: "ssh" | "offline" | "web";
  ssh_username: string | null;
  ssh_port: number;
  template_id: number | null;
  db_type: string | null;
  db_port: number | null;
  db_username: string | null;
  db_os_user: string | null;
  status: "online" | "offline" | "unknown";
  last_checked_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface InspectionTemplate {
  id: number;
  name: string;
  vendor: string;
  model: string | null;
  device_type: string | null;
  type: string;
  config: { command_ids?: number[] };
  description: string | null;
  report_template_id: number | null;
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
  command_type: string;
  model: string | null;
  created_at: string;
  updated_at: string;
}

export interface InspectionBatch {
  id: number;
  name: string | null;
  mode: string;
  status: string;
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
  status: string;
  ai_status: string;
  report_path: string | null;
  error_message: string | null;
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
  created_at: string;
  updated_at: string;
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

export interface InspectionRecord {
  id: number;
  batch_id: number;
  device_id: number;
  status: string;
  command_outputs: string;
  ai_status: string;
  ai_result?: string | null;
  ai_analysis?: string | null;
  ai_suggestions?: string | null;
  command_judgments?: string | null;
  summary_judgment?: string | null;
  report_path?: string | null;
  error_message?: string | null;
  upload_source?: string;
  started_at?: string | null;
  completed_at?: string | null;
  created_at: string;
}

export interface Settings {
  report_max_output_lines: number;
}
