-- Devices (network equipment only)
CREATE TABLE IF NOT EXISTS devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    ip TEXT NOT NULL UNIQUE,
    device_type TEXT NOT NULL,
    vendor TEXT NOT NULL,
    model TEXT,
    ssh_username TEXT,
    ssh_password_encrypted TEXT,
    ssh_port INTEGER NOT NULL DEFAULT 22,
    template_id INTEGER,
    status TEXT NOT NULL DEFAULT 'unknown' CHECK(status IN ('online', 'offline', 'unknown')),
    last_checked_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

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
    config TEXT,
    description TEXT,
    report_template_id INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Command pool (network device commands only)
CREATE TABLE IF NOT EXISTS command_pool (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    vendor TEXT NOT NULL,
    command TEXT NOT NULL,
    description TEXT,
    category TEXT DEFAULT 'general',
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
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'running', 'completed', 'partially_completed', 'failed', 'paused', 'stopped', 'archived')),
    triggered_by TEXT DEFAULT 'manual' CHECK(triggered_by IN ('manual', 'scheduled')),
    device_ids TEXT NOT NULL DEFAULT '[]',
    started_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_batches_status ON inspection_batches(status);

-- Inspection records
CREATE TABLE IF NOT EXISTS inspection_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    batch_id INTEGER NOT NULL,
    device_id INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'running', 'completed', 'stopped', 'failed')),
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

-- System settings
CREATE TABLE IF NOT EXISTS system_settings (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    report_max_output_lines INTEGER NOT NULL DEFAULT 100
);

INSERT OR IGNORE INTO system_settings (id, report_max_output_lines) VALUES (1, 100);
