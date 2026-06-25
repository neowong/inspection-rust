import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Send, Bot, User, Loader2 } from "lucide-react";

interface Message {
  role: "user" | "assistant";
  content: string;
}

const SYSTEM_PROMPT = `你是 AI 巡检助手的智能对话助手，帮助用户通过自然语言操作系统。

## 系统能力

### 设备管理
- list_devices：查询设备列表（支持按类型 switch/router/firewall/loadbalancer/server/database、状态 online/offline、厂商筛选）
- create_device：添加设备（参数：name 名称, ip IP地址, device_type 设备类型, vendor 厂商, ssh_username SSH用户名, ssh_password SSH密码, ssh_port SSH端口默认22）
- update_device：更新设备信息
- delete_device：删除设备（参数：device_id）
- check_device_status：检测单台设备在线状态
- check_all_devices_status：批量检测所有设备状态
- detect_device_model：自动检测设备型号、序列号、出厂日期

### 巡检执行
- list_templates：查询巡检模板列表
- create_template：创建巡检模板
- list_batches：查询巡检任务列表
- create_batch：创建巡检任务（参数：name 任务名称, device_ids 设备ID数组, template_id 模板ID, auto_start 是否自动执行）
- run_batch：执行巡检任务（参数：batch_id）
- pause_batch / stop_batch / restart_batch：暂停/停止/重启任务

### 报告分析
- analyze_record：AI 分析单条巡检记录
- analyze_batch：AI 分析整个批次
- generate_docx_report：生成单条 DOCX 报告
- generate_batch_docx_combined：生成批次合并报告
- get_stats：获取系统统计概览（设备数量、在线率、任务状态等）

### 工具箱
- scan_live_hosts：存活主机扫描（参数：cidr 网段如 192.168.1.0/24）
- scan_ports：TCP 端口扫描（参数：ip, ports 端口列表）
- scan_udp_ports：UDP 端口扫描
- check_web_urls：WEB 检测
- snmp_get / snmp_v3_get：SNMP 查询
- check_zabbix_agent：Zabbix Agent 检测
- trace_route：路由跟踪

## 回复规则
1. 用中文回复
2. 当用户要求执行操作时，说明你将调用哪个接口，确认参数后执行
3. 执行前需要确认的关键操作：删除设备、删除任务、停止正在运行的任务
4. 可以直接执行的查询操作：查询设备、查看状态、查看统计
5. 执行结果用简洁的格式展示
6. 如果用户信息不完整，主动询问缺少的参数`;

export default function ChatPage() {
  const [messages, setMessages] = useState<Message[]>([
    { role: "assistant", content: "你好！我是 AI 巡检助手的对话模式。你可以用自然语言告诉我你想做什么，比如：\n\n• 「查看一下系统状态」\n• 「添加一台 H3C 交换机」\n• 「扫描 192.168.1.0/24 网段的存活主机」\n• 「执行巡检任务」" }
  ]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const handleSend = async () => {
    const text = input.trim();
    if (!text || loading) return;

    setInput("");
    setMessages(prev => [...prev, { role: "user", content: text }]);
    setLoading(true);

    try {
      // 获取当前激活的 AI 配置
      const configs = await invoke<Array<{ id: number }>>("list_ai_configs");
      if (!configs || configs.length === 0) {
        setMessages(prev => [...prev, { role: "assistant", content: "请先在「系统设置」中配置 AI 模型，才能使用对话模式。" }]);
        setLoading(false);
        return;
      }

      // 调用 AI 分析接口（复用现有 AI 能力）
      const result = await invoke<string>("chat_with_ai", {
        systemPrompt: SYSTEM_PROMPT,
        messages: [...messages, { role: "user", content: text }],
      });

      setMessages(prev => [...prev, { role: "assistant", content: result }]);
    } catch (e) {
      setMessages(prev => [...prev, { role: "assistant", content: `错误：${e}` }]);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col h-[calc(100vh-7rem)]">
      {/* 消息区域 */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {messages.map((msg, i) => (
          <div key={i} className={`flex gap-3 ${msg.role === "user" ? "justify-end" : "justify-start"}`}>
            {msg.role === "assistant" && (
              <div className="w-8 h-8 rounded-lg flex items-center justify-center shrink-0"
                style={{ backgroundColor: "hsl(var(--accent) / 0.15)" }}>
                <Bot size={16} style={{ color: "hsl(var(--accent))" }} />
              </div>
            )}
            <div
              className={`max-w-[70%] rounded-xl px-4 py-2.5 text-sm leading-relaxed whitespace-pre-wrap ${
                msg.role === "user"
                  ? "text-white"
                  : "border"
              }`}
              style={msg.role === "user"
                ? { backgroundColor: "hsl(var(--accent))" }
                : { backgroundColor: "hsl(var(--bg-card))", borderColor: "hsl(var(--border))" }
              }
            >
              {msg.content}
            </div>
            {msg.role === "user" && (
              <div className="w-8 h-8 rounded-lg flex items-center justify-center shrink-0"
                style={{ backgroundColor: "hsl(var(--sidebar-active))" }}>
                <User size={16} className="text-white" />
              </div>
            )}
          </div>
        ))}
        {loading && (
          <div className="flex gap-3 justify-start">
            <div className="w-8 h-8 rounded-lg flex items-center justify-center shrink-0"
              style={{ backgroundColor: "hsl(var(--accent) / 0.15)" }}>
              <Bot size={16} style={{ color: "hsl(var(--accent))" }} />
            </div>
            <div className="rounded-xl px-4 py-2.5 border"
              style={{ backgroundColor: "hsl(var(--bg-card))", borderColor: "hsl(var(--border))" }}>
              <Loader2 size={16} className="animate-spin" style={{ color: "hsl(var(--text-tertiary))" }} />
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* 输入区域 */}
      <div className="shrink-0 p-4 border-t" style={{ borderColor: "hsl(var(--border))" }}>
        <div className="flex gap-2 max-w-3xl mx-auto">
          <input
            value={input}
            onChange={e => setInput(e.target.value)}
            onKeyDown={e => e.key === "Enter" && !e.shiftKey && handleSend()}
            placeholder="输入指令，如「查看系统状态」「添加一台交换机」..."
            disabled={loading}
            className="flex-1 h-10 px-4 rounded-lg border text-sm outline-none transition-colors
              focus:border-[hsl(var(--accent))] disabled:opacity-50"
            style={{
              backgroundColor: "hsl(var(--bg-input))",
              borderColor: "hsl(var(--border))",
              color: "hsl(var(--text-primary))",
            }}
          />
          <button
            onClick={handleSend}
            disabled={loading || !input.trim()}
            className="h-10 w-10 rounded-lg flex items-center justify-center transition-colors
              disabled:opacity-40"
            style={{ backgroundColor: "hsl(var(--accent))", color: "white" }}
          >
            <Send size={16} />
          </button>
        </div>
      </div>
    </div>
  );
}
