use tauri::State;
use tracing::{info, warn, debug};
use crate::AppState;

// ============================================================
// 配置文件注释清理
// ============================================================

/// 支持的配置文件类型
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfigFileType {
    pub id: String,
    pub name: String,
    pub extensions: Vec<String>,
    pub comment_patterns: Vec<String>,
}

/// 预定义的配置文件类型
fn get_config_types() -> Vec<ConfigFileType> {
    vec![
        ConfigFileType {
            id: "nginx".into(),
            name: "Nginx".into(),
            extensions: vec!["conf".into()],
            comment_patterns: vec!["#".into()],
        },
        ConfigFileType {
            id: "apache".into(),
            name: "Apache".into(),
            extensions: vec!["conf".into(), "htaccess".into()],
            comment_patterns: vec!["#".into()],
        },
        ConfigFileType {
            id: "zabbix".into(),
            name: "Zabbix".into(),
            extensions: vec!["conf".into()],
            comment_patterns: vec!["#".into()],
        },
        ConfigFileType {
            id: "mysql".into(),
            name: "MySQL".into(),
            extensions: vec!["cnf".into(), "ini".into()],
            comment_patterns: vec!["#".into(), ";".into()],
        },
        ConfigFileType {
            id: "redis".into(),
            name: "Redis".into(),
            extensions: vec!["conf".into()],
            comment_patterns: vec!["#".into()],
        },
        ConfigFileType {
            id: "syslog".into(),
            name: "Syslog/rsyslog".into(),
            extensions: vec!["conf".into()],
            comment_patterns: vec!["#".into()],
        },
        ConfigFileType {
            id: "ssh".into(),
            name: "OpenSSH".into(),
            extensions: vec![],
            comment_patterns: vec!["#".into()],
        },
        ConfigFileType {
            id: "iptables".into(),
            name: "iptables/nftables".into(),
            extensions: vec![],
            comment_patterns: vec!["#".into()],
        },
        ConfigFileType {
            id: "docker".into(),
            name: "Docker Compose".into(),
            extensions: vec!["yml".into(), "yaml".into()],
            comment_patterns: vec!["#".into()],
        },
        ConfigFileType {
            id: "systemd".into(),
            name: "Systemd Service".into(),
            extensions: vec!["service".into(), "timer".into()],
            comment_patterns: vec!["#".into(), ";".into()],
        },
        ConfigFileType {
            id: "generic".into(),
            name: "通用配置".into(),
            extensions: vec![],
            comment_patterns: vec!["#".into(), "//".into(), ";".into()],
        },
    ]
}

/// 去除配置文件注释
fn strip_comments(content: &str, comment_patterns: &[String], keep_empty_lines: bool) -> (String, usize, usize) {
    let original_lines = content.lines().count();
    let mut cleaned_lines = Vec::new();
    let mut removed_count = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        // 检查是否是注释行
        let is_comment = comment_patterns.iter().any(|p| trimmed.starts_with(p));

        if is_comment {
            removed_count += 1;
            continue;
        }

        // 处理行内注释（保留代码部分，去除注释部分）
        let mut clean_line = line.to_string();
        for pattern in comment_patterns {
            if let Some(pos) = clean_line.find(pattern.as_str()) {
                // 确保注释符号不在引号内
                let before = &clean_line[..pos];
                let single_quotes = before.matches('\'').count();
                let double_quotes = before.matches('"').count();
                if single_quotes % 2 == 0 && double_quotes % 2 == 0 {
                    clean_line = clean_line[..pos].trim_end().to_string();
                }
            }
        }

        // 保留空行（可选）
        if !keep_empty_lines && clean_line.trim().is_empty() {
            continue;
        }

        cleaned_lines.push(clean_line);
    }

    let cleaned = cleaned_lines.join("\n");
    (cleaned, original_lines, removed_count)
}

// ============================================================
// Tauri Commands
// ============================================================

/// 获取支持的配置文件类型列表
#[tauri::command]
pub fn get_config_file_types() -> Vec<ConfigFileType> {
    get_config_types()
}

