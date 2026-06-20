# Linux 巡检功能实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为网络设备巡检系统增加 Linux 服务器巡检能力，通过 SSH exec channel 执行命令、sudo 提权、35 条命令集、AI 分析。

**Architecture:** 新增厂商适配层（Vendor Profile）+ Linux exec channel 执行器。现有 Shell 模式厂商零改动。`inspect_one_device()` 根据 vendor profile 选择 Shell 或 Exec 路径，返回格式统一。

**Tech Stack:** Rust (ssh2, indexmap), React/TypeScript, SQLite

---

## 文件结构

| 文件 | 职责 |
|------|------|
| `src-tauri/src/services/vendor_profile.rs` | **新增** — 厂商适配层：ExecMode、SudoMode、VendorProfile 定义 + get_profile() |
| `src-tauri/src/services/linux_runner.rs` | **新增** — exec channel 执行器：run_commands_exec() |
| `src-tauri/src/services/mod.rs` | 注册新模块 |
| `src-tauri/src/db/migrations.rs` | migration 18: command_pool 加 needs_root |
| `src-tauri/src/db/models.rs` | CommandPool 加 needs_root 字段 |
| `src-tauri/src/db/seed_data.rs` | 新增 ~35 条 Linux 命令 |
| `src-tauri/src/commands/inspections.rs` | read_device_inspection_data 加 needs_root 查询；execute_device_ssh 走 profile 分发；build_static_info 加 Linux 解析 |
| `src-tauri/src/services/ai_inspection.rs` | system prompt 追加 Linux 阈值 |
| `src/lib/constants.ts` | VENDORS 加 "Linux"，CATEGORIES 加 Linux 类别 |
| `src/types/index.ts` | CommandPool 加 needs_root |
| `src/pages/DevicesPage.tsx` | device_type 加 "server"，vendor 联动 |
| `src/pages/TemplatesPage.tsx` | Linux Tab + needs_root 标记 |

---

### Task 1: 数据库 — migration + model + seed

**Files:**
- Modify: `src-tauri/src/db/migrations.rs`
- Modify: `src-tauri/src/db/models.rs`
- Modify: `src-tauri/src/db/seed_data.rs`

- [ ] **Step 1: 添加 migration 18 — command_pool 加 needs_root 字段**

在 `src-tauri/src/db/migrations.rs` 末尾、`Ok(())` 之前添加：

```rust
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
```

- [ ] **Step 2: CommandPool struct 加 needs_root 字段**

在 `src-tauri/src/db/models.rs` 的 `CommandPool` struct 中添加字段：

```rust
pub struct CommandPool {
    pub id: i64,
    pub vendor: String,
    pub command: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub model: Option<String>,
    pub needs_root: bool,       // ← 新增
    pub created_at: String,
    pub updated_at: String,
}
```

找到 `command_from_row` 函数（或构建 CommandPool 的 row.get 位置），添加：

```rust
needs_root: row.get::<_, i64>("needs_root").unwrap_or(0) != 0,
```

- [ ] **Step 3: 更新 seed INSERT 语句支持 needs_root**

在 `src-tauri/src/db/seed_data.rs` 中，将 INSERT 语句从 4 列改为 5 列：

```rust
let mut stmt = tx
    .prepare("INSERT OR IGNORE INTO command_pool (vendor, command, description, category, needs_root) VALUES (?1, ?2, ?3, ?4, ?5)")
    .map_err(|e| e.to_string())?;
```

将现有的所有种子数据 tuple 从 `(vendor, cmd, desc, category)` 改为 `(vendor, cmd, desc, category, 0i64)`。每个 `.execute()` 调用增加第 5 个参数 `0`。

- [ ] **Step 4: 添加 Linux 种子命令**

在 `seed_data.rs` 中 `let linux_commands` 数组（约 35 条）：

