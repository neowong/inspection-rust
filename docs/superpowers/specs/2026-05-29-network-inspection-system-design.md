# 网络设备巡检系统 — 设计文档

## 概述

桌面 GUI 应用，通过 SSH 登录网络设备执行巡检命令，借助 AI（OpenAI/Anthropic）自动分析设备状态，生成巡检报告。单人使用，管理几十台设备规模。

## 技术栈

| 层 | 选型 |
|---|---|
| 桌面框架 | Tauri v2 |
| 后端语言 | Rust |
| 前端框架 | React 18 + TypeScript |
| 构建工具 | Vite |
| 样式 | TailwindCSS 3 + CSS 变量（HSL 配色） |
| 数据库 | SQLite（rusqlite bundled） |
| SSH | ssh2 crate |
| AI API | reqwest（OpenAI / Anthropic） |
| 加密 | fernet crate |
| UI 图标 | lucide-react |
| 路由 | react-router-dom |

## 架构

```
┌─────────────────────────────────────────────────┐
│                  Tauri Desktop App               │
│  ┌──────────────┐         ┌──────────────────┐   │
│  │  Rust Backend │  IPC    │  Frontend (React) │   │
│  │               │◄──────►│                   │   │
│  │  SSH 执行     │ invoke  │  Dashboard       │   │
│  │  AI API 调用  │         │  设备管理         │   │
│  │  SQLite 存储  │         │  模板管理         │   │
│  │  报告生成     │         │  巡检执行         │   │
│  │  加密存储     │         │  报告查看         │   │
│  └──────────────┘         └──────────────────┘   │
└─────────────────────────────────────────────────┘
```

通信方式：Rust 通过 `#[tauri::command]` 暴露函数，前端通过 `invoke()` 调用。不走 HTTP，所有逻辑在本地进程内完成。

数据存储：SQLite 单文件存储在用户数据目录 `~/.local/share/inspection-rust/inspection.db`。首次启动自动创建数据库并灌入种子命令数据。

## 数据模型

9 张表，分 4 个功能域：

### 设备域

**devices**
| 字段 | 类型 | 说明 |
|---|---|---|
| id | INTEGER PK | 自增主键 |
| name | TEXT UNIQUE | 设备名称 |
| ip | TEXT UNIQUE | 管理 IP |
| device_type | TEXT | 设备类型（交换机/路由器/防火墙等） |
| vendor | TEXT | 厂商（H3C/华为/思科/锐捷） |
| model | TEXT? | 型号 |
| ssh_username | TEXT? | SSH 用户名 |
| ssh_password_encrypted | TEXT? | SSH 密码（Fernet 加密） |
| ssh_port | INTEGER | 默认 22 |
| template_id | INTEGER? | 关联的巡检模板 |
| status | TEXT | online/offline/unknown |
| last_checked_at | TEXT? | 最后检测时间 |

**device_status_logs** — 设备状态变更日志
| 字段 | 类型 | 说明 |
|---|---|---|
| id | INTEGER PK | |
| device_id | INTEGER FK | 关联 devices |
| old_status | TEXT? | |
| new_status | TEXT | |
| checked_at | TEXT | |

### 模板域

**inspection_templates**
| 字段 | 类型 | 说明 |
|---|---|---|
| id | INTEGER PK | |
| name | TEXT UNIQUE | 模板名称 |
| vendor | TEXT | 适用厂商 |
| model | TEXT? | 适用型号（可选） |
| device_type | TEXT? | 适用类型（可选） |
| config | TEXT? | JSON，存命令 ID 列表 |
| description | TEXT? | 描述 |
| report_template_id | INTEGER? | 关联报告模板 |

**command_pool** — 命令库
| 字段 | 类型 | 说明 |
|---|---|---|
| id | INTEGER PK | |
| vendor | TEXT | 厂商 |
| command | TEXT | 命令文本 |
| description | TEXT? | 描述 |
| category | TEXT? | 分类（version/cpu/memory/interface 等） |
| model | TEXT? | 适用型号 |

### 巡检域

**inspection_batches**
| 字段 | 类型 | 说明 |
|---|---|---|
| id | INTEGER PK | |
| name | TEXT? | 批次名称 |
| status | TEXT | pending/running/completed/partially_completed/failed/paused/stopped |
| triggered_by | TEXT | manual/scheduled |
| device_ids | TEXT | JSON 数组，巡检设备 ID 列表 |
| started_at | TEXT? | |
| completed_at | TEXT? | |

**inspection_records** — 单台设备的巡检结果
| 字段 | 类型 | 说明 |
|---|---|---|
| id | INTEGER PK | |
| batch_id | INTEGER FK | 关联批次 |
| device_id | INTEGER FK | 关联设备 |
| status | TEXT | pending/running/completed/stopped/failed |
| error_message | TEXT? | 错误信息 |
| command_outputs | TEXT | JSON，命令 → 输出文本 |
| ai_status | TEXT | pending/processing/completed/failed |
| ai_result | TEXT? | AI 完整返回 |
| ai_analysis | TEXT? | AI 分析文字 |
| ai_suggestions | TEXT? | AI 建议 |
| command_judgments | TEXT? | JSON，逐条命令的判断 |
| summary_judgment | TEXT? | 整体状态总结 |
| report_path | TEXT? | 生成的报告路径 |

### 配置域

