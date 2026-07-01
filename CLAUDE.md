# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

运维巡检系统 (IT Operations Inspection System) — Rust + Tauri v2 桌面版。通过 SSH 连接网络设备（H3C/华为/思科/锐捷/飞塔）、Linux 服务器，执行巡检命令收集状态数据，调用 AI（OpenAI/Anthropic/DeepSeek）分析结果并生成可编辑 DOCX 报告。

## Tech Stack

- **Desktop**: Tauri v2 (Rust backend + webview frontend)
- **Frontend**: React 18 + Vite 6 + TypeScript + TailwindCSS 3
- **Backend (Rust)**: rusqlite (SQLite bundled), ssh2, reqwest, fernet, serde, chrono, tokio, docx-rs, indexmap, digest, md-5, sha1, sha2, hmac, aes, des, cipher, rand
- **UI**: lucide-react icons, class-variance-authority, tailwind-merge/clsx
- **AI**: OpenAI / Anthropic API via reqwest
- **Routing**: react-router-dom v7
- **Build**: tauri v2 CLI, `npx tauri dev` / `npx tauri build`

## Architecture

```
ai-inspection/
├── src/                          # React frontend (flat structure)
│   ├── main.tsx                  # Entry: BrowserRouter + App
│   ├── App.tsx                   # Routes (6 pages: templates/devices/inspection/reports/settings, AI config merged into settings)
│   ├── index.css                 # CSS variables (HSL theming), scrollbar, animations
│   ├── types/index.ts            # Shared TypeScript interfaces
│   ├── lib/utils.ts              # cn() - tailwind-merge + clsx helper
│   ├── hooks/useKeyboardShortcut.ts  # Global keyboard shortcut registry
│   ├── layouts/AppShell.tsx      # Shell: sidebar nav + status bar + <Outlet/>
│   ├── components/
│   │   ├── DataTable.tsx         # Generic typed table (Column<T> pattern)
│   │   ├── Modal.tsx             # Overlay modal with Escape close (props: open, title, width, onClose, footer, children)
│   │   ├── StatusBadge.tsx       # Status → color dot + Chinese label
│   │   ├── SearchInput.tsx       # Search input with Ctrl+F focus
│   │   ├── ContextMenu.tsx       # Right-click context menu
│   │   ├── Toolbar.tsx           # Flex toolbar wrapper
│   │   └── ui/
│   │       ├── Button.tsx        # cva-based button (primary/secondary/ghost/danger)
│   │       ├── Card.tsx          # Card container
│   │       └── Input.tsx         # Input + Select components
│   └── pages/
│       ├── DashboardPage.tsx     # Stats cards overview
│       ├── ToolsPage.tsx         # Toolbox: subnet calc, alive scan, TCP/UDP port scan, web check, SNMP v2c/v3, Zabbix agent
│       ├── LogAnalysisPage.tsx   # Device log parsing and analysis
│       ├── DevicesPage.tsx       # Device CRUD + status check
│       ├── TemplatesPage.tsx     # Inspection templates + command pool CRUD
│       ├── InspectionPage.tsx    # Batch creation, running, monitoring
│       ├── ReportManagementPage.tsx # AI analysis, DOCX reports
│       ├── SettingsPage.tsx      # AI model config CRUD (integrated)
│       └── AboutPage.tsx         # Open-source about page, donation placeholders, SVG workflow
├── src-tauri/                    # Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json           # App config (1400x900, no devUrl)
│   ├── src/
│   │   ├── main.rs               # fn main() → lib::run()
│   │   ├── lib.rs                # AppState (Mutex<Connection>), run(), all #[tauri::command] handlers registered
│   │   ├── db/
│   │   │   ├── models.rs         # Rust structs (Device, Template, Batch, Record, AiConfig, etc.)
│   │   │   ├── migrations.rs     # Pragmatic version-based migrations
│   │   │   ├── query.rs          # query_all / query_one / count helpers
│   │   │   └── seed_data.rs      # 85 seed commands for H3C/华为/思科/锐捷
│   │   ├── commands/             # Tauri command handlers (each file = domain module)
│   │   │   ├── devices.rs        # list/get/create/update/delete/check-status
│   │   │   ├── templates.rs      # Template CRUD + command pool CRUD + auto-generate
│   │   │   ├── inspections.rs    # Batch CRUD + run/pause/stop/restart/retry
│   │   │   ├── reports.rs        # AI analysis, report generation, report templates
│   │   │   ├── ai_config.rs      # AI model config CRUD + activate/deactivate
│   │   │   └── tools.rs           # Toolbox: scan hosts/ports/UDP, web check, SNMP v2c/v3, Zabbix agent
│   │   └── services/
│   │       ├── crypto.rs         # Fernet encryption (password/API key)
│   │       ├── inspection_runner.rs  # SSH execution via ssh2 (netmiko-style)
│   │       ├── ai_inspection.rs  # AI analysis prompt + API call
│   │       ├── report_config.rs  # DOCX report template config schema + command descriptions
│   │       ├── docx_engine.rs    # DOCX report generator (code-built Word tables, static_info, batch zip/combined)
│   │       ├── json_util.rs      # Shared JSON parse helpers (parse_json_map, parse_json_object)
│   │       ├── template_generator.rs # Auto-generate templates from command pool
│   │       ├── live_scanner.rs   # ICMP ping + TCP fallback(135/445) sweep, CIDR parsing, parallel scan
│   │       ├── port_scanner.rs   # TCP connect scan + UDP scan (connect/ICMP detection)
│   │       ├── web_checker.rs    # HTTP/HTTPS status check, IP defaults to HTTP
│   │       ├── snmp_checker.rs   # SNMP v2c + v3 USM (MD5/SHA1/SHA256 auth, DES/AES128 priv)
│   │       └── zabbix_checker.rs # Zabbix agent passive mode detection (protocol frame)
│   └── sql/001_init.sql          # 8 tables: devices, device_status_logs, inspection_templates, command_pool, inspection_batches, inspection_records, ai_model_configs, report_templates
```