```rust
let linux_commands: Vec<(&str, &str, &str, &str, i64)> = vec![
    // 系统信息
    ("Linux", "hostnamectl", "主机名和系统信息", "system", 0),
    ("Linux", "uname -a", "内核版本", "system", 0),
    ("Linux", "cat /etc/os-release", "发行版信息", "system", 0),
    ("Linux", "uptime", "运行时间和负载", "system", 0),
    ("Linux", "timedatectl", "时区和时间同步", "system", 0),
    // CPU
    ("Linux", "lscpu", "CPU 架构信息", "cpu", 0),
    ("Linux", "cat /proc/cpuinfo", "CPU 详细信息", "cpu", 0),
    ("Linux", "top -bn1 | head -20", "CPU 使用率快照", "cpu", 0),
    // 内存
    ("Linux", "free -h", "内存使用概况", "memory", 0),
    ("Linux", "cat /proc/meminfo", "内存详细信息", "memory", 0),
    // 磁盘
    ("Linux", "df -h", "磁盘使用率", "disk", 0),
    ("Linux", "lsblk", "块设备列表", "disk", 0),
    ("Linux", "iostat -x 1 1", "磁盘 I/O 统计", "disk", 0),
    ("Linux", "fdisk -l", "磁盘分区详情", "disk", 1),
    // 网络
    ("Linux", "ip addr", "网络接口和 IP", "network", 0),
    ("Linux", "ip route", "路由表", "network", 0),
    ("Linux", "ss -tlnp", "监听端口", "network", 0),
    ("Linux", "ss -s", "连接统计", "network", 0),
    ("Linux", "cat /etc/resolv.conf", "DNS 配置", "network", 0),
    // 服务
    ("Linux", "systemctl list-units --type=service --state=running --no-pager", "运行中的服务", "service", 0),
    ("Linux", "systemctl list-units --state=failed --no-pager", "失败的服务", "service", 0),
    // 进程
    ("Linux", "ps aux --sort=-%cpu | head -15", "CPU 占用 Top 进程", "process", 0),
    ("Linux", "ps aux --sort=-%mem | head -15", "内存占用 Top 进程", "process", 0),
    // 日志
    ("Linux", "journalctl -p err --no-pager -n 30", "最近错误日志", "log", 0),
    ("Linux", "dmesg | tail -30", "内核日志", "log", 0),
    ("Linux", "cat /var/log/syslog | tail -30", "系统日志", "log", 1),
    // 安全
    ("Linux", "last -10", "最近登录记录", "security", 0),
    ("Linux", "lastlog | grep -v Never", "所有用户最后登录", "security", 0),
    ("Linux", "cat /etc/passwd | grep -v nologin | grep -v false", "可登录用户", "security", 0),
    ("Linux", "iptables -L -n", "防火墙规则", "security", 1),
    // 硬件/内核
    ("Linux", "dmidecode -t system", "系统硬件信息", "hardware", 1),
    ("Linux", "lspci", "PCI 设备列表", "hardware", 0),
    ("Linux", "cat /proc/loadavg", "负载均值", "hardware", 0),
    ("Linux", "sysctl -a 2>/dev/null | head -30", "内核参数", "hardware", 0),
    // 定时任务
    ("Linux", "crontab -l", "当前用户定时任务", "schedule", 0),
    ("Linux", "systemctl list-timers --no-pager", "systemd 定时器", "schedule", 0),
];
```

在 seed 函数末尾追加 Linux 命令的插入循环：

```rust
for (vendor, command, description, category, needs_root) in &linux_commands {
    stmt.execute(rusqlite::params![vendor, command, description, category, needs_root])
        .map_err(|e| format!("seed linux: {} - {}", command, e))?;
}
```

- [ ] **Step 5: 编译验证**

Run: `cargo check`
Expected: 编译通过，无错误

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/db/migrations.rs src-tauri/src/db/models.rs src-tauri/src/db/seed_data.rs
git commit -m "feat(linux): database migration + model + seed data for Linux inspection"
```

---

### Task 2: Vendor Profile 适配层

**Files:**
- Create: `src-tauri/src/services/vendor_profile.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: 创建 vendor_profile.rs**

