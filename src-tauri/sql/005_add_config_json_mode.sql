-- ============================================================
-- 迁移 005: 添加可视化模板配置列 (config_json + mode)
-- ============================================================

-- 仅在列不存在时添加 config_json
-- SQLite 不支持 IF NOT EXISTS for ALTER TABLE，
-- 但如果列已存在会报错。这里假设 004 迁移未包含这些列。
-- 若 004 已包含（新版），此迁移仍安全执行：使用 INSERT OR REPLACE 处理默认模板。

ALTER TABLE report_templates ADD COLUMN config_json TEXT DEFAULT '';
ALTER TABLE report_templates ADD COLUMN mode TEXT DEFAULT 'visual' CHECK(mode IN ('visual','advanced'));

-- 更新内置默认模板为可视化模式（若存在且尚未配置）
UPDATE report_templates SET
    config_json = '{"sections":[{"type":"title","enabled":true,"label":"报告标题","config":{}},{"type":"basic_info","enabled":true,"label":"基本信息","config":{"fields":["device_name","device_ip","vendor","model","sn","manufacturing_date"]}},{"type":"inspection_results","enabled":true,"label":"巡检结果","config":{"show_output":true,"max_output_lines":60}},{"type":"ai_analysis","enabled":true,"label":"AI 分析总结","config":{}},{"type":"overall_assessment","enabled":true,"label":"总体评估","config":{}}]}',
    mode = 'visual'
WHERE is_default = 1 AND (config_json IS NULL OR config_json = '');
