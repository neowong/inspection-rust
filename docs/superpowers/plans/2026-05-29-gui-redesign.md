# GUI 桌面重构 — 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 Tauri 巡检系统从 WEB 风格重构为桌面原生 GUI 风格，Markdown → PDF 报告管道，砍掉 4 个功能模块，前端交互完全 GUI 化。

**Architecture:** Rust 后端 6 个命令模块 + 4 个服务模块；React 前端 7 个页面 + 通用组件库 + 右键菜单/键盘快捷键系统。Tauri v2 栈不变，数据库 11→9 张表。

**Tech Stack:** Tauri v2, Rust (rusqlite, ssh2, reqwest, fernet), React 18, TypeScript, TailwindCSS, react-markdown

---

## 文件结构

```
src-tauri/src/
├── commands/   (6 modules: devices, templates, inspections, reports, ai_config, settings)
├── services/   (4 modules: crypto, inspection_runner, ai_inspection, report_generator)
├── db/         (models, migrations, query, seed_data — 不变)
├── lib.rs      (AppState + 命令注册 + crud 宏 + AppError)
└── main.rs     (不变)

src/
├── App.tsx              (路由 + 全局 Context/快捷键)
├── layouts/
│   └── AppShell.tsx      (窗口布局: 菜单栏 + 侧边栏 + 内容 + 状态栏)
├── pages/               (7 个页面组件)
│   ├── DashboardPage.tsx
│   ├── DevicesPage.tsx
│   ├── TemplatesPage.tsx
│   ├── InspectionPage.tsx
│   ├── ReportsPage.tsx
│   ├── AiConfigPage.tsx
│   └── SettingsPage.tsx
├── components/          (通用 GUI 组件)
│   ├── ContextMenu.tsx
│   ├── Modal.tsx
│   ├── DataTable.tsx
│   ├── StatusBadge.tsx
│   ├── SearchInput.tsx
│   └── Toolbar.tsx
├── hooks/
│   ├── useInvoke.ts
│   └── useKeyboardShortcut.ts
├── lib/
│   └── utils.ts
└── types/
    └── index.ts
```

---

### Task 1: 砍掉 Chat + Offline + Scheduled 命令模块

**Files:**
- Delete: `src-tauri/src/commands/chat.rs`
- Delete: `src-tauri/src/commands/offline.rs`
- Delete: `src-tauri/src/commands/scheduled_tasks.rs`
- Modify: `src-tauri/src/commands/mod.rs`

- [ ] **Step 1: 删除三个命令模块文件**

Run:
```bash
rm src-tauri/src/commands/chat.rs
rm src-tauri/src/commands/offline.rs
rm src-tauri/src/commands/scheduled_tasks.rs
```

- [ ] **Step 2: 更新 commands/mod.rs**

Read `src-tauri/src/commands/mod.rs`, remove the three `pub mod` lines.

- [ ] **Step 3: 删除对应的服务模块**

Run:
```bash
rm src-tauri/src/services/chat_agent.rs
rm src-tauri/src/services/scheduler.rs
```

- [ ] **Step 4: 更新 services/mod.rs**

Read `src-tauri/src/services/mod.rs`, remove `pub mod chat_agent;` and `pub mod scheduler;`.

- [ ] **Step 5: 验证编译**

Run: `cd src-tauri && cargo check 2>&1`
Expected: 很多 "unresolved import" 和 "cannot find" 错误（因为 lib.rs 还在引用它们），这是预期的。

---

### Task 2: 更新 lib.rs 和 Cargo.toml

**Files:**
- Modify: `src-tauri/src/lib.rs` (删除 chat/offline/scheduled 的注册)
- Modify: `src-tauri/Cargo.toml` (移除 docx-rs, tokio-cron-scheduler, cron)

- [ ] **Step 1: 从 Cargo.toml 移除无用依赖**

Edit `src-tauri/Cargo.toml`, remove:
```
docx-rs = "0.4"
tokio-cron-scheduler = "0.13"
cron = "0.13"
```

- [ ] **Step 2: 从 lib.rs 删除 chat/offline/scheduled 的 handler 注册**

Edit `src-tauri/src/lib.rs`:
```rust
// 删除这些 handler 行:
commands::chat::chat_stream,
commands::offline::export_scripts,
commands::offline::parse_upload_file,
commands::offline::import_with_mapping,
commands::offline::upload_result,
commands::offline::list_imports,
commands::offline::delete_import,
commands::scheduled_tasks::list_tasks,
commands::scheduled_tasks::create_task,
commands::scheduled_tasks::get_task,
commands::scheduled_tasks::update_task,
commands::scheduled_tasks::delete_task,
commands::scheduled_tasks::batch_delete_tasks,
commands::scheduled_tasks::pause_task,
commands::scheduled_tasks::resume_task,
```

- [ ] **Step 3: 验证编译通过**

Run: `cd src-tauri && cargo check 2>&1`
Expected: PASS (无错误)

---

### Task 3: 合并命令库功能到模板模块

**Files:**
- Modify: `src-tauri/src/commands/templates.rs` (追加 command_pool 的 CRUD 命令)
- Delete: `src-tauri/src/commands/command_pool.rs`
- Modify: `src-tauri/src/commands/mod.rs`

- [ ] **Step 1: 读取 command_pool.rs 的命令函数**

Read `src-tauri/src/commands/command_pool.rs`, 记录所有 `#[tauri::command]` 函数签名。