```rust
//! 厂商适配层 — 根据设备厂商返回对应的 SSH 执行策略
//!
//! 网络设备（H3C/华为/思科等）使用交互式 Shell 会话，
//! Linux 服务器使用 exec channel（非交互）。

/// 执行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecMode {
    /// 交互式 Shell — 持久 PTY 会话，提示符检测
    Shell,
    /// Exec channel — 每条命令独立 channel，无需提示符检测
    Exec,
}

/// sudo 提权模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SudoMode {
    /// 不需要提权
    None,
    /// 通过 stdin 写入 sudo 密码（sudo -S）
    PipePassword,
}

/// 厂商行为配置
pub struct VendorProfile {
    pub exec_mode: ExecMode,
    pub sudo_mode: SudoMode,
}

/// 根据厂商名称获取对应的 VendorProfile
///
/// 匹配规则：精确匹配 → 小写模糊匹配 → 默认 Shell
pub fn get_profile(vendor: &str) -> VendorProfile {
    let lower = vendor.to_lowercase();
    match lower.as_str() {
        "linux" | "ubuntu" | "centos" | "rocky" | "debian" | "rhel" | "suse" | "fedora" | "almalinux" => {
            VendorProfile {
                exec_mode: ExecMode::Exec,
                sudo_mode: SudoMode::PipePassword,
            }
        }
        _ => VendorProfile {
            exec_mode: ExecMode::Shell,
            sudo_mode: SudoMode::None,
        },
    }
}
```

- [ ] **Step 2: 在 mod.rs 注册新模块**

在 `src-tauri/src/services/mod.rs` 中添加：

```rust
pub mod vendor_profile;
pub mod linux_runner;  // 下一个 task 创建，先加注释占位
```

- [ ] **Step 3: 编译验证**

Run: `cargo check`
Expected: 编译通过（linux_runner 还不存在，暂时注释掉 `pub mod linux_runner;`）

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/services/vendor_profile.rs src-tauri/src/services/mod.rs
git commit -m "feat(linux): add vendor profile layer for exec/shell dispatch"
```

---

### Task 3: Linux Exec Runner

**Files:**
- Create: `src-tauri/src/services/linux_runner.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: 创建 linux_runner.rs**

