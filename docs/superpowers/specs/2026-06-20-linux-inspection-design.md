# Linux 巡检功能设计文档

> 日期：2026-06-20
> 状态：待审批
> 方案：方案 B — 厂商适配层

## 目标

为网络设备巡检系统增加 Linux 服务器操作系统层面的巡检能力，支持通过 SSH exec channel 执行命令、sudo 提权、全面的命令集（30-40 条），并通过 AI 分析结果。

## 约束

- 不改动现有网络设备巡检逻辑（Shell 模式厂商零改动）
- 复用现有 SSH 连接、批量执行、AI 分析、报告生成的完整链路
- Linux 巡检使用 exec channel（非交互），网络设备保持 shell（交互）
- sudo 密码复用 ssh_password_encrypted 字段

---

## 一、执行引擎架构

### 1.1 当前路径（不变）

```
inspect_one_device()
  → inspection_runner::run_commands()
  → shell 会话（交互式 PTY）
  → 提示符检测 → 输出清洗
```

### 1.2 新增路径

```
inspect_one_device()
  → 读取 device.vendor → 查询 VendorProfile
  → profile.exec_mode == Exec?
      → linux_runner::run_commands_exec()
      → 每条命令一个 exec channel
      → 无需提示符检测，channel EOF 即结束
  → profile.exec_mode == Shell?
      → inspection_runner::run_commands()（现有逻辑）
```

### 1.3 接口统一

两条路径返回相同的 `IndexMap<String, String>`，调用方 `inspect_one_device()` 无需改动结果处理逻辑。

---

## 二、厂商适配层（Vendor Profile）

新增 `src-tauri/src/services/vendor_profile.rs`。

### 2.1 数据结构

```rust
pub struct VendorProfile {
    pub paging_cmds: Vec<String>,           // 登录后发送的去分页命令
    pub prompt_pattern: Option<Regex>,      // 提示符匹配正则（Exec 模式为 None）
    pub exec_mode: ExecMode,               // Shell 或 Exec
    pub sudo_mode: SudoMode,               // None 或 PipePassword
    pub clean_output: Option<fn(&str) -> String>, // 输出清洗函数（Exec 模式为 None）
    pub unrecognized_patterns: Vec<String>, // 无效命令检测模式
}

pub enum ExecMode { Shell, Exec }
pub enum SudoMode { None, PipePassword }
```

### 2.2 内置 Profile

| 厂商 | exec_mode | sudo_mode | paging_cmds | prompt_pattern |
|------|-----------|-----------|-------------|----------------|
| H3C | Shell | None | `screen-length disable` | `<.*>` 结尾 |
| 华为 | Shell | None | `screen-length disable` | `<.*>` / `[.*]` |
| 思科 | Shell | None | `terminal length 0` | `#\s*$` |
| 锐捷 | Shell | None | `terminal length 0` | `#\s*$` |
| 飞塔 | Shell | None | `config system console` / `set output standard` / `end` | `#\s*$` |
| **Linux** | **Exec** | **PipePassword** | **无** | **无（不需要）** |

### 2.3 查询函数

```rust
pub fn get_profile(vendor: &str) -> VendorProfile
```

匹配规则：先精确匹配（`"H3C"` → h3c_profile），再模糊匹配（`"linux"` / `"ubuntu"` / `"centos"` → linux_profile），最后默认 shell profile。

---

## 三、Linux Exec Runner

新增 `src-tauri/src/services/linux_runner.rs`。

### 3.1 核心函数

```rust
pub fn run_commands_exec(
    session: &Session,          // 已建立的 SSH 会话
    commands: &IndexMap<String, String>,  // 命令 → 描述
    needs_root_map: &HashMap<String, bool>, // 命令 → 是否需要 root
    ssh_password: &str,         // sudo 密码（复用 SSH 密码）
    cancel_flag: Option<Arc<AtomicBool>>,
    timeout_per_cmd: Duration,  // 每条命令超时
) -> Result<IndexMap<String, String>>
```

### 3.2 单命令执行流程

```
1. session.channel_open()
2. if needs_root:
     channel.exec("sudo -S sh -c '<escaped_cmd>'")
     channel.write(password + "\n")   // stdin 写入 sudo 密码
   else:
     channel.exec(cmd)
3. loop channel.read() until EOF → 拼接输出
4. channel.close()
5. 返回 (command, output)
```

### 3.3 超时处理

每条命令独立计时，超时后 `channel.close()` 并记录超时提示。连续 2 条超时跳过剩余命令（与现有 Shell runner 行为一致）。

### 3.4 取消支持

每条命令执行前检查 `cancel_flag`，与现有 `run_commands_with_cancel` 逻辑一致。

---

## 四、sudo 提权方案

### 4.1 命令分级

`command_pool` 表新增 `needs_root INTEGER DEFAULT 0` 字段。

