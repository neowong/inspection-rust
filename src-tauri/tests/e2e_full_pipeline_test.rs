use inspection_rust_lib::AppState;

#[test]
fn test_full_inspection_pipeline() {
    // Initialize app state (this will run migrations and seed data)
    let state = AppState::new("test_inspection.db");

    println!("\n=== 1. 验证种子数据 ===");
    {
        let conn = state.db.lock();
        let cmd_count: i64 = conn.query_row("SELECT COUNT(*) FROM command_pool", [], |r| r.get(0)).unwrap();
        println!("命令库数量: {}", cmd_count);
        assert!(cmd_count >= 65, "命令库应该至少有65条种子数据");

        let h3c_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM command_pool WHERE vendor = 'H3C'",
            [],
            |r| r.get(0)
        ).unwrap();
        println!("H3C 命令数量: {}", h3c_count);
        assert!(h3c_count > 0, "应该有 H3C 命令");
    }

    println!("\n=== 2. 创建设备 ===");
    let device_id = {
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO devices (name, ip, device_type, vendor, ssh_username, ssh_password_encrypted, ssh_port, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "测试H3C交换机",
                "10.0.0.1",
                "switch",
                "H3C",
                "admin",
                inspection_rust_lib::services::crypto::CryptoService::encrypt("changeme").unwrap(),
                22,
                "online"
            ],
        ).unwrap();
        conn.last_insert_rowid()
    };
    println!("设备ID: {}", device_id);
    assert!(device_id > 0);

    println!("\n=== 3. 创建巡检模板 ===");
    let template_id = {
        let conn = state.db.lock();
        // 获取前3个 H3C 命令的 ID
        let mut stmt = conn.prepare(
            "SELECT id FROM command_pool WHERE vendor = 'H3C' ORDER BY id LIMIT 3"
        ).unwrap();
        let cmd_ids: Vec<i64> = stmt.query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(cmd_ids.len(), 3, "应该获取到3个命令ID");
        println!("命令IDs: {:?}", cmd_ids);

        let config = serde_json::json!({ "command_ids": cmd_ids }).to_string();

        conn.execute(
            "INSERT INTO inspection_templates (name, vendor, config, description)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                "H3C测试模板",
                "H3C",
                config,
                "测试用巡检模板"
            ],
        ).unwrap();

        let tid = conn.last_insert_rowid();

        // 关联模板到设备
        conn.execute(
            "UPDATE devices SET template_id = ?1 WHERE id = ?2",
            rusqlite::params![tid, device_id],
        ).unwrap();

        tid
    };
    println!("模板ID: {}", template_id);
    assert!(template_id > 0);

    println!("\n=== 4. 创建巡检批次 ===");
    let batch_id = {
        let conn = state.db.lock();
        let device_ids_json = serde_json::json!([device_id]).to_string();

        conn.execute(
            "INSERT INTO inspection_batches (name, status, triggered_by, device_ids)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                "E2E测试批次",
                "pending",
                "manual",
                device_ids_json
            ],
        ).unwrap();
        conn.last_insert_rowid()
    };
    println!("批次ID: {}", batch_id);
    assert!(batch_id > 0);

    println!("\n=== 5. 执行巡检 ===");
    {
        let conn = state.db.lock();

        // 创建巡检记录
        conn.execute(
            "INSERT INTO inspection_records (batch_id, device_id, status) VALUES (?1, ?2, ?3)",
            rusqlite::params![batch_id, device_id, "pending"],
        ).unwrap();
        let record_id = conn.last_insert_rowid();
        println!("记录ID: {}", record_id);

        // 更新批次状态为 running
        conn.execute(
            "UPDATE inspection_batches SET status = 'running', started_at = datetime('now') WHERE id = ?1",
            rusqlite::params![batch_id],
        ).unwrap();

        // 获取设备和模板信息
        let device = conn.query_row(
            "SELECT name, ip, vendor, ssh_username, ssh_password_encrypted, ssh_port, template_id FROM devices WHERE id = ?1",
            rusqlite::params![device_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i32>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            }
        ).unwrap();

        println!("设备: {} ({})", device.0, device.1);

        // 解密密码
        let password = inspection_rust_lib::services::crypto::CryptoService::decrypt(&device.4).unwrap();

        // 获取模板配置
        let config_str: String = conn.query_row(
            "SELECT config FROM inspection_templates WHERE id = ?1",
            rusqlite::params![device.6],
            |row| row.get(0)
        ).unwrap();

        let config: serde_json::Value = serde_json::from_str(&config_str).unwrap();
        let cmd_ids: Vec<i64> = config["command_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect();

        println!("命令ID列表: {:?}", cmd_ids);

        // 获取命令文本
        let mut commands = Vec::new();
        for cmd_id in &cmd_ids {
            let cmd: String = conn.query_row(
                "SELECT command FROM command_pool WHERE id = ?1",
                rusqlite::params![cmd_id],
                |row| row.get(0)
            ).unwrap();
            commands.push(cmd);
        }

        println!("执行命令: {:?}", commands);

        // 执行 SSH 命令
        let source = inspection_rust_lib::services::inspection_runner::SSHSessionSource {
            host: device.1,
            port: device.5 as u16,
            username: device.3,
            password,
        };

        let outputs = inspection_rust_lib::services::inspection_runner::run_commands_with_cancel(
            &source,
            &device.2,
            &commands,
            None,
            None,
        ).unwrap();

        println!("成功获取 {} 条命令输出", outputs.len());
        assert_eq!(outputs.len(), commands.len());

        // 保存输出到记录
        let outputs_json = serde_json::to_string(&outputs).unwrap();
        conn.execute(
            "UPDATE inspection_records SET status = 'completed', command_outputs = ?1, completed_at = datetime('now') WHERE id = ?2",
            rusqlite::params![outputs_json, record_id],
        ).unwrap();

        // 更新批次状态为 completed
        conn.execute(
            "UPDATE inspection_batches SET status = 'completed', completed_at = datetime('now') WHERE id = ?1",
            rusqlite::params![batch_id],
        ).unwrap();

        println!("\n=== 6. 验证结果 ===");
        let final_status: String = conn.query_row(
            "SELECT status FROM inspection_batches WHERE id = ?1",
            rusqlite::params![batch_id],
            |row| row.get(0)
        ).unwrap();
        println!("批次最终状态: {}", final_status);
        assert_eq!(final_status, "completed");

        let record_status: String = conn.query_row(
            "SELECT status FROM inspection_records WHERE id = ?1",
            rusqlite::params![record_id],
            |row| row.get(0)
        ).unwrap();
        println!("记录状态: {}", record_status);
        assert_eq!(record_status, "completed");

        let output_len: i64 = conn.query_row(
            "SELECT LENGTH(command_outputs) FROM inspection_records WHERE id = ?1",
            rusqlite::params![record_id],
            |row| row.get(0)
        ).unwrap();
        println!("输出数据长度: {} bytes", output_len);
        assert!(output_len > 100, "输出数据应该足够长");
    }

    println!("\n=== ✅ 完整巡检流程测试通过 ===");

    // 清理测试数据库
    std::fs::remove_file("test_inspection.db").ok();
    std::fs::remove_file("test_inspection.db-shm").ok();
    std::fs::remove_file("test_inspection.db-wal").ok();
}