```rust
//! Linux exec channel 执行器
//!
//! 每条命令通过独立的 SSH exec channel 执行，无需提示符检测。
//! 需要 root 权限的命令通过 sudo -S + stdin 密码方式提权。

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ssh2::Session;

use crate::services::inspection_runner::SSHSessionSource;

/// 每条命令执行的超时时间
const CMD_TIMEOUT: Duration = Duration::from_secs(30);

/// 连续超时次数阈值，超过则跳过剩余命令
const MAX_CONSECUTIVE_TIMEOUTS: usize = 2;

/// 通过 exec channel 执行一组 Linux 命令
///
/// - 普通命令：`channel.exec(cmd)`
/// - 需要 root 的命令：`channel.exec("sudo -S sh -c '...'")` + stdin 写密码
/// - 每条命令独立 channel，执行完即关闭
/// - 返回 IndexMap<命令, 输出>，保持命令顺序
pub fn run_commands_exec(
    source: &SSHSessionSource,
    commands: &[String],
    needs_root_map: &HashMap<String, bool>,
    cancel: Option<Arc<AtomicBool>>,
    on_progress: Option<Arc<std::sync::Mutex<String>>>,
) -> Result<indexmap::IndexMap<String, String>, String> {
    tracing::info!(
        "Linux SSH exec 开始: {}@{}:{}, 命令数={}",
        source.username, source.host, source.port, commands.len()
    );

    // 1. 建立 SSH 连接（复用现有连接逻辑）
    let session = crate::services::inspection_runner::connect_session(source)?;

    let mut outputs = indexmap::IndexMap::new();
    let mut consecutive_timeouts = 0usize;

    // 2. 逐条命令执行
    for cmd in commands {
        // 取消检查
        if let Some(ref flag) = cancel {
            if flag.load(Ordering::Relaxed) {
                tracing::info!("Linux exec 取消，已执行 {}/{} 条命令", outputs.len(), commands.len());
                break;
            }
        }

        // 更新进度
        if let Some(ref progress) = on_progress {
            if let Ok(mut p) = progress.lock() {
                *p = cmd.clone();
            }
        }

        let needs_root = needs_root_map.get(cmd).copied().unwrap_or(false);

        match exec_single_command(&session, cmd, needs_root, &source.password) {
            Ok(output) => {
                consecutive_timeouts = 0;
                outputs.insert(cmd.clone(), output);
            }
            Err(e) => {
                if e.contains("超时") {
                    consecutive_timeouts += 1;
                    outputs.insert(cmd.clone(), format!("[命令执行超时: {}]", cmd));
                    if consecutive_timeouts >= MAX_CONSECUTIVE_TIMEOUTS {
                        tracing::warn!(
                            "Linux exec 连续 {} 次超时，跳过剩余命令",
                            consecutive_timeouts
                        );
                        // 填充剩余命令为超时
                        for remaining_cmd in commands.iter().skip(outputs.len()) {
                            outputs.insert(
                                remaining_cmd.clone(),
                                "[因前序命令超时已跳过]".to_string(),
                            );
                        }
                        break;
                    }
                } else {
                    // 非超时错误，记录但继续执行
                    outputs.insert(cmd.clone(), format!("[执行错误: {}]", e));
                }
            }
        }
    }

    tracing::info!(
        "Linux exec 完成: {}/{} 条命令成功",
        outputs.len(),
        commands.len()
    );
    Ok(outputs)
}

/// 执行单条命令
fn exec_single_command(
    session: &Session,
    cmd: &str,
    needs_root: bool,
    password: &str,
) -> Result<String, String> {
    let channel = session.channel_open().map_err(|e| format!("打开 channel 失败: {}", e))?;

    if needs_root {
        // sudo -S 通过 stdin 写入密码
        // 用 sh -c 包装以处理管道和特殊字符
        let escaped = cmd.replace('\'', "'\\''");
        let sudo_cmd = format!("sudo -S sh -c '{}'", escaped);
        channel.exec(&sudo_cmd).map_err(|e| format!("exec 失败: {}", e))?;
        // channel.stream(0) 返回 stdin stream
        let mut stdin_stream = channel.stream(0);
        writeln!(stdin_stream, "{}", password).map_err(|e| format!("写入 sudo 密码失败: {}", e))?;
        stdin_stream.flush().ok();
        // 短暂等待 sudo 处理密码
        std::thread::sleep(Duration::from_millis(200));
    } else {
        channel.exec(cmd).map_err(|e| format!("exec 失败: {}", e))?;
    }

    // 读取输出直到 EOF
    let mut output = String::new();
    let mut buf = [0u8; 4096];
    let start = Instant::now();

    loop {
        if start.elapsed() > CMD_TIMEOUT {
            let _ = channel.close();
            return Err("超时".to_string());
        }

        match channel.read(&mut buf) {
            Ok(0) => break, // EOF
            Ok(n) => {
                output.push_str(&String::from_utf8_lossy(&buf[..n]));
            }
            Err(e) => {
                let kind = e.kind();
                if kind == std::io::ErrorKind::WouldBlock || kind == std::io::ErrorKind::TimedOut {
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
                let _ = channel.close();
                return Err(format!("读取输出失败: {}", e));
            }
        }
    }

    // 读取 stderr（sudo 密码提示等）
    let mut stderr = String::new();
    let mut stderr_buf = [0u8; 2048];
    while let Ok(n) = channel.stderr().read(&mut stderr_buf) {
        if n == 0 {
            break;
        }
        stderr.push_str(&String::from_utf8_lossy(&stderr_buf[..n]));
    }

    let _ = channel.close();

    // 清理输出：去除 sudo 密码提示行
    let output = clean_exec_output(&output, &stderr);

    Ok(output)
}

/// 清理 exec channel 输出
///
/// - 去除 sudo 的 "[sudo] password for xxx:" 提示行
/// - 去除尾部空行
fn clean_exec_output(stdout: &str, stderr: &str) -> String {
    let mut result = String::new();

    // stdout 直接使用（exec 模式无回显、无提示符）
    for line in stdout.lines() {
        // 跳过 sudo 密码提示（通常在 stderr，但某些配置可能在 stdout）
        if line.contains("[sudo]") && line.contains("password") {
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }

    // 如果 stdout 为空但 stderr 有内容（某些命令输出到 stderr）
    if result.trim().is_empty() && !stderr.is_empty() {
        for line in stderr.lines() {
            if line.contains("[sudo]") && line.contains("password") {
                continue;
            }
            result.push_str(line);
            result.push('\n');
        }
    }

    result.trim_end().to_string()
}

/// 将命令列表和 needs_root 标记转为 HashMap
pub fn build_needs_root_map(
    commands: &[(String, bool)],
) -> HashMap<String, bool> {
    commands
        .iter()
        .map(|(cmd, needs_root)| (cmd.clone(), *needs_root))
        .collect()
}
```

