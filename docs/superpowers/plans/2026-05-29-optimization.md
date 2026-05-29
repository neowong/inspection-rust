# 四阶段优化实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 对网络设备巡检系统进行安全加固、性能优化、代码质量提升和构建优化。

**Architecture:** 四阶段递进式优化。Phase 1 修复安全问题（CSP + Fernet 密钥），Phase 2 优化性能（Mutex 锁拆分 + 异步化 + HTTP Client 复用），Phase 3 消除代码重复并统一错误处理，Phase 4 优化构建配置。每阶段独立可验证，阶段间无硬依赖。

**Tech Stack:** Rust (rusqlite, ssh2, reqwest, fernet), Tauri v2, React 18, TypeScript, Vite 6

---

## File Structure

### Phase 1: 安全加固
- Modify: `src-tauri/tauri.conf.json` — CSP 策略
- Rewrite: `src-tauri/src/services/crypto.rs` — 密钥管理

### Phase 2: 性能优化
- Modify: `src-tauri/src/commands/inspections.rs` — 锁拆分 + 异步化
- Modify: `src-tauri/src/commands/devices.rs` — 锁拆分
- Modify: `src-tauri/src/services/ai_inspection.rs` — Client 复用

### Phase 3: 代码质量
- Modify: `src-tauri/src/db/models.rs` — 集中行映射函数 + 常量和 now_str
- Modify: `src-tauri/src/commands/inspections.rs` — 移除重复行映射函数
- Modify: `src-tauri/src/commands/devices.rs` — 移除重复行映射函数
- Modify: `src-tauri/src/commands/reports.rs` — 移除重复行映射函数
- Modify: `src-tauri/src/commands/templates.rs` — 移除重复行映射函数
- Create: `src/lib/status.ts` — batchStatusColor 共享
- Create: `src/lib/constants.ts` — VENDORS/CATEGORIES 共享
- Modify: `src/pages/InspectionPage.tsx` — 引用共享函数 + useMemo
- Modify: `src/pages/ReportsPage.tsx` — 引用共享函数 + useMemo 修正
- Modify: `src/pages/DevicesPage.tsx` — 引用共享常量 + useMemo
- Modify: `src/pages/TemplatesPage.tsx` — 引用共享常量 + useMemo
- Modify: `src/types/index.ts` — 类型收紧

### Phase 4: 构建优化
- Modify: `src-tauri/Cargo.toml` — features 最小化 + 移除废弃依赖
- Modify: `vite.config.ts` — 分包策略
- Modify: `tsconfig.json` — target 升级 + strict 增强
- Modify: `.gitignore` — 补充遗漏项

---

## Phase 1: 安全加固

### Task 1: CSP 策略配置

**Files:**
- Modify: `src-tauri/tauri.conf.json:22-24`

- [ ] **Step 1: 修改 CSP 配置**

将 `src-tauri/tauri.conf.json` 第 22-24 行的 `"csp": null` 替换为：

```json
    "security": {
      "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'"
    }
```

- [ ] **Step 2: 验证编译**

Run: `cd /home/neo/study/claude-demo/inspection-rust && cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 编译通过（CSP 不影响 Rust 编译）

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tauri.conf.json
git commit -m "security: add CSP policy to tauri config

Replace null CSP with restrictive policy allowing only self-origin
scripts, inline styles (needed by TailwindCSS), and data URIs for images."
```

---

### Task 2: Fernet 密钥自动生成与持久化

**Files:**
- Rewrite: `src-tauri/src/services/crypto.rs`

- [ ] **Step 1: 重写 crypto.rs**

将 `src-tauri/src/services/crypto.rs` 完整替换为：

```rust
use std::path::PathBuf;
use std::sync::OnceLock;

static FERNET_INSTANCE: OnceLock<fernet::Fernet> = OnceLock::new();

/// 获取密钥文件路径: ~/.local/share/inspection-rust/.key
fn key_file_path() -> Result<PathBuf, String> {
    let dir = dirs::data_dir()
        .ok_or_else(|| "无法获取数据目录".to_string())?
        .join("inspection-rust");
    Ok(dir.join(".key"))
}

/// 加载或创建 Fernet 密钥。
/// 首次启动时生成随机密钥并保存到文件（权限 0600），
/// 后续启动从文件读取。
fn load_or_create_key() -> Result<String, String> {
    let path = key_file_path()?;

    // 确保目录存在
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建密钥目录失败: {}", e))?;
    }

    if path.exists() {
        let key = std::fs::read_to_string(&path)
            .map_err(|e| format!("读取密钥文件失败: {}", e))?;
        let key = key.trim().to_string();
        if key.is_empty() {
            return Err("密钥文件为空".to_string());
        }
        Ok(key)
    } else {
        let key = fernet::Fernet::generate_key();
        std::fs::write(&path, &key)
            .map_err(|e| format!("写入密钥文件失败: {}", e))?;

        // 设置文件权限为 0600（仅 Unix）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("设置密钥文件权限失败: {}", e))?;
        }

        Ok(key)
    }
}

/// 获取全局 Fernet 实例（懒加载单例）
fn get_fernet() -> Result<&'static fernet::Fernet, String> {
    FERNET_INSTANCE.get_or_try_init(|| {
        let key = load_or_create_key()?;
        fernet::Fernet::new(&key)
            .ok_or_else(|| "无效的 Fernet 密钥".to_string())
    })
}

pub struct CryptoService;

impl CryptoService {
    pub fn encrypt(plaintext: &str) -> Result<String, String> {
        let fernet = get_fernet()?;
        Ok(fernet.encrypt(plaintext.as_bytes()))
    }

    pub fn decrypt(encrypted: &str) -> Result<String, String> {
        let fernet = get_fernet()?;
        let bytes = fernet
            .decrypt(encrypted)
            .map_err(|e| format!("解密失败: {}", e))?;
        String::from_utf8(bytes).map_err(|_| "UTF-8 转换失败".to_string())
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cd /home/neo/study/claude-demo/inspection-rust && cargo check --manifest-path src-tauri/Cargo.toml`
Expected: `Finished` 无错误

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/services/crypto.rs
git commit -m "security: auto-generate Fernet key on first launch

