# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

## Project Context

网络设备巡检系统 - Rust + Tauri v2 桌面版。从 Python `../inspection-v3/` 完整复刻，参考其 README 和源码了解领域模型和业务逻辑。

**源项目**: FastAPI + TortoiseORM (SQLite) + LangChain + React, 11 个数据模型, 10 个 API 路由, 7 个服务模块。

## 技术栈

- **桌面框架**: Tauri v2 (Rust)
- **前端**: React 18 + Vite 5 + TypeScript + TailwindCSS
- **数据库**: SQLite (rusqlite bundled)
- **SSH**: ssh2 crate
- **加密**: fernet crate
- **HTTP/AI**: reqwest (OpenAI / Anthropic API)
- **调度**: tokio-cron-scheduler

## 项目结构

```
inspection-rust/
├── src/                          # React 前端
│   ├── App.tsx                   # 路由 (12 页面)
│   ├── components/layout/        # Sidebar + AppLayout
│   ├── features/                 # 各功能页面
│   │   ├── dashboard/            # 仪表盘
│   │   ├── devices/              # 设备管理
│   │   ├── templates/            # 巡检模板
│   │   ├── commands/             # 命令库
│   │   ├── batches/              # 巡检批次 + 详情
│   │   ├── inspection/           # 巡检记录
│   │   ├── scheduled/            # 定时任务
│   │   ├── settings/             # AI配置 + 系统设置
│   │   ├── report-templates/     # 报告模板
│   │   ├── offline/              # 离线巡检
│   │   └── chat/                 # AI对话
│   ├── types/index.ts            # TypeScript 类型定义
│   └── lib/utils.ts              # cn() 工具函数
├── src-tauri/                    # Rust 后端
│   ├── Cargo.toml
│   ├── tauri.conf.json           # Tauri 配置
│   ├── src/
│   │   ├── lib.rs                # 入口 + AppState + 命令注册
│   │   ├── main.rs               # main()
│   │   ├── commands/             # Tauri IPC 命令 (10 模块)
│   │   │   ├── devices.rs        # 设备 CRUD + 状态检测
│   │   │   ├── templates.rs      # 模板 CRUD + 自动生成
│   │   │   ├── command_pool.rs   # 命令库 CRUD
│   │   │   ├── batches.rs        # 批次生命周期
│   │   │   ├── inspection_records.rs  # AI分析 + 报告
│   │   │   ├── offline.rs        # 离线导出/导入
│   │   │   ├── scheduled_tasks.rs# 定时任务 CRUD
│   │   │   ├── ai_config.rs      # AI 模型配置
│   │   │   ├── report_templates.rs    # 报告模板
│   │   │   ├── settings.rs       # 系统设置
│   │   │   └── chat.rs           # AI 对话
│   │   ├── services/             # 业务服务层
│   │   │   ├── crypto.rs         # Fernet 加密
│   │   │   ├── inspection_runner.rs   # SSH 巡检执行
│   │   │   ├── ai_inspection.rs  # AI 评判分析
│   │   │   ├── report_generator.rs    # 报告生成
│   │   │   ├── scheduler.rs      # 设备状态定时检测
│   │   │   ├── template_generator.rs  # 模板自动生成
│   │   │   └── chat_agent.rs     # AI 对话代理
│   │   └── db/
│   │       ├── models.rs         # Rust 数据结构
│   │       ├── migrations.rs     # 数据库迁移
│   │       ├── query.rs          # 查询辅助函数
│   │       └── seed_data.rs      # 65 条默认命令种子
│   └── sql/001_init.sql          # 11 张表初始化
├── package.json
├── vite.config.ts
└── tailwind.config.js
```

## 构建命令

```bash
# 前端
npm run dev          # Vite dev server (port 1420)
npm run build        # 前端生产构建 → dist/

# 后端 (在 src-tauri/ 目录下)
cargo check          # 类型检查
cargo build          # debug 构建 (249MB)
cargo build --release # release 构建 (20MB)

# 桌面应用
cargo tauri dev      # 开发模式 (需要先 npm run dev)
cargo tauri build    # 生产打包 (.deb/.AppImage)
```

## 关键设计决策

- **Tauri IPC 代替 HTTP**: 原项目用 FastAPI 路由，这里用 Tauri `#[tauri::command]` 宏暴露 Rust 函数给前端 `invoke()`
- **同步 SQLite**: 用 `Mutex<Connection>` 管理，所有命令函数通过 `state.db.lock()` 获取连接
- **Fernet 密钥兼容**: 使用原项目的 `MASTER_PASSWORD` 密钥，加密数据可跨项目互通
- **前端无 shadcn/ui**: 未安装 shadcn/cli，所有 UI 组件用手写的 TailwindCSS
- **devUrl 已移除**: `tauri.conf.json` 不含 `devUrl`，release 二进制独立运行，不依赖 localhost

## 运行须知

- **Release 二进制**: `./target/release/inspection-rust` 独立运行，前端已嵌入
- **Debug 二进制**: 需要先 `npm run dev` 启动 Vite (port 1420)
- 首次启动自动创建 SQLite 数据库并灌入 65 条种子命令
- 数据存储在 `~/.local/share/inspection-rust/inspection.db`