- [ ] **Step 2: 在 mod.rs 注册 linux_runner**

确认 `src-tauri/src/services/mod.rs` 中有：

```rust
pub mod vendor_profile;
pub mod linux_runner;
```

- [ ] **Step 3: 编译验证**

Run: `cargo check`
Expected: 可能有 TemplateCommandSpecWithRoot 未定义的错误（在 Task 4 中解决），暂时用临时类型绕过或注释掉 build_needs_root_map

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/services/linux_runner.rs src-tauri/src/services/mod.rs
git commit -m "feat(linux): add exec channel runner with sudo support"
```

---

### Task 4: 巡检分发逻辑 — inspections.rs 改造

**Files:**
- Modify: `src-tauri/src/commands/inspections.rs`

- [ ] **Step 1: TemplateCommandSpec 加 needs_root 字段**

```rust
#[derive(Debug, Clone)]
struct TemplateCommandSpec {
    command: String,
    show_in_report: bool,
    extract_fields: Vec<String>,
    needs_root: bool,  // ← 新增
}
```

- [ ] **Step 2: read_device_inspection_data 查询 needs_root**

修改 command_pool 的 SQL 查询，从 `SELECT id, command` 改为 `SELECT id, command, needs_root`：

```rust
let sql = format!(
    "SELECT id, command, needs_root FROM command_pool WHERE id IN ({})",
    ids.join(",")
);
let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
let rows = stmt
    .query_map(
        &[] as &[&dyn rusqlite::types::ToSql],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2).unwrap_or(0),
            ))
        },
    )
    .map_err(|e| e.to_string())?;
let mut m = std::collections::HashMap::new();
for r in rows {
    if let Ok((id, command, needs_root)) = r {
        m.insert(id, (command, needs_root != 0));
    }
}
```

更新后续使用 `cmd_texts` 的地方：

```rust
let (command, needs_root) = cmd_texts
    .get(cmd_id)
    .cloned()
    .ok_or_else(|| format!("命令 ID {} 不存在", cmd_id))?;