Replace hardcoded MASTER_PASSWORD with file-based key management.
Key is generated randomly on first launch and persisted to
~/.local/share/inspection-rust/.key with 0600 permissions.
Uses OnceLock for singleton Fernet instance."
```

---

## Phase 2: 性能优化

### Task 3: inspections.rs — Mutex 锁拆分 + 异步化

**Files:**
- Modify: `src-tauri/src/commands/inspections.rs:141-275` (execute_device_inspection)
- Modify: `src-tauri/src/commands/inspections.rs:502-577` (run_batch)
- Modify: `src-tauri/src/commands/inspections.rs:401-500` (create_batch)
- Modify: `src-tauri/src/commands/inspections.rs:671-710` (retry_device)

- [ ] **Step 1: 重写 execute_device_inspection 函数**

将 `inspections.rs` 第 141-275 行的 `execute_device_inspection` 函数替换为拆分版本——在锁内读取数据、锁外执行 SSH、再获锁写入结果：

```rust
/// 从数据库读取设备巡检所需的全部信息（在锁内调用）
fn read_device_inspection_data(
    conn: &rusqlite::Connection,
    device_id: i64,
) -> Result<(Device, String, String, Vec<String>), String> {
    // 1. Look up device
    let device_sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
    let device = crate::db::query::query_one(
        conn,
        &device_sql,
        rusqlite::params![device_id],
        device_from_row,
    )?
    .ok_or_else(|| format!("设备 ID {} 不存在", device_id))?;

    // 2. Decrypt SSH password
    let password = match &device.ssh_password_encrypted {
        Some(enc) if !enc.is_empty() => CryptoService::decrypt(enc)?,
        _ => return Err(format!("设备 '{}' 未配置 SSH 密码", device.name)),
    };
    let username = device.ssh_username.clone().unwrap_or_default();

    // 3. Look up template
    let template_id = device
        .template_id
        .ok_or_else(|| format!("设备 '{}' 未关联巡检模板", device.name))?;
    let template_sql = format!(
        "SELECT {} FROM inspection_templates WHERE id = ?1",
        TEMPLATE_COLUMNS
    );
    let template = crate::db::query::query_one(
        conn,
        &template_sql,
        rusqlite::params![template_id],
        template_from_row,
    )?
    .ok_or_else(|| format!("巡检模板 ID {} 不存在", template_id))?;

    // 4. Parse template config for command IDs
    let config_str = template
        .config
        .ok_or_else(|| format!("模板 '{}' 配置为空", template.name))?;
    let config: serde_json::Value = serde_json::from_str(&config_str)
        .map_err(|e| format!("解析模板配置 JSON 失败: {}", e))?;

    let command_ids: Vec<i64> = config["command_ids"]
        .as_array()
        .ok_or_else(|| format!("模板 '{}' 配置缺少 command_ids", template.name))?
        .iter()
        .filter_map(|v| v.as_i64())
        .collect();

    // 5. Fetch commands from command_pool by ID
    let mut commands: Vec<String> = Vec::new();
    for cmd_id in &command_ids {
        let cmd_sql = format!("SELECT {} FROM command_pool WHERE id = ?1", COMMAND_COLUMNS);
        let cmd = crate::db::query::query_one(
            conn,
            &cmd_sql,
            rusqlite::params![cmd_id],
            command_from_row,
        )?
        .ok_or_else(|| format!("命令 ID {} 不存在", cmd_id))?;
        commands.push(cmd.command);
    }

    if commands.is_empty() {
        return Err(format!(
            "设备 '{}' 的巡检模板 '{}' 未包含有效命令",
            device.name, template.name
        ));
    }

    Ok((device, username, password, commands))
}

/// 将巡检记录创建或更新为 running 状态（在锁内调用）
fn create_or_reset_record(
    conn: &rusqlite::Connection,
    batch_id: i64,
    device_id: i64,
) -> Result<i64, String> {
    let now = now_str();
    let existing: Result<i64, _> = conn.query_row(
        "SELECT id FROM inspection_records WHERE batch_id = ?1 AND device_id = ?2",
        rusqlite::params![batch_id, device_id],
        |row| row.get(0),
    );

    let record_id = match existing {
        Ok(id) => {
            conn.execute(
                "UPDATE inspection_records SET status = 'running', error_message = NULL, \
                 command_outputs = '{}', started_at = ?1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, id],
            )
            .map_err(|e| e.to_string())?;
            id
        }
        Err(_) => {
            conn.execute(
                "INSERT INTO inspection_records (batch_id, device_id, status, started_at) \
                 VALUES (?1, ?2, 'running', ?3)",
                rusqlite::params![batch_id, device_id, now],
            )
            .map_err(|e| e.to_string())?;
            conn.last_insert_rowid()
        }
    };
    Ok(record_id)
}

/// 更新巡检记录结果（在锁内调用）
fn update_record_result(
    conn: &rusqlite::Connection,
    record_id: i64,
    status: &str,
    outputs_json: Option<&str>,
    error: Option<&str>,
) -> Result<(), String> {
    let completed_at = now_str();
    match (outputs_json, error) {
        (Some(json), _) => {
            conn.execute(
                "UPDATE inspection_records SET status = ?1, command_outputs = ?2, \
                 completed_at = ?3, updated_at = ?3 WHERE id = ?4",
                rusqlite::params![status, json, completed_at, record_id],
            )
            .map_err(|e| e.to_string())?;
        }
        (_, Some(err)) => {
            conn.execute(
                "UPDATE inspection_records SET status = ?1, error_message = ?2, \
                 completed_at = ?3, updated_at = ?3 WHERE id = ?4",
                rusqlite::params![status, err, completed_at, record_id],
            )
            .map_err(|e| e.to_string())?;
        }
        _ => {}
    }
    Ok(())
}

