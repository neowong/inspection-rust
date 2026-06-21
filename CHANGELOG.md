# 更新日志

## v3.40.0 (2026-06-21)

### 🎨 界面优化
- **仪表盘重新设计**：核心指标大卡片 + 两列分区（设备分类 / 巡检任务），视觉更整洁
- **报告管理重构**：简化为 [AI评判] [人工评判] [下载综合报告] 三个按钮，工作流更清晰
- **设备分类独立为四类**：网络设备 / 安全设备 / 服务器 / 数据库
- 设备列表"最后检测时间"列收窄，操作列加宽
- Select 组件统一高度，Toolbar 按钮齐平
- AI评判/人工评判按钮样式统一

### ⚡ 性能优化
- 设备保存后检测不阻塞 UI（saving 立即置 false）
- check_device_status 改为 async + spawn_blocking，TCP 不阻塞线程池
- 检测全部设备时静态信息采集改为 3 路并发，替代串行
- 检测全部时跳过已有静态信息的设备，避免重复 SSH
- 后台定期检测（每 5 分钟）新增静态信息自动采集

### 🐛 Bug 修复
- 删除的种子命令不再重启后复活（墓碑表机制）
- 综合报告路径持久化到 DB，切批次/刷新后可随时下载
- 添加设备时密码框不再显示上一条的 mask
- 切换批次时清除 loading 状态，避免跨批次残留
- 下载综合报告按钮始终显示，无单报告时灰色禁用
- 添加命令时检查同厂商下是否重复，友好报错
- 图标改为 RGBA 白色背景，修复桌面快捷方式透明问题（Tauri 要求 RGBA 格式）
- WebView2Loader.dll 和 WebView2Setup.exe 加入 bundle.resources，修复安装包缺失
- WebView2 安装失败时弹窗提示而非闪退（直接调用 user32.dll MessageBoxW）
- 启动阶段写 startup.log 到 exe 目录，便于排查 Win10 LTSC 等环境问题

### 🔒 安全
- 清理测试文件中的硬编码内网 IP 和密码
- 设计文档中的 Fernet 密钥替换为占位符
- 删除包含内网信息的巡检报告文件

### 📝 其它
- 关于页增加微信好友二维码和问题反馈邮箱
- 用户手册全面扩充：系统简介、设备分类、工具箱、Linux 巡检、常见问题
- 后端 SQL 新增 security_device_count、database_count、report_count
- 仪表盘统计用 location.key + focus 事件刷新
- 新增 CHANGELOG.md

---

## v3.40.6 (2026-06-21)

### 🐛 关键修复
- **Windows 启动崩溃彻底修复**：精简版 Win11 / Win10 LTSC 等环境下程序安装后无界面闪退
  - panic hook 移至 `main()` 第一行，确保任何崩溃都能弹 MessageBox + 写日志到 `%TEMP%`
  - WebView2 强制软件渲染（`--disable-gpu`），兼容无 GPU 驱动精简系统
  - WebView2 安装器释放到 TEMP（替代 Program Files），永不会权限失败
  - 启动全程 debug log 埋点到 `%TEMP%\inspection-debug.log`，可精准定位崩溃点
- **数据库迁移修复**：全新安装 `is_default` 索引在列存在前创建导致崩溃，移除过早索引
- **种子数据一致性**：`INSERT OR IGNORE` 改为 `ON CONFLICT DO UPDATE`，升级用户 `needs_root` 与全新安装一致

### 🔒 可靠性
- CI 门禁工作流（每次 push 跑 tsc/build/check/clippy + 15 项测试）
- 全新安装迁移 + 种子一致性专项测试，确保开发/生产环境完全一致
- `Tauri::run()` 失败时 panic 带完整上下文，不再无声消失
- 离线 WebView2 安装器检测：将 `MicrosoftEdgeWebView2RuntimeInstallerX64.exe` 放 exe 同目录自动离线安装

### 📝 其它
- releaseDraft 改为 false，构建完直接发布不再草稿
- 清理所有历史版本 Release 和 Tag，仅保留 v3.40.6

---

## v3.40.1-3.40.2 (2026-06-21)

### 🐛 Bug 修复
- WebView2 检测优化：reg.exe 查询加 `CREATE_NO_WINDOW`，消除 7 个子进程闪窗
- `windows_subsystem = "windows"` 恢复，release 不再分配控制台窗口
- 移除无效 `catch_unwind`（`panic = "abort"` 下是死代码），保留 panic hook 写日志
- 早期调试日志写入 + 控制台窗口临时启用（调试阶段）

---

## v3.3.0 (2026-06-20)

### ✨ 新功能
- **Linux 服务器巡检**：exec channel 执行，支持 sudo 提权，4 路并行连接
- **飞塔 (FortiGate) 设备检测**：get system status 解析型号/SN/主机名
- **设备账号认证状态**：auth_status 双徽章（在线状态 + 账号状态）
- **设备类型分类筛选**：网络设备 / 安全设备 / 服务器

### 🔧 改进
- 品牌升级：网工 → 运维，覆盖全部核心文件
- 全局代码审查：修复 4 个逻辑错误、删除 2 处死代码、35 项 Clippy 警告归零
- 13/13 单元测试通过

---

## v3.2.0 (2026-06-19)

### ✨ 新功能
- DOCX 报告重构：列定义模板引擎，代码生成 Word 报告
- 开源版品牌：OpenInspect Logo、关于页、SVG 使用流程图
- H3C 专用报告模板
- 并行设备检测 + AI 健康检查

---

## v3.0.0 (2026-06-03)

### ✨ 新功能
- 工具箱全套：存活扫描、TCP/UDP 端口、WEB 检测、SNMP v2c/v3、Zabbix Agent
- 巡检页面重构：AI 分析 + 报告集成，左右分栏布局
- 报告模板可视化编辑器：6 区块拖拽排序 + WYSIWYG 预览
- 批量创建非阻塞、SpinInput 组件
