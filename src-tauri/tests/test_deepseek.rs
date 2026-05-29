// Test DeepSeek API integration and full inspection pipeline
use inspection_rust_lib::{AppState, services::crypto::CryptoService};
use rusqlite::params;

#[test]
fn test_deepseek_configuration() {
    // Initialize app state (this will run migrations including 002)
    let state = AppState::new("test_deepseek.db");

    println!("\n=== 1. 验证 DeepSeek 迁移 ===");
    {
        let conn = state.db.lock();
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        println!("数据库版本: {}", version);
        assert_eq!(version, 2, "数据库应该升级到版本 2");

        // 验证 provider 约束
        let check_sql = "SELECT sql FROM sqlite_master WHERE type='table' AND name='ai_model_configs'";
        let schema: String = conn.query_row(check_sql, [], |r| r.get(0)).unwrap();
        println!("Schema: {}", schema);
        assert!(schema.contains("'deepseek'"), "Schema 应该包含 deepseek provider");
    }

    println!("\n=== 2. 配置 DeepSeek API ===");
    let api_key = "sk-33078a3ec9bb48df8cd984c11424556b";
    let model_id = "deepseek-chat";

    {
        let conn = state.db.lock();

        // 先停用所有现有配置
        conn.execute("UPDATE ai_model_configs SET is_active = 0", []).unwrap();

        // 创建 DeepSeek 配置
        let encrypted_key = CryptoService::encrypt(api_key).unwrap();

        conn.execute(
            "INSERT INTO ai_model_configs (name, provider, model_id, api_key_encrypted, base_url, is_active)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "DeepSeek Chat",
                "deepseek",
                model_id,
                encrypted_key,
                "https://api.deepseek.com",
                1  // is_active
            ],
        ).unwrap();

        let config_id = conn.last_insert_rowid();
        println!("DeepSeek 配置 ID: {}", config_id);

        // 验证配置
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM ai_model_configs WHERE provider = 'deepseek' AND is_active = 1",
            [],
            |r| r.get(0)
        ).unwrap();
        println!("激活的 DeepSeek 配置数量: {}", count);
        assert_eq!(count, 1);
    }

    println!("\n=== 3. 测试 DeepSeek API 调用 ===");
    {
        let conn = state.db.lock();

        // 获取激活的配置
        let config = conn.query_row(
            "SELECT model_id, api_key_encrypted, base_url FROM ai_model_configs WHERE is_active = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            }
        ).unwrap();

        let decrypted_key = CryptoService::decrypt(&config.1).unwrap();
        println!("模型: {}", config.0);
        println!("Base URL: {}", config.2);
        println!("API Key (前10字符): {}...", &decrypted_key[..10]);

        // 创建测试命令输出
        let mut test_outputs = std::collections::HashMap::new();
        test_outputs.insert(
            "display version".to_string(),
            "H3C Comware Software, Version 7.1.070, Release 6328P03\nH3C S5130S-28S-HPWR-EI uptime is 210 weeks".to_string()
        );
        test_outputs.insert(
            "display cpu-usage".to_string(),
            "Slot 1 CPU 0 CPU usage:\n      61% in last 5 seconds\n      29% in last 1 minute\n      25% in last 5 minutes".to_string()
        );

        println!("\n调用 DeepSeek API 分析...");

        // 使用 tokio runtime 调用 async 函数
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            inspection_rust_lib::services::ai_inspection::analyze_with_openai(
                &decrypted_key,
                &config.0,
                &config.2,
                &test_outputs,
            ).await
        });

        match result {
            Ok(analysis) => {
                println!("✅ DeepSeek API 调用成功!");
                println!("分析结果:\n{}", serde_json::to_string_pretty(&analysis).unwrap());

                // 验证结果结构
                assert!(analysis.get("summary").is_some(), "应该包含 summary");
                assert!(analysis.get("overall").is_some(), "应该包含 overall");
                assert!(analysis.get("items").is_some(), "应该包含 items");
            }
            Err(e) => {
                panic!("DeepSeek API 调用失败: {}", e);
            }
        }
    }

    println!("\n=== ✅ DeepSeek 配置和 API 测试通过 ===");

    // 清理
    std::fs::remove_file("test_deepseek.db").ok();
    std::fs::remove_file("test_deepseek.db-shm").ok();
    std::fs::remove_file("test_deepseek.db-wal").ok();
}

