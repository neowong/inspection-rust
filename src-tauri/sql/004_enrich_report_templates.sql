-- ============================================================
-- 迁移 004: 增强报告模板表 - 支持内容级模板编辑
-- ============================================================

-- 新增列：模板内容、输出格式、是否默认、描述、示例数据
ALTER TABLE report_templates ADD COLUMN content TEXT DEFAULT '';
ALTER TABLE report_templates ADD COLUMN format TEXT DEFAULT 'markdown' CHECK(format IN ('markdown','html'));
ALTER TABLE report_templates ADD COLUMN is_default INTEGER NOT NULL DEFAULT 0;
ALTER TABLE report_templates ADD COLUMN description TEXT DEFAULT '';
ALTER TABLE report_templates ADD COLUMN sample_data TEXT DEFAULT '{}';

-- 插入内置默认 Markdown 模板
INSERT INTO report_templates (name, vendor, file_path, content, format, is_default, description, created_at, updated_at)
VALUES (
    '内置默认报告模板',
    NULL,
    '',
    '# {{device_name}} 巡检报告

> 生成时间: {{report_timestamp}}

## 基本信息

| 项目 | 内容 | 项目 | 内容 |
|------|------|------|------|
| 设备名称 | {{device_name}} | IP 地址 | {{device_ip}} |
| 厂商 | {{vendor}} | 型号 | {{model}} |
| 序列号 | {{sn}} | 生产日期 | {{manufacturing_date}} |

## 巡检结果

{{#each command_judgments}}
### {{command}}

- 状态: {{status}}
- 结果: {{finding}}
- 建议: {{suggestion}}

```
{{output}}
```
{{/each}}

## AI 分析总结

{{ai_analysis}}

## 总体评估

{{summary}}
',
    'markdown',
    1,
    '系统内置的默认报告模板，包含基本信息、设备详情、巡检结果、AI 分析总结和总体评估。',
    datetime('now'),
    datetime('now')
);
