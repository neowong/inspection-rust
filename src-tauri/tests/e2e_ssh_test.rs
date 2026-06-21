use inspection_rust_lib::services::inspection_runner::{SSHSessionSource, run_commands_with_cancel};

#[test]
fn test_ssh_inspection_commands() {
    let source = SSHSessionSource {
        host: "10.0.0.1".to_string(),
        port: 22,
        username: "admin".to_string(),
        password: "changeme".to_string(),
    };

    let commands = vec![
        "display version".to_string(),
        "display cpu-usage".to_string(),
        "display memory".to_string(),
    ];

    println!("\n=== 测试多条命令 ===");
    match run_commands_with_cancel(&source, "H3C", &commands, None, None) {
        Ok(outputs) => {
            println!("成功! 获取 {} 条输出\n", outputs.len());
            assert_eq!(outputs.len(), commands.len());

            for cmd in &commands {
                if let Some(output) = outputs.get(cmd) {
                    println!("--- {} ---", cmd);
                    let preview: String = output.chars().take(200).collect();
                    println!("{}...\n", preview);
                    assert!(!output.is_empty());
                } else {
                    panic!("缺少命令 {} 的输出", cmd);
                }
            }
        }
        Err(e) => {
            panic!("失败: {:?}", e);
        }
    }
}

#[test]
fn test_ssh_single_command() {
    let source = SSHSessionSource {
        host: "10.0.0.1".to_string(),
        port: 22,
        username: "admin".to_string(),
        password: "changeme".to_string(),
    };

    let commands = vec!["display version".to_string()];

    println!("\n=== 测试单条命令 ===");
    match run_commands_with_cancel(&source, "H3C", &commands, None, None) {
        Ok(outputs) => {
            assert_eq!(outputs.len(), 1);
            println!("成功!");
        }
        Err(e) => {
            panic!("失败: {:?}", e);
        }
    }
}
