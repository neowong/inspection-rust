# 更新日志

## v3.55.2 (2026-06-28)

### ✨ 新功能

- **数据库客户端预检**：巡检执行前自动检测服务器是否安装数据库命令行工具（`psql`/`mysql`/`redis-cli` 等），未安装时跳过该厂商所有命令并标注 `[跳过]`，避免 N 条命令逐个失败

### 🐛 Bug 修复

- **PostgreSQL 设备版本检测不准**：`db_version` 字段原用 `psql --version` 获取客户端版本，改为 `SELECT version()` 获取真实服务端版本
- **种子数据标签修正**：`psql --version` → "psql 客户端版本"，`SELECT version()` → "PostgreSQL 服务端版本"

## v3.55.1 (2026-06-28)

### 🐛 Bug 修复

- **PostgreSQL SQL 命令密码认证失败**：`psql` 默认走 Unix socket + peer 认证，忽略 `PGPASSWORD` 环境变量导致密码错。现已自动补 `-h localhost -d postgres` 强制 TCP 密码认证
- **数据库设备报告提示符错误**：报告命令前缀显示 `<hostname>`（网络设备风格）而非 Linux `[user@host ~]#`，因 `device_prompt()` 未识别数据库厂商。现已对 database 设备和 MySQL/PostgreSQL 等厂商使用 Linux 风格提示符
- **种子数据残留 docker/podman 前缀命令**：移除 Linux 厂商下 `docker ps` / `podman ps` 两条耦合命令（v34 迁移同步清理已有 DB）

## v3.55.0 (2026-06-28)

### ✨ 新功能

- **运行时 OS 版本检测**：匿名统计上报的操作系统信息从编译期 `windows`/`linux` 升级为运行时精确版本（如 `Windows 11 Pro` / `Ubuntu 26.04`），用于分版本优化兼容性

### 🔧 改进

- **隐私说明**：匿名统计仅收集操作系统版本、软件版本号、匿名设备标识（机器名+MAC 的 SHA-256 哈希），**不收集** IP 地址、用户名、设备密码、巡检数据等任何敏感信息。数据仅用于问题定位和版本优化。

