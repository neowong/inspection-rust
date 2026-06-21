# 网络设备巡检系统 — 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 构建一个 Tauri v2 桌面应用，通过 SSH 巡检网络设备，AI 分析结果并生成报告。

**Architecture:** Rust 后端处理 SSH 执行、AI API 调用、SQLite 存储；React 前端通过 Tauri IPC 调用 Rust 命令。同步 SQLite 通过 `Mutex<Connection>` 管理，SSH 密码和 API Key 用 Fernet 加密。

**Tech Stack:** Tauri v2, Rust, React 18, TypeScript, TailwindCSS 3, SQLite (rusqlite bundled), ssh2, reqwest, fernet

---

### Phase 0: 项目初始化

### Task 0.1: 初始化前端项目

**Files:**
- Create: `package.json`
- Create: `tsconfig.json`
- Create: `vite.config.ts`
- Create: `tailwind.config.js`
- Create: `postcss.config.js`
- Create: `index.html`
- Create: `.gitignore`

- [ ] **Step 1: 创建 package.json**

```json
{
  "name": "inspection-rust",
  "private": true,
  "version": "3.0.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-dialog": "^2",
    "@tauri-apps/plugin-fs": "^2",
    "@tauri-apps/plugin-shell": "^2",
    "class-variance-authority": "^0.7.1",
    "clsx": "^2.1.1",
    "lucide-react": "^0.468.0",
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "react-markdown": "^10.1.0",
    "react-router-dom": "^7.1.0",
    "remark-gfm": "^4.0.1",
    "tailwind-merge": "^2.6.0"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@types/react": "^18.3.12",
    "@types/react-dom": "^18.3.1",
    "@vitejs/plugin-react": "^4.3.4",
    "autoprefixer": "^10.4.20",
    "postcss": "^8.4.49",
    "tailwindcss": "^3.4.17",
    "typescript": "^5.7.2",
    "vite": "^6.0.5"
  }
}
```

- [ ] **Step 2: 安装 npm 依赖**

Run: `npm install`

- [ ] **Step 3: 创建 tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "isolatedModules": true,
    "moduleDetection": "force",
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": false,
    "noUnusedParameters": false,
    "noFallthroughCasesInSwitch": true,
    "baseUrl": ".",
    "paths": { "@/*": ["./src/*"] }
  },
  "include": ["src"]
}
```

- [ ] **Step 4: 创建 vite.config.ts**

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],
  resolve: { alias: { "@": path.resolve(__dirname, "./src") } },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
}));
```

- [ ] **Step 5: 创建 tailwind.config.js**

```js
/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        primary: { DEFAULT: "hsl(var(--primary))", foreground: "hsl(var(--primary-foreground))" },
        secondary: { DEFAULT: "hsl(var(--secondary))", foreground: "hsl(var(--secondary-foreground))" },
        destructive: { DEFAULT: "hsl(var(--destructive))", foreground: "hsl(var(--destructive-foreground))" },
        muted: { DEFAULT: "hsl(var(--muted))", foreground: "hsl(var(--muted-foreground))" },
        accent: { DEFAULT: "hsl(var(--accent))", foreground: "hsl(var(--accent-foreground))" },
        card: { DEFAULT: "hsl(var(--card))", foreground: "hsl(var(--card-foreground))" },
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
    },
  },
  plugins: [],
};
```

- [ ] **Step 6: 创建 postcss.config.js**

```js
export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
};
```

- [ ] **Step 7: 创建 index.html**

```html
<!DOCTYPE html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/vite.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>网络设备巡检系统</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 8: 创建 .gitignore**

```
node_modules/
dist/
src-tauri/target/
src-tauri/gen/
.idea/
.vscode/
*.swp
*.swo
.DS_Store
Thumbs.db
```

- [ ] **Step 9: 创建 src-tauri 目录结构和 Cargo.toml**

Run: `mkdir -p src-tauri/src/commands src-tauri/src/services src-tauri/src/db src-tauri/sql src-tauri/icons`

```toml
[package]
name = "inspection-rust"
version = "3.0.0"
description = "网络设备巡检系统 - Rust + Tauri 桌面版"
authors = ["neo"]
edition = "2021"

