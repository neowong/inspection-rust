-- Devices
CREATE TABLE IF NOT EXISTS devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    group_name TEXT NOT NULL DEFAULT 'network' CHECK(group_name IN ('network', 'system')),
    name TEXT NOT NULL UNIQUE,
    ip TEXT NOT NULL UNIQUE,
    device_type TEXT NOT NULL,
    vendor TEXT NOT NULL,
    model TEXT,
    inspection_mode TEXT NOT NULL DEFAULT 'ssh' CHECK(inspection_mode IN ('ssh', 'offline', 'web')),
    ssh_username TEXT,
    ssh_password_encrypted TEXT,
    ssh_port INTEGER NOT NULL DEFAULT 22,
    web_url TEXT,
    web_port INTEGER,
    template_id INTEGER,
    db_type TEXT CHECK(db_type IS NULL OR db_type IN ('mysql', 'postgresql', 'oracle')),
    db_port INTEGER,
    db_username TEXT,
    db_password_encrypted TEXT,
    db_os_user TEXT,
    status TEXT NOT NULL DEFAULT 'unknown' CHECK(status IN ('online', 'offline', 'unknown')),
    last_checked_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_devices_group ON devices(group_name);
CREATE INDEX IF NOT EXISTS idx_devices_vendor ON devices(vendor);
CREATE INDEX IF NOT EXISTS idx_devices_status ON devices(status);
CREATE INDEX IF NOT EXISTS idx_devices_template_id ON devices(template_id);

-- Device status change logs
CREATE TABLE IF NOT EXISTS device_status_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id INTEGER NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    old_status TEXT,
    new_status TEXT NOT NULL,
    checked_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_status_logs_device ON device_status_logs(device_id);

-- Inspection templates
CREATE TABLE IF NOT EXISTS inspection_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    vendor TEXT NOT NULL,
    model TEXT,
    device_type TEXT,
    template_type TEXT NOT NULL CHECK(template_type IN ('ssh', 'web')),
    config TEXT,
    description TEXT,
    report_template_id INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Command pool
CREATE TABLE IF NOT EXISTS command_pool (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    vendor TEXT NOT NULL,
    command TEXT NOT NULL,
    description TEXT,
    category TEXT DEFAULT 'general',
    command_type TEXT DEFAULT 'ssh' CHECK(command_type IN ('ssh', 'db')),
    model TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(vendor, command)
);

CREATE INDEX IF NOT EXISTS idx_command_pool_vendor ON command_pool(vendor);

-- Inspection batches
CREATE TABLE IF NOT EXISTS inspection_batches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT,
    mode TEXT NOT NULL DEFAULT 'ssh' CHECK(mode IN ('ssh', 'offline', 'mixed')),
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'running', 'completed', 'partially_completed', 'failed', 'paused', 'stopped', 'archived')),
    triggered_by TEXT DEFAULT 'manual' CHECK(triggered_by IN ('manual', 'scheduled')),
    scheduled_task_id INTEGER,
    device_ids TEXT NOT NULL DEFAULT '[]',
    started_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_batches_status ON inspection_batches(status);
CREATE INDEX IF NOT EXISTS idx_batches_mode ON inspection_batches(mode);
CREATE INDEX IF NOT EXISTS idx_batches_scheduled_task ON inspection_batches(scheduled_task_id);

-- Inspection records
CREATE TABLE IF NOT EXISTS inspection_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    batch_id INTEGER NOT NULL,
    device_id INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'running', 'completed', 'stopped', 'failed')),
    upload_source TEXT DEFAULT 'ssh' CHECK(upload_source IN ('ssh', 'offline', 'web')),
    error_message TEXT,
    command_outputs TEXT NOT NULL DEFAULT '{}',
    ai_status TEXT NOT NULL DEFAULT 'pending' CHECK(ai_status IN ('pending', 'processing', 'completed', 'failed')),
    ai_result TEXT,
    ai_analysis TEXT,
    ai_suggestions TEXT,
    command_judgments TEXT,
    summary_judgment TEXT,
    report_path TEXT,
    started_at TEXT,
    completed_at TEXT,
    timestamp TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_records_batch ON inspection_records(batch_id);
CREATE INDEX IF NOT EXISTS idx_records_device ON inspection_records(device_id);
CREATE INDEX IF NOT EXISTS idx_records_ai_status ON inspection_records(ai_status);

-- Scheduled tasks
CREATE TABLE IF NOT EXISTS scheduled_tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    cron_expression TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    device_ids TEXT NOT NULL DEFAULT '[]',
    next_run_at TEXT,
    last_run_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- AI model configs
CREATE TABLE IF NOT EXISTS ai_model_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    provider TEXT NOT NULL CHECK(provider IN ('openai', 'anthropic')),
    model_id TEXT NOT NULL,
    api_key_encrypted TEXT NOT NULL,
    base_url TEXT,
    is_active INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Report templates
CREATE TABLE IF NOT EXISTS report_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    vendor TEXT,
    file_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Offline log imports
CREATE TABLE IF NOT EXISTS offline_log_imports (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    filename TEXT NOT NULL,
    file_path TEXT NOT NULL,
    mode TEXT DEFAULT 'upload' CHECK(mode IN ('upload', 'script')),
    parsed_devices TEXT DEFAULT '[]',
    batch_id INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- System settings
CREATE TABLE IF NOT EXISTS system_settings (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    report_max_output_lines INTEGER NOT NULL DEFAULT 100
);

INSERT OR IGNORE INTO system_settings (id, report_max_output_lines) VALUES (1, 100);