- [ ] **Step 2: 将命令池函数追加到 templates.rs 末尾**

Read `src-tauri/src/commands/templates.rs`, 在文件末尾追加 command_pool.rs 中的全部函数：`list_vendors`, `list_commands`, `get_command`, `create_command`, `update_command`, `delete_command`, `batch_delete_commands`.

- [ ] **Step 3: 删除 command_pool.rs 并更新 mod.rs**

Run:
```bash
rm src-tauri/src/commands/command_pool.rs
```

Edit `src-tauri/src/commands/mod.rs`, remove `pub mod command_pool;`.

- [ ] **Step 4: 更新 lib.rs handler 注册**

Edit `src-tauri/src/lib.rs`，将原来引用 `commands::command_pool::*` 的命令改为 `commands::templates::*` 对应的函数名。

- [ ] **Step 5: 验证编译**

Run: `cd src-tauri && cargo check 2>&1`
Expected: PASS

---

### Task 4: 合并 batches → inspections.rs（仅批次生命周期 + 记录删除）

**Files:**
- Create: `src-tauri/src/commands/inspections.rs` (仅 batches.rs 全部 + inspection_records.rs 中的 delete_record/batch_delete_records)
- Delete: `src-tauri/src/commands/batches.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 inspections.rs**

创建 `src-tauri/src/commands/inspections.rs`，包含：
- 从 `batches.rs` 复制全部函数（批次生命周期）
- 从 `inspection_records.rs` 仅复制 `delete_record` 和 `batch_delete_records`

```bash
cat src-tauri/src/commands/batches.rs > src-tauri/src/commands/inspections.rs
# 然后手动从 inspection_records.rs 复制 delete_record / batch_delete_records 并清理重复 use
```

- [ ] **Step 2: 删除 batches.rs**

Run: `rm src-tauri/src/commands/batches.rs`

Edit `src-tauri/src/commands/mod.rs`, replace `pub mod batches;` with `pub mod inspections;`.

- [ ] **Step 3: 更新 lib.rs handler 前缀**

`commands::batches::` → `commands::inspections::`  
`commands::inspection_records::delete_record` → `commands::inspections::delete_record`  
`commands::inspection_records::batch_delete_records` → `commands::inspections::batch_delete_records`

- [ ] **Step 4: 验证编译**

Run: `cd src-tauri && cargo check 2>&1`
Expected: "unresolved import" 关于 inspection_records 的分析/报告函数（下一步处理）

---

### Task 5: 创建 reports.rs（AI 分析 + Markdown 报告 + 报告模板）

**Files:**
- Create: `src-tauri/src/commands/reports.rs`
- Modify: `src-tauri/src/commands/inspection_records.rs` (删除已移出的函数)
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 reports.rs**

创建 `src-tauri/src/commands/reports.rs`，合并三个来源：
1. 从 `inspection_records.rs` 复制 AI/报告函数: `analyze_record`, `analyze_batch`, `generate_report`, `generate_batch_reports`, `download_report`, `download_batch_report`, `preview_template_context`, `get_active_ai_config`
2. 从 `report_templates.rs` 复制全部模板管理函数
3. 清理重复 `use` 语句，统一模块头

```rust
use crate::AppState;
use tauri::State;
// ... 所有分析 + 报告 + 模板管理函数
```

- [ ] **Step 2: 清理 inspection_records.rs**

从 `inspection_records.rs` 删除已移走的 8 个函数（保留 `delete_record` 和 `batch_delete_records`，这些已并入 inspections.rs）。

- [ ] **Step 3: 更新 mod.rs 和 lib.rs**

Edit `src-tauri/src/commands/mod.rs`: 添加 `pub mod reports;`  
Edit `src-tauri/src/lib.rs`: 将所有 `commands::inspection_records::analyze_*` / `commands::inspection_records::generate_*` / `commands::report_templates::*` 改为 `commands::reports::*`

- [ ] **Step 4: 删除空文件**

Run:
```bash
rm src-tauri/src/commands/report_templates.rs
rm src-tauri/src/commands/inspection_records.rs  # 内容已全部迁移到 inspections.rs 和 reports.rs
```

Edit `src-tauri/src/commands/mod.rs`, remove `pub mod inspection_records;` and `pub mod report_templates;`.

- [ ] **Step 5: 验证编译**

Run: `cd src-tauri && cargo check 2>&1`
Expected: PASS

---

### Task 6: Markdown 报告生成器

**Files:**
- Rewrite: `src-tauri/src/services/report_generator.rs`

- [ ] **Step 1: 重写 report_generator.rs**

用以下内容完全替换：

```rust
/// Markdown 巡检报告生成服务
use std::collections::HashMap;
use std::path::Path;