> 💡 **如需完全离线使用**：可从 [GitHub Releases](https://github.com/neowong/inspection-rust/releases) 下载 **internal 版**（仅 Windows，不含统计上报功能，保留问题反馈）。

## v3.54.1 (2026-06-28)

### 🐛 Bug 修复（全局代码审计）

**P1 关键修复:**

- **报告模板 is_default 事务完整性**：`update_report_template` 设默认时清零与字段更新未在同一事务，若字段更新失败则 DB 中零个默认模板，导致 `analyze_batch` 回退到空白默认配置
- **数据库端口未校验**：`db_port` 创建/更新时绕过 `validate_port()`，负数或溢出值直接拼入 shell 命令（如 `psql -p -1`）
- **Zabbix 缓冲区溢出 panic**：固定 65536 字节接收缓冲区，payload 声明在 65524~10M 之间时 `buf[total..]` 索引越界导致进程崩溃
- **端口扫描 JoinError 端口错报**：4 处 tokio 任务 JoinError 将端口 hardcode 为 0，改用 `(port, JoinHandle)` 元组正确跟踪
- **AI 客户端假兜底 panic**：`get_client()` 的 `unwrap_or_else` 构建完全相同的 reqwest client，第一次失败第二次必然 panic
- **巡检页 shake 校验无效**：`triggerShake("template_name")` 与 `shakeFields.has("name")` key 不匹配 + CSS 类名 `"shake"` 应为 `"animate-shake"`，导致空名称提交无视觉反馈
- **friendlyError 空值崩溃**：`JSON.stringify(undefined)` 返回 `undefined`，后续 `.includes()` 抛出 TypeError

**P2 修复:**

- **删除设备孤立报告文件**：`delete_device` / `batch_delete_devices` 删除数据库记录但不清理磁盘上的 .docx 文件，现已事务前收集路径、事务后 `safe_remove_report()`
- **重启批次孤立报告文件**：`restart_batch` 设 `report_path=NULL` 不删文件 + 与在途 SSH 任务竞态写入旧结果，现收集并清理文件 + 300ms 停止窗口降低竞态
- **AI 配置多行激活**：`update_ai_config` 可直接设 `is_active=1` 不清零其他配置，现包裹事务先清零再更新
- **SSH 通道写入错误静默丢弃**：`write_all_nb` 非 WouldBlock 写入错误被 `return` 吞掉，现返回 `Result` 并传播命令写入失败
- **DOCX 目录函数死参数**：`build_device_catalog` 的 `_items` 参数从未使用，已移除并文档化

### 🔧 改进

- **删除死代码 `shell_escape_dq`**：带 `#[allow(dead_code)]` 从未调用，`shell_quote_single` 已取代
- **clippy 自动修复**：2 处 collapsible_if + 1 处 useless_format

## v3.54.0 (2026-06-27)

### ✨ 新功能
- **数据库巡检多厂商模板**：数据库模板支持混合选择操作系统和数据库命令（如 Linux + MySQL），右侧可选命令面板显示厂商标签页（全部/Linux/MySQL/...），已选命令显示厂商徽章
- **命令与部署方式解耦**：命令库只存裸命令，执行引擎按设备部署方式自动包装——包安装直接注入认证，容器通过 `docker exec -e` 传递密码环境变量
- **数据库认证自动注入**：所有部署方式的数据库命令都自动注入 `db_username` / `db_password`，支持 MySQL（MYSQL_PWD）和 PostgreSQL（PGPASSWORD）

### 🐛 Bug 修复
- **数据库巡检认证失败**：包安装的数据库巡检命令未注入认证信息，导致 Access denied
- **密码泄露到 shell**：`shell_escape_dq` 在单引号上下文中错误转义反斜杠，导致密码被当作命令执行
- **报告巡检项显示原始命令**：容器包装后的命令未映射回原始命令，报告中显示 `docker exec ...` 而非命令描述
- **报告密码泄露**：SSH 回显的 `MYSQL_PWD='xxx'` 出现在报告输出中，通过 `-e` 传递和回显剥离修复
- **报告静态信息字段缺失**：`db_version` 和 `instance_name` 在报告引擎中未映射，设备信息表中为空
- **进度显示泄露密码**：巡检进度写入 DB 时包含明文密码，新增 `sanitize_cmd_for_display` 脱敏

### 🔧 改进
- **移除 k8s 部署选项**：前端下拉框和后端检测逻辑移除 Kubernetes 支持
- **种子数据清理**：删除数据库厂商下 13 条 docker/podman 前缀命令，新增 Linux 厂商下 `docker ps` / `podman ps` 容器管理命令
- **数据库迁移 v33**：自动清理用户数据库中旧的耦合命令

## v3.53.0 (2026-06-26)

### ✨ 新功能
- **AI 模式**：侧边栏新增「AI 模式」入口，基于自然语言对话控制系统
  - 支持 Function Calling（Tool Use），可真实调用系统接口
  - 可用工具：scan_live_hosts / list_devices / get_stats / check_device_status / update_device / create_device / list_templates / update_template / list_batches / run_batch / analyze_batch
  - 示例问题引导，点击自动填充输入框
  - 模型选择器：输入框内切换已配置的 AI 模型
  - Claude.ai 风格 UI：大圆角输入框 + 欢迎页 + 胶囊建议
  - 聊天历史持久化（localStorage），按日期分组
  - Markdown 渲染（react-markdown + remark-gfm）
  - 消息复制，切换页面后状态保持
- **侧边栏重构**：传统模式 / AI 模式可切换，默认传统模式
  - 传统模式收起时显示导航图标
  - AI 模式收起时显示聊天历史图标
  - 品牌区域点击切换收起/展开
- **报告封面独立定制**：封面和报告内容分离配置
  - 封面仅组合报告输出，单设备报告不输出
  - 预览两页 1:1 比例（封面页 + 报告页）
  - 目录可选项，勾选后在封面和设备报告之间插入目录页
  - 封面排版重设计：标题/分割线/副标题/日期底部
  - 封面无页眉页脚，正文页码从目录开始
- **仪表盘设备分类新增「其它」**
- **设备管理筛选新增「其它」选项**

### 🐛 Bug 修复
- **Linux 关闭按钮失效**：`visible:false` + `window.show()` 在 WebKitGTK 下导致标题栏装饰未初始化，改为 `visible:true` + `transparent:true`
- **启动闪烁优化**：非 Linux 平台 setup 入口立即 hide+show 消除白屏
- **Windows 日志文件行尾修复**：tracing 默认写 LF，Windows 记事本不识别，添加 CRLF 转换器
- **版本检测 internal- 前缀兼容**：`internal-v3.53.0` tag 去掉 `internal-` 前缀后再比较，避免误报更新
- **设备检测逻辑重构**：合并双线程为单线程 60s 全量轮询，try_lock 改 lock，分批 50 台并发 TCP 探测，前端 30s 自动刷新
- **AI 分析状态跨页面保持**：processingBatches 升级为模块级 ref
- **get_stats 死锁修复**：重复 `let db = state.db.lock()` 导致 parking_lot 死锁
- **tool_calls 每轮都发送 tools**：修复第二轮无工具可用导致 AI 只说不做
- **execute_tool 改用 async**：消除 `block_on` 死锁
- **对话历史 title 丢失**：useRef 解决 stale closure
- **chatId ↔ session.id 循环依赖**：去掉 sync effect
- **预览缩放自适应**：根据容器宽度等比缩小
- **综合报告封面使用定制模板**：优先取 cover.title 非空的模板
- **build_cover 真正使用模板标题/副标题**：不再硬编码项目名
- **DeepSeek 响应兼容、URL 拼接、AI 配置测试连接**
- **Modal 稳定性修复**

### 🔧 改进
- **发版流程**：master 全平台，internal 仅 Windows
- **构建目标**：Windows 改为 MSI（避免 SmartScreen），去掉 AppImage
- **侧边栏**：AI 模式入口图标改为 Bot，收起后显示聊天历史
- **报告预览**：两页 1:1 比例，封面/报告分页展示
- **报告模板**：「设备类别」→「厂商」，表单与表格统一
- **设备类型**：新增「其它」选项，支持自定义厂商
- **设备检测**：简化为 60s 全量并发轮询，前端 30s 自动刷新

## v3.52.0 (2026-06-25)

### ✨ 新功能
- **新建巡检任务 UI 改进**：去掉「创建后自动执行」复选框，改为「仅创建」+「创建并执行」两个按钮
- **版本号同步脚本**：`npm run version <x.y.z>` 一键同步 `package.json` / `Cargo.toml` / `tauri.conf.json` 三处版本号

### 🎨 界面优化
- **启动时消除黑色方块闪烁**：窗口默认隐藏，WebView 加载完成后显示

### 🔒 安全修复
- **sh-c 命令注入防护**：容器名/DB用户名入库白名单校验 `[A-Za-z0-9_.:-]`；`db_username` 单引号包裹；`db_password` 双引号层转义（修复 `$`/反引号被外层 shell 展开）
- **sh-c 转义改为单引号**：`docker exec`/`kubectl exec` 内层命令从双引号 `sh -c "..."` 改为单引号 `sh -c '...'`，避免 `$`/反引号注入（L8）
- **路径删除/复制统一校验**：`canonicalize()` + `starts_with(reports_dir)` 覆盖所有路径操作（H5/M2/M3/M5/L1）
- **AI base_url scheme 校验**：`create`/`update_ai_config` 校验 `http://`/`https://`（L6）
- **AI 客户端禁重定向** + 兜底带超时（L7/M11）
- **AI 错误体/debug 日志打码**：`redact_secrets` 对 `sk-*`/`Bearer` 替换（L4/L5）
- **ip2region 下载 30MB 上限**（L11）
- **AboutPage open() 限 github.com 前缀**（L9）
- **ToolsPage 外链 rel=noopener**（L10）

### 🐛 Bug 修复
- **SNMP 空密码死循环**：`localize_key!` 宏 `chunk=0` 时 `while remaining > 0` 死循环，加 `break`（H3）
- **SSH EAGAIN 命令丢失**：新增 `write_all_nb` WouldBlock 重试循环，覆盖命令写入/密码写入/分页符写入（H4）
- **finalize 覆盖 stopped 状态**：用户停止后子记录重算成 `partially_completed`，加 stopped 保留守卫（逻辑 H1）
- **多默认模板**：`update_report_template` 设 `is_default=1` 时先事务清空旧默认（H6）
- **cancel flag 泄漏**：`run_batch` early-return 清理注册的 cancel flag（M1）
- **analyze_record 静默吞错**：DB 回写失败改 `tracing::error!`（M3）
- **LiveScanner 监听器泄漏**：unlistenRef + 卸载清理（H8）
- **useShakeValidation 定时器泄漏**：useEffect 卸载清理（M13）

### 🧹 死代码清理
- 删除 `get_device`（未注册、前端无调用）
- 删除 `generate_batch_docx_zip`（历史残留，已被 combined 替代）
- `detect_device_model` / `track_usage` 去掉冗余 `#[tauri::command]`

## v3.51.0 (2026-06-24)

### ✨ 新功能
- **AI 评判提示词**：命令库新增「AI 评判提示词」字段，填写期望阈值/判断标准后，AI 评判时自动拼入 prompt 作为 `【期望】`，使结果更准确
- **自定义厂商**：命令库新增 `+` 按钮，支持添加内置列表之外的厂商；自定义厂商自动出现在 Tab 栏和下拉框，排在其它之上
- **报告重生成**：批次工具栏按钮根据状态动态切换文案，「AI评判」已有结果时变为「重新AI评判」，「人工评判」已有报告时变为「重新生成」；多任务并发不互锁；操作完成后显示绿色反馈提示
- **启动清理**：意外退出后重新打开，卡在「分析中」的记录自动置为 failed，可重新分析

### 🔧 改进
- **命令分类整理**：`cpu`/`memory` 合并为「性能」；`fan`/`power` 归入「硬件信息」；`vlan` 归入「接口」；`env` 改名为「运行环境」
- **分类下拉中文同步**：命令分类 Select 选项改为中文标签，与 CommandList 展示一致
- **IP 归属地修复**：ip2region 对私有 IP 返回 "Reserved" 时正确显示为「局域网」

### 🐛 Bug 修复
- **切批次状态丢失**：切换批次后回来仍保留原任务的加载状态和反馈提示
- **generateAllReports 闭包引用**：操作开始时提前捕获 records，避免切批次后引用别的任务
- **flashBatchDone 串到别的任务**：加批次 ID 校验，切走后不显示反馈
- **多任务互锁**：Task A 运行时 Task B 的按钮仍然可用（processingBatches 独立追踪）

## v3.50.1 (2026-06-23)

### 🐛 Bug 修复
- **版本号显示修复**：关于页面硬编码旧版本号 → 改为动态调用 `get_app_version()`，始终显示真实版本
- **误报更新修复**：关于页面用旧版本号对比 GitHub Releases 导致误报 → 使用真实版本号对比
- **统计服务端 IP 恢复**：上报记录 IP 列恢复，从 X-Forwarded-For 提取客户端真实 IP
- **统计 Dashboard 反馈区恢复**：重新部署含反馈模块的最新版 Dashboard
- **统计服务端 Docker 网络持久化**：容器重启后自动连接 nginx 网络，不再出现 502

## v3.50.0 (2026-06-23)

### 🔒 全局安全审计
- SNMP BER 解码：DES 密钥长度修复、OID 解码溢出保护、整数解析边界检查
- Zabbix 协议解析：checked_add 溢出保护、载荷 10MB 上限
- SSH 连接日志：增加 TCP/总耗时统计，便于排查
- 数据库密码安全：MySQL 使用 MYSQL_PWD 环境变量、PostgreSQL 单引号转义
- AI 调试日志：移除完整 prompt 输出（可能含设备配置敏感信息），仅记录长度
- 统计服务端安全加固：JWT_SECRET/ADMIN_PASSWORD 强制要求、速率限制、输入校验

### 🐛 Bug 修复
- `parking_lot::Mutex` 替换 `std::sync::Mutex` 避免中毒
- `run_batch` 竞态窗口修复（cancel flag 先于 status 注册）
- `stop_batch` 已完成批次保护
- `retry_device` 竞态修复
- SQL 参数化替代字符串拼接
- 报告文件路径穿越防护
- Traceroute 目标输入校验
- 端口扫描实时事件推送 + unwrap 容错

### 🏗️ CI/CD
- macOS Universal 构建 (x86_64 + aarch64)
- 版本号统一管理：前后端共用 `env!("CARGO_PKG_VERSION")`

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
