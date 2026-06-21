# OpenInspect 运维巡检系统

基于 Rust + Tauri v2 的桌面端运维巡检工具。通过 SSH 连接网络设备与 Linux 服务器执行巡检命令，调用 AI 分析结果并生成可编辑 DOCX 报告。

## 功能特性

- **设备管理** — 支持 H3C、华为、思科、锐捷、飞塔等网络设备、Linux 服务器和数据库，自动检测型号/SN/主机名
- **巡检模板** — 可视化模板编辑器，内置 85+ 预置命令，支持拖拽排序
- **批量巡检** — 多设备并发 SSH 执行，实时进度追踪，支持暂停/重试
- **AI 分析** — 集成 OpenAI / Anthropic / DeepSeek，逐条命令评判生成分析报告
- **报告生成** — 可编辑 DOCX 报告，支持 AI 评判/人工评判，单设备/批量 ZIP/合并 DOCX
- **工具箱** — 存活扫描、TCP/UDP 端口扫描、WEB 检测、SNMP v2c/v3、Zabbix Agent 探测
- **日志分析** — 设备日志解析与 AI 分析
- **Linux 巡检** — exec channel 执行，sudo 提权，4 路并行连接
- **设备分类** — 网络设备 / 安全设备 / 服务器 / 数据库

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri v2 |
| 前端 | React 18 + Vite 6 + TypeScript + TailwindCSS |
| 后端 | Rust (rusqlite, ssh2, reqwest, tokio) |
| AI | OpenAI / Anthropic / DeepSeek API |
| 数据库 | SQLite (bundled) |

## 快速开始

详细操作见 [用户操作手册](docs/USER_MANUAL.md)。

1. **配置 AI 模型**：在"系统设置"中添加并激活 AI 供应商（可选，支持人工评判）。
2. **维护命令库**：按厂商分类录入巡检命令，支持 H3C/华为/思科/锐捷/飞塔/Linux/数据库。
3. **设计报告模板**：配置封面、列定义、页眉页脚，右侧 A4 实时预览。
4. **创建巡检模板**：选择巡检项与静态信息命令。
5. **添加设备**：录入 IP、SSH 凭据并绑定模板，保存后自动检测连通性和型号。
6. **执行巡检**：创建批次并运行，多设备并发执行。
7. **生成报告**：AI 评判或人工评判后生成 DOCX 报告。

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
│   ├── pages/            # 9 个页面
│   ├── components/       # 通用组件 (DataTable, Modal, Button 等)
│   └── hooks/            # 自定义 Hooks
├── src-tauri/            # Rust 后端
│   ├── src/commands/     # Tauri 命令处理器
│   ├── src/services/     # 14 个业务服务模块
│   └── src/db/           # 数据库模型与迁移 (20 次迁移)
└── docs/                 # 用户手册、设计文档
```

## 更新日志

详见 [CHANGELOG.md](CHANGELOG.md)。

## 许可证

开源许可证待补充。