/// 构建单设备报告的 Markdown 字符串
pub fn build_markdown(ctx: &HashMap<String, serde_json::Value>) -> String {
    let device_name = ctx.get("device_name").and_then(|v| v.as_str()).unwrap_or("未知设备");
    let device_ip = ctx.get("device_ip").and_then(|v| v.as_str()).unwrap_or("");
    let vendor = ctx.get("device_vendor").and_then(|v| v.as_str()).unwrap_or("-");
    let model = ctx.get("device_model").and_then(|v| v.as_str()).unwrap_or("-");
    let sn = ctx.get("device_sn").and_then(|v| v.as_str()).unwrap_or("-");
    let hostname = ctx.get("device_hostname").and_then(|v| v.as_str()).unwrap_or("-");
    let os_release = ctx.get("os_release").and_then(|v| v.as_str()).unwrap_or("-");
    let kernel = ctx.get("kernel").and_then(|v| v.as_str()).unwrap_or("-");
    let cpu = ctx.get("cpu_cores").and_then(|v| v.as_str()).unwrap_or("-");
    let mem = ctx.get("mem_total").and_then(|v| v.as_str()).unwrap_or("-");
    let mfg_date = ctx.get("manufacturing_date").and_then(|v| v.as_str()).unwrap_or("-");
    let summary = ctx.get("summary").and_then(|v| v.as_str()).unwrap_or("");

    let command_outputs: HashMap<String, String> = ctx.get("command_outputs")
        .and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();

    let command_judgments: HashMap<String, String> = ctx.get("command_judgments")
        .and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();

    let ts = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");

    let mut md = String::new();
    md.push_str(&format!("# {} 巡检报告\n\n", device_name));
    md.push_str(&format!("> 生成时间: {}\n\n", ts));

    // 基本信息
    md.push_str("## 基本信息\n\n");
    md.push_str("| 项目 | 内容 |\n|------|------|\n");
    md.push_str(&format!("| 设备名称 | {} |\n", device_name));
    md.push_str(&format!("| IP 地址 | {} |\n", device_ip));
    md.push_str(&format!("| 厂商 | {} |\n", vendor));
    md.push_str(&format!("| 型号 | {} |\n", model));
    md.push_str(&format!("| 序列号 | {} |\n", sn));
    md.push_str(&format!("| 主机名 | {} |\n", hostname));
    if os_release != "-" { md.push_str(&format!("| OS | {} |\n", os_release)); }
    if kernel != "-" { md.push_str(&format!("| 内核 | {} |\n", kernel)); }
    if cpu != "-" { md.push_str(&format!("| CPU | {} |\n", cpu)); }
    if mem != "-" { md.push_str(&format!("| 内存 | {} |\n", mem)); }
    if mfg_date != "-" { md.push_str(&format!("| 出厂日期 | {} |\n", mfg_date)); }
    md.push_str("\n");

    // 巡检记录
    md.push_str("## 巡检记录\n\n");
    if command_outputs.is_empty() {
        md.push_str("（无命令输出）\n\n");
    } else {
        md.push_str("| 序号 | 巡检项目 | 评判结论 |\n|------|---------|----------|\n");
        for (i, (cmd, output)) in command_outputs.iter().enumerate() {
            let judgment = command_judgments.get(cmd)
                .map(|s| s.split('\x00').next().unwrap_or(""))
                .unwrap_or("");
            let output_short = output.lines().take(3).collect::<Vec<_>>().join("  \n");
            let status_icon = if judgment.contains("[OK]") { "✅" }
                else if judgment.contains("[WARNING]") { "⚠️" }
                else if judgment.contains("[CRITICAL]") { "🔴" }
                else { "ℹ️" };
            md.push_str(&format!("| {} | **{}**<br/>{} | {} {} |\n",
                i + 1, cmd, output_short, status_icon, judgment));
        }
        md.push_str("\n");
    }

    // AI 总结
    if !summary.is_empty() {
        md.push_str("## AI 分析总结\n\n");
        md.push_str(&format!("{}\n", summary));
    }

    md
}