## Key Patterns

- **Tauri IPC instead of HTTP**: All Rust functions exposed as `#[tauri::command]`, called from frontend via `invoke("command_name", { args })`
- **Sync SQLite**: `Mutex<Connection>` in `AppState` — all commands acquire `state.db.lock()`
- **Flat pages, not features**: Unlike the Python predecessor, pages are a single directory, not per-feature subdirectories
- **CSS variable theming**: All colors use HSL variables (`--bg-app`, `--text-primary`, `--accent`, etc.) — no Tailwind color classes beyond what's needed
- **Custom UI components, no shadcn/ui**: Button uses `class-variance-authority` for variants; Modal, DataTable, etc. are hand-rolled
- **DataTable generic pattern**: `DataTable<T>` with typed `Column<T>[]` config for rendering
- **Chinese-first**: All labels, messages, and prompts in Chinese. AI inspection prompts are Chinese.
- **Form standard pattern**: Pages with modal forms use `saving` + `saveError` states, `<Button loading={saving}>`, and error alert box `.bg-[hsl(var(--danger)_/_0.1)]` for validation
- **Per-action loading state**: 同一行/同一目标对象上有多个会触发后台异步操作的按钮时，loading state 必须按动作区分（如 `{id, action: "analyze" | "direct"}`），不能让两个按钮共享同一个标量 `loading={generating === r.id}`，否则点击 A 按钮 B 也会跟着转圈。同一对象的"另一个动作"应在当前动作进行时设为 `disabled`
- **DataTable**: Supports `onRowClick`, `onRowDoubleClick`, `selectedKey` props. TemplatesPage is reference implementation
- **Config field encoding**: Template `config` is stored as JSON string in SQLite, `JSON.stringify()` on frontend, `serde_json::from_str()` on list. Frontend types declare `config: {...}` object, invoke params must send string
- **API key/password fields**: Rust models use `api_key_encrypted` / `ssh_password_encrypted`. Frontend send `api_key_encrypted` / `ssh_password_encrypted` (NOT `api_key` / `ssh_password`)
- **SSH (netmiko-style)**: libssh2 only (no system sshpass). Persistent shell channel per device. `extract_prompt` → base_prompt (strips terminator). `output_contains_prompt` uses `contains()` not `ends_with()`. Per-command timeout 15s, 2 consecutive timeouts → skip remaining. `screen-length disable` must succeed.
- **Device concurrency**: `run_batch` / `create_batch`(auto_start) spawn `tokio::spawn` per device, shared `Arc<Mutex<Connection>>`. `inspect_one_device()` per-device flow. Progress tracked via `Arc<std::sync::Mutex<String>>` → DB poller every 2s.
- **Background tasks**: `lib.rs` spawns std thread for 5-minute device status polling (`poll_device_statuses`), uses `try_lock` to avoid blocking.
- **Report generation**: DOCX reports built directly from `ReportTemplateConfig` (`report_templates.config_json`). Batch zip/combined resolve the report template **per-record** (per device vendor), not from the first record — multi-vendor batches use each device's matched template. Static-info commands (`show_in_report=false`) are filtered out by `inspect_one_device::visible_outputs` and do **not** need a second filter in `docx_engine` (the old `is_static_info_command` string-match was removed as redundant/buggy).
- **tsconfig `noEmit: true` is REQUIRED**: Without it, `tsc` generates stale `.js` files in `src/` that Vite loads instead of `.tsx` — causing "changes not reflected" bugs
- **Branding**: `public/open-inspection-logo.svg` used as open-source sidebar logo. App icons (`icon.ico`, `*.png`) in `src-tauri/icons/` generated from server rack SVG.
- **Windows ping reliability**: `live_scanner.rs` parses ping output for `TTL=`/`time=` instead of exit code, since Windows exits 0 even on timeout. Uses `CREATE_NO_WINDOW` to suppress cmd popups. Falls back to TCP connect on ports 135/445 when ICMP is blocked.
- **中国输入法兼容**: All toolbox IP input fields use `style={{ imeMode: "disabled" }}` to force English mode, avoiding manual IME switching.
- **Sticky headers**: All page headers use `sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm relative`
- **Dashboard cards**: Clickable with `cursor-pointer` + `navigate(path)`. Summary + detail cards both have path field.
- **Command pool UI**: Vendor tabs + collapsible category groups (ChevronDown/Right). Each command shows edit/delete icons on hover. Custom vendors supported via `+` button in Select; custom vendors auto-appear in tabs and sort before built-in vendors. Category groups: `performance` (cpu+memory), `hardware` (fan+power+hardware), `interface` (interface+vlan), `env` (运行环境). Commands have `expectation` field for AI judgment hints.
- **AI judgment with expectations**: `command_pool.expectation` stores per-command AI hints (e.g., "CPU usage should be < 80%"). During AI analysis, expectations are loaded and injected as `【期望】` into the prompt alongside `【命令】` and `【输出】`.
- **Report regeneration**: Batch toolbar buttons dynamically change text based on state: no AI result → "AI评判", has result → "重新AI评判"; no report → "人工评判", has report → "重新生成". `processingBatches` map tracks per-batch loading independently.
- **Shell injection prevention**: `instance_name`/`db_username` validated at create/update boundary with `validate_shell_identifier()` (whitelist `[A-Za-z0-9_.:-]`). `wrap_cmd` uses single-quote `sh -c '...'` wrapping (double-quote inner `$`/backtick survived). `write_all_nb` handles WouldBlock retries in SSH non-blocking mode.
- **Path safety**: `safe_report_path()` uses `canonicalize().starts_with(reports_dir)` for all file delete/copy operations. `safe_remove_report()` is the unified delete helper used by `delete_record_report` and `delete_batch`.
- **DOCX report engine**: Uses `docx-rs` (0.4) to generate Word reports directly from `ReportTemplateConfig` (stored in `report_templates.config_json`), not from uploaded `.docx` templates. Report columns default to `序号 / 项目 / 巡检内容 / 评判结论`; command outputs are rendered as `<sysname>command` plus original output (first bare command echo stripped). Device static info comes from `inspection_records.static_info` first, then `devices.sysname`/device fallback. Header/footer use Word paragraph borders for single separator lines.
- **Command order / static info commands**: `inspection_runner::run_commands` returns `IndexMap<String, String>` to preserve execution order. Inspection template config uses `commands[{command_id,purpose,show_in_report,extract_fields}]`; commands with `purpose: "static_info"` can extract `sysname`, `model`, `serial_number`, `manufacturing_date` into `inspection_records.static_info` and are hidden from report details when `show_in_report=false`. Template editor supports drag-and-drop command reordering.
- **SpinInput component**: Custom number input with hidden native spinners + ChevronUp/Down buttons. Use `<SpinInput>` instead of `<input type="number">` for timeout/port fields.
- **Toolbox IP validation**: All tools that take IP input (`scan_ports`, `scan_udp_ports`, `snmp_v2c_get`, `snmp_v3_get`, `check_zabbix_agent`) validate with `parse::<IpAddr>()` at entry, returning "请输入有效的 IP 地址" on failure.
- **UDP scan**: Uses `socket.connect()` before `send()`/`recv()` — connected UDP sockets receive ICMP Port Unreachable as `ECONNREFUSED`. Without `connect()`, ICMP errors are not delivered. Protocol probes for DNS(53), SNMP(161), NTP(123).
- **SNMP v3**: Self-implemented ASN.1 BER codec. Key localization: 1MB password hashing → Ku → Kul = Hash(Ku||engineID||Ku). Auth key lengths: MD5=16, SHA1=20, SHA256=32. MAC always 12 bytes. msgData is SEQUENCE (0x30) unencrypted, OCTET STRING (0x04) when encrypted. Engine discovery via empty GET → REPORT, auto-retry on `notInTimeWindow`.
- **Zabbix protocol**: Frame format `ZBXD\x01` + LE64 length + JSON. Response read in two phases: header (13 bytes) → parse length → read rest. Shows raw hex on parse failure for debugging.
- **Batch creation non-blocking**: `create_batch`(auto_start) and `run_batch` spawn `tokio::spawn` background tasks and return immediately — frontend shows batch instantly, 3s polling updates progress. Helper `await_handles_and_finalize()` updates final status.
- **Window initialization**: `visible: true` + `transparent: true` + `decorations: true` in `tauri.conf.json`. Never use `visible: false` + `window.show()` — Linux WebKitGTK 下标题栏装饰不会正确初始化，导致关闭按钮失效。body 内联 `background-color` 减少闪烁。
- **Windows 日志 CRLF**: tracing_subscriber 默认写 `\n`，Windows 记事本需要 `\r\n`。`CrlfWriter<W>` + `CrlfMakeWriter<M>` 包装器在 `#[cfg(windows)]` 下自动转换。
- **版本检测 internal- 前缀兼容**: `check_update` 的 GitHub API 可能返回 `internal-vx.y.z` tag。需先 `trim_start_matches("internal-")` 再 `trim_start_matches('v')`，避免解析错误导致误报更新。
- **数据库巡检多厂商模板**: 数据库模板（`DB_VENDORS`）支持从多个厂商混合选择命令（如 Linux + MySQL），右侧可选命令面板显示厂商标签页。非数据库模板保持单一厂商过滤。`TemplateCommandSpec` 新增 `vendor` 字段用于执行时区分命令来源。
- **命令与部署方式解耦**: 命令库只存裸命令（如 `mysql -e 'SHOW STATUS'`），不包含 `docker exec` / `podman exec` 前缀。执行引擎 `wrap_for_deployment` 按命令的 `vendor` 和设备的 `deployment` 自动包装：OS 厂商命令在宿主机执行，数据库命令按部署方式注入认证后执行。
- **数据库认证注入**: 所有部署方式的数据库命令都自动注入 `db_username` / `db_password`。包安装：`MYSQL_PWD='xxx' mysql -u'root' ...`；容器：`docker exec -e 'MYSQL_PWD=xxx' mysql sh -c '...'`。密码用 `shell_quote_docker`（单引号包裹），绝不用 `shell_escape_dq`（双引号转义在单引号上下文中会损坏密码）。
- **Shell 转义上下文规则**: `shell_escape_dq`（`\`→`\\`）仅用于 `sh -c "..."` 双引号上下文；`sh -c '...'` 单引号上下文中反斜杠是字面值，只用 `shell_quote_single`（`'`→`'\''`）。混用会导致密码泄露到 shell。
- **报告回显剥离**: `strip_command_echo` 支持四种形态：裸命令、带提示符前缀、容器多行回显（`docker exec ...`）、包安装带环境变量前缀（`MYSQL_PWD='xxx' mysql ...`）。进度显示通过 `sanitize_cmd_for_display` 脱敏 `-e KEY=VALUE` 和 `MYSQL_PWD='xxx'` 模式。
- **报告输出 key 映射**: `execute_device_ssh` 返回的 `command_outputs` key 通过 `wrapped_to_orig` 映射回原始命令，使 `cmd_descs` 能正确匹配命令描述。报告巡检项显示描述而非包装后的命令。
- **设备导入导出**: CSV 格式，Tauri dialog plugin 弹出保存框。导出 17 列（密码不导出），导入支持带/不带表头两种格式，自动检测。必填项 name/ip/type/vendor，type 支持中英文。冲突检测：同名跳过、同类型同 IP 跳过。模板按名称自动匹配。`import_devices_csv`（Rust）包含完整字段解析、唯一性检查、密码加密。前端三页签（网络设备/服务器/数据库）提供不同示例。
- **模板删除引用检查**: 前端调用 `check_template_devices` 预查引用，模态内展示引用设备列表，有引用时隐藏删除按钮。后端 `delete_template` 和 `batch_delete_templates` 使用 `async` + `Result<Struct, String>` 模式。
- **Tauri v2 关键坑**: sync 命令返回 `Result::Err(String)` 时，JS invoke promise 既不 resolve 也不 reject，永远 pending。必须使用 `async fn` + 返回 `Ok({ok: false, error: "..."})` 结构体，前端检查 `res.ok` 判断。
- **TFTP 协议要点**: 块大小 512B（标准兼容）；文件大小恰好为 512 整数倍时必须额外发送空 DATA 包（0 字节数据）表示传输结束；每个客户端应在独立 tokio::spawn 中处理，共享 Arc<UdpSocket>；UDP 低端口 (<1024) 在 Linux 需要 `setcap cap_net_bind_service=+ep`。
- **全局审计 (2026-07-01)**: 修复 15 项问题 — Tauri sync Err 不 reject、TFTP 重写（Arc+独立任务+空 DATA+错误恢复）、CSV 导入事务、前端 stale closure/ref 修复、Modal backdrop、批量操作并发保护等。详见 `memory/session-2026-07-01-round2.md` 和 `memory/tftp-known-issues.md`。
- **部署方式**: 仅支持 `direct`（包安装）/ `docker` / `podman`，已移除 k8s 支持。

## Windows 交叉编译注意事项

- Cargo profile 的 `strip` 值在 `x86_64-pc-windows-gnu` 目标上不支持 `"debug"`，需用 `"symbols"` 或 `"debuginfo"`
	- Release profile 配置：`lto = "fat"` + `codegen-units = 1` + `panic = "abort"`，release 二进制约 19MB
- 只跑 `cargo build --target x86_64-pc-windows-gnu` 不会构建前端（Loader2 动画等）也不会正确处理图标嵌入，须先执行 `npm run build` 再编译 Rust
- 前端改动（新增图标、组件等）必须经过 `npm run build` 才会打包进二进制
- 图标文件 `icon.ico` 和 `*.png` 通过 tauri-build 的 build.rs 处理嵌入
- **BFD 链接器 DLL 导出限制**：`cdylib` 导出符号超过 65535 个时，MinGW BFD 链接器会报 `export ordinal too large`。解决方案：
  1. `.cargo/config.toml` 中设置 `linker = "/home/neo/.local/bin/mingw-lld-wrapper"` 切换为 LLD
  2. 依赖包装脚本 `scripts/mingw-lld-wrapper`（翻译 `-Wl,` 前缀、过滤 gcc 驱动参数、添加 CRT 对象、`--exclude-all-symbols`）
  3. 需要 `ld.lld` 在 PATH 中（可用 Rust 自带的 `rust-lld` 做符号链接：`ln -s $(rustup which rust-lld) ~/.local/bin/ld.lld`）

## Dev Commands

```bash
# Frontend dev server (port 1420)
npm run dev

