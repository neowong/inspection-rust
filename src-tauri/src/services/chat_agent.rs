/// AI 对话助手服务（LangChain 替代）
///
/// 对应 Python: backend/app/services/chat_agent.py
/// 提供巡检系统全流程管理的 AI 对话功能
use tracing::info;

pub const CHAT_SYSTEM_PROMPT: &str = r#"你是一个自动化设备巡检系统的AI助手，支持完整的设备巡检全流程管理。

# 系统覆盖范围
## 1. 网络设备（group=network, mode=ssh）
   厂商: Cisco/思科, H3C/华三, 华为, 锐捷, 深信服
   类型: 交换机, 路由器, 防火墙, 无线控制器
   巡检方式: SSH 登录执行命令

## 2. Linux 发行版（group=system, mode=ssh）
   厂商: CentOS, Ubuntu, RHEL, openEuler, Linux
   类型: 服务器
   巡检方式: SSH 登录执行命令

## 3. 主流数据库（group=system, mode=ssh）
   厂商: MySQL, PostgreSQL, Oracle
   类型: 数据库
   巡检方式: SSH 登录后执行 SQL 或系统命令

# 设备管理
- 列出设备、添加设备、更新设备、删除设备
- 检查设备在线状态

# 命令库管理
- 列出命令、添加命令、更新命令、删除命令

# 模板管理
- 列出模板、查看模板详情、创建模板
- AI 生成模板（根据厂商自动推荐命令）

# 巡检任务
- 创建批次、执行/暂停/停止/重启
- 查询批次状态

# AI 分析与报告
- AI 分析巡检结果
- 生成和下载巡检报告

# 定时任务
- 创建定时巡检任务、管理 cron 表达式

# 离线巡检
- 导出命令清单、导入执行结果

# 展示规范
1. 使用真实名称，不暴露数据库ID
2. 列表用 Markdown 表格
3. 操作成功后提示用户后续可进行的操作"#;

/// 构建带系统上下文的对话消息
pub fn build_chat_messages(
    system_context: &str,
    conversation: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    let mut messages = vec![serde_json::json!({
        "role": "system",
        "content": format!("{}\n\n{}", CHAT_SYSTEM_PROMPT, system_context),
    })];

    for msg in conversation {
        messages.push(msg.clone());
    }

    messages
}

/// 构建系统上下文（含设备列表、统计信息等）
pub fn build_system_context(db: &rusqlite::Connection) -> String {
    let device_count: i64 = db.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get(0)).unwrap_or(0);
    let template_count: i64 = db.query_row("SELECT COUNT(*) FROM inspection_templates", [], |r| r.get(0)).unwrap_or(0);
    let command_count: i64 = db.query_row("SELECT COUNT(*) FROM command_pool", [], |r| r.get(0)).unwrap_or(0);
    let batch_count: i64 = db.query_row("SELECT COUNT(*) FROM inspection_batches", [], |r| r.get(0)).unwrap_or(0);
    let online_count: i64 = db.query_row("SELECT COUNT(*) FROM devices WHERE status='online'", [], |r| r.get(0)).unwrap_or(0);

    let mut ctx = format!(
        "当前系统状态:\n- {} 台设备 ({} 在线)\n- {} 个巡检模板\n- {} 条命令\n- {} 个巡检批次\n",
        device_count, online_count, template_count, command_count, batch_count,
    );

    // Recent batches
    if let Ok(mut stmt) = db.prepare("SELECT id, name, mode, status FROM inspection_batches ORDER BY created_at DESC LIMIT 5") {
        if let Ok(rows) = stmt.query_map([], |r| Ok((
            r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?
        ))) {
            let batches: Vec<_> = rows.filter_map(|r| r.ok()).collect();
            if !batches.is_empty() {
                ctx.push_str("\n最近巡检批次:\n");
                for (id, name, mode, status) in batches {
                    ctx.push_str(&format!("  - #{} {} ({}) - {}\n", id, name.unwrap_or_default(), mode, status));
                }
            }
        }
    }

    ctx
}

/// 按关键词路由到操作类型（简化版意图识别）
pub fn route_user_intent(query: &str) -> &str {
    let q = query.to_lowercase();
    if q.contains("设备") && (q.contains("列表") || q.contains("查看") || q.contains("列出")) { return "list_devices"; }
    if q.contains("设备") && (q.contains("添加") || q.contains("新增") || q.contains("创建")) { return "add_device"; }
    if q.contains("设备") && (q.contains("删除") || q.contains("移除")) { return "delete_device"; }
    if q.contains("模板") && (q.contains("列表") || q.contains("查看")) { return "list_templates"; }
    if q.contains("模板") && (q.contains("生成") || q.contains("创建")) { return "generate_template"; }
    if q.contains("命令") && (q.contains("列表") || q.contains("查看")) { return "list_commands"; }
    if q.contains("批次") || q.contains("标签页") { return "list_batches"; }
    if q.contains("巡检") && (q.contains("开始") || q.contains("创建") || q.contains("新建")) { return "create_batch"; }
    if q.contains("巡检") && (q.contains("状态") || q.contains("进展")) { return "batch_status"; }
    if q.contains("报告") || q.contains("下载") { return "download_report"; }
    if q.contains("统计") || q.contains("仪表盘") || q.contains("概况") { return "get_stats"; }
    if q.contains("定时") { return "list_tasks"; }
    if q.contains("AI") && q.contains("分析") { return "ai_analyze"; }
    "general_chat"
}