/// 生成报告文件并更新数据库
pub fn generate_report_file(
    db: &rusqlite::Connection,
    record_id: i64,
    output_dir: &Path,
) -> Result<String, String> {
    let record: Option<(i64, String, Option<String>)> = db.query_row(
        "SELECT batch_id, command_outputs, summary_judgment FROM inspection_records WHERE id=?1",
        rusqlite::params![record_id],
        |r| Ok((r.get(0)?, r.get::<_, String>(1)?, r.get(2)?)),
    ).ok();

    let Some((batch_id, cmd_outputs, summary)) = record else {
        return Err("记录不存在".into());
    };

    let device_id: i64 = db.query_row(
        "SELECT device_id FROM inspection_records WHERE id=?1",
        rusqlite::params![record_id], |r| r.get(0)
    ).map_err(|e| e.to_string())?;

    let (name, ip, vendor, model): (String, String, String, Option<String>) = db.query_row(
        "SELECT name, ip, vendor, model FROM devices WHERE id=?1",
        rusqlite::params![device_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    ).map_err(|e| e.to_string())?;

    let outputs: HashMap<String, serde_json::Value> = serde_json::from_str(&cmd_outputs).unwrap_or_default();
    let judgments: HashMap<String, serde_json::Value> = db.query_row(
        "SELECT command_judgments FROM inspection_records WHERE id=?1",
        rusqlite::params![record_id],
        |r| r.get::<_, Option<String>>(0),
    ).ok().flatten().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();

    let mut ctx = HashMap::new();
    ctx.insert("device_name".into(), serde_json::json!(name));
    ctx.insert("device_ip".into(), serde_json::json!(ip));
    ctx.insert("device_vendor".into(), serde_json::json!(vendor));
    ctx.insert("device_model".into(), serde_json::json!(model.unwrap_or_default()));
    ctx.insert("command_outputs".into(), serde_json::to_value(&outputs).unwrap_or_default());
    ctx.insert("command_judgments".into(), serde_json::to_value(&judgments).unwrap_or_default());
    ctx.insert("summary".into(), serde_json::json!(summary.unwrap_or_default()));

    let md = build_markdown(&ctx);

    let batch_dir = output_dir.join(format!("batch{}", batch_id));
    std::fs::create_dir_all(&batch_dir).ok();
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filepath = batch_dir.join(format!("device{}_{}.md", device_id, ts));

    std::fs::write(&filepath, &md).map_err(|e| e.to_string())?;

    let path_str = filepath.to_string_lossy().to_string();
    db.execute("UPDATE inspection_records SET report_path=?1 WHERE id=?2",
        rusqlite::params![path_str, record_id])
        .map_err(|e| e.to_string())?;

    tracing::info!("Markdown 报告已生成: {}", path_str);
    Ok(path_str)
}

/// 读取 Markdown 报告内容
pub fn read_report(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("读取报告失败: {}", e))
}
```

- [ ] **Step 2: 删除 report_generator.rs 中旧的 HTML 相关函数**

确保 `generate_html_report`, `merge_reports`, `parse_hostname`, `parse_os_release`, `parse_kernel`, `parse_cpu_cores`, `parse_mem_total`, `parse_manufacturing_date`, `parse_device_model`, `parse_device_sn`, `build_report_context`, `html_escape` 等旧函数全部删除。

- [ ] **Step 3: 验证编译**

Run: `cd src-tauri && cargo check 2>&1`
Expected: PASS（如果 inspections.rs 有引用旧的 HTML 函数，会有错误，需要逐一修复）

---

### Task 7: 数据库迁移 — 删除两张废弃表

**Files:**
- Modify: `src-tauri/src/db/migrations.rs`

- [ ] **Step 1: 添加删除废弃表的 migration**

Edit `src-tauri/src/db/migrations.rs`, 在现有 migration 逻辑后追加:

```rust
if version < 2 {
    conn.execute_batch("DROP TABLE IF EXISTS offline_log_imports;")?;
    conn.execute_batch("DROP TABLE IF EXISTS scheduled_tasks;")?;
    conn.execute_batch("PRAGMA user_version = 2;")?;
}
```

- [ ] **Step 2: 更新 001_init.sql**

Edit `src-tauri/sql/001_init.sql`，删除 `offline_log_imports` 和 `scheduled_tasks` 的 CREATE TABLE 语句（保留其他 9 张表不变）。

- [ ] **Step 3: 验证编译**

Run: `cd src-tauri && cargo check 2>&1`
Expected: PASS

---

### Task 8: 前端文件清理

**Files:**
- Delete: `src/features/chat/ChatPage.tsx`
- Delete: `src/features/offline/OfflinePage.tsx`
- Delete: `src/features/scheduled/ScheduledTasksPage.tsx`
- Delete: `src/features/commands/CommandsPage.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: 删除废弃前端页面**

Run:
```bash
rm -rf src/features/chat
rm -rf src/features/offline
rm -rf src/features/scheduled
rm -rf src/features/commands
```

- [ ] **Step 2: 更新 App.tsx 路由**

**旧版** `src/App.tsx` 有 15 个 import 和 12 条 Route。

**新版** 精简为 7 个页面（不含 layout）:

```tsx
import { Routes, Route } from "react-router-dom";
import AppShell from "./layouts/AppShell";
import DashboardPage from "./pages/DashboardPage";
import DevicesPage from "./pages/DevicesPage";
import TemplatesPage from "./pages/TemplatesPage";
import InspectionPage from "./pages/InspectionPage";
import ReportsPage from "./pages/ReportsPage";
import AiConfigPage from "./pages/AiConfigPage";
import SettingsPage from "./pages/SettingsPage";

export default function App() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route path="/" element={<DashboardPage />} />
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

- [ ] **Step 3: 验证前端编译**

Run: `npm run build 2>&1 | tail -5`
Expected: Build fails（因为指向了不存在的 pages/ 目录）, 下一步会创建

---

### Task 9: 创建 AppShell 布局

**Files:**
- Create: `src/layouts/AppShell.tsx`
- Delete: `src/components/layout/AppLayout.tsx`
- Delete: `src/components/layout/Sidebar.tsx`

- [ ] **Step 1: 创建 AppShell**

创建 `src/layouts/AppShell.tsx`:
```tsx
import { useState, useEffect } from "react";
import { Outlet, useNavigate, useLocation } from "react-router-dom";

type PageKey = "dashboard" | "devices" | "templates" | "inspection" | "reports" | "ai-config" | "settings";

const NAV_ITEMS: { key: PageKey; label: string; icon: string; path: string }[] = [
  { key: "dashboard",   label: "仪表盘",   icon: "📊", path: "/" },
  { key: "devices",     label: "设备管理", icon: "📡", path: "/devices" },
  { key: "templates",   label: "巡检模板", icon: "📋", path: "/templates" },
  { key: "inspection",  label: "执行巡检", icon: "🔍", path: "/inspection" },
  { key: "reports",     label: "巡检报告", icon: "📄", path: "/reports" },
  { key: "ai-config",   label: "AI 配置",  icon: "🤖", path: "/ai-config" },
  { key: "settings",    label: "系统设置", icon: "⚙️", path: "/settings" },
];