/// 清理配置文件内容（去除注释）
/// 返回清理后的内容、原始行数、去除行数
#[tauri::command]
pub fn clean_config_content(
    content: String,
    config_type: String,
    keep_empty_lines: Option<bool>,
) -> Result<serde_json::Value, String> {
    let types = get_config_types();
    let config = types.iter().find(|t| t.id == config_type)
        .unwrap_or_else(|| types.iter().find(|t| t.id == "generic").unwrap());

    let keep_empty = keep_empty_lines.unwrap_or(false);
    let (cleaned, original_lines, removed_lines) = strip_comments(&content, &config.comment_patterns, keep_empty);

    Ok(serde_json::json!({
        "cleaned": cleaned,
        "original_lines": original_lines,
        "removed_lines": removed_lines,
        "remaining_lines": original_lines - removed_lines,
        "reduction_percent": if original_lines > 0 {
            (removed_lines as f64 / original_lines as f64 * 100.0).round() as i64
        } else {
            0
        }
    }))
}

/// AI 分析配置文件
#[tauri::command]
pub async fn analyze_config(
    content: String,
    config_type: String,
    filename: Option<String>,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("配置分析开始: config_type={}, filename={:?}, content_size={}", config_type, filename, content.len());

    // 1. 清理注释
    let types = get_config_types();
    let config = types.iter().find(|t| t.id == config_type)
        .unwrap_or_else(|| types.iter().find(|t| t.id == "generic").unwrap());

    let (cleaned, original_lines, removed_lines) = strip_comments(&content, &config.comment_patterns, false);

    if cleaned.trim().is_empty() {
        return Err("配置文件内容为空（清理注释后）".to_string());
    }

    // 2. 检查内容大小（限制 50KB）
    if cleaned.len() > 50 * 1024 {
        return Err(format!("清理后配置文件过大 ({}KB)，请缩减内容后重试", cleaned.len() / 1024));
    }

    // 3. 获取 AI 配置
    let (provider, model, api_key, base_url) = {
        let conn = state.db.lock();
        let config = crate::db::query::query_one(
            &conn,
            "SELECT id, name, provider, model_id, api_key_encrypted, base_url, \
             is_active, created_at, updated_at \
             FROM ai_model_configs WHERE is_active = 1 LIMIT 1",
            &[],
            |row| {
                Ok(crate::db::models::AiModelConfig {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    provider: row.get(2)?,
                    model_id: row.get(3)?,
                    api_key_encrypted: row.get(4)?,
                    base_url: row.get(5)?,
                    is_active: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        )?
        .ok_or_else(|| "未找到激活的 AI 配置，请先在设置页面配置 AI 模型".to_string())?;

        let decrypted_key = crate::services::crypto::CryptoService::decrypt(&config.api_key_encrypted)?;
        (
            config.provider,
            config.model_id,
            decrypted_key,
            config.base_url.unwrap_or_default(),
        )
    };

    // 4. 构建提示词
    let filename_info = filename.map(|f| format!("文件名: {}", f)).unwrap_or_default();
    let prompt = format!(
        "你是一位资深的 Linux/网络运维工程师。请分析以下 {} 配置文件，找出潜在问题并给出优化建议。

{}

【配置文件内容】（已去除注释，共 {} 行，原始 {} 行，已去除 {} 行注释）
```
{}
```

请从以下维度分析：
1. 【安全问题】权限、认证、加密、暴露面等安全隐患
2. 【性能问题】可能导致性能瓶颈的配置
3. 【兼容性问题】版本兼容、废弃语法等
4. 【最佳实践】不符合行业最佳实践的配置
5. 【潜在风险】可能导致服务异常的配置

请用 JSON 格式输出：
```json
{{
  \"summary\": \"配置文件总体评估（一句话）\",
  \"risk_level\": \"low|medium|high|critical\",
  \"issues\": [
    {{
      \"category\": \"security|performance|compatibility|best_practice|risk\",
      \"severity\": \"info|warning|critical\",
      \"line_hint\": \"相关配置项名称或行号\",
      \"description\": \"问题描述\",
      \"suggestion\": \"修复建议\"
    }}
  ],
  \"optimizations\": [\"优化建议1\", \"优化建议2\"]
}}
```",
        config.name,
        filename_info,
        cleaned.lines().count(),
        original_lines,
        removed_lines,
        cleaned,
    );

    // 5. 调用 AI API
    info!("调用 AI 分析配置: provider={}, model={}", provider, model);
    let base_url = if base_url.is_empty() {
        match provider.as_str() {
            "deepseek" => "https://api.deepseek.com".to_string(),
            _ => "https://api.openai.com".to_string(),
        }
    } else {
        base_url
    };

    let url = crate::services::ai_inspection::build_chat_url(&base_url);
    let client = crate::services::ai_inspection::get_client();

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "你是一位资深的 Linux/网络运维工程师，擅长分析各种服务配置文件。请用中文回答。"},
            {"role": "user", "content": &prompt}
        ],
        "temperature": 0.2,
        "max_tokens": 4096
    });

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("AI 请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!("AI 配置分析失败: status={}, body={}", status, &text[..text.len().min(500)]);
        return Err(format!("AI API 返回错误 {}: {}", status, text));
    }

    let result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析 AI 响应失败: {}", e))?;

    let content = result
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|msg| msg.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    // 6. 解析 AI 响应
    // 尝试从响应中提取 JSON
    let analysis = extract_json_from_response(&content);

    let issues_count = analysis.get("issues").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
    info!("配置分析完成: config_type={}, issues_count={}", config.name, issues_count);

    Ok(serde_json::json!({
        "analysis": analysis,
        "raw_response": content,
        "stats": {
            "original_lines": original_lines,
            "removed_lines": removed_lines,
            "analyzed_lines": cleaned.lines().count(),
            "config_type": config.name,
        }
    }))
}