/// 执行单台设备的 SSH 巡检（锁外调用，包含耗时的 SSH 操作）
fn execute_device_ssh(
    device: &Device,
    username: &str,
    password: &str,
    commands: &[String],
) -> Result<std::collections::HashMap<String, String>, String> {
    let source = SSHSessionSource {
        host: device.ip.clone(),
        port: device.ssh_port as u16,
        username: username.to_string(),
        password: password.to_string(),
    };
    inspection_runner::run_commands(&source, &device.vendor, commands)
}
```

- [ ] **Step 2: 重写 run_batch 为 async**

将 `inspections.rs` 第 502-577 行的 `run_batch` 函数替换为：

```rust
/// 运行指定批次，对批次内的每台设备执行 SSH 巡检命令。
/// 异步执行，SSH 操作在锁外进行，不阻塞 tokio 工作线程。
#[tauri::command]
pub async fn run_batch(batch_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    // 1. 读取批次信息和设备列表（短暂获锁）
    let device_ids = {
        let conn = state.db.lock();
        let sql = format!(
            "SELECT {} FROM inspection_batches WHERE id = ?1",
            BATCH_COLUMNS
        );
        let batch = crate::db::query::query_one(
            &conn,
            &sql,
            rusqlite::params![batch_id],
            batch_from_row,
        )?
        .ok_or_else(|| format!("巡检批次 ID {} 不存在", batch_id))?;

        let device_ids_str = batch.device_ids.unwrap_or_else(|| "[]".to_string());
        let ids: Vec<i64> = serde_json::from_str(&device_ids_str)
            .map_err(|e| format!("解析设备ID列表失败: {}", e))?;

        if ids.is_empty() {
            let now = now_str();
            conn.execute(
                "UPDATE inspection_batches SET status = 'completed', started_at = ?1, \
                 completed_at = ?1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, batch_id],
            )
            .map_err(|e| e.to_string())?;
            return Ok(());
        }

        // 更新批次状态为 running
        let now = now_str();
        conn.execute(
            "UPDATE inspection_batches SET status = 'running', started_at = ?1, \
             updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, batch_id],
        )
        .map_err(|e| e.to_string())?;

        ids
    }; // 锁释放

    // 2. 逐台设备执行（锁外 SSH）
    let mut completed_count = 0u32;
    let mut failed_count = 0u32;

    for device_id in &device_ids {
        // 读取数据（短暂获锁）
        let (device, username, password, commands) = {
            let conn = state.db.lock();
            read_device_inspection_data(&conn, *device_id)?
        }; // 锁释放

        // 创建记录（短暂获锁）
        let record_id = {
            let conn = state.db.lock();
            create_or_reset_record(&conn, batch_id, *device_id)?
        }; // 锁释放

        // SSH 执行（锁外，spawn_blocking 避免阻塞 tokio）
        let ssh_result = {
            let device_clone = device.clone();
            let username_clone = username.clone();
            let password_clone = password.clone();
            let commands_clone = commands.clone();
            tokio::task::spawn_blocking(move || {
                execute_device_ssh(&device_clone, &username_clone, &password_clone, &commands_clone)
            })
            .await
            .map_err(|e| format!("SSH 任务调度失败: {}", e))?
        };

        // 写入结果（短暂获锁）
        {
            let conn = state.db.lock();
            match ssh_result {
                Ok(outputs) => {
                    let outputs_json = serde_json::to_string(&outputs)
                        .map_err(|e| format!("序列化命令输出失败: {}", e))?;
                    update_record_result(&conn, record_id, "completed", Some(&outputs_json), None)?;
                    completed_count += 1;
                }
                Err(err) => {
                    update_record_result(&conn, record_id, "failed", None, Some(&err))?;
                    failed_count += 1;
                    eprintln!("设备 {} 巡检失败: {}", device_id, err);
                }
            }
        } // 锁释放
    }

    // 3. 更新批次最终状态（短暂获锁）
    let final_status = if failed_count == 0 {
        "completed"
    } else if completed_count == 0 {
        "failed"
    } else {
        "partially_completed"
    };

    {
        let conn = state.db.lock();
        let now = now_str();
        conn.execute(
            "UPDATE inspection_batches SET status = ?1, completed_at = ?2, updated_at = ?2 \
             WHERE id = ?3",
            rusqlite::params![final_status, now, batch_id],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}
```

- [ ] **Step 3: 重写 create_batch 中的 auto_start 逻辑**

将 `inspections.rs` 第 401-500 行的 `create_batch` 函数替换为异步版本：

```rust
/// 创建巡检批次。若 auto_start = true，则为每台设备创建记录并立即执行 SSH 巡检。
#[tauri::command]
pub async fn create_batch(
    data: BatchCreate,
    auto_start: Option<bool>,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let device_ids = data.device_ids.clone().unwrap_or_else(|| "[]".to_string());

    // 插入批次记录（短暂获锁）
    let batch_id = {
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO inspection_batches (name, status, triggered_by, device_ids, started_at, completed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                data.name,
                data.status.as_deref().unwrap_or("pending"),
                data.triggered_by.as_deref().unwrap_or("manual"),
                device_ids,
                data.started_at,
                data.completed_at,
            ],
        )
        .map_err(|e| e.to_string())?;
        conn.last_insert_rowid()
    };

    if auto_start.unwrap_or(false) {
        let parsed_ids: Vec<i64> = serde_json::from_str(&device_ids)
            .map_err(|e| format!("解析设备ID列表失败: {}", e))?;

        if parsed_ids.is_empty() {
            let conn = state.db.lock();
            let now = now_str();
            conn.execute(
                "UPDATE inspection_batches SET status = 'completed', started_at = ?1, \
                 completed_at = ?1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, batch_id],
            )
            .map_err(|e| e.to_string())?;
        } else {
            // 更新为 running
            {
                let conn = state.db.lock();
                let now = now_str();
                conn.execute(
                    "UPDATE inspection_batches SET status = 'running', started_at = ?1, \
                     updated_at = ?1 WHERE id = ?2",
                    rusqlite::params![now, batch_id],
                )
                .map_err(|e| e.to_string())?;
            }

            let mut completed_count = 0u32;
            let mut failed_count = 0u32;

            for device_id in &parsed_ids {
                let (device, username, password, commands) = {
                    let conn = state.db.lock();
                    // 创建 pending 记录
                    conn.execute(
                        "INSERT INTO inspection_records (batch_id, device_id, status) \
                         VALUES (?1, ?2, 'pending')",
                        rusqlite::params![batch_id, device_id],
                    )
                    .map_err(|e| e.to_string())?;

                    read_device_inspection_data(&conn, *device_id)?
                };

                let ssh_result = {
                    let device_clone = device.clone();
                    let username_clone = username.clone();
                    let password_clone = password.clone();
                    let commands_clone = commands.clone();
                    tokio::task::spawn_blocking(move || {
                        execute_device_ssh(&device_clone, &username_clone, &password_clone, &commands_clone)
                    })
                    .await
                    .map_err(|e| format!("SSH 任务调度失败: {}", e))?
                };

                // 查找刚创建的 record_id 并更新
                {
                    let conn = state.db.lock();
                    let record_id: i64 = conn.query_row(
                        "SELECT id FROM inspection_records WHERE batch_id = ?1 AND device_id = ?2",
                        rusqlite::params![batch_id, device_id],
                        |row| row.get(0),
                    ).map_err(|e| e.to_string())?;

                    // 先更新为 running
                    let now = now_str();
                    conn.execute(
                        "UPDATE inspection_records SET status = 'running', started_at = ?1, updated_at = ?1 WHERE id = ?2",
                        rusqlite::params![now, record_id],
                    ).map_err(|e| e.to_string())?;

                    match ssh_result {
                        Ok(outputs) => {
                            let outputs_json = serde_json::to_string(&outputs)
                                .map_err(|e| format!("序列化命令输出失败: {}", e))?;
                            update_record_result(&conn, record_id, "completed", Some(&outputs_json), None)?;
                            completed_count += 1;
                        }
                        Err(err) => {
                            update_record_result(&conn, record_id, "failed", None, Some(&err))?;
                            failed_count += 1;
                            eprintln!("设备 {} 巡检失败: {}", device_id, err);
                        }
                    }
                }
            }

            let final_status = if failed_count == 0 {
                "completed"
            } else if completed_count == 0 {
                "failed"
            } else {
                "partially_completed"
            };

            {
                let conn = state.db.lock();
                let now = now_str();
                conn.execute(
                    "UPDATE inspection_batches SET status = ?1, completed_at = ?2, updated_at = ?2 \
                     WHERE id = ?3",
                    rusqlite::params![final_status, now, batch_id],
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }

    // Return the created batch
    let conn = state.db.lock();
    let query_sql = format!(
        "SELECT {} FROM inspection_batches WHERE id = ?1",
        BATCH_COLUMNS
    );
    let batch = crate::db::query::query_one(
        &conn,
        &query_sql,
        rusqlite::params![batch_id],
        batch_from_row,
    )?
    .ok_or_else(|| "创建巡检批次后查询失败".to_string())?;

    Ok(serde_json::json!(batch))
}
```

- [ ] **Step 4: 重写 retry_device 为 async**

将 `inspections.rs` 第 671-710 行的 `retry_device` 替换为：

```rust
/// 重试单条巡检记录，重置为 pending 后重新执行 SSH 巡检。
#[tauri::command]
pub async fn retry_device(record_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    // 读取记录信息并重置状态（短暂获锁）
    let (batch_id, device_id, device, username, password, commands) = {
        let conn = state.db.lock();

        let record_sql = format!(
            "SELECT {} FROM inspection_records WHERE id = ?1",
            RECORD_COLUMNS
        );
        let record = crate::db::query::query_one(
            &conn,
            &record_sql,
            rusqlite::params![record_id],
            record_from_row,
        )?
        .ok_or_else(|| format!("巡检记录 ID {} 不存在", record_id))?;

        let batch_id = record.batch_id;
        let device_id = record.device_id;

        // Reset record to pending
        let now = now_str();
        conn.execute(
            "UPDATE inspection_records SET status = 'running', error_message = NULL, \
             command_outputs = '{}', started_at = ?1, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, record_id],
        )
        .map_err(|e| e.to_string())?;

        // Set batch to running
        conn.execute(
            "UPDATE inspection_batches SET status = 'running', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, batch_id],
        )
        .map_err(|e| e.to_string())?;

        let (device, username, password, commands) = read_device_inspection_data(&conn, device_id)?;
        (batch_id, device_id, device, username, password, commands)
    }; // 锁释放

    // SSH 执行（锁外）
    let ssh_result = {
        let device_clone = device.clone();
        let username_clone = username.clone();
        let password_clone = password.clone();
        let commands_clone = commands.clone();
        tokio::task::spawn_blocking(move || {
            execute_device_ssh(&device_clone, &username_clone, &password_clone, &commands_clone)
        })
        .await
        .map_err(|e| format!("SSH 任务调度失败: {}", e))?
    };

    // 写入结果（短暂获锁）
    let conn = state.db.lock();
    match ssh_result {
        Ok(outputs) => {
            let outputs_json = serde_json::to_string(&outputs)
                .map_err(|e| format!("序列化命令输出失败: {}", e))?;
            update_record_result(&conn, record_id, "completed", Some(&outputs_json), None)?;
        }
        Err(err) => {
            update_record_result(&conn, record_id, "failed", None, Some(&err))?;
        }
    }

    Ok(())
}
```

- [ ] **Step 5: 验证编译**

Run: `cd /home/neo/study/claude-demo/inspection-rust && cargo check --manifest-path src-tauri/Cargo.toml`
Expected: `Finished` 无错误。可能出现 `unused variable` 警告（`batch_id` 和 `device_id` 在 `retry_device` 中），可忽略。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/inspections.rs
git commit -m "perf: split Mutex locks and async SSH in inspections

- Split execute_device_inspection into read/ssh/write phases
- SSH execution happens outside DB lock via spawn_blocking
- run_batch, create_batch, retry_device now async
- Each device gets brief lock acquire/release instead of
  holding lock for entire batch duration"
```

---

### Task 4: devices.rs — check_device_status 锁拆分

**Files:**
- Modify: `src-tauri/src/commands/devices.rs:349-398` (check_device_status_inner)

- [ ] **Step 1: 重写 check_device_status_inner**

将 `devices.rs` 第 349-398 行的 `check_device_status_inner` 替换为锁拆分版本：

```rust
/// 检查单个设备连通状态（内部实现）
/// 拆分为：读取设备信息 → TCP 检测（锁外）→ 写入结果
fn check_device_status_inner(
    app_state: &AppState,
    device_id: i64,
) -> Result<serde_json::Value, String> {
    // 1. 读取设备信息（短暂获锁）
    let device = {
        let conn = app_state.db.lock();
        let sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_COLUMNS);
        crate::db::query::query_one(
            &conn,
            &sql,
            rusqlite::params![device_id],
            device_from_row,
        )?
        .ok_or_else(|| format!("设备 ID {} 不存在", device_id))?
    }; // 锁释放

    // 2. TCP 连接检测（锁外，5 秒超时）
    let ip_addr = IpAddr::from_str(&device.ip)
        .map_err(|_| format!("无法解析设备 IP 地址: {}", device.ip))?;
    let socket_addr = SocketAddr::new(ip_addr, device.ssh_port as u16);

    let new_status = match TcpStream::connect_timeout(&socket_addr, Duration::from_secs(5)) {
        Ok(_stream) => "online",
        Err(_) => "offline",
    };

    let now = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    // 3. 写入结果（短暂获锁）
    {
        let conn = app_state.db.lock();

        conn.execute(
            "INSERT INTO device_status_logs (device_id, old_status, new_status, checked_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![device_id, device.status, new_status, now],
        )
        .map_err(|e| e.to_string())?;

        conn.execute(
            "UPDATE devices SET status = ?1, last_checked_at = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![new_status, now, now, device_id],
        )
        .map_err(|e| e.to_string())?;
    } // 锁释放

    Ok(serde_json::json!({
        "device_id": device_id,
        "old_status": device.status,
        "new_status": new_status,
        "checked_at": now,
    }))
}
```

- [ ] **Step 2: 验证编译**

Run: `cd /home/neo/study/claude-demo/inspection-rust && cargo check --manifest-path src-tauri/Cargo.toml`
Expected: `Finished` 无错误

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/devices.rs
git commit -m "perf: split Mutex lock in check_device_status

TCP timeout check (5s) now happens outside DB lock,
preventing lock contention during status checks."
```

---

### Task 5: ai_inspection.rs — reqwest Client 复用

**Files:**
- Modify: `src-tauri/src/services/ai_inspection.rs:84` and `src-tauri/src/services/ai_inspection.rs:159`

- [ ] **Step 1: 添加全局 Client 单例**

在 `ai_inspection.rs` 文件顶部（第 1 行之后）添加：

```rust
use std::sync::OnceLock;

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(reqwest::Client::new)
}
```

- [ ] **Step 2: 替换 OpenAI 函数中的 Client::new()**

将 `ai_inspection.rs` 第 84 行的 `let client = reqwest::Client::new();` 替换为：

```rust
    let client = get_client();
```

- [ ] **Step 3: 替换 Anthropic 函数中的 Client::new()**

将 `ai_inspection.rs` 第 159 行的 `let client = reqwest::Client::new();` 替换为：

```rust
    let client = get_client();
```

- [ ] **Step 4: 验证编译**

Run: `cd /home/neo/study/claude-demo/inspection-rust && cargo check --manifest-path src-tauri/Cargo.toml`
Expected: `Finished` 无错误

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/ai_inspection.rs
git commit -m "perf: reuse reqwest Client singleton for AI API calls

Use OnceLock to create a single reqwest::Client instance,
enabling TCP connection and TLS session reuse across calls."
```

---

## Phase 3: 代码质量

### Task 6: 后端行映射函数 + now_str 集中到 db/models.rs

**Files:**
- Modify: `src-tauri/src/db/models.rs` — 添加 row_from 函数 + 常量 + now_str
- Modify: `src-tauri/src/commands/inspections.rs` — 移除本地重复定义，改为 use
- Modify: `src-tauri/src/commands/devices.rs` — 移除本地重复定义，改为 use
- Modify: `src-tauri/src/commands/reports.rs` — 移除本地重复定义，改为 use
- Modify: `src-tauri/src/commands/templates.rs` — 移除本地重复定义，改为 use

- [ ] **Step 1: 在 db/models.rs 末尾添加公共函数和常量**

在 `src-tauri/src/db/models.rs` 文件末尾追加：

```rust
// ============================
// 公共工具函数
// ============================

/// 返回当前时间戳字符串
pub fn now_str() -> String {
    chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

// ============================
// SQL 列定义常量
// ============================

pub const DEVICE_COLUMNS: &str =
    "id, name, ip, device_type, vendor, model, ssh_username, ssh_password_encrypted, \
     ssh_port, template_id, status, last_checked_at, created_at, updated_at";

pub const TEMPLATE_COLUMNS: &str =
    "id, name, vendor, model, device_type, config, description, report_template_id, template_type, \
     created_at, updated_at";

pub const COMMAND_COLUMNS: &str =
    "id, vendor, command, description, category, model, created_at, updated_at";

pub const BATCH_COLUMNS: &str =
    "id, name, status, triggered_by, device_ids, started_at, completed_at, created_at, updated_at";

pub const RECORD_COLUMNS: &str =
    "id, batch_id, device_id, status, error_message, command_outputs, ai_status, ai_result, \
     ai_analysis, ai_suggestions, command_judgments, summary_judgment, report_path, \
     started_at, completed_at, created_at, updated_at";

pub const REPORT_TEMPLATE_COLUMNS: &str =
    "id, name, vendor, file_path, created_at, updated_at";

// ============================
// 行映射函数（统一去重）
// ============================

pub fn device_from_row(row: &rusqlite::Row) -> rusqlite::Result<Device> {
    Ok(Device {
        id: row.get(0)?,
        name: row.get(1)?,
        ip: row.get(2)?,
        device_type: row.get(3)?,
        vendor: row.get(4)?,
        model: row.get(5)?,
        ssh_username: row.get(6)?,
        ssh_password_encrypted: row.get(7)?,
        ssh_port: row.get(8)?,
        template_id: row.get(9)?,
        status: row.get(10)?,
        last_checked_at: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

pub fn status_log_from_row(row: &rusqlite::Row) -> rusqlite::Result<DeviceStatusLog> {
    Ok(DeviceStatusLog {
        id: row.get(0)?,
        device_id: row.get(1)?,
        old_status: row.get(2)?,
        new_status: row.get(3)?,
        checked_at: row.get(4)?,
    })
}

pub fn template_from_row(row: &rusqlite::Row) -> rusqlite::Result<InspectionTemplate> {
    Ok(InspectionTemplate {
        id: row.get(0)?,
        name: row.get(1)?,
        vendor: row.get(2)?,
        model: row.get(3)?,
        device_type: row.get(4)?,
        config: row.get(5)?,
        description: row.get(6)?,
        report_template_id: row.get(7)?,
        template_type: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

pub fn command_from_row(row: &rusqlite::Row) -> rusqlite::Result<CommandPool> {
    Ok(CommandPool {
        id: row.get(0)?,
        vendor: row.get(1)?,
        command: row.get(2)?,
        description: row.get(3)?,
        category: row.get(4)?,
        model: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

pub fn batch_from_row(row: &rusqlite::Row) -> rusqlite::Result<InspectionBatch> {
    Ok(InspectionBatch {
        id: row.get(0)?,
        name: row.get(1)?,
        status: row.get(2)?,
        triggered_by: row.get(3)?,
        device_ids: row.get(4)?,
        started_at: row.get(5)?,
        completed_at: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

pub fn record_from_row(row: &rusqlite::Row) -> rusqlite::Result<InspectionRecord> {
    Ok(InspectionRecord {
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

pub fn report_template_from_row(row: &rusqlite::Row) -> rusqlite::Result<ReportTemplate> {
    Ok(ReportTemplate {
        id: row.get(0)?,
        name: row.get(1)?,
        vendor: row.get(2)?,
        file_path: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}
```

- [ ] **Step 2: 清理 inspections.rs 中的重复定义**

在 `inspections.rs` 中：

1. 删除第 15-32 行的 `BATCH_COLUMNS`、`RECORD_COLUMNS`、`DEVICE_COLUMNS`、`TEMPLATE_COLUMNS`、`COMMAND_COLUMNS` 常量定义
2. 删除第 38-120 行的 `batch_from_row`、`record_from_row`、`device_from_row`、`template_from_row`、`command_from_row` 函数定义
3. 删除第 127-131 行的 `now_str()` 函数定义
4. 修改文件顶部的 use 语句，在第 5-7 行替换为：

```rust
use crate::db::models::{
    BatchCreate, CommandPool, Device, InspectionBatch, InspectionRecord, InspectionTemplate,
    BATCH_COLUMNS, RECORD_COLUMNS, DEVICE_COLUMNS, TEMPLATE_COLUMNS, COMMAND_COLUMNS,
    batch_from_row, record_from_row, device_from_row, template_from_row, command_from_row,
    now_str,
};
```

- [ ] **Step 3: 清理 devices.rs 中的重复定义**

在 `devices.rs` 中：

1. 删除第 16 行的 `DEVICE_COLUMNS` 常量定义
2. 删除第 22-49 行的 `device_from_row` 和 `status_log_from_row` 函数定义
3. 修改文件顶部的 use 语句（第 5 行）替换为：

```rust
use crate::db::models::{
    Device, DeviceCreate, DeviceUpdate, DeviceStatusLog,
    DEVICE_COLUMNS, device_from_row, status_log_from_row,
};
```

- [ ] **Step 4: 清理 reports.rs 中的重复定义**

在 `reports.rs` 中：

1. 删除第 14-23 行的 `RECORD_COLUMNS`、`DEVICE_COLUMNS`、`REPORT_TEMPLATE_COLUMNS` 常量定义
2. 删除第 29-79 行的 `record_from_row`、`device_from_row`、`report_template_from_row` 函数定义
3. 删除第 119-123 行的 `now_str()` 函数定义
4. 修改文件顶部的 use 语句（第 6 行）替换为：

```rust
use crate::db::models::{
    AiModelConfig, Device, InspectionRecord, ReportTemplate,
    RECORD_COLUMNS, DEVICE_COLUMNS, REPORT_TEMPLATE_COLUMNS,
    record_from_row, device_from_row, report_template_from_row, now_str,
};
```

- [ ] **Step 5: 清理 templates.rs 中的重复定义**

在 `templates.rs` 中：

1. 删除第 14-16 行的 `TEMPLATE_COLUMNS` 和 `COMMAND_COLUMNS` 常量定义
2. 删除第 22-49 行的 `template_from_row` 和 `command_from_row` 函数定义
3. 修改文件顶部的 use 语句（第 5-7 行）替换为：

```rust
use crate::db::models::{
    CommandCreate, CommandPool, CommandUpdate, InspectionTemplate, TemplateCreate, TemplateUpdate,
    TEMPLATE_COLUMNS, COMMAND_COLUMNS, template_from_row, command_from_row,
};
```

- [ ] **Step 6: 验证编译**

Run: `cd /home/neo/study/claude-demo/inspection-rust && cargo check --manifest-path src-tauri/Cargo.toml`
Expected: `Finished` 无错误

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/db/models.rs src-tauri/src/commands/inspections.rs \
  src-tauri/src/commands/devices.rs src-tauri/src/commands/reports.rs \
  src-tauri/src/commands/templates.rs
git commit -m "refactor: centralize row mappers, constants, and now_str in db/models.rs

Eliminate ~120 lines of duplicated row_from functions and column
constants across devices, inspections, reports, and templates modules."
```

---

### Task 7: 前端共享常量与工具函数

**Files:**
- Create: `src/lib/status.ts`
- Create: `src/lib/constants.ts`

- [ ] **Step 1: 创建 status.ts**

创建 `src/lib/status.ts`：

```typescript
export type BatchStatus = "pending" | "running" | "completed" | "failed" | "stopped";

export function batchStatusColor(status: string): BatchStatus {
  if (status === "pending" || status === "waiting") return "pending";
  if (status === "running" || status === "in_progress") return "running";
  if (status === "completed") return "completed";
  if (status === "failed") return "failed";
  if (status === "stopped" || status === "paused") return "stopped";
  return "pending";
}
```

- [ ] **Step 2: 创建 constants.ts**

创建 `src/lib/constants.ts`：

```typescript
export const VENDORS = ["H3C", "华为", "思科", "锐捷"] as const;

export const CATEGORIES = [
  "version", "clock", "cpu", "memory", "hardware",
  "interface", "vlan", "log", "protocol", "general",
] as const;
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/status.ts src/lib/constants.ts
git commit -m "refactor: add shared status and constants modules

Extract batchStatusColor and VENDORS/CATEGORIES constants
from duplicated page-level definitions."
```

---

### Task 8: 前端页面引用共享模块 + useMemo 修正

**Files:**
- Modify: `src/pages/InspectionPage.tsx`
- Modify: `src/pages/ReportsPage.tsx`
- Modify: `src/pages/DevicesPage.tsx`
- Modify: `src/pages/TemplatesPage.tsx`

- [ ] **Step 1: 更新 InspectionPage.tsx**

1. 在文件顶部 import 区添加：
```typescript
import { batchStatusColor } from "../lib/status";
```
2. 删除第 106-113 行的 `batchStatusColor` 函数定义

- [ ] **Step 2: 更新 ReportsPage.tsx**

1. 在文件顶部 import 区，将 `useCallback` 替换为 `useMemo`（如果尚未导入 `useMemo`），并添加：
```typescript
import { useMemo } from "react";
import { batchStatusColor } from "../lib/status";
```
2. 删除第 11-18 行的 `batchStatusColor` 函数定义
3. 将第 71-90 行的 `parsedOutputs` 和 `aiResult` 从 `useCallback` 改为 `useMemo`：

```typescript
  const parsedOutputs = useMemo(() => {
    if (!selectedRecord?.command_outputs) return [];
    try {
      const parsed = JSON.parse(selectedRecord.command_outputs);
      if (Array.isArray(parsed)) return parsed;
      return [{ command: "output", content: selectedRecord.command_outputs }];
    } catch {
      return [{ command: "output", content: selectedRecord.command_outputs }];
    }
  }, [selectedRecord?.command_outputs]);

  const aiResult = useMemo(() => {
    if (!selectedRecord?.ai_result) return null;
    try {
      return JSON.parse(selectedRecord.ai_result);
    } catch {
      return null;
    }
  }, [selectedRecord?.ai_result]);
```

4. 在渲染中使用这两个变量时，去掉函数调用括号（如果之前是 `parsedOutputs()`，改为 `parsedOutputs`；如果是 `aiResult()`，改为 `aiResult`）

- [ ] **Step 3: 更新 DevicesPage.tsx**

1. 在文件顶部 import 区添加：
```typescript
import { useMemo } from "react";
import { VENDORS } from "../lib/constants";
```
2. 删除第 29 行的 `const VENDORS = ["H3C", "华为", "思科", "锐捷"];`
3. 将第 62-64 行的 `filteredDevices` 包裹 `useMemo`：

```typescript
  const filteredDevices = useMemo(() => devices.filter((d) =>
    !searchText || d.name.toLowerCase().includes(searchText.toLowerCase()) || d.ip.includes(searchText)
  ), [devices, searchText]);
```

- [ ] **Step 4: 更新 TemplatesPage.tsx**

1. 在文件顶部 import 区添加：
```typescript
import { useMemo } from "react";
import { VENDORS, CATEGORIES } from "../lib/constants";
```
2. 删除第 12-13 行的 `VENDORS` 和 `CATEGORIES` 常量定义
3. 将第 77-84 行的 `filteredTemplates` 和 `filteredCommands` 包裹 `useMemo`：

```typescript
  const filteredTemplates = useMemo(() => templates.filter((t) =>
    !templateSearch || t.name.toLowerCase().includes(templateSearch.toLowerCase())
  ), [templates, templateSearch]);

  const filteredCommands = useMemo(() => commands.filter((c) =>
    !cmdSearch || c.command.toLowerCase().includes(cmdSearch.toLowerCase()) || (c.description && c.description.toLowerCase().includes(cmdSearch.toLowerCase()))
  ), [commands, cmdSearch]);
```

- [ ] **Step 5: 验证前端构建**

Run: `cd /home/neo/study/claude-demo/inspection-rust && npm run build`
Expected: `Build completed` 无错误

- [ ] **Step 6: Commit**

```bash
git add src/pages/InspectionPage.tsx src/pages/ReportsPage.tsx \
  src/pages/DevicesPage.tsx src/pages/TemplatesPage.tsx
git commit -m "refactor: use shared modules and add useMemo to pages

- Import batchStatusColor from lib/status
- Import VENDORS/CATEGORIES from lib/constants
- Wrap filteredDevices/Templates/Commands in useMemo
- Fix useCallback→useMemo for parsedOutputs/aiResult in ReportsPage"
```

---

### Task 9: TypeScript 类型收紧

**Files:**
- Modify: `src/types/index.ts`

- [ ] **Step 1: 更新类型定义**

将 `src/types/index.ts` 第 43-82 行替换为收紧版本：

```typescript
export type BatchStatusType =
  | "pending" | "running" | "completed" | "failed"
  | "stopped" | "paused" | "waiting" | "in_progress" | "partially_completed";

export type RecordStatusType =
  | "pending" | "running" | "completed" | "failed"
  | "stopped" | "skipped";

export type AiStatusType =
  | "none" | "pending" | "running" | "completed" | "failed";

export interface InspectionBatch {
  id: number;
  name: string | null;
  status: BatchStatusType;
  triggered_by: string;
  device_ids: number[];
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
  records: InspectionRecordSummary[];
}

export interface InspectionRecordSummary {
  id: number;
  batch_id: number;
  device_id: number;
  status: RecordStatusType;
  ai_status: AiStatusType;
  report_path: string | null;
  error_message: string | null;
}

export interface InspectionRecord {
  id: number;
  batch_id: number;
  device_id: number;
  status: RecordStatusType;
  command_outputs: string | null;
  ai_status: AiStatusType;
  ai_result: string | null;
  ai_analysis: string | null;
  ai_suggestions: string | null;
  command_judgments: string | null;
  summary_judgment: string | null;
  report_path: string | null;
  error_message: string | null;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
}
```

- [ ] **Step 2: 验证前端构建**

Run: `cd /home/neo/study/claude-demo/inspection-rust && npm run build`
Expected: `Build completed` 无错误。如果有类型不匹配错误，修复对应的页面文件中的类型断言。

- [ ] **Step 3: Commit**

```bash
git add src/types/index.ts
git commit -m "refactor: tighten TypeScript types with union types

Replace string status fields with discriminated union types
for BatchStatus, RecordStatus, and AiStatus. Unify optional
fields to use | null consistently."
```

---

## Phase 4: 构建优化

### Task 10: Cargo.toml 优化

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: 修改 Cargo.toml**

将 `src-tauri/Cargo.toml` 完整替换为：

```toml
[package]
name = "inspection-rust"
version = "3.0.0"
edition = "2021"
description = "网络设备巡检系统"

[lib]
name = "inspection_rust_lib"
crate-type = ["cdylib", "rlib"]

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-shell = "2"
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"
tauri-plugin-process = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.31", features = ["bundled"] }
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "sync"] }
reqwest = { version = "0.12", features = ["json"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid = { version = "1", features = ["v4"] }
fernet = "0.2"
ssh2 = "0.9"
async-trait = "0.1"
thiserror = "1"
anyhow = "1"
dirs = "6"
parking_lot = "0.12"
serde_yml = "0.0.12"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

变更说明：
- 移除 `staticlib`（对 Tauri 无意义）
- `tokio features` 从 `["full"]` 最小化为 `["rt-multi-thread", "macros", "time", "sync"]`
- 移除 `log = "0.4"`（`tracing` 已涵盖）
- `serde_yaml` → `serde_yml`（前者已废弃）

- [ ] **Step 2: 修复 serde_yaml 引用（如有）**

Run: `cd /home/neo/study/claude-demo/inspection-rust && grep -rn "serde_yaml" src-tauri/src/`

如果有引用，将 `serde_yaml` 替换为 `serde_yml`。

- [ ] **Step 3: 验证编译**

Run: `cd /home/neo/study/claude-demo/inspection-rust && cargo check --manifest-path src-tauri/Cargo.toml`
Expected: `Finished` 无错误

- [ ] **Step 4: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "build: optimize Cargo.toml dependencies and features

- Remove staticlib crate type (unnecessary for Tauri)
- Minimize tokio features from 'full' to needed subset
- Remove redundant 'log' crate (tracing covers it)
- Replace deprecated serde_yaml with serde_yml"
```

---

### Task 11: Vite + TypeScript + .gitignore 构建优化

**Files:**
- Modify: `vite.config.ts`
- Modify: `tsconfig.json`
- Modify: `.gitignore`

- [ ] **Step 1: 更新 vite.config.ts**

将 `vite.config.ts` 完整替换为：

```typescript
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    sourcemap: false,
    rollupOptions: {
      output: {
        manualChunks: {
          react: ["react", "react-dom", "react-router-dom"],
          tauri: ["@tauri-apps/api"],
        },
      },
    },
  },
});
```

- [ ] **Step 2: 更新 tsconfig.json**

将 `tsconfig.json` 第 3 行 `"target": "ES2020"` 替换为 `"target": "ES2022"`，并在 `"noFallthroughCasesInSwitch": true` 之后添加：

```json
    "noUncheckedIndexedAccess": true,
