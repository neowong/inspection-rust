use rusqlite::Connection;

/// 运行数据库迁移
///
/// 使用 PRAGMA user_version 跟踪已应用的迁移版本。
/// 版本 1：初始化全部数据库表结构。
pub fn run_migrations(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
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
            "UPDATE report_templates SET config_json = '{\"sections\":[{\"type\":\"title\",\"enabled\":true,\"label\":\"报告标题\",\"config\":{}},{\"type\":\"basic_info\",\"enabled\":true,\"label\":\"基本信息\",\"config\":{\"fields\":[\"device_name\",\"device_ip\",\"vendor\",\"model\",\"sn\",\"manufacturing_date\"]}},{{\"type\":\"inspection_results\",\"enabled\":true,\"label\":\"巡检结果\",\"config\":{\"show_output\":true,\"max_output_lines\":60}},{\"type\":\"ai_analysis\",\"enabled\":true,\"label\":\"AI 分析总结\",\"config\":{}},{\"type\":\"overall_assessment\",\"enabled\":true,\"label\":\"总体评估\",\"config\":{}}]}', mode = 'visual' WHERE is_default = 1 AND (config_json IS NULL OR config_json = '');"
        )?;

        conn.execute_batch("PRAGMA user_version = 5")?;
    }

    if version < 6 {
        // 添加设备 SN 和出厂日期字段
        let has_sn: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('devices') WHERE name = 'serial_number'")
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

    Ok(())
}