[lib]
name = "inspection_rust_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-shell = "2"
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"
tauri-plugin-process = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.32", features = ["bundled"] }
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid = { version = "1", features = ["v4"] }
fernet = "0.2"
ssh2 = "0.9"
log = "0.4"
async-trait = "0.1"
thiserror = "2"
anyhow = "1"
dirs = "6"
parking_lot = "0.12"
serde_yaml = "0.9"
```

- [ ] **Step 10: 创建 build.rs**

```rust
fn main() {
    tauri_build::build();
}
```

- [ ] **Step 11: 创建 tauri.conf.json**

```json
{
  "$schema": "https://raw.githubusercontent.com/nicovrc/tauri-v2-schema/refs/tags/v2.5.0/tauri.conf.schema.json",
  "productName": "网络设备巡检系统",
  "version": "3.0.0",
  "identifier": "com.inspection.rust",
  "build": {
    "frontendDist": "../dist",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "windows": [
      {
        "title": "网络设备巡检系统",
        "width": 1400,
        "height": 900,
        "minWidth": 1024,
        "minHeight": 700,
        "resizable": true,
        "fullscreen": false
      }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

- [ ] **Step 12: 验证 Rust 编译**

Run: `cargo check`
Expected: 编译成功，无错误

- [ ] **Step 13: 提交**

```bash
git add -A
git commit -m "chore: 项目脚手架 — Tauri v2 + React + TypeScript 初始化"
```

---

### Phase 1: 数据库层

### Task 1.1: SQL Schema + Rust Models + Query 辅助

**Files:**
- Create: `src-tauri/sql/001_init.sql`
- Create: `src-tauri/src/db/models.rs`
- Create: `src-tauri/src/db/query.rs`
- Create: `src-tauri/src/db/migrations.rs`
- Create: `src-tauri/src/db/seed_data.rs`
- Create: `src-tauri/src/db/mod.rs`

- [ ] **Step 1: 创建 SQL schema**

Create `src-tauri/sql/001_init.sql` with 9 tables matching the spec:
- `devices` — name/ip unique, status CHECK, device_type/vendor/model/ssh fields, template_id FK, timestamps
- `device_status_logs` — device_id FK CASCADE, old/new status, checked_at
- `inspection_templates` — name UNIQUE, vendor, model, device_type, config TEXT (JSON), description, report_template_id
- `command_pool` — vendor, command, description, category, model; UNIQUE(vendor, command)
- `inspection_batches` — name, status CHECK (8 states), triggered_by CHECK, device_ids TEXT '[]', started_at, completed_at
- `inspection_records` — batch_id FK, device_id FK, status CHECK, command_outputs TEXT '{}', ai_status CHECK, ai_result/ai_analysis/ai_suggestions/command_judgments/summary_judgment TEXT, report_path, timestamps
- `ai_model_configs` — name, provider CHECK (openai/anthropic), model_id, api_key_encrypted, base_url, is_active INTEGER
- `report_templates` — name, vendor, file_path
- `system_settings` — id INTEGER PK CHECK(id=1), report_max_output_lines DEFAULT 100; INSERT OR IGNORE default row

Add indexes on `devices(vendor)`, `devices(status)`, `devices(template_id)`, `device_status_logs(device_id)`, `command_pool(vendor)`, `inspection_batches(status)`, `inspection_records(batch_id)`, `inspection_records(device_id)`, `inspection_records(ai_status)`.

- [ ] **Step 2: 创建 Rust models**

`src-tauri/src/db/models.rs` — Serde structs matching all 9 tables plus create/update DTOs:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Device {
    pub id: i64,
    pub name: String,
    pub ip: String,
    pub device_type: String,
    pub vendor: String,
    pub model: Option<String>,
    pub ssh_username: Option<String>,
    pub ssh_password_encrypted: Option<String>,
    pub ssh_port: i64,
    pub template_id: Option<i64>,
    pub status: String,
    pub last_checked_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct DeviceCreate {
    pub name: String,
    pub ip: String,
    pub device_type: String,
    pub vendor: String,
    pub model: Option<String>,
    pub ssh_username: Option<String>,
    pub ssh_password: Option<String>,
    pub ssh_port: Option<i64>,
    pub template_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct DeviceUpdate {
    pub name: Option<String>,
    pub ip: Option<String>,
    pub device_type: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub ssh_username: Option<String>,
    pub ssh_password: Option<String>,
    pub ssh_port: Option<i64>,
    pub template_id: Option<i64>,
}

// ... (same pattern for all 9 tables)
```

Include all models: `Device`, `DeviceCreate`, `DeviceUpdate`, `InspectionTemplate`, `TemplateCreate`, `TemplateUpdate`, `CommandPool`, `CommandCreate`, `CommandUpdate`, `InspectionBatch`, `BatchCreate`, `InspectionRecord`, `AiModelConfig`, `AiConfigCreate`, `AiConfigUpdate`, `ReportTemplate`, `DeviceStatusLog`, `SystemSettings`.

Key: Exclude sensitive fields from serialization with `#[serde(skip_serializing)]` on `ssh_password_encrypted` and `api_key_encrypted`.

- [ ] **Step 3: 创建 query.rs**

Three helpers: `query_all`, `query_one`, `count`. All return `Result<T, String>`, take `&Connection`, SQL string, params slice, and row mapping closure.

- [ ] **Step 4: 创建 migrations.rs**

Read `PRAGMA user_version`, run `001_init.sql` if version < 1, bump to version 1. Remove offline/scheduled tables (version 2 cleanup, can be omitted for fresh).

- [ ] **Step 5: 创建 seed_data.rs**

Insert 65+ seed commands for H3C, 华为, 思科, 锐捷 vendors. Categories: version, clock, hardware, cpu, memory, power, fan, env, interface, vlan, log, protocol, general. Each entry: (vendor, command, description, category).

Skip insert if `command_pool` already has rows.

- [ ] **Step 6: 创建 db/mod.rs**

```rust
pub mod models;
pub mod migrations;
pub mod query;
pub mod seed_data;
```

- [ ] **Step 7: 验证编译**

Run: `cargo check`
Expected: 编译通过

- [ ] **Step 8: 提交**

```bash
git add -A && git commit -m "feat: 数据库层 — 9 表 schema + Rust 模型 + 种子数据"
```

---

### Phase 2: Rust 后端服务层

### Task 2.1: 加密服务 + SSH 执行器

**Files:**
- Create: `src-tauri/src/services/crypto.rs`
- Create: `src-tauri/src/services/inspection_runner.rs`
- Create: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: 创建 crypto.rs**

```rust
const MASTER_PASSWORD: &str = "<your-fernet-key>";

pub struct CryptoService;

impl CryptoService {
    pub fn encrypt(plaintext: &str) -> Result<String, String> {
        let fernet = fernet::Fernet::new(MASTER_PASSWORD).ok_or("Invalid Fernet key")?;
        Ok(fernet.encrypt(plaintext.as_bytes()).to_string())
    }

    pub fn decrypt(encrypted: &str) -> Result<String, String> {
        let fernet = fernet::Fernet::new(MASTER_PASSWORD).ok_or("Invalid Fernet key")?;
        let bytes = fernet.decrypt(encrypted).map_err(|e| format!("解密失败: {}", e))?;
        String::from_utf8(bytes).map_err(|_| "UTF-8 转换失败".into())
    }
}
```

- [ ] **Step 2: 创建 inspection_runner.rs**

SSH 执行器，使用 ssh2 crate：
- `is_network_vendor(vendor) -> bool` — 判断厂商是否是网络设备
- `run_commands(host, port, username, password, commands)` — 核心函数：
  - 建立 TCP 连接（10s 超时）
  - SSH 认证（密码方式）
  - 分厂商做分页禁用 (H3C: `screen-length disable`, Cisco: `terminal length 0`)
  - 逐条执行命令，收集输出
  - 每条命令 30s 超时
  - 返回 `HashMap<String, String>` (command → output)
- 内联函数 `ssh_run_netmiko_style` 处理设备特定交互逻辑

```rust
pub struct SSHSessionSource {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

pub fn run_commands(
    source: &SSHSessionSource,
    vendor: &str,
    commands: &[String],
) -> Result<HashMap<String, String>, String> {
    // TCP connect -> SSH handshake -> password auth
    // vendor-specific pagination disable
    // execute each command, read output
    // return command->output map
}
```

- [ ] **Step 3: 创建 services/mod.rs**

```rust
pub mod crypto;
pub mod inspection_runner;
pub mod ai_inspection;
pub mod report_generator;
pub mod template_generator;
```

- [ ] **Step 4: 验证编译**

Run: `cargo check`

- [ ] **Step 5: 提交**

```bash
git add -A && git commit -m "feat: 加密服务 + SSH 执行器"
```

### Task 2.2: AI 分析 + 报告生成 + 模板生成

**Files:**
- Create: `src-tauri/src/services/ai_inspection.rs`
- Create: `src-tauri/src/services/report_generator.rs`
- Create: `src-tauri/src/services/template_generator.rs`

- [ ] **Step 1: 创建 ai_inspection.rs**

```rust
pub const SYSTEM_PROMPT: &str = r#"你是一位专业的 IT 运维巡检工程师...（中文 Prompt，要求 JSON 格式返回）"#;

pub async fn analyze_command_outputs(
    api_key: &str,
    model: &str,
    base_url: &str,
    command_outputs: &HashMap<String, String>,
) -> Result<serde_json::Value, String> {
    // Build messages: system prompt + user message with command outputs
    // Call OpenAI-compatible API (reqwest POST)
    // Parse response JSON
    // Return ai_result JSON
}
```

Support both OpenAI and Anthropic providers. For Anthropic, use messages API with `x-api-key` header. For OpenAI, use chat completions API.

Prompt instructs LLM to return JSON: `{ "summary": "...", "overall": "ok/info/warning/critical", "items": [{ "command": "...", "title": "...", "status": "...", "finding": "...", "suggestion": "..." }] }`

- [ ] **Step 2: 创建 report_generator.rs**

```rust
pub fn build_markdown(ctx: &HashMap<String, serde_json::Value>) -> String {
    // Build Markdown string with sections:
    // # {device_name} 巡检报告
    // ## 基本信息 (table: 名称, IP, 厂商, 型号, 序列号, 主机名, OS, CPU, 内存...)
    // ## 巡检结果 (per command: title, status badge, finding, suggestion, raw output)
    // ## 总结
}
```

Use chrono for timestamp formatting. Read from context HashMap with fallback defaults.

- [ ] **Step 3: 创建 template_generator.rs**

```rust
pub fn generate_template(
    db: &rusqlite::Connection,
    vendor: &str,
    model: Option<&str>,
    device_type: Option<&str>,
) -> Result<serde_json::Value, String> {
    // Select commands from command_pool WHERE vendor = vendor
    // Sort by category priority (version > clock > disk > cpu > memory > hardware > interface > protocol > other)
    // Return JSON with selected command IDs
}
```

- [ ] **Step 4: 验证编译**

Run: `cargo check`

- [ ] **Step 5: 提交**

```bash
git add -A && git commit -m "feat: AI 分析 + 报告生成 + 模板生成服务"
```

---

### Phase 3: Rust 命令处理器

### Task 3.1: 设备命令 (devices.rs)

**Files:**
- Create: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/commands/devices.rs`

- [ ] **Step 1: 创建 commands/mod.rs**

```rust
pub mod devices;
pub mod templates;
pub mod inspections;
pub mod reports;
pub mod ai_config;
pub mod settings;
```

- [ ] **Step 2: 创建 devices.rs**

Command functions (all `#[tauri::command]`):
- `list_devices(vendor: Option<String>, status: Option<String>, state)` — 动态过滤查询
- `get_device(device_id: i64, state)` — 单设备查询
- `create_device(data: DeviceCreate, state)` — 校验 IP/名称唯一性，加密 SSH 密码，插入
- `update_device(device_id: i64, data: DeviceUpdate, state)` — 可选字段更新，密码变更时重新加密
- `delete_device(device_id: i64, state)`
- `batch_delete_devices(ids: Vec<i64>, state)`
- `check_device_status(device_id: i64, state)` — 尝试 SSH 连接判断在线/离线，记录状态日志
- `check_all_devices_status(state)` — 批量状态检测
- `get_device_status_log(device_id: i64, state)` — 查询状态变更历史

Helper functions:
- `device_from_row(row)` — `Row → Device` 映射
- `validate_ip(ip)` — 四段 0-255 校验
- `check_unique(db, name, ip, exclude_id)` — 唯一性检查

- [ ] **Step 3: 验证编译**

Run: `cargo check`

- [ ] **Step 4: 提交**

```bash
git add -A && git commit -m "feat: 设备 CRUD 命令 + 状态检测"
```

### Task 3.2: 模板命令 (templates.rs)

**Files:**
- Create: `src-tauri/src/commands/templates.rs`

- [ ] **Step 1: 创建 templates.rs**

Template commands:
- `list_templates(vendor: Option<String>, state)` — 含设备计数（JOIN devices COUNT）
- `get_template(template_id: i64, state)`
- `create_template(data: TemplateCreate, state)` — config 字段序列化
- `update_template(template_id: i64, data: TemplateUpdate, state)`
- `delete_template(template_id: i64, state)`
- `batch_delete_templates(ids: Vec<i64>, state)`
- `auto_generate_template(vendor: String, model: Option<String>, device_type: Option<String>, state)` — 调用 template_generator

Command pool commands:
- `list_vendors(state)` — SELECT DISTINCT vendor
- `list_commands(vendor: Option<String>, category: Option<String>, state)`
- `get_command(command_id: i64, state)`
- `create_command(data: CommandCreate, state)`
- `update_command(command_id: i64, data: CommandUpdate, state)`
- `delete_command(command_id: i64, state)`
- `batch_delete_commands(ids: Vec<i64>, state)`

- [ ] **Step 2: 验证编译**

Run: `cargo check`

- [ ] **Step 3: 提交**

```bash
git add -A && git commit -m "feat: 模板 + 命令池 CRUD 命令"
```

### Task 3.3: 巡检批次命令 (inspections.rs)

**Files:**
- Create: `src-tauri/src/commands/inspections.rs`

- [ ] **Step 1: 创建 inspections.rs**

Batch commands:
- `list_batches(status: Option<String>, state)` — 含每个批次的记录摘要
- `create_batch(data: BatchCreate, state)` — 创建设备记录，可选立即开始
- `get_batch(batch_id: i64, state)` — 含完整记录列表
- `run_batch(batch_id: i64, state)` — 异步启动 SSH 巡检，逐设备执行
- `pause_batch(batch_id: i64, state)` / `stop_batch(batch_id: i64, state)`
- `restart_batch(batch_id: i64, state)` — 重置状态后重新 run
- `retry_device(record_id: i64, state)` — 重试单个失败设备
- `delete_batch(batch_id: i64, state)` / `batch_delete_batches(ids: Vec<i64>, state)`

Record commands:
- `delete_record(record_id: i64, state)` / `batch_delete_records(ids: Vec<i64>, state)`

Run logic (spawn tokio task):
1. Update batch status → running
2. For each device in device_ids:
   - Create InspectionRecord
   - Call inspection_runner::run_commands
   - Update record with command outputs
3. After all devices complete → update batch status to completed/partially_completed

- [ ] **Step 2: 验证编译**

Run: `cargo check`

- [ ] **Step 3: 提交**

```bash
git add -A && git commit -m "feat: 巡检批次 CRUD + 执行/暂停/停止/重试"
```

### Task 3.4: 报告 + AI 配置 + 设置命令

**Files:**
- Create: `src-tauri/src/commands/reports.rs`
- Create: `src-tauri/src/commands/ai_config.rs`
- Create: `src-tauri/src/commands/settings.rs`

- [ ] **Step 1: 创建 reports.rs**

```rust
// AI Analysis
pub async fn analyze_record(record_id: i64, state) — AI 分析单条记录
pub async fn analyze_batch(batch_id: i64, state) — 批量 AI 分析

// Report generation
pub async fn generate_report(record_id: i64, state) — 生成单设备 Markdown 报告
pub async fn generate_batch_reports(batch_id: i64, state) — 生成本批次所有报告
pub async fn download_report(record_id: i64, state) — 选择保存路径
pub async fn download_batch_report(batch_id: i64, state) — 打包下载

// Report templates
pub fn list_report_templates(state)
pub fn upload_template(file_path, name, vendor, state) — 复制文件到数据目录
pub fn download_template(template_id, state)
pub fn preview_template(template_id, state) — 读取模板文件内容
pub fn preview_template_context(template_id, state) — 模板上下文预览
pub fn delete_report_template(template_id, state)
pub fn batch_delete_report_templates(ids, state)

// Helper
pub fn get_active_ai_config(state) — 获取当前激活的 AI 配置
```

- [ ] **Step 2: 创建 ai_config.rs**

```rust
pub fn list_ai_configs(state)
pub fn create_ai_config(data: AiConfigCreate, state) — 加密 API Key 后存储
pub fn update_ai_config(config_id, data: AiConfigUpdate, state)
pub fn delete_ai_config(config_id, state)
pub fn activate_ai_config(config_id, state) — 取消其他激活，激活当前
pub fn deactivate_ai_config(config_id, state) — 取消激活
```

- [ ] **Step 3: 创建 settings.rs**

```rust
pub fn get_settings(state) — 查询 system_settings 单行
pub fn update_settings(data) — 更新 system_settings
```

- [ ] **Step 4: 验证编译**

Run: `cargo check`

- [ ] **Step 5: 提交**

```bash
git add -A && git commit -m "feat: 报告/AI 配置/设置命令"
```

### Task 3.5: lib.rs — 将所有命令注册到 Tauri

**Files:**
- Create: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/main.rs`

- [ ] **Step 1: 创建 main.rs**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    inspection_rust_lib::run();
}
```

- [ ] **Step 2: 创建 lib.rs**

```rust
pub mod db;
pub mod commands;
pub mod services;

use std::sync::Arc;
use parking_lot::Mutex;
use rusqlite::Connection;
use tauri::Manager;

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
}

impl AppState {
    pub fn new(db_path: &str) -> Self {
        let conn = Connection::open(db_path).expect("Failed to open database");
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .expect("Failed to set PRAGMAs");
        db::migrations::run_migrations(&conn).expect("Failed to run migrations");
        db::seed_data::seed_command_pool(&conn).ok();
        Self { db: Arc::new(Mutex::new(conn)) }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt().with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info".into()),
    ).init();

    let app_data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("inspection-rust");
    std::fs::create_dir_all(&app_data_dir).ok();
    let db_path = app_data_dir.join("inspection.db");
    let state = AppState::new(db_path.to_str().unwrap());

    // Create data directories
    let data_dir = app_data_dir.join("data");
    for sub in &["reports", "report_templates", "uploads", "logs"] {
        std::fs::create_dir_all(data_dir.join(sub)).ok();
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            // All commands from all modules
            commands::devices::list_devices,
            // ... (40+ commands)
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Register ALL commands from devices, templates, inspections, reports, ai_config, settings modules. Plus two inline commands:
- `get_stats(state)` — COUNT queries for dashboard stats
- `health_check()` — return `{"status": "ok", "version": "3.0.0"}`

- [ ] **Step 3: 验证编译**

Run: `cargo check`
Expected: 所有命令正确注册，编译通过

- [ ] **Step 4: 提交**

```bash
git add -A && git commit -m "feat: lib.rs 入口 — AppState + 命令注册"
```

---

### Phase 4: 前端基础层

### Task 4.1: CSS + 类型 + 工具函数

**Files:**
- Create: `src/index.css`
- Create: `src/types/index.ts`
- Create: `src/lib/utils.ts`
- Create: `src/main.tsx`

- [ ] **Step 1: 创建 index.css**

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

@layer base {
  :root {
    --bg-app: 220 14% 96%;
    --bg-content: 220 14% 98%;
    --bg-card: 0 0% 100%;
    --bg-hover: 220 14% 94%;
    --bg-active: 217 91% 95%;
    --text-primary: 220 15% 15%;
    --text-secondary: 220 8% 46%;
    --text-tertiary: 220 8% 65%;
    --accent: 217 91% 56%;
    --accent-subtle: 217 91% 56% / 0.1;
    --success: 142 71% 40%;
    --warning: 38 92% 45%;
    --danger: 0 72% 48%;
    --info: 217 91% 56%;
    --border: 220 13% 88%;
    --border-light: 220 13% 92%;
    --radius: 0.5rem;
  }
  * { border-color: hsl(var(--border)); }
  body {
    background-color: hsl(var(--bg-content));
    color: hsl(var(--text-primary));
    font-family: ui-sans-serif, system-ui, ...;
    font-size: 14px;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
  }
}

::-webkit-scrollbar { width: 5px; height: 5px; }
::-webkit-scrollbar-thumb { background: hsl(var(--text-tertiary) / 0.35); border-radius: 3px; }

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(4px); }
  to { opacity: 1; transform: translateY(0); }
}
.animate-in { animation: fadeIn .2s ease-out; }
```

- [ ] **Step 2: 创建 TypeScript 接口**

`src/types/index.ts` — 所有接口与 Rust models 一一对应：Device, InspectionTemplate, CommandPool, InspectionBatch, InspectionRecord, AiModelConfig, ReportTemplate, Stats, InspectionRecordSummary, Settings。

- [ ] **Step 3: 创建 utils.ts**

```ts
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

- [ ] **Step 4: 创建 main.tsx**

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </React.StrictMode>,
);
```

- [ ] **Step 5: 提交**

```bash
git add -A && git commit -m "feat: 前端基础 — CSS 变量主题 + 类型定义 + 入口"
```

### Task 4.2: 基础 UI 组件

**Files:**
- Create: `src/components/ui/Button.tsx`
- Create: `src/components/ui/Card.tsx`
- Create: `src/components/ui/Input.tsx`

- [ ] **Step 1: 创建 Button.tsx**

cva 多态按钮，4 种 variant (primary/secondary/ghost/danger)，3 种 size (sm/md/icon)，支持 loading 状态 (Loader2 旋转图标 + disabled)，type 默认 "button"。

- [ ] **Step 2: 创建 Card.tsx**

带圆角边框的卡片容器，支持 padding toggle，`cn()` 组合 className。

- [ ] **Step 3: 创建 Input.tsx**

Input 组件：支持 size (sm/md)，统一的 focus/disabled 样式。
Select 组件（同一文件导出）：与 Input 风格统一。

- [ ] **Step 4: 提交**

```bash
git add -A && git commit -m "feat: 基础 UI 组件 — Button/Card/Input/Select"
```

### Task 4.3: 共享业务组件

**Files:**
- Create: `src/components/DataTable.tsx`
- Create: `src/components/Modal.tsx`
- Create: `src/components/StatusBadge.tsx`
- Create: `src/components/SearchInput.tsx`
- Create: `src/components/ContextMenu.tsx`
- Create: `src/components/Toolbar.tsx`

- [ ] **Step 1: 创建 DataTable.tsx**

泛型组件 `DataTable<T>`:
```tsx
interface Column<T> {
  key: string;
  header: string;
  width?: string;
  render: (row: T) => React.ReactNode;
}

interface Props<T> {
  columns: Column<T>[];
  data: T[];
  rowKey: (row: T) => string | number;
  onRowDoubleClick?: (row: T) => void;
  onContextMenu?: (e: React.MouseEvent, row: T) => void;
  emptyText?: string;
}
```

带 border 的圆角表格，sticky header，`max-h-[60vh]` 滚动。

- [ ] **Step 2: 创建 Modal.tsx**

```tsx
interface Props {
  open: boolean;
  title: string;
  width?: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  onClose: () => void;
}
```

半透明黑色遮罩，白色卡片居中，Escape 关闭，点击遮罩关闭，点击内容区不关闭。

- [ ] **Step 3: 创建 StatusBadge.tsx**

```tsx
type Status = "online" | "offline" | "unknown" | "ok" | "warning" | "critical" | "info" | "pending" | "running" | "completed" | "failed" | "stopped";
```

颜色圆点 + 中文标签。每个 status 有对应的 `STYLES`, `LABELS`, `DOT_COLORS` 字典。

- [ ] **Step 4: 创建 SearchInput.tsx**

搜索输入框，搜索图标在左侧，清除按钮（有文字时显示），Ctrl+F 自动聚焦选中。

- [ ] **Step 5: 创建 ContextMenu.tsx**

```tsx
export interface ContextMenuItem {
  label: string;
  separator?: boolean;
  danger?: boolean;
  disabled?: boolean;
  action?: () => void;
}
```

右键菜单浮层，点击外部关闭。

- [ ] **Step 6: 创建 Toolbar.tsx**

简单 flex-wrap 容器。

- [ ] **Step 7: 提交**

```bash
git add -A && git commit -m "feat: 共享组件 — DataTable/Modal/StatusBadge/SearchInput/ContextMenu/Toolbar"
```

### Task 4.4: AppShell 布局 + 路由

**Files:**
- Create: `src/hooks/useKeyboardShortcut.ts`
- Create: `src/layouts/AppShell.tsx`
- Create: `src/App.tsx`

- [ ] **Step 1: 创建 useKeyboardShortcut.ts**

```tsx
import { useEffect } from "react";

const shortcuts = new Map<string, () => void>();

export function registerShortcut(key: string, handler: () => void) {
  shortcuts.set(key, handler);
  return () => shortcuts.delete(key);
}

export function useGlobalShortcuts() {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey;
      if (ctrl && e.key === "f") e.preventDefault();
      if (ctrl && e.key === "s") e.preventDefault();
      const key = `${ctrl ? "Ctrl+" : ""}${e.key}`;
      const fn = shortcuts.get(key);
      if (fn) { e.preventDefault(); fn(); }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);
}
```

- [ ] **Step 2: 创建 AppShell.tsx**

侧边栏布局 (224px / collapsed 56px)：
- Brand header: NetInspect logo (Gauge 图标)
- 导航分组：巡检工作流 (模板/设备/执行/报告) + 系统 (AI 配置/设置)
- 每项：图标 + 中文标签，激活状态高亮（左侧蓝色指示条）
- 折叠/展开按钮（底部）
- 内容区：`<Outlet/>`
- 底部状态栏：绿色圆点 + 状态消息 + v3.1

- [ ] **Step 3: 创建 App.tsx**

```tsx
import { Routes, Route, Navigate } from "react-router-dom";
import AppShell from "./layouts/AppShell";
import { useGlobalShortcuts } from "./hooks/useKeyboardShortcut";
// Import all 7 pages...

export default function App() {
  useGlobalShortcuts();
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route path="/" element={<Navigate to="/templates" replace />} />
        <Route path="/devices" element={<DevicesPage />} />
        <Route path="/templates" element={<TemplatesPage />} />
        <Route path="/inspection" element={<InspectionPage />} />
        <Route path="/reports" element={<ReportsPage />} />
        <Route path="/ai-config" element={<AiConfigPage />} />
        <Route path="/settings" element={<SettingsPage />} />
      </Route>
    </Routes>
  );
}
```

Default redirect to `/templates`.

- [ ] **Step 4: 提交**

```bash
git add -A && git commit -m "feat: AppShell 布局 + 路由 + 全局快捷键"
```

---

### Phase 5: 前端页面

### Task 5.1: 仪表盘页面

**Files:**
- Create: `src/pages/DashboardPage.tsx`

- [ ] **Step 1: 创建 DashboardPage.tsx**

加载 stats（invoke get_stats），8 个统计卡片网格（2x4 或 4x2）：
- 设备总数 / 在线 / 离线 / 模板 / 命令 / 批次 / 进行中 / 已完成
- 每个卡片：渐变色背景（tailwind gradient），大数字，中文标签
- 页面标题："仪表盘"

### Task 5.2: 设备管理页面

**Files:**
- Create: `src/pages/DevicesPage.tsx`

- [ ] **Step 1: 创建 DevicesPage.tsx**

- 顶部 Toolbar：添加按钮 + 厂商过滤下拉 + 状态过滤下拉 + 搜索框
- DataTable 展示：名称、IP、厂商、型号、状态、模板、最后检测时间
- 右键菜单：编辑、删除、检测状态
- 添加/编辑 Modal：表单字段（名称、IP、厂商、型号、SSH 用户名、密码、端口、模板选择）
- 批次删除支持（多选 checkbox）

### Task 5.3: 巡检模板页面

**Files:**
- Create: `src/pages/TemplatesPage.tsx`

- [ ] **Step 1: 创建 TemplatesPage.tsx**

分两区：
**模板区**：Toolbar（添加 + 厂商过滤 + 搜索），DataTable（名称、厂商、命令数、描述、操作），右键菜单，添加/编辑 Modal
**命令池区**：Toolbar（添加 + 厂商过滤 + 分类过滤 + 搜索），DataTable（厂商、命令、描述、分类、操作），右键菜单
- 模板编辑：可选关联命令（多选），关联报告模板
- 自动生成模板按钮：选厂商 → 调用 auto_generate_template

### Task 5.4: 执行巡检页面

**Files:**
- Create: `src/pages/InspectionPage.tsx`

- [ ] **Step 1: 创建 InspectionPage.tsx**

- 批次列表（顶部，最新一批置顶）：名称、状态、进度、时间
- 批次详情（选中批次后展示）：该批次所有设备记录列表，每设备显示状态、AI 状态
- 创建新批次 Modal：选择设备（多选），命名
- 操作按钮：执行、暂停、停止、重试失败设备

### Task 5.5: 巡检报告页面

**Files:**
- Create: `src/pages/ReportsPage.tsx`

- [ ] **Step 1: 创建 ReportsPage.tsx**

- 批次列表：选择批次
- 记录列表：该批次下所有设备记录
- 选中记录后：
  - 查看命令输出
  - AI 分析按钮 → 调用 analyze_record
  - 显示 AI 分析结果（summary, items with status/finding/suggestion）
  - 生成报告按钮 → 调用 generate_report → 显示 Markdown 预览（react-markdown）
  - 下载报告按钮

### Task 5.6: AI 配置 + 系统设置页面

**Files:**
- Create: `src/pages/AiConfigPage.tsx`
- Create: `src/pages/SettingsPage.tsx`

- [ ] **Step 1: 创建 AiConfigPage.tsx**

- 配置列表：名称、Provider、模型、状态（激活/未激活）、操作
- 添加/编辑 Modal：名称、Provider 下拉、Model ID、API Key、Base URL（可选）
- 激活/停用按钮

- [ ] **Step 2: 创建 SettingsPage.tsx**

- 报告设置：输出行数上限（数字输入）
- 保存按钮 → invoke update_settings

- [ ] **Step 3: 提交所有页面**

```bash
git add -A && git commit -m "feat: 所有前端页面 — Dashboard/Devices/Templates/Inspection/Reports/AiConfig/Settings"
```

---

### Phase 6: 验证与收尾

### Task 6.1: 完整编译验证

- [ ] **Step 1: 全量编译**

Run: `cargo check` (Rust)
Run: `npx tsc --noEmit` (TypeScript)
Expected: 零错误

- [ ] **Step 2: 构建验证**

Run: `npm run build`
Expected: `dist/` 目录生成

- [ ] **Step 3: 最终提交**

```bash
git add -A && git commit -m "chore: 完整编译验证通过"
```
