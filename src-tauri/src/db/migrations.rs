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
                "title": "H3C 设备巡检报告",
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
        // is_default 列由 v4 添加，vendor 列在 001_init 即存在；此处为全新安装与升级库统一补建
        // （IF NOT EXISTS 幂等）。注意 001_init.sql 不再创建 is_default 索引——建表时该列尚不存在。
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_report_templates_is_default ON report_templates(is_default);
             CREATE INDEX IF NOT EXISTS idx_report_templates_vendor ON report_templates(vendor);",
        )?;
        conn.execute_batch("PRAGMA user_version = 17")?;
    }

    // ── v18: command_pool 增加 needs_root 字段（Linux sudo 支持） ──
    if version < 18 {
        let has_column: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('command_pool') WHERE name = 'needs_root'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .map(|c| c > 0)
            .unwrap_or(false);
        if !has_column {
            conn.execute_batch("ALTER TABLE command_pool ADD COLUMN needs_root INTEGER DEFAULT 0;")
                .map_err(|e| format!("migration 18: {}", e))?;
        }
        conn.execute_batch("PRAGMA user_version = 18;")
            .map_err(|e| format!("migration 18: {}", e))?;
    }

    // ── v19: devices 增加 cpu_cores、memory_gb 字段（Linux 服务器静态信息） ──
    if version < 19 {
        for col in &["cpu_cores", "memory_gb"] {
            let has: bool = conn
                .prepare(&format!("SELECT COUNT(*) FROM pragma_table_info('devices') WHERE name = '{}'", col))
                .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
                .map(|c| c > 0)
                .unwrap_or(false);
            if !has {
                let ty = if *col == "cpu_cores" { "INTEGER" } else { "REAL" };
                conn.execute_batch(&format!("ALTER TABLE devices ADD COLUMN {} {};", col, ty))
                    .map_err(|e| format!("migration 19: {}", e))?;
            }
        }
        conn.execute_batch("PRAGMA user_version = 19;")
            .map_err(|e| format!("migration 19: {}", e))?;
    }

    // ── v20: devices 增加 auth_status / auth_message（SSH 账号验证状态） ──
    if version < 20 {
        for (col, ty) in &[("auth_status", "TEXT DEFAULT 'unknown'"), ("auth_message", "TEXT")] {
            let has: bool = conn
                .prepare(&format!("SELECT COUNT(*) FROM pragma_table_info('devices') WHERE name = '{}'", col))
                .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
                .map(|c| c > 0)
                .unwrap_or(false);
            if !has {
                conn.execute_batch(&format!("ALTER TABLE devices ADD COLUMN {} {};", col, ty))
                    .map_err(|e| format!("migration 20: {}", e))?;
            }
        }
        conn.execute_batch("PRAGMA user_version = 20;")
            .map_err(|e| format!("migration 20: {}", e))?;
    }

    // ── v21: inspection_batches 增加 combined_report_path（综合报告持久化） ──
    if version < 21 {
        let has_col: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('inspection_batches') WHERE name = 'combined_report_path'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .map(|c| c > 0)
            .unwrap_or(false);
        if !has_col {
            conn.execute_batch("ALTER TABLE inspection_batches ADD COLUMN combined_report_path TEXT;")
                .map_err(|e| format!("migration 21: {}", e))?;
        }

        // 回填历史批次：扫描 reports 目录下的 batch_{id}_*.docx，匹配到最新文件
        let reports_dir = crate::APP_DATA_DIR
            .get()
            .map(|d| d.join("data").join("reports"))
            .filter(|d| d.exists());
        if let Some(dir) = reports_dir {
            // 读取所有批次 ID 及其现有 combined_report_path
            let mut batch_paths: std::collections::HashMap<i64, (String, u64)> =
                std::collections::HashMap::new();
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    // 匹配 batch_{id}_*.docx 或 batch_{id}_*.zip
                    if let Some(rest) = name.strip_prefix("batch_") {
                        if let Some(idx) = rest.find('_') {
                            if let Ok(batch_id) = rest[..idx].parse::<i64>() {
                                if name.ends_with(".docx") || name.ends_with(".zip") {
                                    let modified = entry.metadata().ok()
                                        .and_then(|m| m.modified().ok())
                                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                        .map(|d| d.as_secs())
                                        .unwrap_or(0);
                                    // 保留每个批次最新的文件
                                    batch_paths
                                        .entry(batch_id)
                                        .and_modify(|(old_path, old_ts)| {
                                            if modified > *old_ts {
                                                *old_path = name.clone();
                                                *old_ts = modified;
                                            }
                                        })
                                        .or_insert_with(|| (name.clone(), modified));
                                }
                            }
                        }
                    }
                }
            }
            // 回填到 DB
            for (batch_id, (filename, _)) in &batch_paths {
                let full_path = dir.join(filename).to_string_lossy().to_string();
                let _ = conn.execute(
                    "UPDATE inspection_batches SET combined_report_path = ?1 WHERE id = ?2 AND combined_report_path IS NULL",
                    rusqlite::params![full_path, batch_id],
                );
                tracing::info!(
                    "migration 21: 回填批次 #{} 的综合报告路径: {}",
                    batch_id,
                    full_path
                );
            }
        }

        conn.execute_batch("PRAGMA user_version = 21;")
            .map_err(|e| format!("migration 21: {}", e))?;
    }

    // ── v22: 内置报告模板 + H3C 接入交换机巡检模板 ──
    if version < 22 {
        // ---- 报告模板 ----
        // 按大类决定字段集（与前端 vendorCategory 对齐）
        let is_linux = |v: &str| {
            ["linux","ubuntu","centos","rocky","debian","rhel","suse","fedora","alma"]
                .iter().any(|o| v.to_lowercase().contains(o))
        };
        let is_db = |v: &str| {
            ["mysql","postgres","oracle","sql","达梦","mariadb","mssql","redis","mongo"]
                .iter().any(|o| v.to_lowercase().contains(o))
        };

        let make_config = |title: &str, color: &str, vendor: &str| -> serde_json::Value {
            let mut fields = vec![
                serde_json::json!({"key":"name","label":"设备名称","visible":true}),
                serde_json::json!({"key":"ip","label":"管理地址","visible":true}),
                serde_json::json!({"key":"vendor","label":"设备厂商","visible":true}),
                serde_json::json!({"key":"inspect_time","label":"巡检时间","visible":true}),
            ];
            if is_linux(vendor) {
                fields.push(serde_json::json!({"key":"os_release","label":"发行版","visible":true}));
                fields.push(serde_json::json!({"key":"kernel_version","label":"内核版本","visible":true}));
                fields.push(serde_json::json!({"key":"cpu_cores","label":"CPU 核心","visible":true}));
                fields.push(serde_json::json!({"key":"memory_gb","label":"内存(GB)","visible":true}));
                fields.push(serde_json::json!({"key":"model","label":"设备型号","visible":false}));
                fields.push(serde_json::json!({"key":"sn","label":"序列号","visible":false}));
                fields.push(serde_json::json!({"key":"mfg_date","label":"出厂日期","visible":false}));
                fields.push(serde_json::json!({"key":"hostname","label":"主机名","visible":false}));
            } else if is_db(vendor) {
                fields.push(serde_json::json!({"key":"db_version","label":"数据库版本","visible":true}));
                fields.push(serde_json::json!({"key":"instance_name","label":"实例名","visible":true}));
                fields.push(serde_json::json!({"key":"os_release","label":"宿主机 OS","visible":true}));
                fields.push(serde_json::json!({"key":"cpu_cores","label":"宿主机 CPU 核心","visible":true}));
                fields.push(serde_json::json!({"key":"memory_gb","label":"宿主机 内存(GB)","visible":true}));
                fields.push(serde_json::json!({"key":"model","label":"宿主机 型号","visible":false}));
                fields.push(serde_json::json!({"key":"sn","label":"宿主机 序列号","visible":false}));
                fields.push(serde_json::json!({"key":"mfg_date","label":"宿主机 出厂日期","visible":false}));
                fields.push(serde_json::json!({"key":"sysname","label":"宿主机 主机名","visible":false}));
                fields.push(serde_json::json!({"key":"kernel_version","label":"宿主机 内核版本","visible":false}));
            } else {
                fields.push(serde_json::json!({"key":"model","label":"设备型号","visible":true}));
                fields.push(serde_json::json!({"key":"sn","label":"序列号","visible":true}));
                fields.push(serde_json::json!({"key":"mfg_date","label":"出厂日期","visible":true}));
                fields.push(serde_json::json!({"key":"sysname","label":"主机名","visible":true}));
            }
            serde_json::json!({
                "cover": {
                    "title": title,
                    "subtitle": "运维巡检中心",
                    "logo_path": "",
                    "primary_color": color
                },
                "device_info": {
                    "enabled": true, "layout": "two_column",
                    "fields": fields
                },
                "command_table": {
                    "columns": [
                        {"key":"seq","label":"序号","width":6,"visible":true},
                        {"key":"item","label":"巡检项目","width":16,"visible":true},
                        {"key":"output","label":"巡检内容","width":58,"visible":true},
                        {"key":"ai_judgment","label":"评判结论","width":20,"visible":true}
                    ],
                    "output_max_lines": 15
                },
                "summary": {"enabled": true, "title": "巡检总结", "show_problem_table": true},
                "header": format!("{}巡检报告", title),
                "footer": "第 {{page}} 页 / 共 {{total}} 页"
            })
        };

        let builtin_reports: &[(&str, &str, &str, &str)] = &[
            ("Ubuntu 服务器模板", "Linux", "#E95420", "Ubuntu 服务器巡检报告模板"),
            ("CentOS 服务器模板", "Linux", "#262577", "CentOS/RHEL 服务器巡检报告模板"),
            ("遥遥领先专用模板",   "华为",  "#CF0A2C", "华为 VRP 网络设备巡检报告模板"),
            ("思科 专用模板",   "思科",  "#005073", "Cisco IOS/IOS-XE 网络设备巡检报告模板"),
            ("MySQL 数据库模板", "MySQL",      "#4479A1", "MySQL 数据库巡检报告模板（含宿主机信息）"),
            ("PostgreSQL 数据库模板", "PostgreSQL", "#336791", "PostgreSQL 数据库巡检报告模板（含宿主机信息）"),
            ("Oracle 数据库模板", "Oracle",     "#C74634", "Oracle 数据库巡检报告模板（含宿主机信息）"),
        ];

        for (name, vendor, color, desc) in builtin_reports {
            let exists: i64 = conn
                .prepare("SELECT COUNT(*) FROM report_templates WHERE name = ?1")
                .and_then(|mut stmt| stmt.query_row(rusqlite::params![name], |row| row.get(0)))
                .unwrap_or(0);
            if exists == 0 {
                let cfg = make_config(name, color, vendor);
                conn.execute(
                    "INSERT INTO report_templates (name, vendor, is_default, description, config_json) VALUES (?1, ?2, 0, ?3, ?4)",
                    rusqlite::params![name, vendor, desc, serde_json::to_string(&cfg).unwrap_or_default()],
                )?;
                tracing::info!("migration 22: 内置报告模板 '{}' 已添加", name);
            }
        }

        // ---- H3C 接入交换机巡检模板 ----
        {
            let name = "H3C 接入交换机";
            let exists: i64 = conn
                .prepare("SELECT COUNT(*) FROM inspection_templates WHERE name = ?1")
                .and_then(|mut stmt| stmt.query_row(rusqlite::params![name], |row| row.get(0)))
                .unwrap_or(0);
            if exists == 0 {
                // 收集 H3C 接入交换机常用命令的 ID
                let cmd_names: &[&str] = &[
                    "display version",
                    "display device",
                    "display cpu-usage",
                    "display memory",
                    "display fan",
                    "display power",
                    "display environment",
                    "display interface brief",
                    "display logbuffer",
                    "display current-configuration",
                    "display vlan brief",
                    "display stp brief",
                    "display mac-address",
                    "display arp",
                    "display ip routing-table",
                ];
                let mut cmd_configs: Vec<serde_json::Value> = Vec::new();
                for cmd in cmd_names {
                    if let Ok(id) = conn.query_row(
                        "SELECT id FROM command_pool WHERE vendor='H3C' AND command=?1",
                        rusqlite::params![cmd],
                        |r| r.get::<_, i64>(0),
                    ) {
                        cmd_configs.push(serde_json::json!({
                            "command_id": id,
                            "purpose": "inspection",
                            "show_in_report": true,
                            "extract_fields": []
                        }));
                    }
                }
                if !cmd_configs.is_empty() {
                    let config = serde_json::json!({ "commands": cmd_configs });
                    conn.execute(
                        "INSERT INTO inspection_templates (name, vendor, config, description) VALUES (?1, 'H3C', ?2, 'H3C 接入交换机标准巡检模板，覆盖设备状态/接口/二层/三层关键信息')",
                        rusqlite::params![name, serde_json::to_string(&config).unwrap_or_default()],
                    )?;
                    tracing::info!("migration 22: 内置巡检模板 '{}' ({} 条命令) 已添加", name, cmd_configs.len());
                }
            }
        }

        conn.execute_batch("PRAGMA user_version = 22;")
            .map_err(|e| format!("migration 22: {}", e))?;
    }

    // ── v23: 修正华为模板名称 → 遥遥领先专用模板 ──
    if version < 23 {
        // 若目标名已存在（v22 已插入），则删除旧名的；否则直接改名
        let target_exists: i64 = conn
            .prepare("SELECT COUNT(*) FROM report_templates WHERE name = '遥遥领先专用模板'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get(0)))
            .unwrap_or(0);
        if target_exists > 0 {
            // 新名已存在 → 删掉旧名记录
            conn.execute("DELETE FROM report_templates WHERE name = '华为 专用模板'", [])?;
            conn.execute("DELETE FROM report_templates WHERE name = '华为 遥遥领先专用模板'", [])?;
        } else {
            // 新名不存在 → 直接改名
            conn.execute("UPDATE report_templates SET name = '遥遥领先专用模板' WHERE name = '华为 专用模板'", [])?;
            conn.execute("UPDATE report_templates SET name = '遥遥领先专用模板' WHERE name = '华为 遥遥领先专用模板'", [])?;
        }
        conn.execute_batch("PRAGMA user_version = 23;")
            .map_err(|e| format!("migration 23: {}", e))?;
    }

    // ── v24: Linux 报告模板补全设备信息字段（os_release/cpu_cores/memory_gb） ──
    if version < 24 {
        let linux_fields = serde_json::json!([
            {"key":"name","label":"设备名称","visible":true},
            {"key":"ip","label":"管理地址","visible":true},
            {"key":"vendor","label":"设备厂商","visible":true},
            {"key":"inspect_time","label":"巡检时间","visible":true},
            {"key":"os_release","label":"发行版","visible":true},
            {"key":"cpu_cores","label":"CPU 核心","visible":true},
            {"key":"memory_gb","label":"内存(GB)","visible":true},
        ]);
        // 更新已有 Linux 模板时，若已有自定义字段则保留（只补 v22 遗留的）
        for name in &["Ubuntu 服务器模板", "CentOS 服务器模板"] {
            let current: Option<String> = conn
                .query_row(
                    "SELECT config_json FROM report_templates WHERE name = ?1 AND config_json NOT LIKE '%os_release%'",
                    rusqlite::params![name],
                    |r| r.get(0),
                )
                .ok();
            if let Some(cfg_str) = current {
                if let Ok(mut cfg) = serde_json::from_str::<serde_json::Value>(&cfg_str) {
                    cfg["device_info"]["fields"] = linux_fields.clone();
                    let new_str = serde_json::to_string(&cfg).unwrap_or_default();
                    conn.execute(
                        "UPDATE report_templates SET config_json = ?1 WHERE name = ?2",
                        rusqlite::params![new_str, name],
                    )?;
                    tracing::info!("migration 24: 模板 '{}' 已补全 Linux 信息字段", name);
                }
            }
        }

        conn.execute_batch("PRAGMA user_version = 24;")
            .map_err(|e| format!("migration 24: {}", e))?;
    }

    // ── v25: 补全所有 Linux 发行版模板的缺失字段 ──
    if version < 25 {
        let linux_vendors = ["Linux","Ubuntu","CentOS","Rocky","Debian","RHEL","SUSE","Fedora","AlmaLinux"];
        let server_fields: Vec<(&str, &str)> = vec![
            ("os_release", "发行版"),
            ("kernel_version", "内核版本"),
            ("cpu_cores",  "CPU 核心"),
            ("memory_gb",  "内存(GB)"),
            ("model",      "设备型号"),
            ("sn",         "序列号"),
            ("mfg_date",   "出厂日期"),
            ("hostname",   "主机名"),
        ];

        for vendor in &linux_vendors {
            // 找出该厂商下缺少服务器字段的模板
            let mut stmt = conn
                .prepare("SELECT id, config_json FROM report_templates WHERE vendor = ?1 AND config_json NOT LIKE '%os_release%'")
                .map_err(|e| format!("v25 prepare: {}", e))?;
            let rows: Vec<(i64, String)> = stmt
                .query_map(rusqlite::params![vendor], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(|e| format!("v25 query: {}", e))?
                .filter_map(|r| r.ok())
                .collect();

            for (id, cfg_str) in rows {
                if let Ok(mut cfg) = serde_json::from_str::<serde_json::Value>(&cfg_str) {
                    if let Some(fields) = cfg["device_info"]["fields"].as_array_mut() {
                        let existing_keys: Vec<String> = fields
                            .iter()
                            .filter_map(|f| f["key"].as_str().map(String::from))
                            .collect();
                        for (key, label) in &server_fields {
                            if !existing_keys.iter().any(|k| k == key) {
                                fields.push(serde_json::json!({"key": key, "label": label, "visible": false}));
                            }
                        }
                        let new_str = serde_json::to_string(&cfg).unwrap_or_default();
                        conn.execute("UPDATE report_templates SET config_json = ?1 WHERE id = ?2",
                            rusqlite::params![new_str, id])?;
                        tracing::info!("v25: 模板 id={} vendor={} 补全服务器字段", id, vendor);
                    }
                }
            }
        }

        // 同样处理数据库模板：补全宿主机物理机字段
        let db_vendors = ["MySQL","PostgreSQL","Oracle","SQL Server","达梦","Redis","MongoDB"];
        let db_host_fields: Vec<(&str, &str)> = vec![
            ("model",      "宿主机 型号"),
            ("sn",         "宿主机 序列号"),
            ("mfg_date",   "宿主机 出厂日期"),
            ("sysname",    "宿主机 主机名"),
        ];
        for vendor in &db_vendors {
            let mut stmt = conn
                .prepare("SELECT id, config_json FROM report_templates WHERE vendor = ?1 AND config_json NOT LIKE '%宿主机 型号%'")
                .map_err(|e| format!("v25 db prepare: {}", e))?;
            let rows: Vec<(i64, String)> = stmt
                .query_map(rusqlite::params![vendor], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(|e| format!("v25 db query: {}", e))?
                .filter_map(|r| r.ok())
                .collect();
            for (id, cfg_str) in rows {
                if let Ok(mut cfg) = serde_json::from_str::<serde_json::Value>(&cfg_str) {
                    if let Some(fields) = cfg["device_info"]["fields"].as_array_mut() {
                        let existing_keys: Vec<String> = fields
                            .iter()
                            .filter_map(|f| f["key"].as_str().map(String::from))
                            .collect();
                        for (key, label) in &db_host_fields {
                            if !existing_keys.iter().any(|k| k == key) {
                                fields.push(serde_json::json!({"key": key, "label": label, "visible": false}));
                            }
                        }
                        let new_str = serde_json::to_string(&cfg).unwrap_or_default();
                        conn.execute("UPDATE report_templates SET config_json = ?1 WHERE id = ?2",
                            rusqlite::params![new_str, id])?;
                        tracing::info!("v25: 数据库模板 id={} 补全宿主机物理字段", id);
                    }
                }
            }
        }

        conn.execute_batch("PRAGMA user_version = 25;")
            .map_err(|e| format!("migration 25: {}", e))?;
    }

    // ── v26: devices 增加 deployment 字段（数据库部署方式） ──
    if version < 26 {
        let has: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('devices') WHERE name = 'deployment'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .map(|c| c > 0)
            .unwrap_or(false);
        if !has {
            conn.execute_batch("ALTER TABLE devices ADD COLUMN deployment TEXT DEFAULT '';")
                .map_err(|e| format!("migration 26: {}", e))?;
        }
        conn.execute_batch("PRAGMA user_version = 26;")
            .map_err(|e| format!("migration 26: {}", e))?;
    }

    // ── v27: devices 增加数据库专属字段 ──
    if version < 27 {
        for (col, ty) in &[
            ("db_version", "TEXT DEFAULT ''"),
            ("instance_name", "TEXT DEFAULT ''"),
            ("db_username", "TEXT DEFAULT ''"),
            ("db_password_encrypted", "TEXT DEFAULT ''"),
        ] {
            let has: bool = conn
                .prepare(&format!("SELECT COUNT(*) FROM pragma_table_info('devices') WHERE name = '{}'", col))
                .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
                .map(|c| c > 0)
                .unwrap_or(false);
            if !has {
                conn.execute_batch(&format!("ALTER TABLE devices ADD COLUMN {} {};", col, ty))
                    .map_err(|e| format!("migration 27: {}", e))?;
            }
        }
        conn.execute_batch("PRAGMA user_version = 27;")
            .map_err(|e| format!("migration 27: {}", e))?;
    }

    // ── v28: devices 增加 db_port 字段 ──
    if version < 28 {
        let has: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('devices') WHERE name = 'db_port'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .map(|c| c > 0)
            .unwrap_or(false);
        if !has {
            conn.execute_batch("ALTER TABLE devices ADD COLUMN db_port INTEGER DEFAULT 3306;")
                .map_err(|e| format!("migration 28: {}", e))?;
        }
        conn.execute_batch("PRAGMA user_version = 28;")
            .map_err(|e| format!("migration 28: {}", e))?;
    }

    // ── v29: 跳过（ip UNIQUE 约束保留，按 device_type 唯一由应用层 check_unique 保证）──
    if version < 29 {
        conn.execute_batch("PRAGMA user_version = 29;")
            .map_err(|e| format!("migration 29: {}", e))?;
    }

    // ── v30: devices 增加 kernel_version 字段 ──
    if version < 30 {
        let has: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('devices') WHERE name = 'kernel_version'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .map(|c| c > 0)
            .unwrap_or(false);
        if !has {
            conn.execute_batch("ALTER TABLE devices ADD COLUMN kernel_version TEXT DEFAULT '';")
                .map_err(|e| format!("migration 30: {}", e))?;
        }
        conn.execute_batch("PRAGMA user_version = 30;")
            .map_err(|e| format!("migration 30: {}", e))?;
    }

    // ── v31: 移除 devices.ip 的 UNIQUE 约束（允许同 IP 不同设备类型）──
    if version < 31 {
        // 检查当前 ip 列是否仍有 UNIQUE 约束（v29 跳过后需要此修复）
        let create_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='devices'",
                [],
                |row| row.get(0),
            )
            .unwrap_or_default();

        let has_ip_unique = create_sql.contains("ip TEXT NOT NULL UNIQUE");

        if has_ip_unique {
            // 事务内重建，避免 devices_old 残留问题
            conn.execute_batch(
                "BEGIN TRANSACTION;
                 CREATE TABLE devices_new (
                     id                    INTEGER PRIMARY KEY AUTOINCREMENT,
                     name                  TEXT NOT NULL UNIQUE,
                     ip                    TEXT NOT NULL,
                     device_type           TEXT NOT NULL,
                     vendor                TEXT NOT NULL,
                     model                 TEXT,
                     ssh_username          TEXT,
                     ssh_password_encrypted TEXT,
                     ssh_port              INTEGER NOT NULL DEFAULT 22,
                     template_id           INTEGER REFERENCES inspection_templates(id),
                     status                TEXT NOT NULL DEFAULT 'unknown'
                                           CHECK(status IN ('online','offline','unknown')),
                     last_checked_at       TEXT,
                     serial_number         TEXT,
                     manufacturing_date    TEXT,
                     sysname               TEXT,
                     cpu_cores             INTEGER,
                     memory_gb             REAL,
                     auth_status           TEXT,
                     auth_message          TEXT,
                     deployment            TEXT DEFAULT '',
                     db_version            TEXT DEFAULT '',
                     instance_name         TEXT DEFAULT '',
                     db_username           TEXT DEFAULT '',
                     db_password_encrypted TEXT DEFAULT '',
                     db_port               INTEGER DEFAULT 3306,
                     kernel_version        TEXT DEFAULT '',
                     created_at            TEXT NOT NULL DEFAULT (datetime('now')),
                     updated_at            TEXT NOT NULL DEFAULT (datetime('now'))
                 );
                 INSERT INTO devices_new (id, name, ip, device_type, vendor, model, ssh_username, ssh_password_encrypted, ssh_port, template_id, status, last_checked_at, serial_number, manufacturing_date, sysname, cpu_cores, memory_gb, auth_status, auth_message, deployment, db_version, instance_name, db_username, db_password_encrypted, db_port, kernel_version, created_at, updated_at) SELECT id, name, ip, device_type, vendor, model, ssh_username, ssh_password_encrypted, ssh_port, template_id, status, last_checked_at, serial_number, manufacturing_date, sysname, cpu_cores, memory_gb, auth_status, auth_message, deployment, db_version, instance_name, db_username, db_password_encrypted, db_port, kernel_version, created_at, updated_at FROM devices;
                 DROP TABLE devices;
                 ALTER TABLE devices_new RENAME TO devices;
                 CREATE INDEX IF NOT EXISTS idx_devices_vendor      ON devices(vendor);
                 CREATE INDEX IF NOT EXISTS idx_devices_status      ON devices(status);
                 CREATE INDEX IF NOT EXISTS idx_devices_template_id ON devices(template_id);
                 COMMIT;",
            )
            .map_err(|e| format!("migration 31: {}", e))?;
        }

        conn.execute_batch("PRAGMA user_version = 31;")
            .map_err(|e| format!("migration 31: {}", e))?;
    }

    // ── v32: command_pool 增加 expectation 字段（AI 评判提示词）──
    if version < 32 {
        conn.execute_batch("ALTER TABLE command_pool ADD COLUMN expectation TEXT;")
            .map_err(|e| format!("migration 32: {}", e))?;
        conn.execute_batch("PRAGMA user_version = 32;")
            .map_err(|e| format!("migration 32: {}", e))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 全新数据库（user_version=0）跑完所有迁移不应报错。
    /// 复现 Linux 全新安装崩溃：001_init.sql 曾在 is_default 列被 v4 添加前就建索引。
    #[test]
    fn test_fresh_migrations_complete() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&mut conn).expect("fresh migrations must complete without error");

        // 校验关键列与索引确实存在
        let has_is_default: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('report_templates') WHERE name = 'is_default'")
            .and_then(|mut s| s.query_row([], |r| r.get::<_, i64>(0)))
            .map(|c| c > 0)
            .unwrap();
        assert!(has_is_default, "is_default 列应存在");

        let idx_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_report_templates_is_default'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx_count, 1, "is_default 索引应已创建");

        // 再次运行应幂等无错
        run_migrations(&mut conn).expect("re-run migrations must be idempotent");
    }

    /// 全新安装执行种子数据后，devices / command_pool 关键列值应与升级库一致。
    /// 验证 seed 的 ON CONFLICT DO UPDATE 正确写入 needs_root，不出现
    /// 开发环境旧库（INSERT OR IGNORE 跳过 → needs_root=0）与生产全新安装不一致的问题。
    #[test]
    fn test_fresh_seed_data_consistent() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&mut conn).unwrap();

        // seed 需要 APP_DATA_DIR，但这里只测 command_pool 不依赖文件系统
        crate::db::seed_data::seed_command_pool(&mut conn).unwrap();

        // 验证已知 sudo 命令的 needs_root=1
        let cases: &[(&str, &str)] = &[
            ("Linux", "fdisk -l"),
            ("Linux", "dmidecode -t system"),
            ("Linux", "iptables -L -n"),
        ];
        for (vendor, cmd) in cases {
            let needs_root: i64 = conn
                .query_row(
                    "SELECT needs_root FROM command_pool WHERE vendor=?1 AND command=?2",
                    rusqlite::params![vendor, cmd],
                    |r| r.get(0),
                )
                .unwrap_or_else(|_| panic!("命令 {}/{} 不存在", vendor, cmd));
            assert_eq!(needs_root, 1, "需要 sudo 的命令 {}/{} 的 needs_root 应为 1", vendor, cmd);
        }

        // 验证普通命令 needs_root=0
        let needs_root: i64 = conn
            .query_row(
                "SELECT needs_root FROM command_pool WHERE vendor='Linux' AND command='uname -a'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(needs_root, 0, "普通命令 uname -a 的 needs_root 应为 0");

        // 再次执行 seed 应幂等（不报错，值不变）
        crate::db::seed_data::seed_command_pool(&mut conn).unwrap();
    }
}