export default function AppShell() {
  const navigate = useNavigate();
  const location = useLocation();
  const [collapsed, setCollapsed] = useState(false);
  const [statusMsg, setStatusMsg] = useState("就绪");

  // 根据当前路径确定选中的导航项
  const activeKey = NAV_ITEMS.find(item => item.path === location.pathname)?.key ?? "dashboard";

  // 全局状态栏消息可通过 custom event 更新
  useEffect(() => {
    const handler = (e: CustomEvent) => setStatusMsg(e.detail);
    window.addEventListener("statusbar-message" as any, handler);
    return () => window.removeEventListener("statusbar-message" as any, handler);
  }, []);

  return (
    <div className="h-screen flex flex-col bg-gray-100 text-gray-900 select-none">
      {/* 菜单栏 */}
      <header className="h-7 bg-gray-200 border-b border-gray-300 flex items-center px-2 text-xs gap-1 shrink-0">
        <span className="font-semibold mr-2">网络设备巡检系统</span>
        {/* 菜单暂用简单按钮模拟，后续可替换为原生菜单 */}
        <button className="px-2 hover:bg-gray-300 rounded" onClick={() => navigate("/devices")}>设备</button>
        <button className="px-2 hover:bg-gray-300 rounded" onClick={() => navigate("/inspection")}>巡检</button>
        <button className="px-2 hover:bg-gray-300 rounded" onClick={() => navigate("/reports")}>报告</button>
        <span className="flex-1" />
        <button className="px-2 hover:bg-gray-300 rounded" onClick={() => setCollapsed(!collapsed)}>
          {collapsed ? "☰" : "☰"}
        </button>
      </header>

      <div className="flex flex-1 overflow-hidden">
        {/* 侧边栏 */}
        <nav className={`${collapsed ? "w-12" : "w-40"} bg-gray-800 text-gray-200 shrink-0 transition-all flex flex-col pt-1`}>
          {NAV_ITEMS.map(item => (
            <button
              key={item.key}
              onClick={() => navigate(item.path)}
              className={`flex items-center gap-2 px-3 py-2 text-xs hover:bg-gray-700 transition-colors
                ${activeKey === item.key ? "bg-gray-700 text-white border-l-2 border-blue-400" : ""}`}
            >
              <span className="text-base shrink-0">{item.icon}</span>
              {!collapsed && <span className="truncate">{item.label}</span>}
            </button>
          ))}
        </nav>

        {/* 内容区域 */}
        <main className="flex-1 overflow-auto p-3">
          <Outlet />
        </main>
      </div>

      {/* 状态栏 */}
      <footer className="h-6 bg-gray-200 border-t border-gray-300 flex items-center px-3 text-xs text-gray-600 shrink-0 gap-3">
        <span>✅ {statusMsg}</span>
        <span className="flex-1" />
        <span>v3.1.0</span>
      </footer>
    </div>
  );
}
```

- [ ] **Step 2: 删除旧布局文件**

Run:
```bash
rm src/components/layout/AppLayout.tsx
rm src/components/layout/Sidebar.tsx
```

- [ ] **Step 3: 验证前端编译**

Run: `npm run build 2>&1 | tail -5`
Expected: 仍然 FAIL（页面文件还不存在），下一步创建占位页面

---

### Task 10: 创建 7 个占位页面 + 更新类型定义

**Files:**
- Create: `src/pages/DashboardPage.tsx`
- Create: `src/pages/DevicesPage.tsx`
- Create: `src/pages/TemplatesPage.tsx`
- Create: `src/pages/InspectionPage.tsx`
- Create: `src/pages/ReportsPage.tsx`
- Create: `src/pages/AiConfigPage.tsx`
- Create: `src/pages/SettingsPage.tsx`
- Modify: `src/types/index.ts` (删除 chat/offline/scheduled 相关类型)

- [ ] **Step 1: 创建占位页面**

对每个页面创建最简单的占位组件。以 DashboardPage 为例，创建 `src/pages/DashboardPage.tsx`:
```tsx
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Stats {
  device_count: number;
  online_device_count: number;
  offline_device_count: number;
  template_count: number;
  command_count: number;
  batch_count: number;
  pending_batch_count: number;
  completed_batch_count: number;
}

export default function DashboardPage() {
  const [stats, setStats] = useState<Stats | null>(null);

  useEffect(() => {
    invoke<Stats>("get_stats").then(setStats).catch(console.error);
  }, []);

  if (!stats) return <div className="p-4 text-gray-500">加载中...</div>;

  return (
    <div>
      <h1 className="text-lg font-bold mb-3">仪表盘</h1>
      <div className="grid grid-cols-4 gap-3 mb-4">
        <StatCard label="设备总数" value={stats.device_count} color="text-blue-600" />
        <StatCard label="在线设备" value={stats.online_device_count} color="text-green-600" />
        <StatCard label="离线设备" value={stats.offline_device_count} color="text-red-600" />
        <StatCard label="巡检模板" value={stats.template_count} color="text-purple-600" />
        <StatCard label="命令库" value={stats.command_count} color="text-orange-600" />
        <StatCard label="巡检批次" value={stats.batch_count} color="text-teal-600" />
        <StatCard label="进行中" value={stats.pending_batch_count} color="text-yellow-600" />
        <StatCard label="已完成" value={stats.completed_batch_count} color="text-green-700" />
      </div>
    </div>
  );
}

