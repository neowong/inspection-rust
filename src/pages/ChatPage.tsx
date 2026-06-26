import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Send, Bot, User, Loader2, Sparkles, Server, Play, Search, BarChart3 } from "lucide-react";

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

const SUGGESTIONS = [
  { icon: BarChart3, text: "查看系统状态", prompt: "帮我查看一下系统当前的状态概览" },
  { icon: Server, text: "添加设备", prompt: "我想添加一台网络设备" },
  { icon: Play, text: "执行巡检", prompt: "帮我执行一次巡检任务" },
  { icon: Search, text: "扫描网络", prompt: "帮我扫描一下网段内的存活主机" },
];

export default function ChatPage() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleSend = async (text?: string) => {
    const msg = (text || input).trim();
    if (!msg || loading) return;

    setInput("");
    setMessages(prev => [...prev, { role: "user", content: msg }]);
    setLoading(true);

    try {
      const configs = await invoke<Array<{ id: number }>>("list_ai_configs");
      if (!configs || configs.length === 0) {
        setMessages(prev => [...prev, { role: "assistant", content: "请先在「系统设置」中配置并激活一个 AI 模型，才能使用对话模式。" }]);
        setLoading(false);
        return;
      }

      const result = await invoke<string>("chat_with_ai", {
        systemPrompt: SYSTEM_PROMPT,
        messages: [...messages, { role: "user", content: msg }],
      });

      setMessages(prev => [...prev, { role: "assistant", content: result }]);
    } catch (e) {
      setMessages(prev => [...prev, { role: "assistant", content: `抱歉，出现了错误：${e}` }]);
    } finally {
      setLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const isEmpty = messages.length === 0;

  return (
    <div className="flex flex-col h-[calc(100vh-7rem)]">
      {/* 消息区域 */}
      <div className="flex-1 overflow-y-auto">
        {isEmpty ? (
          /* 欢迎界面 */
          <div className="flex flex-col items-center justify-center h-full px-4">
            <div className="w-14 h-14 rounded-2xl flex items-center justify-center mb-5"
              style={{ backgroundColor: "hsl(var(--accent) / 0.12)" }}>
              <Sparkles size={28} style={{ color: "hsl(var(--accent))" }} />
            </div>
            <h2 className="text-xl font-semibold mb-2" style={{ color: "hsl(var(--text-primary))" }}>
              有什么可以帮你的？
            </h2>
            <p className="text-sm mb-8" style={{ color: "hsl(var(--text-tertiary))" }}>
              我可以帮你管理设备、执行巡检、扫描网络等操作
            </p>
            <div className="grid grid-cols-2 gap-3 w-full max-w-md">
              {SUGGESTIONS.map((s, i) => {
                const Icon = s.icon;
                return (
                  <button
                    key={i}
                    onClick={() => handleSend(s.prompt)}
                    className="flex items-center gap-3 px-4 py-3 rounded-xl border text-left text-sm transition-all
                      hover:shadow-sm hover:border-[hsl(var(--accent) / 0.3)]"
                    style={{
                      borderColor: "hsl(var(--border))",
                      color: "hsl(var(--text-secondary))",
                      backgroundColor: "hsl(var(--bg-card))",
                    }}
                  >
                    <Icon size={16} style={{ color: "hsl(var(--accent))" }} className="shrink-0" />
                    <span>{s.text}</span>
                  </button>
                );
              })}
            </div>
          </div>
        ) : (
          /* 消息列表 */
          <div className="max-w-3xl mx-auto px-4 py-6 space-y-6">
            {messages.map((msg, i) => (
              <div key={i} className={`flex gap-3 ${msg.role === "user" ? "justify-end" : ""}`}>
                {msg.role === "assistant" && (
                  <div className="w-8 h-8 rounded-lg flex items-center justify-center shrink-0 mt-0.5"
                    style={{ backgroundColor: "hsl(var(--accent) / 0.12)" }}>
                    <Bot size={16} style={{ color: "hsl(var(--accent))" }} />
                  </div>
                )}
                <div className={`max-w-[80%] ${msg.role === "user" ? "order-first" : ""}`}>
                  <div
                    className={`rounded-2xl px-4 py-3 text-sm leading-relaxed whitespace-pre-wrap ${
                      msg.role === "user"
                        ? "rounded-tr-md"
                        : "rounded-tl-md border"
                    }`}
                    style={msg.role === "user"
                      ? { backgroundColor: "hsl(var(--accent))", color: "white" }
                      : { backgroundColor: "hsl(var(--bg-card))", borderColor: "hsl(var(--border))", color: "hsl(var(--text-primary))" }
                    }
                  >
                    {msg.content}
                  </div>
                </div>
                {msg.role === "user" && (
                  <div className="w-8 h-8 rounded-lg flex items-center justify-center shrink-0 mt-0.5"
                    style={{ backgroundColor: "hsl(var(--sidebar-active))" }}>
                    <User size={16} className="text-white" />
                  </div>
                )}
              </div>
            ))}
            {loading && (
              <div className="flex gap-3">
                <div className="w-8 h-8 rounded-lg flex items-center justify-center shrink-0"
                  style={{ backgroundColor: "hsl(var(--accent) / 0.12)" }}>
                  <Bot size={16} style={{ color: "hsl(var(--accent))" }} />
                </div>
                <div className="rounded-2xl rounded-tl-md px-4 py-3 border"
                  style={{ backgroundColor: "hsl(var(--bg-card))", borderColor: "hsl(var(--border))" }}>
                  <div className="flex items-center gap-2">
                    <Loader2 size={14} className="animate-spin" style={{ color: "hsl(var(--text-tertiary))" }} />
                    <span className="text-sm" style={{ color: "hsl(var(--text-tertiary))" }}>思考中...</span>
                  </div>
                </div>
              </div>
            )}
            <div ref={messagesEndRef} />
          </div>
        )}
      </div>

      {/* 输入区域 */}
      <div className="shrink-0 pb-4 pt-2">
        <div className="max-w-3xl mx-auto px-4">
          <div className="relative flex items-end rounded-2xl border shadow-sm transition-colors
            focus-within:border-[hsl(var(--accent) / 0.5)]"
            style={{
              backgroundColor: "hsl(var(--bg-input))",
              borderColor: "hsl(var(--border))",
            }}
          >
            <textarea
              ref={inputRef}
              value={input}
              onChange={e => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="输入指令，如「查看系统状态」「添加一台交换机」..."
              disabled={loading}
              rows={1}
              className="flex-1 resize-none bg-transparent px-4 py-3 text-sm outline-none
                placeholder:text-[hsl(var(--text-tertiary))] disabled:opacity-50 max-h-32"
              style={{ color: "hsl(var(--text-primary))" }}
              onInput={(e) => {
                const target = e.target as HTMLTextAreaElement;
                target.style.height = "auto";
                target.style.height = Math.min(target.scrollHeight, 128) + "px";
              }}
            />
            <button
              onClick={() => handleSend()}
              disabled={loading || !input.trim()}
              className="flex items-center justify-center w-9 h-9 mr-1.5 mb-1.5 rounded-xl transition-all
                disabled:opacity-30"
              style={{
                backgroundColor: input.trim() ? "hsl(var(--accent))" : "transparent",
                color: input.trim() ? "white" : "hsl(var(--text-tertiary))",
              }}
            >
              <Send size={16} />
            </button>
          </div>
          <p className="text-center text-[11px] mt-2" style={{ color: "hsl(var(--text-tertiary))" }}>
            AI 巡检助手 · 对话模式
          </p>
        </div>
      </div>
    </div>
  );
}