commands.push(TemplateCommandSpec {
    command,
    show_in_report,
    extract_fields,
    needs_root,
});
```

- [ ] **Step 3: execute_device_ssh 改为 profile 分发**

```rust
fn execute_device_ssh(
    device: &Device,
    username: &str,
    password: &str,
    commands: &[TemplateCommandSpec],  // 改为接收完整 spec
    on_progress: Option<Arc<std::sync::Mutex<String>>>,
    cancel: Arc<AtomicBool>,
) -> Result<indexmap::IndexMap<String, String>, String> {
    let port = u16::try_from(device.ssh_port)
        .ok()
        .filter(|&p| p > 0)
        .ok_or_else(|| format!("设备 '{}' SSH 端口非法: {}", device.name, device.ssh_port))?;
    let source = SSHSessionSource {
        host: device.ip.clone(),
        port,
        username: username.to_string(),
        password: password.to_string(),
    };

    let profile = crate::services::vendor_profile::get_profile(&device.vendor);

    match profile.exec_mode {
        crate::services::vendor_profile::ExecMode::Exec => {
            // Linux exec channel 模式
            let cmd_strings: Vec<String> = commands.iter().map(|s| s.command.clone()).collect();
            let needs_root_map: std::collections::HashMap<String, bool> = commands
                .iter()
                .map(|s| (s.command.clone(), s.needs_root))
                .collect();
            crate::services::linux_runner::run_commands_exec(
                &source,
                &cmd_strings,
                &needs_root_map,
                Some(cancel),
                on_progress,
            )
        }
        crate::services::vendor_profile::ExecMode::Shell => {
            // 现有 Shell 模式（不变）
            let cmd_strings: Vec<String> = commands.iter().map(|s| s.command.clone()).collect();
            inspection_runner::run_commands_with_cancel(
                &source,
                &device.vendor,
                &cmd_strings,
                on_progress,
                Some(cancel),
            )
        }
    }
}
```

- [ ] **Step 4: 更新 inspect_one_device 中的调用**

在 `inspect_one_device` 中，`execute_device_ssh` 的调用需要传入 `&commands`（完整 spec）而不是 `&cmd_strings`。找到类似这样的代码：

```rust
let cmd_strings: Vec<String> = commands.iter().map(|s| s.command.clone()).collect();
// ...
execute_device_ssh(&device, &username, &password, &cmd_strings, ...)
```

改为：

```rust
execute_device_ssh(&device, &username, &password, &commands, ...)
```

- [ ] **Step 5: build_static_info 增加 Linux 解析**

在 `build_static_info` 的 `match field.as_str()` 中添加 Linux 特有字段的解析：

```rust
"sysname" => {
    // 先尝试网络设备格式，再尝试 Linux hostnamectl 格式
    extract_sysname(output)
        .or_else(|| extract_hostnamectl_field(output, "Static hostname"))
}
```

添加新函数：

```rust
/// 从 hostnamectl 输出中提取字段值
fn extract_hostnamectl_field(output: &str, field_name: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        // hostnamectl 输出格式: "   Static hostname: xxx"
        if let Some(rest) = trimmed.strip_prefix(field_name) {
            let value = rest.trim_start().strip_prefix(':').unwrap_or(rest).trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}
```

更新 `extract_model` 以支持 Linux dmidecode 输出：

```rust
fn extract_model(output: &str) -> Option<String> {
    extract_by_patterns(
        output,
        &[
            "DEVICE_NAME",
            "PRODUCT_NAME",
            "Product Name",
            "Version",
            "Platform Type",
            "Model",
            "Operating System",       // ← 新增：hostnamectl
            "Manufacturer",           // ← 新增：dmidecode
        ],
    )
    .map(|value| {
        value
            .split_whitespace()
            .next()
            .unwrap_or(value.as_str())
            .trim_matches(',')
            .to_string()
    })
}
```

更新 `extract_by_patterns` 中的 serial_number 模式以支持 dmidecode：

```rust
"serial_number" | "sn" => extract_by_patterns(
    output,
    &["DEVICE_SERIAL_NUMBER", "SERIAL_NUMBER", "Serial Number", "Serial-Number"],
),
```

这个已经包含了 `"Serial Number"` 模式，dmidecode 输出格式为 `Serial Number: XXX`，可以匹配。

- [ ] **Step 6: 编译验证**

Run: `cargo check`
Expected: 编译通过

- [ ] **Step 7: 提交**

```bash
git add src-tauri/src/commands/inspections.rs
git commit -m "feat(linux): dispatch to exec/shell runner based on vendor profile"
```

---

### Task 5: inspection_runner 导出 connect_session

**Files:**
- Modify: `src-tauri/src/services/inspection_runner.rs`

- [ ] **Step 1: 将 connect_session 改为 pub**

找到 `connect_session` 函数定义：

```rust
pub fn connect_session(source: &SSHSessionSource) -> Result<Session, String> {
```

确认已经是 `pub`。如果不是，改为 `pub`。

- [ ] **Step 2: 编译验证**

Run: `cargo check`
Expected: 编译通过

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/services/inspection_runner.rs
git commit -m "feat(linux): export connect_session for linux_runner reuse"
```

---

### Task 6: AI 分析增强

**Files:**
- Modify: `src-tauri/src/services/ai_inspection.rs`

- [ ] **Step 1: 在 SYSTEM_PROMPT 末尾追加 Linux 阈值参考**

找到 `SYSTEM_PROMPT` 常量，在其末尾（return JSON 说明之后）追加：

```
当分析 Linux 服务器巡检数据时，参考以下阈值：
- CPU 使用率 > 80% → warning, > 95% → critical
- 内存使用率 > 85% → warning, > 95% → critical
- 磁盘使用率 > 80% → warning, > 90% → critical
- load average > CPU 核心数 → warning
- failed services > 0 → warning
- 关键端口未监听（如 22/80/443） → info
- /var/log 中有 error 级别日志 → warning
```

- [ ] **Step 2: 编译验证**

Run: `cargo check`
Expected: 编译通过

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/services/ai_inspection.rs
git commit -m "feat(linux): add Linux threshold references to AI system prompt"
```

---

### Task 7: 前端 — 常量 + 类型 + 设备页 + 模板页

**Files:**
- Modify: `src/lib/constants.ts`
- Modify: `src/types/index.ts`
- Modify: `src/pages/DevicesPage.tsx`
- Modify: `src/pages/TemplatesPage.tsx`

- [ ] **Step 1: constants.ts — VENDORS 加 Linux，CATEGORIES 加新类别**

```ts
export const VENDORS = ["H3C", "华为", "思科", "锐捷", "飞塔", "Linux", "其它"] as const;

export const CATEGORIES = [
  "version", "clock", "cpu", "memory", "hardware", "storage",
  "interface", "vlan", "log", "protocol", "vpn", "ha", "security", "wireless", "general",
  "system", "disk", "network", "service", "process", "schedule",  // ← Linux 新增
] as const;
```

- [ ] **Step 2: types/index.ts — CommandPool 加 needs_root**

```ts
export interface CommandPool {
  id: number;
  vendor: string;
  command: string;
  description: string | null;
  category: string | null;
  model: string | null;
  needs_root: boolean;  // ← 新增
  created_at: string;
  updated_at: string;
}
```

- [ ] **Step 3: DevicesPage.tsx — device_type 加 server，vendor 联动**

找到 device_type 相关逻辑。当前 `device_type` 是自动推导的：

```ts
device_type: form.model ? "switch" : "router",
```

改为增加 `server` 选项。在表单中添加 device_type 下拉：

```tsx
<select
  value={form.device_type || ""}
  onChange={(e) => setForm({ ...form, device_type: e.target.value })}
>
  <option value="switch">交换机</option>
  <option value="router">路由器</option>
  <option value="firewall">防火墙</option>
  <option value="loadbalancer">负载均衡</option>
  <option value="server">服务器</option>
</select>
```

添加 vendor 联动逻辑：

```ts
// 当 device_type 变为 server 时，自动推荐 vendor=Linux
const handleDeviceTypeChange = (type: string) => {
  const updates: any = { device_type: type };
  if (type === "server" && (!form.vendor || ["H3C", "华为", "思科", "锐捷", "飞塔"].includes(form.vendor))) {
    updates.vendor = "Linux";
  }
  setForm({ ...form, ...updates });
};
```

- [ ] **Step 4: TemplatesPage.tsx — CATEGORY_LABELS 加 Linux 类别**

在 `CATEGORY_LABELS` 中添加：

```ts
const CATEGORY_LABELS: Record<string, string> = {
  // ... 现有 ...
  system: "系统信息",    // ← 新增
  disk: "磁盘",         // ← 新增
  network: "网络",       // ← 新增
  service: "服务",       // ← 新增
  process: "进程",       // ← 新增
  schedule: "定时任务",   // ← 新增
};
```

在命令列表中，为 `needs_root` 的命令显示 🔒 标记。找到命令渲染位置，添加：

```tsx
{cmd.needs_root && <span title="需要 root 权限">🔒</span>}
```

- [ ] **Step 5: 前端构建验证**

Run: `npm run build`
Expected: 构建成功，无 TypeScript 错误

- [ ] **Step 6: 提交**

```bash
git add src/lib/constants.ts src/types/index.ts src/pages/DevicesPage.tsx src/pages/TemplatesPage.tsx
git commit -m "feat(linux): frontend - vendor/type options, category labels, needs_root badge"
```

---

### Task 8: 全量编译 + 端到端验证

- [ ] **Step 1: Rust 编译**

Run: `cargo build`
Expected: 编译成功

- [ ] **Step 2: 前端构建**

Run: `npm run build`
Expected: 构建成功

- [ ] **Step 3: 桌面应用启动验证**

Run: `npx tauri dev`
Expected: 应用启动，设备管理页可见"服务器"类型和"Linux"厂商选项

- [ ] **Step 4: 功能验证清单**

1. 设备管理：创建 device_type=server, vendor=Linux 的设备
2. 模板管理：命令池 Tab 可见 Linux，命令列表显示 🔒 标记
3. 创建 Linux 巡检模板，选择包含 needs_root 命令
4. 执行巡检（如有 Linux 测试机）：验证 exec channel + sudo 提权
5. AI 分析：验证 Linux 阈值生效

- [ ] **Step 5: 最终提交**

```bash
git add -A
git commit -m "feat(linux): complete Linux server inspection support"
```