/// 从 AI 响应中提取 JSON
fn extract_json_from_response(response: &str) -> serde_json::Value {
    // 尝试直接解析
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(response) {
        return v;
    }

    // 尝试从 markdown 代码块中提取
    if let Some(start) = response.find("```json") {
        let json_start = start + 7;
        if let Some(end) = response[json_start..].find("```") {
            let json_str = &response[json_start..json_start + end];
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str.trim()) {
                return v;
            }
        }
    }

    // 尝试从 { 开始提取
    if let Some(start) = response.find('{') {
        // 找到匹配的 }
        let mut depth = 0;
        let mut end = start;
        for (i, ch) in response[start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        if end > start {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&response[start..end]) {
                return v;
            }
        }
    }

    // 返回默认结构
    serde_json::json!({
        "summary": response,
        "risk_level": "unknown",
        "issues": [],
        "optimizations": []
    })
}

/// SSH 远程读取配置文件
#[tauri::command]
pub async fn read_remote_config(
    device_id: i64,
    file_path: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("SSH 读取远程配置: device_id={}, file_path={}", device_id, file_path);

    // 获取设备信息
    let (ip, port, username, password) = {
        let conn = state.db.lock();
        let sql = "SELECT ip, ssh_port, ssh_username, ssh_password_encrypted FROM devices WHERE id = ?1";
        crate::db::query::query_one(&conn, sql, rusqlite::params![device_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?
        .ok_or_else(|| format!("设备 ID {} 不存在", device_id))?
    };

    let password = password.ok_or_else(|| "设备未配置 SSH 密码".to_string())?;
    let decrypted_password = crate::services::crypto::CryptoService::decrypt(&password)?;

    // 保存文件路径用于后续检测类型
    let file_path_clone = file_path.clone();

    // 通过 SSH 读取文件
    let file_content = tokio::task::spawn_blocking(move || {
        read_file_via_ssh(&ip, port as u32, &username, &decrypted_password, &file_path)
    })
    .await
    .map_err(|e| format!("SSH 任务失败: {}", e))??;

    // 自动检测配置类型
    let config_type = detect_config_type(&file_path_clone, &file_content);
    info!("远程配置读取完成: detected_type={}, content_size={}", config_type, file_content.len());

    Ok(serde_json::json!({
        "content": file_content,
        "file_path": file_path_clone,
        "config_type": config_type,
        "device_id": device_id,
    }))
}

/// 通过 SSH 读取文件内容
fn read_file_via_ssh(ip: &str, port: u32, username: &str, password: &str, file_path: &str) -> Result<String, String> {
    use ssh2::Session;
    use std::io::Read;
    use std::net::TcpStream;

    debug!("SSH 连接: {}:{}, file_path={}", ip, port, file_path);
    let tcp = TcpStream::connect(format!("{}:{}", ip, port))
        .map_err(|e| { warn!("TCP 连接失败: {}:{}, error={}", ip, port, e); format!("TCP 连接失败: {}", e) })?;

    let mut sess = Session::new()
        .map_err(|e| format!("创建 SSH 会话失败: {}", e))?;

    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("SSH 握手失败: {}", e))?;

    sess.userauth_password(username, password)
        .map_err(|e| format!("SSH 认证失败: {}", e))?;

    if !sess.authenticated() {
        warn!("SSH 认证失败: {}@{}", username, ip);
        return Err("SSH 认证失败".to_string());
    }
    debug!("SSH 认证成功: {}@{}", username, ip);

    let mut channel = sess.channel_session()
        .map_err(|e| format!("打开 SSH 通道失败: {}", e))?;

    // 使用 cat 读取文件，处理权限问题
    let cmd = format!("cat {} 2>&1 || echo '[ERROR] 权限不足或文件不存在'", file_path);
    channel.exec(&cmd)
        .map_err(|e| format!("执行命令失败: {}", e))?;

    let mut output = String::new();
    channel.read_to_string(&mut output)
        .map_err(|e| format!("读取输出失败: {}", e))?;

    channel.wait_close()
        .map_err(|e| format!("关闭通道失败: {}", e))?;

    if output.contains("[ERROR]") {
        warn!("远程文件读取失败: {}:{}, path={}", ip, port, file_path);
        return Err(output.trim().to_string());
    }

    debug!("文件读取完成: {}:{}, path={}, size={}bytes", ip, port, file_path, output.len());
    Ok(output)
}

/// 根据文件路径和内容自动检测配置类型
fn detect_config_type(file_path: &str, content: &str) -> String {
    let path_lower = file_path.to_lowercase();

    if path_lower.contains("nginx") {
        return "nginx".to_string();
    }
    if path_lower.contains("apache") || path_lower.contains("httpd") {
        return "apache".to_string();
    }
    if path_lower.contains("zabbix") {
        return "zabbix".to_string();
    }
    if path_lower.contains("mysql") || path_lower.contains("mariadb") {
        return "mysql".to_string();
    }
    if path_lower.contains("redis") {
        return "redis".to_string();
    }
    if path_lower.contains("syslog") || path_lower.contains("rsyslog") {
        return "syslog".to_string();
    }
    if path_lower.contains("sshd") || path_lower == "/etc/ssh/sshd_config" {
        return "ssh".to_string();
    }
    if path_lower.contains("docker-compose") || path_lower.contains("compose.yml") {
        return "docker".to_string();
    }
    if path_lower.ends_with(".service") || path_lower.ends_with(".timer") {
        return "systemd".to_string();
    }

    // 根据内容特征检测
    if content.contains("server {") && content.contains("listen ") {
        return "nginx".to_string();
    }
    if content.contains("<VirtualHost") {
        return "apache".to_string();
    }
    if content.contains("[mysqld]") || content.contains("[mysql]") {
        return "mysql".to_string();
    }
    if content.contains("bind ") && content.contains("port ") && content.contains("daemonize") {
        return "redis".to_string();
    }

    "generic".to_string()
}
