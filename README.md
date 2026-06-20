# OpenInspect 运维巡检系统

OpenInspect 是基于 Rust + Tauri v2 的桌面端运维巡检工具，通过 SSH 连接网络设备与 Linux 服务器执行巡检命令，调用 AI 分析结果并生成可编辑 DOCX 报告。

## 功能特性

- **设备管理** — 支持 H3C、华为、思科、锐捷、飞塔等网络设备和 Linux 服务器，批量导入/导出
- **巡检模板** — 可视化模板编辑器，内置 85+ 预置命令，支持拖拽排序
- **批量巡检** — 多设备并发执行，实时进度追踪，支持暂停/重试
- **AI 分析** — 集成 OpenAI / Anthropic / DeepSeek，自动生成巡检分析报告
- **报告生成** — 生成可编辑 DOCX 报告，支持在线模板编辑、A4 实时预览、静态信息采集与批量导出
- **工具箱** — 存活扫描、端口扫描、WEB 检测、SNMP v2c/v3、Zabbix Agent 探测
- **日志分析** — 设备日志解析与分析
- **关于页面** — 展示 OpenInspect 项目说明、SVG 使用流程图和打赏二维码占位

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri v2 |
| 前端 | React 18 + Vite 6 + TypeScript + TailwindCSS |
| 后端 | Rust (rusqlite, ssh2, reqwest, tokio) |
| AI | OpenAI / Anthropic / DeepSeek API |
| 数据库 | SQLite (bundled) |

## 使用流程

详细操作见 [用户操作手册](docs/USER_MANUAL.md)。

1. **配置 AI 模型**：在“系统设置”中添加并激活 OpenAI / Anthropic / DeepSeek API 配置。
2. **维护命令库**：在“巡检模板 → 命令库”中按厂商维护巡检命令和命令说明。
3. **创建报告模板**：在“巡检模板 → 报告模板”中新建 DOCX 报告模板，配置封面、设备信息、巡检明细列、页眉页脚，并在右侧 A4 预览中实时查看效果。
4. **创建巡检模板**：选择厂商和命令，按需要把命令标记为：
   - `巡检项`：执行结果进入报告明细和 AI 分析。
   - `静态信息`：仅用于提取 `sysname`、`model`、`serial_number`、`manufacturing_date` 等字段，不显示在报告明细。
5. **添加设备**：录入 SSH 信息并绑定巡检模板；H3C 设备可自动检测型号、SN、出厂日期和 sysname。
6. **执行巡检**：在“巡检执行”中创建批次并运行，系统会保存命令输出和本次巡检静态信息快照。
7. **生成报告**：在“报告管理”中执行 AI 分析后生成 DOCX；单设备可下载单份报告，批次可下载 ZIP 或合并 DOCX。

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
│   ├── pages/            # 页面：仪表盘/工具箱/日志/设备/模板/巡检/报告/设置/关于
│   ├── components/       # 通用组件 (DataTable, Modal, Button 等)
│   └── hooks/            # 自定义 Hooks
├── src-tauri/            # Rust 后端
│   ├── src/commands/     # Tauri 命令处理器
│   ├── src/services/     # 业务服务 (SSH, AI, 报告, 扫描等)
│   └── src/db/           # 数据库模型与迁移
└── data/                 # 运行时数据 (数据库, 报告, 日志)
```

## 许可证

开源许可证待补充。发布前请根据实际需要选择 MIT、Apache-2.0 或其它许可证。
