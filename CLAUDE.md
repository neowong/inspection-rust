# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

网络设备巡检系统 (Network Device Inspection System) — Rust + Tauri v2 桌面版。通过 SSH 连接网络设备（H3C/华为/思科/锐捷），执行巡检命令收集状态数据，调用 AI（OpenAI/Anthropic）分析结果并生成 Markdown 报告。

## Tech Stack

- **Desktop**: Tauri v2 (Rust backend + webview frontend)
- **Frontend**: React 18 + Vite 6 + TypeScript + TailwindCSS 3
- **Backend (Rust)**: rusqlite (SQLite bundled), ssh2, reqwest, fernet, serde, chrono, tokio
- **UI**: lucide-react icons, class-variance-authority, tailwind-merge/clsx
- **AI**: OpenAI / Anthropic API via reqwest
- **Routing**: react-router-dom v7
- **Build**: tauri v2 CLI, `npx tauri dev` / `npx tauri build`

## Architecture

```
inspection-rust/
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
│       ├── DevicesPage.tsx       # Device CRUD + status check
│       ├── TemplatesPage.tsx     # Inspection templates + command pool CRUD
│       ├── InspectionPage.tsx    # Batch creation, running, monitoring
│       ├── ReportsPage.tsx       # AI analysis, reports, report templates
│       ├── AiConfigPage.tsx      # (legacy, deprecated — AI config merged into SettingsPage)
│       └── SettingsPage.tsx      # System settings + AI model config CRUD (integrated)
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
│   │   │   └── seed_data.rs      # 65 seed commands for H3C/华为/思科/锐捷
│   │   ├── commands/             # Tauri command handlers (each file = domain module)
│   │   │   ├── devices.rs        # list/get/create/update/delete/check-status
│   │   │   ├── templates.rs      # Template CRUD + command pool CRUD + auto-generate
│   │   │   ├── inspections.rs    # Batch CRUD + run/pause/stop/restart/retry
│   │   │   ├── reports.rs        # AI analysis, report generation, report templates
│   │   │   ├── ai_config.rs      # AI model config CRUD + activate/deactivate
│   │   │   └── settings.rs       # System settings
│   │   └── services/
│   │       ├── crypto.rs         # Fernet encryption (password/API key)
│   │       ├── inspection_runner.rs  # SSH execution via ssh2 (netmiko-style)
│   │       ├── ai_inspection.rs  # AI analysis prompt + API call
│   │       ├── report_generator.rs   # Markdown report builder
│   │       └── template_generator.rs # Auto-generate templates from command pool
│   └── sql/001_init.sql          # 9 tables: devices, device_status_logs, inspection_templates, command_pool, inspection_batches, inspection_records, ai_model_configs, report_templates, system_settings
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
- **DataTable**: Supports `onRowClick`, `onRowDoubleClick`, `selectedKey` props. TemplatesPage is reference implementation
- **Config field encoding**: Template `config` is stored as JSON string in SQLite, `JSON.stringify()` on frontend, `serde_json::from_str()` on list. Frontend types declare `config: {...}` object, invoke params must send string
- **API key/password fields**: Rust models use `api_key_encrypted` / `ssh_password_encrypted`. Frontend send `api_key_encrypted` / `ssh_password_encrypted` (NOT `api_key` / `ssh_password`)
- **SSH (netmiko-style)**: libssh2 only (no system sshpass). Persistent shell channel per device. `extract_prompt` → base_prompt (strips terminator). `output_contains_prompt` uses `contains()` not `ends_with()`. Per-command timeout 15s, 2 consecutive timeouts → skip remaining. `screen-length disable` must succeed.
- **Device concurrency**: `run_batch` / `create_batch`(auto_start) spawn `tokio::spawn` per device, shared `Arc<Mutex<Connection>>`. `inspect_one_device()` per-device flow. Progress tracked via `Arc<std::sync::Mutex<String>>` → DB poller every 2s.
- **Background tasks**: `lib.rs` spawns std thread for 5-minute device status polling (`poll_device_statuses`), uses `try_lock` to avoid blocking.
- **Export**: `export_batch_csv` writes CSV with BOM to `data/reports/`. Fields escaped for commas/newlines/quotes.
- **tsconfig `noEmit: true` is REQUIRED**: Without it, `tsc` generates stale `.js` files in `src/` that Vite loads instead of `.tsx` — causing "changes not reflected" bugs
- **Branding**: `public/network-internet-web-svgrepo-com.svg` (network globe icon) used as app logo in sidebar
- **Sticky headers**: All page headers use `sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm relative`
- **Dashboard cards**: Clickable with `cursor-pointer` + `navigate(path)`. Summary + detail cards both have path field.
- **Command pool UI**: Vendor tabs + collapsible category groups (ChevronDown/Right). Each command shows edit/delete icons on hover.

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
cargo build --release         # release (~20MB binary)

# Production desktop build
npx tauri build               # produces .deb / .AppImage

# Frontend build only
npm run build
```

## Data & State

- SQLite DB auto-created at `~/.local/share/inspection-rust/inspection.db`
- Data dirs: `reports/`, `report_templates/`, `uploads/`, `logs/`
- 65 seed commands loaded on first launch
- Fernet key (`MASTER_PASSWORD`) hardcoded in `crypto.rs` — encrypted data compatible with Python predecessor
- Release binary is standalone (frontend embedded, no devUrl)
