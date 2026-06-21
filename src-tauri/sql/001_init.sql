-- ============================================================
-- 网络设备巡检系统 - 数据库初始化脚本
-- Network Device Inspection System - Database Initialization
-- ============================================================

-- 设备表
CREATE TABLE IF NOT EXISTS devices (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    name                  TEXT NOT NULL UNIQUE,                          -- 设备名称
    ip                    TEXT NOT NULL UNIQUE,                          -- 管理 IP 地址
    device_type           TEXT NOT NULL,                                 -- 设备类型 (switch/router/firewall/loadbalancer)
    vendor                TEXT NOT NULL,                                 -- 厂商 (H3C/华为/思科/锐捷)
    model                 TEXT,                                          -- 型号
    ssh_username          TEXT,                                          -- SSH 用户名
    ssh_password_encrypted TEXT,                                         -- SSH 密码（加密存储）
    ssh_port              INTEGER NOT NULL DEFAULT 22,                   -- SSH 端口
    template_id           INTEGER REFERENCES inspection_templates(id),   -- 关联巡检模板
    status                TEXT NOT NULL DEFAULT 'unknown'                -- 在线状态
                          CHECK(status IN ('online','offline','unknown')),
    last_checked_at       TEXT,                                          -- 上次检测时间
    created_at            TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at            TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 设备状态变更日志表
CREATE TABLE IF NOT EXISTS device_status_logs (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id INTEGER NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    old_status TEXT,
    new_status TEXT NOT NULL,
    checked_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 巡检模板表
CREATE TABLE IF NOT EXISTS inspection_templates (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    name              TEXT NOT NULL UNIQUE,                     -- 模板名称
    vendor            TEXT NOT NULL,                            -- 厂商
    model             TEXT,                                     -- 型号
    device_type       TEXT,                                     -- 设备类型
    config            TEXT,                                     -- 配置项 (JSON, 存储命令ID列表)
    description       TEXT,                                     -- 描述
    report_template_id INTEGER,                                 -- 关联报告模板
    created_at        TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at        TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 命令库表
CREATE TABLE IF NOT EXISTS command_pool (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    vendor      TEXT NOT NULL,                                  -- 厂商
    command     TEXT NOT NULL,                                  -- 命令
    description TEXT,                                           -- 描述
    category    TEXT DEFAULT 'general',                         -- 分类
    model       TEXT,                                           -- 适用型号
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(vendor, command)
);

-- 巡检批次表
CREATE TABLE IF NOT EXISTS inspection_batches (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    name         TEXT,                                                    -- 批次名称
    status       TEXT NOT NULL DEFAULT 'pending'                          -- 批次状态
                 CHECK(status IN ('pending','running','completed',
                         'partially_completed','failed','paused',
                         'stopped','archived')),
    triggered_by TEXT DEFAULT 'manual'                                    -- 触发方式
                 CHECK(triggered_by IN ('manual','scheduled')),
    device_ids   TEXT NOT NULL DEFAULT '[]',                              -- 设备ID列表 (JSON)
    started_at   TEXT,
    completed_at TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 巡检记录表（每个设备每次巡检的详细记录）
CREATE TABLE IF NOT EXISTS inspection_records (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    batch_id          INTEGER NOT NULL,                                   -- 关联批次
    device_id         INTEGER NOT NULL,                                   -- 关联设备
    status            TEXT NOT NULL DEFAULT 'pending'                     -- 执行状态
                      CHECK(status IN ('pending','running','completed','stopped','failed')),
    error_message     TEXT,                                               -- 错误信息
    command_outputs   TEXT NOT NULL DEFAULT '{}',                         -- 命令执行结果 (JSON)
    ai_status         TEXT NOT NULL DEFAULT 'pending'                     -- AI 分析状态
                      CHECK(ai_status IN ('pending','processing','completed','failed')),
    ai_result         TEXT,                                               -- AI 分析结论 (JSON)
    ai_analysis       TEXT,                                               -- AI 详细分析文本
    ai_suggestions    TEXT,                                               -- AI 建议
    command_judgments TEXT,                                               -- 逐命令判断结果 (JSON)
    summary_judgment  TEXT,                                               -- 综合判断结果 (JSON)
    report_path       TEXT,                                               -- 报告文件路径
    started_at        TEXT,
    completed_at      TEXT,
    created_at        TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at        TEXT NOT NULL DEFAULT (datetime('now'))
);

-- AI 模型配置表
CREATE TABLE IF NOT EXISTS ai_model_configs (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    name               TEXT NOT NULL,                          -- 配置名称
    provider           TEXT NOT NULL                           -- 提供商
                       CHECK(provider IN ('openai','anthropic','deepseek')),
    model_id           TEXT NOT NULL,                          -- 模型 ID
    api_key_encrypted  TEXT NOT NULL,                          -- API 密钥（加密存储）
    base_url           TEXT,                                   -- 自定义 API 地址
    is_active          INTEGER NOT NULL DEFAULT 0,             -- 是否激活
    created_at         TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at         TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 报告模板表
CREATE TABLE IF NOT EXISTS report_templates (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,                               -- 模板名称
    vendor     TEXT,                                        -- 适用厂商
    file_path  TEXT NOT NULL,                               -- 模板文件路径
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================
-- 索引
-- ============================================================
CREATE INDEX IF NOT EXISTS idx_devices_vendor      ON devices(vendor);
CREATE INDEX IF NOT EXISTS idx_devices_status      ON devices(status);
CREATE INDEX IF NOT EXISTS idx_devices_template_id ON devices(template_id);
CREATE INDEX IF NOT EXISTS idx_device_status_logs_device_id ON device_status_logs(device_id);
CREATE INDEX IF NOT EXISTS idx_command_pool_vendor ON command_pool(vendor);
CREATE INDEX IF NOT EXISTS idx_inspection_batches_status ON inspection_batches(status);
CREATE INDEX IF NOT EXISTS idx_inspection_records_batch_id  ON inspection_records(batch_id);
CREATE INDEX IF NOT EXISTS idx_inspection_records_device_id ON inspection_records(device_id);
CREATE INDEX IF NOT EXISTS idx_inspection_records_ai_status ON inspection_records(ai_status);
CREATE INDEX IF NOT EXISTS idx_report_templates_vendor     ON report_templates(vendor);
-- 注意：is_default 列由迁移 v4（004_enrich_report_templates.sql）通过 ALTER 添加，
-- 此处建表时该列尚不存在，故 is_default 索引改由迁移 v17 在列添加后创建。
