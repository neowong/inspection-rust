use rusqlite::Connection;

/// 运行数据库迁移
///
/// 使用 PRAGMA user_version 跟踪已应用的迁移版本。
/// 版本 1：初始化全部数据库表结构。
pub fn run_migrations(conn: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    // 启用 WAL 模式和外键约束
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap_or(0);

    if version < 1 {
        conn.execute_batch(include_str!("../../sql/001_init.sql"))?;
        conn.execute_batch("PRAGMA user_version = 1")?;
    }

    if version < 2 {
        conn.execute_batch(include_str!("../../sql/002_add_deepseek_provider.sql"))?;
        conn.execute_batch("PRAGMA user_version = 2")?;
    }

    if version < 3 {
        conn.execute_batch(include_str!("../../sql/003_add_template_type.sql"))?;
        conn.execute_batch("PRAGMA user_version = 3")?;
    }

    if version < 4 {
        conn.execute_batch(include_str!("../../sql/004_enrich_report_templates.sql"))?;
        conn.execute_batch("PRAGMA user_version = 4")?;
    }

    if version < 5 {
        // Check if config_json column already exists (could be added by updated 004)
        let has_config_json: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('report_templates') WHERE name = 'config_json'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .map(|c| c > 0)
            .unwrap_or(false);

        if !has_config_json {
            conn.execute_batch("ALTER TABLE report_templates ADD COLUMN config_json TEXT DEFAULT ''; ALTER TABLE report_templates ADD COLUMN mode TEXT DEFAULT 'visual' CHECK(mode IN ('visual','advanced'));")?;
        }

        // Update default template config (safe to run regardless)
        conn.execute_batch(
            "UPDATE report_templates SET config_json = '{\"sections\":[{\"type\":\"title\",\"enabled\":true,\"label\":\"报告标题\",\"config\":{}},{\"type\":\"basic_info\",\"enabled\":true,\"label\":\"基本信息\",\"config\":{\"fields\":[\"device_name\",\"device_ip\",\"vendor\",\"model\",\"sn\",\"manufacturing_date\"]}},{\"type\":\"inspection_results\",\"enabled\":true,\"label\":\"巡检结果\",\"config\":{\"show_output\":true,\"max_output_lines\":60}},{\"type\":\"ai_analysis\",\"enabled\":true,\"label\":\"AI 分析总结\",\"config\":{}},{\"type\":\"overall_assessment\",\"enabled\":true,\"label\":\"总体评估\",\"config\":{}}]}', mode = 'visual' WHERE is_default = 1 AND (config_json IS NULL OR config_json = '');"
        )?;

        conn.execute_batch("PRAGMA user_version = 5")?;
    }

    if version < 6 {
        // 添加设备 SN 和出厂日期字段
        let has_sn: bool = conn
            .prepare(
                "SELECT COUNT(*) FROM pragma_table_info('devices') WHERE name = 'serial_number'",
            )
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .map(|c| c > 0)
            .unwrap_or(false);
        if !has_sn {
            conn.execute_batch("ALTER TABLE devices ADD COLUMN serial_number TEXT; ALTER TABLE devices ADD COLUMN manufacturing_date TEXT;")?;
        }
        conn.execute_batch("PRAGMA user_version = 6")?;
    }

    if version < 7 {
        // 报告模板增强：自定义 CSS、页眉、页脚
        let has_custom_css: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('report_templates') WHERE name = 'custom_css'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .map(|c| c > 0)
            .unwrap_or(false);
        if !has_custom_css {
            conn.execute_batch(
                "ALTER TABLE report_templates ADD COLUMN custom_css TEXT DEFAULT '';
                 ALTER TABLE report_templates ADD COLUMN page_header TEXT DEFAULT '';
                 ALTER TABLE report_templates ADD COLUMN page_footer TEXT DEFAULT '';",
            )?;
        }
        conn.execute_batch("PRAGMA user_version = 7")?;
    }

    if version < 8 {
        // 移除 format 列的 CHECK 约束，支持 docx 格式
        // SQLite 无法直接修改 CHECK，需重建表
        // 使用事务保护：INSERT 失败时回滚，避免数据丢失
        let tx = conn.transaction()?;
        tx.execute_batch(
            "ALTER TABLE report_templates RENAME TO report_templates_old;
             CREATE TABLE report_templates (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL,
                 vendor TEXT,
                 file_path TEXT NOT NULL,
                 content TEXT DEFAULT '',
                 format TEXT DEFAULT 'markdown',
                 is_default INTEGER NOT NULL DEFAULT 0,
                 description TEXT DEFAULT '',
                 sample_data TEXT DEFAULT '{}',
                 config_json TEXT DEFAULT '',
                 mode TEXT DEFAULT 'visual',
                 custom_css TEXT DEFAULT '',
                 page_header TEXT DEFAULT '',
                 page_footer TEXT DEFAULT '',
                 created_at TEXT NOT NULL DEFAULT (datetime('now')),
                 updated_at TEXT NOT NULL DEFAULT (datetime('now'))
             );
             INSERT INTO report_templates (id, name, vendor, file_path, content, format, is_default, description, sample_data, config_json, mode, custom_css, page_header, page_footer, created_at, updated_at)
                 SELECT id, name, vendor, file_path, content, format, is_default, description, sample_data, config_json, mode, custom_css, page_header, page_footer, created_at, updated_at
                 FROM report_templates_old;
             DROP TABLE report_templates_old;"
        )?;
        tx.commit()?;
        conn.execute_batch("PRAGMA user_version = 8")?;
    }

    if version < 9 {
        // 报告模板：把旧的"区块拼装"配置重置为新的"列定义驱动"配置
        // 所有模板的 format 切到 'docx'；config_json 旧格式的统一回填默认值
        let new_config = crate::services::report_config::default_config_json();

        conn.execute_batch("UPDATE report_templates SET format = 'docx', mode = 'visual';")?;

        conn.execute(
            "UPDATE report_templates SET config_json = ?1 \
             WHERE config_json IS NULL OR TRIM(config_json) = '' OR config_json LIKE '%\"sections\"%'",
            rusqlite::params![&new_config],
        )?;

        // 兜底：保证至少存在一条 is_default 模板
        let has_default: i64 = conn
            .prepare("SELECT COUNT(*) FROM report_templates WHERE is_default = 1")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get(0)))
            .unwrap_or(0);
        if has_default == 0 {
            conn.execute(
                "INSERT INTO report_templates (name, vendor, file_path, content, format, is_default, description, config_json, mode) \
                 VALUES ('内置默认模板', NULL, '', '', 'docx', 1, '系统默认报告模板', ?1, 'visual')",
                rusqlite::params![&new_config],
            )?;
        }

        conn.execute_batch("PRAGMA user_version = 9")?;
    }

    if version < 10 {
        // 报告模板列定义改版：去掉 ai_finding 列，巡检明细只保留 4 列
        // 不再兼容旧报告模板，所有模板统一重置为新默认结构
        let new_config = crate::services::report_config::default_config_json();
        conn.execute(
            "UPDATE report_templates SET config_json = ?1, format = 'docx', mode = 'visual'",
            rusqlite::params![&new_config],
        )?;
        conn.execute_batch("PRAGMA user_version = 10")?;
    }

    if version < 11 {
        // 设备静态信息：真实 CLI sysname，用于报告还原终端提示符
        let has_sysname: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('devices') WHERE name = 'sysname'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .map(|c| c > 0)
            .unwrap_or(false);
        if !has_sysname {
            conn.execute_batch("ALTER TABLE devices ADD COLUMN sysname TEXT;")?;
        }
        conn.execute_batch("PRAGMA user_version = 11")?;
    }

    if version < 12 {
        // 本次巡检静态信息快照：sysname/model/SN/出厂日期等
        let has_static_info: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('inspection_records') WHERE name = 'static_info'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .map(|c| c > 0)
            .unwrap_or(false);
        if !has_static_info {
            conn.execute_batch("ALTER TABLE inspection_records ADD COLUMN static_info TEXT;")?;
        }
        conn.execute_batch("PRAGMA user_version = 12")?;
    }

    if version < 13 {
        // 巡检模板配置升级：command_ids → commands[{command_id,purpose,show_in_report,extract_fields}]
        let mut stmt = conn.prepare("SELECT id, config FROM inspection_templates")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        let mut updates: Vec<(i64, String)> = Vec::new();
        for row in rows {
            let (id, config_opt) = row?;
            let Some(config_str) = config_opt else {
                continue;
            };
            let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&config_str) else {
                continue;
            };
            if value.get("commands").is_some() {
                continue;
            }
            let Some(ids) = value.get("command_ids").and_then(|v| v.as_array()) else {
                continue;
            };
            let commands: Vec<serde_json::Value> = ids
                .iter()
                .filter_map(|v| v.as_i64())
                .map(|id| {
                    serde_json::json!({
                        "command_id": id,
                        "purpose": "inspection",
                        "show_in_report": true,
                        "extract_fields": [],
                    })
                })
                .collect();
            value = serde_json::json!({ "commands": commands });
            updates.push((id, serde_json::to_string(&value)?));
        }
        drop(stmt);
        for (id, config) in updates {
            conn.execute(
                "UPDATE inspection_templates SET config = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![config, id],
            )?;
        }
        conn.execute_batch("PRAGMA user_version = 13")?;
    }

    if version < 14 {
        // 报告模板瘦身：彻底移除 md/html 时代遗留列，仅保留 DOCX 在线模板所需字段
        // 使用事务保护：INSERT 失败时回滚，避免数据丢失
        let tx = conn.transaction()?;
        tx.execute_batch(
            "ALTER TABLE report_templates RENAME TO report_templates_old;
             CREATE TABLE report_templates (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL,
                 vendor TEXT,
                 is_default INTEGER NOT NULL DEFAULT 0,
                 description TEXT DEFAULT '',
                 config_json TEXT DEFAULT '',
                 created_at TEXT NOT NULL DEFAULT (datetime('now')),
                 updated_at TEXT NOT NULL DEFAULT (datetime('now'))
             );
             INSERT INTO report_templates (id, name, vendor, is_default, description, config_json, created_at, updated_at)
                 SELECT id, name, vendor, is_default, description, config_json, created_at, updated_at
                 FROM report_templates_old;
             DROP TABLE report_templates_old;"
        )?;
        tx.commit()?;
        conn.execute_batch("PRAGMA user_version = 14")?;
    }

    if version < 15 {
        // 报告最大输出行数已移动到报告模板配置中，删除旧 system_settings 表
        conn.execute_batch("DROP TABLE IF EXISTS system_settings; PRAGMA user_version = 15")?;
    }

    if version < 16 {
        // 内置 H3C 专用报告模板
        let h3c_config = serde_json::json!({
            "cover": {
                "title": "H3C 网络设备巡检报告",
                "subtitle": "运维巡检中心",
                "logo_path": "",
                "primary_color": "#0066CC"
            },
            "device_info": {
                "enabled": true,
                "layout": "two_column",
                "fields": [
                    { "key": "name",         "label": "设备名称",   "visible": true },
                    { "key": "ip",           "label": "管理地址",   "visible": true },
                    { "key": "vendor",       "label": "设备厂商",   "visible": true },
                    { "key": "model",        "label": "设备型号",   "visible": true },
                    { "key": "sn",           "label": "序列号",     "visible": true },
                    { "key": "mfg_date",     "label": "出厂日期",   "visible": true },
                    { "key": "inspect_time", "label": "巡检时间",   "visible": true }
                ]
            },
            "command_table": {
                "columns": [
                    { "key": "seq",         "label": "序号",     "width": 6,  "visible": true },
                    { "key": "item",        "label": "巡检项目", "width": 16, "visible": true },
                    { "key": "output",      "label": "巡检内容", "width": 58, "visible": true },
                    { "key": "ai_judgment", "label": "评判结论", "width": 20, "visible": true }
                ],
                "output_max_lines": 15
            },
            "summary": {
                "enabled": true,
                "title": "巡检总结",
                "show_problem_table": true
            },
            "header": "H3C 设备巡检报告",
            "footer": "第 {{page}} 页 / 共 {{total}} 页"
        });
        let h3c_config_str = serde_json::to_string(&h3c_config).unwrap_or_default();

        // 检查是否已存在同名模板，避免重复插入
        let exists: i64 = conn
            .prepare("SELECT COUNT(*) FROM report_templates WHERE name = 'H3C 专用模板'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get(0)))
            .unwrap_or(0);

        if exists == 0 {
            conn.execute(
                "INSERT INTO report_templates (name, vendor, is_default, description, config_json) \
                 VALUES ('H3C 专用模板', 'H3C', 0, 'H3C Comware 设备巡检报告模板，含序列号和出厂日期', ?1)",
                rusqlite::params![h3c_config_str],
            )?;
        }

        conn.execute_batch("PRAGMA user_version = 16")?;
    }

    // 飞塔 (FortiGate) 命令由 seed_data.rs 统一提供，不再在迁移中插入。
    // 历史迁移 17 曾在此插入飞塔命令，但与 seed_data 重复，且会导致全新安装时
    // seed_command_pool 误判“表非空”而跳过其他厂商种子数据。已移除。

    if version < 17 {
        // report_templates 补建索引：按 is_default / vendor 查询排序频繁（reports.rs 多处）。
        // 全新安装已由 001_init.sql 建立，此处为升级库补建（IF NOT EXISTS 幂等）。
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_report_templates_is_default ON report_templates(is_default);
             CREATE INDEX IF NOT EXISTS idx_report_templates_vendor ON report_templates(vendor);",
        )?;
        conn.execute_batch("PRAGMA user_version = 17")?;
    }

    Ok(())
}
