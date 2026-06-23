# AI巡检助手 (OpenInspect)

基于 Rust + Tauri v2 的桌面端运维巡检工具。通过 SSH 连接网络设备、Linux 服务器与数据库，执行巡检命令采集状态，调用 AI 分析并生成可编辑 DOCX 报告。

## 功能特性

- **设备管理** — 网络设备（H3C/华为/思科/锐捷/飞塔）、Linux 服务器、数据库（MySQL/PostgreSQL/Oracle/SQL Server/达梦/Redis/MongoDB）。自动检测型号、SN、主机名、CPU、内存、内核版本、数据库版本等静态信息
- **设备复制** — 基于已有设备快速复制配置，清空静态信息和 IP，便于批量录入同型号设备
- **巡检模板** — 可视化模板编辑器，命令库按厂商+类别分组，拖拽排序，支持模板复制
- **报告模板** — 分类驱动（网络/Linux/数据库），可视化拼装封面、基本信息字段、列定义、页眉页脚，A4 实时预览
- **批量巡检** — 多设备并发 SSH 执行，实时进度，支持暂停/停止/重启/重试，部分完成状态
- **AI 分析** — 集成 OpenAI 兼容 API / DeepSeek，逐条命令评判生成分析报告
- **报告生成** — DOCX 报告（docx-rs 直接生成，不依赖 Office），支持 AI 评判/人工评判，单设备/批量 ZIP/合并 DOCX
- **工具箱** — 存活扫描、TCP/UDP 端口扫描（实时显示 + SMB/FTP/邮件/远程登录等常用服务预设）、路由跟踪（离线 IP 归属地解析，一键下载启用）、WEB 检测、SNMP v2c/v3、Zabbix Agent 探测
- **日志分析** — 设备日志解析与 AI 分析，CSV 导出
- **数据库容器部署** — 支持 Docker/Podman/K8s，按容器名直连，退出码区分"容器未运行"与"客户端未安装"
- **新版本检测** — 启动时自动检查 GitHub Releases，状态栏提示 + 关于页面手动检查 + 下载链接
- **问题反馈** — 关于页面直接提交反馈（问题/需求/其他），同步到统计 Dashboard
- **匿名使用统计** — 启动时匿名上报设备数/版本/OS（SHA-256 哈希，不收集 IP/用户名），Dashboard: https://neowong.eu.org/stats/

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri v2 |
| 前端 | React 18 + Vite 6 + TypeScript + TailwindCSS 3 |
| 后端 | Rust (rusqlite, ssh2, reqwest, tokio, docx-rs) |
| AI | OpenAI 兼容 API / DeepSeek |
| 数据库 | SQLite (bundled) |

## 快速开始

详细操作见 [用户操作手册](docs/USER_MANUAL.md)。

1. **配置 AI 模型**：系统设置 → 添加并激活 AI 供应商（可选，支持人工评判）
2. **维护命令库**：按厂商+类别录入巡检命令，内置网络设备/Linux/数据库预置命令
3. **设计报告模板**：选分类（网络/Linux/数据库）→ 配置封面、列定义、页眉页脚
4. **创建巡检模板**：选厂商 → 从命令库勾选命令加入模板（拖拽排序）
5. **添加设备**：录入 IP、SSH 凭据、关联模板；数据库设备填部署方式+容器名+DB 凭据。保存后自动检测连通性和静态信息
6. **执行巡检**：创建批次 → 勾选设备 → 运行，多设备并发执行
7. **生成报告**：AI 评判或人工评判后生成 DOCX

## 开发

```bash
# 安装依赖
npm install

# 前端开发服务器 (port 1420)
npm run dev

# 桌面端开发 (另开终端)
npx tauri dev

# 类型检查
npx tsc --noEmit          # 前端
cargo check               # Rust
cargo clippy --all-targets -- -D warnings   # CI 级别检查

# 构建
npm run build             # 仅前端
cargo build --release     # Rust release (~15MB)
npm run build:release     # 前端 + Rust 一步编译
npx tauri build           # 生产安装包 (.deb / .AppImage)
```

> Windows 交叉编译详见 [CLAUDE.md](CLAUDE.md) 的"Windows 交叉编译注意事项"。

## 项目结构

```
inspection-neo/
├── src/                     # React 前端
│   ├── pages/               # 10 个页面（仪表盘/设备/模板/巡检/报告/工具箱/日志/设置/关于）
│   ├── components/          # 通用组件（DataTable/Modal/Button/Card/Input/ContextMenu 等）
│   ├── layouts/AppShell.tsx # 侧边栏 + 状态栏 + 路由出口
│   └── lib/                 # 常量、状态映射、工具函数
├── src-tauri/               # Rust 后端
│   ├── src/commands/        # Tauri 命令（devices/templates/inspections/reports/ai_config/tools）
│   ├── src/services/        # 业务服务（SSH/AI/DOCX/扫描器/SNMP/crypto 等）
│   ├── src/db/              # 模型、迁移（v1–v31）、种子命令
│   └── sql/                 # 初始化 SQL
└── docs/                    # 用户手册
```

## 数据与状态

- SQLite 数据库：`~/.local/share/inspection-rust/inspection.db`（Windows 为 `%APPDATA%\inspection-rust\`）
- 数据目录：`reports/`、`report_templates/`、`uploads/`、`logs/`
- 升级兼容：结构变更走增量迁移（`PRAGMA user_version`），保证旧版本数据无损升级
- 日志：文件日志在数据目录 `logs/`，启动调试日志在系统临时目录

## 更新日志

详见 [CHANGELOG.md](CHANGELOG.md)。

## 许可证

开源许可证待补充。