需要 root 的命令（`needs_root=1`）：
- `dmidecode -t system` — 硬件信息
- `cat /var/log/syslog` — 系统日志
- `lastlog` — 登录记录
- `iptables -L -n` — 防火墙规则
- `fdisk -l` — 磁盘分区
- `lsof -i -P` — 网络连接
- `cat /etc/shadow` — 密码文件（安全审计）

不需要 root 的命令（`needs_root=0`）：
- `uname -a`、`free -h`、`df -h`、`ip addr`、`uptime`、`ps aux` 等

### 4.2 执行策略

```
if needs_root:
    channel.exec("sudo -S sh -c '<escaped_cmd>'")
    channel.write(ssh_password + "\n")
else:
    channel.exec(cmd)
```

密码通过 stdin 传入 `sudo -S`，不在命令行暴露。

---

## 五、Linux 命令集

`vendor="Linux"`，约 35 条命令。

### 5.1 系统信息（5 条，含 2 条静态信息）

| 命令 | 描述 | category | needs_root | purpose |
|------|------|----------|------------|---------|
| `hostnamectl` | 主机名和系统信息 | system | 0 | static_info |
| `uname -a` | 内核版本 | system | 0 | inspection |
| `cat /etc/os-release` | 发行版信息 | system | 0 | static_info |
| `uptime` | 运行时间和负载 | system | 0 | inspection |
| `timedatectl` | 时区和时间同步 | system | 0 | inspection |

### 5.2 CPU（3 条）

| 命令 | 描述 | category | needs_root |
|------|------|----------|------------|
| `lscpu` | CPU 架构信息 | cpu | 0 |
| `cat /proc/cpuinfo` | CPU 详细信息 | cpu | 0 |
| `top -bn1 \| head -20` | CPU 使用率快照 | cpu | 0 |

### 5.3 内存（2 条）

| 命令 | 描述 | category | needs_root |
|------|------|----------|------------|
| `free -h` | 内存使用概况 | memory | 0 |
| `cat /proc/meminfo` | 内存详细信息 | memory | 0 |

### 5.4 磁盘（4 条）

| 命令 | 描述 | category | needs_root |
|------|------|----------|------------|
| `df -h` | 磁盘使用率 | disk | 0 |
| `lsblk` | 块设备列表 | disk | 0 |
| `iostat -x 1 1` | 磁盘 I/O 统计 | disk | 0 |
| `fdisk -l` | 磁盘分区详情 | disk | 1 |

### 5.5 网络（5 条）

| 命令 | 描述 | category | needs_root |
|------|------|----------|------------|
| `ip addr` | 网络接口和 IP | network | 0 |
| `ip route` | 路由表 | network | 0 |
| `ss -tlnp` | 监听端口 | network | 0 |
| `ss -s` | 连接统计 | network | 0 |
| `cat /etc/resolv.conf` | DNS 配置 | network | 0 |

### 5.6 服务（3 条）

| 命令 | 描述 | category | needs_root |
|------|------|----------|------------|
| `systemctl list-units --type=service --state=running --no-pager` | 运行中的服务 | service | 0 |
| `systemctl list-units --state=failed --no-pager` | 失败的服务 | service | 0 |
| `systemctl list-units --type=service --state=running --no-pager \| wc -l` | 服务计数 | service | 0 |

### 5.7 进程（2 条）

| 命令 | 描述 | category | needs_root |
|------|------|----------|------------|
| `ps aux --sort=-%cpu \| head -15` | CPU 占用 Top 进程 | process | 0 |
| `ps aux --sort=-%mem \| head -15` | 内存占用 Top 进程 | process | 0 |

### 5.8 日志（3 条）

| 命令 | 描述 | category | needs_root |
|------|------|----------|------------|
| `journalctl -p err --no-pager -n 30` | 最近错误日志 | log | 0 |
| `dmesg \| tail -30` | 内核日志 | log | 0 |
| `cat /var/log/syslog \| tail -30` | 系统日志 | log | 1 |

### 5.9 安全（5 条）

| 命令 | 描述 | category | needs_root |
|------|------|----------|------------|
| `last -10` | 最近登录记录 | security | 0 |
| `lastlog \| grep -v Never` | 所有用户最后登录 | security | 0 |
| `cat /etc/passwd \| grep -v nologin \| grep -v false` | 可登录用户 | security | 0 |
| `ss -tlnp` | 监听端口（安全视角） | security | 0 |
| `iptables -L -n` | 防火墙规则 | security | 1 |

### 5.10 硬件/内核（4 条）

| 命令 | 描述 | category | needs_root |
|------|------|----------|------------|
| `dmidecode -t system` | 系统硬件信息 | hardware | 1 |
| `lspci` | PCI 设备列表 | hardware | 0 |
| `cat /proc/loadavg` | 负载均值 | hardware | 0 |
| `sysctl -a 2>/dev/null \| head -30` | 内核参数（部分） | hardware | 0 |