**ai_model_configs**
| 字段 | 类型 | 说明 |
|---|---|---|
| id | INTEGER PK | |
| name | TEXT | 配置名称 |
| provider | TEXT | openai / anthropic |
| model_id | TEXT | 模型 ID（gpt-4 / claude-3 等） |
| api_key_encrypted | TEXT | API Key（Fernet 加密） |
| base_url | TEXT? | 自定义 API 地址 |
| is_active | INTEGER | 是否启用（0/1） |

**report_templates**
| 字段 | 类型 | 说明 |
|---|---|---|
| id | INTEGER PK | |
| name | TEXT | 模板名称 |
| vendor | TEXT? | 适用厂商 |
| file_path | TEXT | 模板文件路径 |
| created_at | TEXT | |

**system_settings** — 单行系统设置
| 字段 | 类型 | 说明 |
|---|---|---|
| id | INTEGER PK | 固定为 1 |
| report_max_output_lines | INTEGER | 报告输出行数上限（默认 100） |

## 前端页面

7 个路由页面，统一由 AppShell 包裹（侧边栏 + 状态栏）：

1. **仪表盘** `/` — 设备/模板/批次统计卡片，快速查看整体状态
2. **设备管理** `/devices` — 设备列表（表格）、新增/编辑/删除、手动状态检测
3. **巡检模板** `/templates` — 模板列表 CRUD、命令库 CRUD、按厂商自动生成模板
4. **执行巡检** `/inspection` — 批次创建、选择设备、执行/暂停/停止、实时进度
5. **巡检报告** `/reports` — 历史批次列表、逐条记录查看、AI 分析、报告生成导出
6. **AI 配置** `/ai-config` — 多组 AI 模型配置管理、激活/停用
7. **系统设置** `/settings` — 报告参数等系统设置

侧边栏导航分组：
- **巡检工作流**: 模板 → 设备 → 执行 → 报告（按业务流程排序）
- **系统**: AI 配置、系统设置

UI 风格：柔和浅色主题，HSL CSS 变量配色，无过度设计，功能优先。

## Rust 后端模块

```
src-tauri/src/
├── main.rs                # fn main() → lib::run()
├── lib.rs                 # AppState, run(), 命令注册, get_stats/health_check
├── db/
│   ├── models.rs          # 所有数据结构的 Serde 定义
│   ├── migrations.rs      # PRAGMA user_version 版本迁移
│   ├── query.rs           # query_all / query_one / count 辅助
│   └── seed_data.rs       # 各厂商 65 条默认命令种子
├── commands/
│   ├── devices.rs         # list/get/create/update/delete/batch_delete /
│   │                      # check_status/check_all/get_status_log
│   ├── templates.rs       # 模板 CRUD + 命令池 CRUD + 自动生成模板
│   ├── inspections.rs     # 批次 CRUD + run/pause/stop/restart/retry
│   ├── reports.rs         # AI 分析（单条/批量）+ 报告生成 + 报告模板管理
│   ├── ai_config.rs       # AI 配置 CRUD + activate/deactivate
│   └── settings.rs        # get/update 系统设置
└── services/
    ├── crypto.rs           # Fernet encrypt/decrypt
    ├── inspection_runner.rs# SSH2 连接执行（分页禁用 + 超时 + 重试）
    ├── ai_inspection.rs    # AI API 调用 + 中文评判 Prompt
    ├── report_generator.rs # Markdown 报告构建
    └── template_generator.rs# 按厂商/型号从命令池推荐命令
```

## 前端组件体系

```
src/
├── main.tsx / App.tsx / index.css / types/index.ts / lib/utils.ts
├── hooks/useKeyboardShortcut.ts  # 全局快捷键注册（Ctrl+F 搜索等）
├── layouts/AppShell.tsx          # 侧边栏导航 + 底部状态栏 + Outlet
├── components/
│   ├── DataTable.tsx      # 泛型表格，Column<T> 配置驱动
│   ├── Modal.tsx          # 模态弹窗，Escape 关闭
│   ├── StatusBadge.tsx    # 状态徽标（颜色圆点 + 中文标签）
│   ├── SearchInput.tsx    # 搜索输入框（Ctrl+F 聚焦）
│   ├── ContextMenu.tsx    # 右键上下文菜单
│   ├── Toolbar.tsx        # 操作按钮栏
│   └── ui/Button.tsx      # cva 多态按钮
│     └── ui/Card.tsx      # 卡片容器
│     └── ui/Input.tsx     # 输入框 + Select
└── pages/                 # 7 个页面组件
```

## 关键设计决策

- **Tauri IPC 代替 HTTP**: 不启动本地服务器，所有调用通过 Tauri invoke 直达 Rust 函数
- **同步 SQLite + Mutex**: `Mutex<Connection>` 管理数据库连接，命令函数按需加锁
- **Fernet 加密**: SSH 密码和 AI API Key 均加密存储，密钥可跨系统互通
- **模板配置存命令 ID 列表**: 模板的 config 字段存 `{command_ids: [1,2,3]}` JSON，灵活可扩展
- **CSS 变量主题**: 所有颜色使用 HSL 变量，避免硬编码色值，便于后续暗色模式
- **无外部 UI 库**: 不使用 shadcn/ui 或组件库，Button/Card/Input 等基础组件手工实现，保持包体小巧