#[test]
fn test_full_pipeline_with_deepseek() {
    // 完整的端到端测试：设备 → 模板 → 批次 → 巡检 → AI分析 → 报告
    let state = AppState::new("test_full_deepseek.db");

    println!("\n=== 完整巡检流程测试 (DeepSeek) ===\n");

    // 1. 配置 DeepSeek
    let api_key = "sk-33078a3ec9bb48df8cd984c11424556b";
    {
        let conn = state.db.lock();
        conn.execute("UPDATE ai_model_configs SET is_active = 0", []).unwrap();
        let encrypted_key = CryptoService::encrypt(api_key).unwrap();
        conn.execute(
            "INSERT INTO ai_model_configs (name, provider, model_id, api_key_encrypted, base_url, is_active)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params!["DeepSeek", "deepseek", "deepseek-chat", encrypted_key, "https://api.deepseek.com", 1],
        ).unwrap();
    }
    println!("✅ DeepSeek API 已配置");

    // 2. 创建设备
    let device_id = {
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO devices (name, ip, device_type, vendor, ssh_username, ssh_password_encrypted, ssh_port, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                "H3C-Test-Switch",
                "192.168.9.254",
                "switch",
                "H3C",
                "admin",
                CryptoService::encrypt("Ahope@2021").unwrap(),
                22,
                "online"
            ],
        ).unwrap();
        conn.last_insert_rowid()
    };
    println!("✅ 设备已创建 (ID: {})", device_id);

    // 3. 创建模板
    let template_id = {
        let conn = state.db.lock();
        let mut stmt = conn.prepare("SELECT id FROM command_pool WHERE vendor = 'H3C' LIMIT 3").unwrap();
        let cmd_ids: Vec<i64> = stmt.query_map([], |row| row.get(0)).unwrap().filter_map(|r| r.ok()).collect();

        let config = serde_json::json!({ "command_ids": cmd_ids }).to_string();
        conn.execute(
            "INSERT INTO inspection_templates (name, vendor, config) VALUES (?1, ?2, ?3)",
            params!["H3C-Template", "H3C", config],
        ).unwrap();
        let tid = conn.last_insert_rowid();
        conn.execute("UPDATE devices SET template_id = ?1 WHERE id = ?2", params![tid, device_id]).unwrap();
        tid
    };
    println!("✅ 模板已创建 (ID: {})", template_id);

    // 4. 创建批次
    let batch_id = {
        let conn = state.db.lock();
        let device_ids = serde_json::json!([device_id]).to_string();
        conn.execute(
            "INSERT INTO inspection_batches (name, status, device_ids) VALUES (?1, ?2, ?3)",
            params!["DeepSeek-Test-Batch", "pending", device_ids],
        ).unwrap();
        conn.last_insert_rowid()
    };
    println!("✅ 批次已创建 (ID: {})", batch_id);

    // 5. 执行巡检
    let record_id = {
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO inspection_records (batch_id, device_id, status) VALUES (?1, ?2, ?3)",
            params![batch_id, device_id, "pending"],
        ).unwrap();
        let rid = conn.last_insert_rowid();

        // 获取设备信息
        let device = conn.query_row(
            "SELECT ip, vendor, ssh_username, ssh_password_encrypted, ssh_port, template_id FROM devices WHERE id = ?1",
            params![device_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i32>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            }
        ).unwrap();

        let password = CryptoService::decrypt(&device.3).unwrap();

        // 获取命令
        let config_str: String = conn.query_row(
            "SELECT config FROM inspection_templates WHERE id = ?1",
            params![device.5],
            |row| row.get(0)
        ).unwrap();
        let config: serde_json::Value = serde_json::from_str(&config_str).unwrap();
        let cmd_ids: Vec<i64> = config["command_ids"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();

        let mut commands = Vec::new();
        for cmd_id in &cmd_ids {
            let cmd: String = conn.query_row("SELECT command FROM command_pool WHERE id = ?1", params![cmd_id], |row| row.get(0)).unwrap();
            commands.push(cmd);
        }

        // 执行 SSH
        conn.execute("UPDATE inspection_batches SET status = 'running' WHERE id = ?1", params![batch_id]).unwrap();
        conn.execute("UPDATE inspection_records SET status = 'running' WHERE id = ?1", params![rid]).unwrap();

        let source = inspection_rust_lib::services::inspection_runner::SSHSessionSource {
            host: device.0,
            port: device.4 as u16,
            username: device.2,
            password,
        };

        let outputs = inspection_rust_lib::services::inspection_runner::run_commands(
            &source,
            &device.1,
            &commands,
        ).unwrap();

        let outputs_json = serde_json::to_string(&outputs).unwrap();
        conn.execute(
            "UPDATE inspection_records SET status = 'completed', command_outputs = ?1 WHERE id = ?2",
            params![outputs_json, rid],
        ).unwrap();
        conn.execute("UPDATE inspection_batches SET status = 'completed' WHERE id = ?1", params![batch_id]).unwrap();

        println!("✅ 巡检完成，获取 {} 条命令输出", outputs.len());
        rid
    };

    // 6. AI 分析
    println!("\n🤖 调用 DeepSeek 进行 AI 分析...");
    {
        let conn = state.db.lock();

        // 获取命令输出
        let outputs_json: String = conn.query_row(
            "SELECT command_outputs FROM inspection_records WHERE id = ?1",
            params![record_id],
            |row| row.get(0)
        ).unwrap();
        let outputs: std::collections::HashMap<String, String> = serde_json::from_str(&outputs_json).unwrap();

        // 获取 AI 配置
        let config = conn.query_row(
            "SELECT model_id, api_key_encrypted, base_url FROM ai_model_configs WHERE is_active = 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        ).unwrap();
        let api_key = CryptoService::decrypt(&config.1).unwrap();

        conn.execute("UPDATE inspection_records SET ai_status = 'processing' WHERE id = ?1", params![record_id]).unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let analysis = rt.block_on(async {
            inspection_rust_lib::services::ai_inspection::analyze_with_openai(
                &api_key,
                &config.0,
                &config.2,
                &outputs,
            ).await
        }).unwrap();

        let analysis_json = serde_json::to_string(&analysis).unwrap();

        // Transform AI result into command_judgments and summary
        let summary = analysis.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string();

        let mut judgments = serde_json::Map::new();
        if let Some(items) = analysis.get("items").and_then(|v| v.as_array()) {
            for item in items {
                if let Some(command) = item.get("command").and_then(|v| v.as_str()) {
                    let judgment = serde_json::json!({
                        "status": item.get("status").and_then(|v| v.as_str()).unwrap_or("unknown"),
                        "finding": item.get("finding").and_then(|v| v.as_str()).unwrap_or(""),
                        "suggestion": item.get("suggestion").and_then(|v| v.as_str()).unwrap_or(""),
                    });
                    judgments.insert(command.to_string(), judgment);
                }
            }
        }
        let judgments_json = serde_json::to_string(&serde_json::Value::Object(judgments)).unwrap();

        conn.execute(
            "UPDATE inspection_records SET ai_status = 'completed', ai_result = ?1, command_judgments = ?2, summary_judgment = ?3 WHERE id = ?4",
            params![analysis_json, judgments_json, summary, record_id],
        ).unwrap();

        println!("✅ AI 分析完成");
        println!("Summary: {}", analysis["summary"].as_str().unwrap_or("N/A"));
        println!("Overall: {}", analysis["overall"].as_str().unwrap_or("N/A"));
    }

    // 7. 生成报告
    let report_path = {
        let conn = state.db.lock();
        let record = conn.query_row(
            "SELECT id, batch_id, device_id, status, error_message, command_outputs, ai_status, ai_result, ai_analysis, ai_suggestions, command_judgments, summary_judgment, report_path, started_at, completed_at, created_at, updated_at FROM inspection_records WHERE id = ?1",
            params![record_id],
            |row| {
                Ok(inspection_rust_lib::db::models::InspectionRecord {
                    id: row.get(0)?,
                    batch_id: row.get(1)?,
                    device_id: row.get(2)?,
                    status: row.get(3)?,
                    error_message: row.get(4)?,
                    command_outputs: row.get(5)?,
                    ai_status: row.get(6)?,
                    ai_result: row.get(7)?,
                    ai_analysis: row.get(8)?,
                    ai_suggestions: row.get(9)?,
                    command_judgments: row.get(10)?,
                    summary_judgment: row.get(11)?,
                    report_path: row.get(12)?,
                    started_at: row.get(13)?,
                    completed_at: row.get(14)?,
                    created_at: row.get(15)?,
                    updated_at: row.get(16)?,
                })
            }
        ).unwrap();

        let device = conn.query_row(
            "SELECT name, ip, vendor, model FROM devices WHERE id = ?1",
            params![record.device_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, Option<String>>(3)?))
        ).unwrap();

        let mut context = std::collections::HashMap::new();
        context.insert("device_name".to_string(), serde_json::Value::String(device.0));
        context.insert("device_ip".to_string(), serde_json::Value::String(device.1));
        context.insert("vendor".to_string(), serde_json::Value::String(device.2));
        if let Some(model) = device.3 {
            context.insert("model".to_string(), serde_json::Value::String(model));
        }

        if let Some(outputs) = &record.command_outputs {
            if let Ok(outputs_val) = serde_json::from_str::<serde_json::Value>(outputs) {
                context.insert("command_outputs".to_string(), outputs_val);
            }
        }

        if let Some(ai_result) = &record.ai_result {
            if let Ok(ai_val) = serde_json::from_str::<serde_json::Value>(ai_result) {
                // Extract summary
                if let Some(summary) = ai_val.get("summary").and_then(|v| v.as_str()) {
                    context.insert("summary".to_string(), serde_json::Value::String(summary.to_string()));
                }

                // Transform items array into command_judgments map
                if let Some(items) = ai_val.get("items").and_then(|v| v.as_array()) {
                    let mut judgments = serde_json::Map::new();
                    for item in items {
                        if let Some(command) = item.get("command").and_then(|v| v.as_str()) {
                            let judgment = serde_json::json!({
                                "status": item.get("status").and_then(|v| v.as_str()).unwrap_or("unknown"),
                                "finding": item.get("finding").and_then(|v| v.as_str()).unwrap_or(""),
                                "suggestion": item.get("suggestion").and_then(|v| v.as_str()).unwrap_or(""),
                            });
                            judgments.insert(command.to_string(), judgment);
                        }
                    }
                    context.insert("command_judgments".to_string(), serde_json::Value::Object(judgments));
                }
            }
        }

        let report_content = inspection_rust_lib::services::report_generator::build_markdown(&context);
        let report_path = format!("data/reports/report_{}.md", record_id);
        std::fs::create_dir_all("data/reports").ok();
        std::fs::write(&report_path, &report_content).unwrap();

        conn.execute("UPDATE inspection_records SET report_path = ?1 WHERE id = ?2", params![report_path, record_id]).unwrap();

        println!("✅ 报告已生成: {}", report_path);
        report_path
    };

    // 8. 验证最终状态
    {
        let conn = state.db.lock();
        let batch_status: String = conn.query_row("SELECT status FROM inspection_batches WHERE id = ?1", params![batch_id], |row| row.get(0)).unwrap();
        let record_status: String = conn.query_row("SELECT status FROM inspection_records WHERE id = ?1", params![record_id], |row| row.get(0)).unwrap();
        let ai_status: String = conn.query_row("SELECT ai_status FROM inspection_records WHERE id = ?1", params![record_id], |row| row.get(0)).unwrap();
        let has_report: bool = conn.query_row("SELECT report_path IS NOT NULL FROM inspection_records WHERE id = ?1", params![record_id], |row| row.get(0)).unwrap();

        println!("\n=== 最终状态验证 ===");
        println!("批次状态: {}", batch_status);
        println!("记录状态: {}", record_status);
        println!("AI 状态: {}", ai_status);
        println!("报告: {}", if has_report { "✅ 已生成" } else { "❌ 未生成" });

        assert_eq!(batch_status, "completed");
        assert_eq!(record_status, "completed");
        assert_eq!(ai_status, "completed");
        assert!(has_report);
    }

    println!("\n=== ✅ 完整巡检流程测试通过 (DeepSeek) ===");
    println!("报告位置: {}", report_path);

    // 清理
    std::fs::remove_file("test_full_deepseek.db").ok();
    std::fs::remove_file("test_full_deepseek.db-shm").ok();
    std::fs::remove_file("test_full_deepseek.db-wal").ok();
}
