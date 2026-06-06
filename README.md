# 网络设备巡检系统

基于 Rust + Tauri v2 的桌面端网络设备巡检工具，通过 SSH 连接网络设备执行巡检命令，调用 AI 分析结果并生成报告。

## 功能特性

- **设备管理** — 支持 H3C、华为、思科、锐捷等厂商设备，批量导入/导出
- **巡检模板** — 可视化模板编辑器，内置 85+ 预置命令，支持拖拽排序
- **批量巡检** — 多设备并发执行，实时进度追踪，支持暂停/重试
- **AI 分析** — 集成 OpenAI / Anthropic / DeepSeek，自动生成巡检分析报告
- **报告生成** — Markdown / DOCX / HTML 多格式报告，支持自定义报告模板
- **工具箱** — 存活扫描、端口扫描、WEB 检测、SNMP v2c/v3、Zabbix Agent 探测
- **日志分析** — 设备日志解析与分析

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri v2 |
| 前端 | React 18 + Vite 6 + TypeScript + TailwindCSS |
| 后端 | Rust (rusqlite, ssh2, reqwest, tokio) |
| AI | OpenAI / Anthropic / DeepSeek API |
| 数据库 | SQLite (bundled) |

## 开发

```bash
# 安装依赖
npm install

# 前端开发服务器 (port 1420)
npm run dev

# 桌面端开发 (另开终端)
npx tauri dev

# 类型检查
cargo check

# 构建
npm run build:release    # 前端 + Rust 一步编译
npm run build:win        # Windows 交叉编译
npx tauri build          # 生产安装包 (.deb / .AppImage)
```

## 项目结构

```
inspection-rust/
├── src/                  # React 前端
│   ├── pages/            # 7 个页面：仪表盘/工具箱/日志/设备/模板/巡检/报告/设置
│   ├── components/       # 通用组件 (DataTable, Modal, Button 等)
│   └── hooks/            # 自定义 Hooks
├── src-tauri/            # Rust 后端
│   ├── src/commands/     # Tauri 命令处理器
│   ├── src/services/     # 业务服务 (SSH, AI, 报告, 扫描等)
│   └── src/db/           # 数据库模型与迁移
└── data/                 # 运行时数据 (数据库, 报告, 日志)
```

## 许可证

私有项目，未经授权禁止使用。