```

- [ ] **Step 3: 更新 .gitignore**

在 `.gitignore` 文件末尾追加：

```

# Claude Code
.claude/

# TypeScript build info
*.tsbuildinfo

# Coverage
coverage/
```

- [ ] **Step 4: 验证前端构建**

Run: `cd /home/neo/study/claude-demo/inspection-rust && npm run build`
Expected: `Build completed` 无错误。构建产物应包含多个 chunk（react-*.js, tauri-*.js, index-*.js）。

- [ ] **Step 5: 清理已删除的 AiConfigPage**

Run: `cd /home/neo/study/claude-demo/inspection-rust && ls src/pages/AiConfigPage.tsx 2>/dev/null`

如果文件存在（标记为 deprecated），删除它：
```bash
git rm src/pages/AiConfigPage.tsx
```

同时检查 `src/App.tsx` 中是否有对 `AiConfigPage` 的引用，如有则删除对应的 import 和 Route。

- [ ] **Step 6: 全量验证**

Run: `cd /home/neo/study/claude-demo/inspection-rust && npm run build && cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 前端和 Rust 编译均无错误

- [ ] **Step 7: Commit**

```bash
git add vite.config.ts tsconfig.json .gitignore
git commit -m "build: optimize Vite bundling, upgrade tsconfig, clean gitignore

- Add manualChunks for react/tauri vendor splitting
- Disable sourcemaps in production
- Fix __dirname for ESM compatibility
- Upgrade tsconfig target to ES2022
- Enable noUncheckedIndexedAccess for stricter type safety
- Add .claude/ and *.tsbuildinfo to gitignore"
```

---

## Verification Checklist

完成所有任务后，执行以下验证：

- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` — Rust 编译无错误
- [ ] `npm run build` — 前端构建无错误，产物有多个 chunk
- [ ] `npx tauri dev` — 应用正常启动
- [ ] 检查 `~/.local/share/inspection-rust/.key` 文件是否存在（首次启动后）
- [ ] 创建批次并执行巡检，验证不卡顿（其他操作不被阻塞）