### 5.11 定时任务（2 条）

| 命令 | 描述 | category | needs_root |
|------|------|----------|------------|
| `crontab -l` | 当前用户定时任务 | schedule | 0 |
| `systemctl list-timers --no-pager` | systemd 定时器 | schedule | 0 |

### 5.12 静态信息提取

以下命令的 `purpose` 设为 `"static_info"`，`show_in_report=false`：

| 命令 | 提取字段 |
|------|----------|
| `hostnamectl` | `sysname`（hostname）、`model`（OS + kernel） |
| `dmidecode -t system` | `serial_number`（Serial Number）、`manufacturing_date`（不适用，留空）、`model`（Manufacturer + Product） |
| `cat /etc/os-release` | 备用 OS 信息提取 |

`build_static_info()` 新增 Linux 解析逻辑：
- `hostnamectl` 输出匹配 `Static hostname: xxx` → sysname
- `hostnamectl` 输出匹配 `Operating System: xxx` → model
- `dmidecode` 输出匹配 `Serial Number: xxx` → serial_number
- `dmidecode` 输出匹配 `Manufacturer: xxx` + `Product Name: xxx` → model

---

## 六、AI 分析增强

### 6.1 现有 prompt 保持不变

通用 prompt 已经 vendor 无关，Linux 命令输出直接适用。

### 6.2 补充 Linux 参考阈值

在 system prompt 末尾追加一段：

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

实现方式：`ai_inspection.rs` 的 `build_system_prompt()` 根据 `device_type` 或 `vendor` 判断是否追加 Linux 阈值段。

---

## 七、数据库变更

### 7.1 Migration（新增 migration 文件）

```sql
-- command_pool 新增 needs_root 字段
ALTER TABLE command_pool ADD COLUMN needs_root INTEGER DEFAULT 0;

-- devices.device_type 约束扩展
-- SQLite 不支持 ALTER CHECK，使用触发器或重建方式
-- 推荐：新建触发器校验 device_type 值
```

### 7.2 种子数据

`seed_data.rs` 新增 ~35 条 Linux 命令，`vendor="Linux"`，`needs_root` 按上表标记。

---

## 八、前端改动

### 8.1 设备管理页（DevicesPage.tsx）

- `device_type` 下拉选项加 `"服务器"`（值 `server`）
- `vendor` 下拉选项加 `"Linux"`
- 选择 `device_type=server` 时，自动联动推荐 `vendor=Linux`

### 8.2 模板页（TemplatesPage.tsx）

- 命令池厂商 Tab 加 "Linux"
- Linux 命令列表显示 🔒 标记表示 `needs_root`
- 模板编辑器支持 Linux 命令选择

### 8.3 巡检页 / 报告页

无改动 — 执行流程和报告生成对 vendor 透明。

---

## 九、文件变更清单

| 文件 | 变更类型 | 说明 |
|------|----------|------|
| `src-tauri/src/services/vendor_profile.rs` | **新增** | 厂商适配层定义 |
| `src-tauri/src/services/linux_runner.rs` | **新增** | exec channel 执行器 |
| `src-tauri/src/services/mod.rs` | 小改 | 注册新模块 |
| `src-tauri/src/commands/inspections.rs` | 小改 | `inspect_one_device` 根据 profile 选择 runner；`build_static_info` 加 Linux 解析 |
| `src-tauri/src/db/models.rs` | 小改 | CommandPool 加 needs_root 字段 |
| `src-tauri/src/db/migrations.rs` | 小改 | 新增 migration |
| `src-tauri/src/db/seed_data.rs` | 中改 | 新增 ~35 条 Linux 命令 |
| `src-tauri/src/services/ai_inspection.rs` | 小改 | prompt 追加 Linux 阈值 |
| `src-tauri/src/services/inspection_runner.rs` | 小改 | exec channel 调用入口（或由 linux_runner 独立处理） |
| `src/pages/DevicesPage.tsx` | 小改 | 加 server 类型和 Linux 厂商选项 |
| `src/pages/TemplatesPage.tsx` | 小改 | Linux 命令池 Tab + needs_root 标记 |
| `src/types/index.ts` | 小改 | CommandPool 类型加 needs_root |

---

## 十、风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| sudo 密码在 exec channel stdin 中传输 | 低 — SSH 加密通道内 | 已是最佳实践 |
| 某些命令在非交互 exec 下行为不同 | 中 — 输出格式可能变化 | 所有命令加管道避免交互（`top -bn1`、`--no-pager`） |
| 精细化 needs_root 管理 | 低 — 需要维护标记 | seed 数据一次性设定，用户可自定义 |
| exec channel 在某些 SSH 服务器上被禁用 | 低 — 少数受限环境 | 文档中注明要求，失败时给出明确错误提示 |
