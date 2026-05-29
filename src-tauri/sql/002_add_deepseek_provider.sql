-- 添加 DeepSeek AI 提供商支持
-- DeepSeek 使用 OpenAI 兼容的 API 格式

-- 由于 SQLite 不支持 ALTER TABLE 修改 CHECK 约束，
-- 我们需要重建 ai_model_configs 表

-- 1. 创建新表
CREATE TABLE IF NOT EXISTS ai_model_configs_new (
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

-- 2. 复制数据到新表
INSERT INTO ai_model_configs_new SELECT * FROM ai_model_configs;

-- 3. 删除旧表
DROP TABLE ai_model_configs;

-- 4. 重命名新表
ALTER TABLE ai_model_configs_new RENAME TO ai_model_configs;