function StatCard({ label, value, color }: { label: string; value: number; color: string }) {
  return (
    <div className="bg-white rounded border border-gray-200 p-3 text-center">
      <div className={`text-2xl font-bold ${color}`}>{value}</div>
      <div className="text-xs text-gray-500 mt-1">{label}</div>
    </div>
  );
}
```

其余 6 个页面创建类似占位组件，每个只显示 `<h1>页面标题</h1>` + 占位内容。具体内容在后续任务中逐个实现。

- [ ] **Step 2: 清理 types/index.ts**

删除 types 中的 `ChatMessage`, `OfflineLogImport`, `ScheduledTask`, `TaskCreate`, `TaskUpdate` 类型。保留其他类型不变。

- [ ] **Step 3: 验证前端编译**

Run: `npm run build 2>&1 | tail -5`
Expected: PASS（第一次完整编译通过）

---

### Task 11: 通用 GUI 组件库

**Files:**
- Create: `src/components/ContextMenu.tsx`
- Create: `src/components/Modal.tsx`
- Create: `src/components/DataTable.tsx`
- Create: `src/components/StatusBadge.tsx`
- Create: `src/components/SearchInput.tsx`
- Create: `src/components/Toolbar.tsx`

- [ ] **Step 1: 创建 ContextMenu 组件**

`src/components/ContextMenu.tsx`:
```tsx
import { useEffect, useRef, useState } from "react";

export interface ContextMenuItem {
  label: string;
  separator?: boolean;
  danger?: boolean;
  disabled?: boolean;
  action?: () => void;
}

interface Props {
  items: ContextMenuItem[];
  visible: boolean;
  x: number;
  y: number;
  onClose: () => void;
}

export default function ContextMenu({ items, visible, x, y, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    if (visible) {
      document.addEventListener("mousedown", handler);
      return () => document.removeEventListener("mousedown", handler);
    }
  }, [visible, onClose]);

  if (!visible) return null;

  return (
    <div
      ref={ref}
      className="fixed z-50 bg-white border border-gray-300 rounded shadow-lg py-1 min-w-[140px] text-xs"
      style={{ left: x, top: y }}
    >
      {items.map((item, i) =>
        item.separator ? (
          <div key={i} className="border-t border-gray-200 my-1" />
        ) : (
          <button
            key={i}
            disabled={item.disabled}
            className={`w-full text-left px-3 py-1.5 hover:bg-blue-50 disabled:text-gray-400 disabled:hover:bg-white ${
              item.danger ? "text-red-600 hover:bg-red-50" : ""
            }`}
            onClick={() => { item.action?.(); onClose(); }}
          >
            {item.label}
          </button>
        )
      )}
    </div>
  );
}
```

- [ ] **Step 2: 创建 Modal 组件**

`src/components/Modal.tsx`:
```tsx
import { useEffect, useRef } from "react";

interface Props {
  open: boolean;
  title: string;
  width?: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  onClose: () => void;
}

export default function Modal({ open, title, width = "max-w-lg", children, footer, onClose }: Props) {
  const overlayRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    if (open) { document.addEventListener("keydown", handler); return () => document.removeEventListener("keydown", handler); }
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div ref={overlayRef} className="fixed inset-0 z-40 flex items-center justify-center bg-black/30" onClick={e => { if (e.target === overlayRef.current) onClose(); }}>
      <div className={`bg-white rounded shadow-xl ${width} w-full mx-4 max-h-[80vh] flex flex-col`}>
        <div className="flex items-center justify-between px-4 py-2 border-b">
          <h2 className="text-sm font-semibold">{title}</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-600 text-lg leading-none">&times;</button>
        </div>
        <div className="flex-1 overflow-auto p-4 text-sm">{children}</div>
        {footer && <div className="flex justify-end gap-2 px-4 py-2 border-t bg-gray-50">{footer}</div>}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: 创建 StatusBadge 组件**

`src/components/StatusBadge.tsx`:
```tsx
type Status = "online" | "offline" | "unknown" | "ok" | "warning" | "critical" | "info" | "pending" | "running" | "completed" | "failed" | "stopped";

const STYLES: Record<Status, string> = {
  online: "bg-green-100 text-green-700 border-green-300",
  offline: "bg-red-100 text-red-700 border-red-300",
  unknown: "bg-gray-100 text-gray-500 border-gray-300",
  ok: "bg-green-100 text-green-700 border-green-300",
  warning: "bg-yellow-100 text-yellow-700 border-yellow-300",
  critical: "bg-red-100 text-red-700 border-red-300",
  info: "bg-blue-100 text-blue-700 border-blue-300",
  pending: "bg-gray-100 text-gray-500 border-gray-300",
  running: "bg-blue-100 text-blue-700 border-blue-300 animate-pulse",
  completed: "bg-green-100 text-green-700 border-green-300",
  failed: "bg-red-100 text-red-700 border-red-300",
  stopped: "bg-yellow-100 text-yellow-700 border-yellow-300",
};

const LABELS: Record<Status, string> = {
  online: "在线", offline: "离线", unknown: "未知",
  ok: "正常", warning: "警告", critical: "严重", info: "信息",
  pending: "等待中", running: "执行中", completed: "已完成", failed: "失败", stopped: "已停止",
};

export default function StatusBadge({ status }: { status: Status }) {
  return (
    <span className={`inline-block px-1.5 py-0.5 rounded border text-[11px] font-medium ${STYLES[status] || STYLES.unknown}`}>
      {LABELS[status] || status}
    </span>
  );
}
```

- [ ] **Step 4: 创建 SearchInput 组件**

`src/components/SearchInput.tsx`:
```tsx
import { useRef, useEffect } from "react";

interface Props {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
}

export default function SearchInput({ value, onChange, placeholder = "搜索..." }: Props) {
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === "f") {
        e.preventDefault();
        ref.current?.focus();
        ref.current?.select();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  return (
    <div className="relative">
      <span className="absolute left-2 top-1/2 -translate-y-1/2 text-gray-400 text-xs">🔍</span>
      <input
        ref={ref}
        type="text"
        value={value}
        onChange={e => onChange(e.target.value)}
        placeholder={placeholder}
        className="pl-6 pr-2 py-1 border border-gray-300 rounded text-xs w-48 focus:outline-none focus:border-blue-400"
      />
      {value && (
        <button className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 text-xs" onClick={() => onChange("")}>
          ✕
        </button>
      )}
    </div>
  );
}
```

- [ ] **Step 5: 创建 DataTable 组件**

`src/components/DataTable.tsx`:
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
  selected?: Set<string | number>;
  onSelect?: (keys: Set<string | number>) => void;
  onRowDoubleClick?: (row: T) => void;
  onContextMenu?: (e: React.MouseEvent, row: T) => void;
  emptyText?: string;
}