# Desktop dev (run after npm run dev in another terminal)
npx tauri dev

# Rust type check
cargo check

# Rust build
cargo build                   # debug
cargo build --release         # release (15MB)
npm run build:release         # frontend + Rust 一步编译

# Windows cross-compile
npm run build:win

# Production desktop bundle (installer)
npx tauri build               # produces .msi / .deb

# Frontend build only
npm run build

# Version sync (updates Cargo.toml + tauri.conf.json + package.json)
npm run version 3.53.0
```

### Release profile 优化

```
[profile.release]
opt-level = "z"        # 体积优先
lto = "fat"            # 链接去死代码 (~30%缩减)
codegen-units = 1      # 最大化单函数优化
strip = "symbols"      # 剥离符号
panic = "abort"        # 移除展开表
```

注意：**不使用 UPX**，因为压缩后的 exe 易被杀软误报。release 二进制约 15MB。
不需要额外工具（upx 等），`cargo build --release` 即可。|

## Data & State

- SQLite DB auto-created at `~/.local/share/inspection-rust/inspection.db`
- Data dirs: `reports/`, `report_templates/`, `uploads/`, `logs/`
- Seed commands: `seed_command_pool` uses `INSERT OR IGNORE` (relies on `UNIQUE(vendor, command)`), runs **every launch** and is idempotent — new vendors are added incrementally. Never gate seeding on `COUNT > 0` (that caused a fresh-install bug where migration 17's fortigate rows made seed skip all other vendors).
- **Pause/stop/retry cancellation**: `pause_batch`/`stop_batch` set the `batch_cancels` AtomicBool flag so running SSH tasks stop at the next command boundary; `finalize_batch_status` preserves a `paused` batch (won't auto-overwrite to completed/stopped). `retry_device` registers a cancel flag under the batch_id and finalizes the batch when it owns the batch; `restart_batch` cancels in-flight tasks and clears stale flags before resetting.
- Fernet key: first launch auto-generates random key via `Fernet::generate_key()` and stores in `~/.local/share/inspection-rust/.key` (permissions 0o600). NOT hardcoded — legacy `MASTER_PASSWORD` references are outdated.
- Release binary is standalone (frontend embedded, no devUrl)
- **数据库容器部署**: 用户直接在表单填容器名（`instance_name` 字段），`docker exec <容器名> sh -c "mysql ..."` 直连执行。通过退出码区分错误：127=客户端未安装，其他非0=容器未运行。不再用端口发现（docker-compose 内部网络不映射端口时失效）
- **IP 唯一性按设备类型**: DB 层无 UNIQUE 约束（v31 迁移移除），应用层 `check_unique()` 按 `device_type` 检查。同 IP 可以加 Linux 设备和数据库设备
- **仪表盘筛选导航**: 使用 URL 参数 `?type=switch,router` / `?status=online` / `?tab=commands`，目标页面读取参数同步筛选器，筛选器变更回写 URL
- **模板命令简化**: `purpose` / `show_in_report` / `extract_fields` 已从模板命令配置中移除，所有命令都是巡检项。静态信息采集独立于模板运行
- **SSH TCP 超时不回退**: `connect_session()` 检测到 TCP 连接失败直接返回错误，不再浪费 10s 试旧算法
- **linux_runner TCP 预检**: `run_commands_exec()` 入口处 3s TCP 探测，不通直接返回，避免 N 个 worker 同时超时
- **检测结果 `_warn` 字段**: `detect_db_info_sync` 返回 JSON 含 `_warn` 键，前端解析后以 warn 级别提示具体原因（密码错/端口不对/客户端未安装）