export default function DataTable<T>({ columns, data, rowKey, selected, onSelect, onRowDoubleClick, onContextMenu, emptyText = "暂无数据" }: Props<T>) {
  return (
    <div className="border border-gray-300 rounded overflow-hidden">
      <div className="overflow-auto max-h-[60vh]">
        <table className="w-full text-xs">
          <thead className="bg-gray-100 sticky top-0 z-10">
            <tr>
              {onSelect && (
                <th className="w-8 px-1 py-1.5 border-b border-gray-300">
                  <input type="checkbox" className="w-3.5 h-3.5" />
                </th>
              )}
              {columns.map(col => (
                <th key={col.key} className="text-left px-2 py-1.5 border-b border-gray-300 font-medium text-gray-600" style={{ width: col.width }}>
                  {col.header}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data.length === 0 ? (
              <tr><td colSpan={columns.length + (onSelect ? 1 : 0)} className="text-center py-8 text-gray-400">{emptyText}</td></tr>
            ) : data.map(row => (
              <tr
                key={rowKey(row)}
                className="border-b border-gray-100 hover:bg-blue-50/50 cursor-default"
                onDoubleClick={() => onRowDoubleClick?.(row)}
                onContextMenu={e => onContextMenu?.(e, row)}
              >
                {onSelect && (
                  <td className="px-1 py-1">
                    <input type="checkbox" className="w-3.5 h-3.5" />
                  </td>
                )}
                {columns.map(col => (
                  <td key={col.key} className="px-2 py-1 text-gray-700">{col.render(row)}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
```

- [ ] **Step 6: 创建 Toolbar 组件**

`src/components/Toolbar.tsx`:
```tsx
interface Props {
  children: React.ReactNode;
}

export default function Toolbar({ children }: Props) {
  return (
    <div className="flex items-center gap-2 mb-2 flex-wrap">
      {children}
    </div>
  );
}
```

- [ ] **Step 7: 验证前端编译**

Run: `npm run build 2>&1 | tail -5`
Expected: PASS

---

### Task 12: DevicesPage — 设备管理

**Files:**
- Rewrite: `src/pages/DevicesPage.tsx`

- [ ] **Step 1: 实现设备管理页面**

创建完整的 `src/pages/DevicesPage.tsx`:

核心交互：左侧分组树（network/system）+ 右侧表格 + 右键菜单 + 实时搜索 + 模态编辑对话框 + 批量状态检测。

关键代码结构：
```tsx
export default function DevicesPage() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [search, setSearch] = useState("");
  const [groupFilter, setGroupFilter] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [editModal, setEditModal] = useState<Device | null>(null);
  const [contextMenu, setContextMenu] = useState<{x:number;y:number;device:Device}|null>(null);

  // 加载设备列表
  const loadDevices = () => invoke<Device[]>("list_devices").then(setDevices);

  // 过滤
  const filtered = devices.filter(d => {
    if (groupFilter && d.group_name !== groupFilter) return false;
    if (search && !d.name.includes(search) && !d.ip.includes(search)) return false;
    return true;
  });

  // 右键菜单项
  const ctxItems = contextMenu ? [
    { label: "编辑设备", action: () => setEditModal(contextMenu.device) },
    { label: "手动巡检", action: () => handleInspect(contextMenu.device.id) },
    {},
    { label: "删除设备", danger: true, action: () => handleDelete(contextMenu.device.id) },
  ] : [];

  // ...
}
```

完整实现需包含: 加载设备列表、搜索过滤、分组筛选、表格展示、右键菜单、模态编辑对话框、批量删除确认、状态刷新、双击编辑。由于此处代码量较大（~200行），具体内容参见原 `src/features/devices/DevicesPage.tsx` 的业务逻辑，重构为 GUI 交互模式。

- [ ] **Step 2: 验证前端编译**

Run: `npm run build 2>&1 | tail -5`
Expected: PASS

---

### Task 13: TemplatesPage — 三栏模板编辑

**Files:**
- Rewrite: `src/pages/TemplatesPage.tsx`

- [ ] **Step 1: 实现三栏模板页面**

`src/pages/TemplatesPage.tsx`:
- 左栏: 模板列表（可选择/右键/新建）
- 中栏: 选中模板的详情编辑（名称/厂商/型号/类型/描述）+ 命令列表（拖拽排序，每行有移除按钮）
- 右栏: 命令库面板（按厂商筛选、搜索、每行有"加入模板"按钮）

核心数据流：选中模板 → 加载 config（含 command_ids）→ 显示命令列表。从命令库加入命令时更新模板 config。

- [ ] **Step 2: 验证前端编译**

Run: `npm run build 2>&1 | tail -5`
Expected: PASS

---

### Task 14: InspectionPage — CI/CD 风格巡检执行

**Files:**
- Rewrite: `src/pages/InspectionPage.tsx`

- [ ] **Step 1: 实现巡检执行页面**

`src/pages/InspectionPage.tsx`:
- 顶部操作栏：设备多选 + 批次名称 + 开始/暂停/停止按钮
- 进度条（批次整体进度）
- 设备执行状态列表（设备名、状态徽标、耗时、展开查看详情按钮）
- 展开显示该设备的命令执行输出（等宽字体终端风格）
- 完成通知 + 跳转报告按钮

- [ ] **Step 2: 验证前端编译**

Run: `npm run build 2>&1 | tail -5`
Expected: PASS

---

### Task 15: ReportsPage — Markdown 预览 + PDF 导出

**Files:**
- Rewrite: `src/pages/ReportsPage.tsx`

- [ ] **Step 1: 安装 react-markdown 并实现报告页面**

Run: `npm install react-markdown`

`src/pages/ReportsPage.tsx`:
- 双视图：列表视图 / 预览视图
- 列表视图：按批次树形分组、设备名称、时间、AI 判断徽标
- 预览视图：
  - 工具栏：返回列表、导出 PDF、打印、导出 Markdown
  - 内容区：react-markdown 渲染 .md 文件内容
  - AI 分析折叠面板
- 导出 PDF：`window.print()` + CSS `@media print` 隐藏界面 chrome
- 批量操作：多选 → 批量导出 / 批量删除

添加打印样式到 `index.html` 或全局 CSS:
```css
@media print {
  header, nav, footer, .no-print { display: none !important; }
  main { margin: 0 !important; padding: 0 !important; }
}
```

- [ ] **Step 2: 验证前端编译**

Run: `npm run build 2>&1 | tail -5`
Expected: PASS

---

### Task 16: AiConfigPage + SettingsPage

**Files:**
- Rewrite: `src/pages/AiConfigPage.tsx`
- Rewrite: `src/pages/SettingsPage.tsx`

- [ ] **Step 1: 实现 AI 配置页面**

`src/pages/AiConfigPage.tsx`:
- 表单布局：Provider 下拉、Model ID、API Key（密码框）、Base URL
- 测试连接按钮
- 下方表格：已配置的模型列表（名称、Provider、激活状态、操作）
- 激活/停用切换、删除配置

- [ ] **Step 2: 实现系统设置页面**

`src/pages/SettingsPage.tsx`:
- 选项卡：报告设置 / 网络设置 / 关于
- 报告：最大输出行数、报告保存路径
- 网络：SSH 超时、并发数
- 关于：版本号、描述

- [ ] **Step 3: 验证前端编译**

Run: `npm run build 2>&1 | tail -5`
Expected: PASS

---

### Task 17: 键盘快捷键系统

**Files:**
- Create: `src/hooks/useKeyboardShortcut.ts`
- Modify: `src/App.tsx` (包裹快捷键 Context)

- [ ] **Step 1: 创建快捷键 hook**

`src/hooks/useKeyboardShortcut.ts`:
```tsx
import { useEffect } from "react";

type ShortcutHandler = () => void;

const shortcuts = new Map<string, ShortcutHandler>();

export function registerShortcut(key: string, handler: ShortcutHandler) {
  shortcuts.set(key, handler);
  return () => shortcuts.delete(key);
}

export function useGlobalShortcuts() {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey;
      const key = `${ctrl ? "Ctrl+" : ""}${e.key}`;

      if (key === "Ctrl+f") { e.preventDefault(); }
      if (key === "Ctrl+s") { e.preventDefault(); }

      const fn = shortcuts.get(key);
      if (fn) { e.preventDefault(); fn(); }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);
}
```

- [ ] **Step 2: 在 App.tsx 中启用全局快捷键**

在 `App` 组件中调用 `useGlobalShortcuts()`。

- [ ] **Step 3: 验证功能**

`npm run dev`，打开应用测试 Ctrl+F 搜索、Escape 关闭。

---

### Task 18: 最终集成验证

- [ ] **Step 1: 完整构建后端**

Run: `cd src-tauri && cargo build --release 2>&1 | tail -3`
Expected: PASS, 二进制生成在 `target/release/inspection-rust`

- [ ] **Step 2: 完整构建前端**

Run: `npm run build 2>&1`
Expected: PASS, 产物在 `dist/`

- [ ] **Step 3: Tauri 开发模式运行**

Run: 终端1: `npm run dev`  
终端2: `cd src-tauri && cargo run`  
验证: 应用窗口正常打开、菜单栏显示中文、侧边栏导航切换、设备列表加载

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "feat: 桌面 GUI 重构 — 砍掉4个功能模块 + Markdown报告管道 + GUI交互模式"
```
