import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Bot, User, Loader2, Sparkles, Server, Play, Search, ChevronDown, Check, ArrowUp, Plus, FileText, Wrench, Monitor } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

interface Message {
  role: "user" | "assistant";
  content: string;
}

interface AiConfig {
  id: number;
  name: string;
  model_id: string;
  is_active: number;
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
  { icon: Monitor, text: "查看状态", prompt: "帮我查看一下系统当前的状态概览" },
  { icon: Server, text: "添加设备", prompt: "我想添加一台网络设备" },
  { icon: Play, text: "执行巡检", prompt: "帮我执行一次巡检任务" },
  { icon: Search, text: "扫描网络", prompt: "帮我扫描一下网段内的存活主机" },
  { icon: Wrench, text: "工具箱", prompt: "打开工具箱" },
  { icon: FileText, text: "生成报告", prompt: "帮我生成巡检报告" },
];

export default function ChatPage() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [configs, setConfigs] = useState<AiConfig[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [showModelList, setShowModelList] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const modelListRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    invoke<AiConfig[]>("list_ai_configs").then(list => {
      setConfigs(list);
      const active = list.find(c => c.is_active);
      if (active) setSelectedId(active.id);
      else if (list.length > 0) setSelectedId(list[0]!.id);
    }).catch(() => {});
  }, []);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (modelListRef.current && !modelListRef.current.contains(e.target as Node)) {
        setShowModelList(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const selectedConfig = configs.find(c => c.id === selectedId);

  const handleSend = async (text?: string) => {
    const msg = (text || input).trim();
    if (!msg || loading) return;

    if (!selectedId) {
      setMessages(prev => [...prev, { role: "user", content: msg }, { role: "assistant", content: "请先在系统设置中添加并激活一个 AI 模型。" }]);
      return;
    }

    setInput("");
    setMessages(prev => [...prev, { role: "user", content: msg }]);
    setLoading(true);

    try {
      const result = await invoke<string>("chat_with_ai", {
        configId: selectedId,
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
    <div className="flex flex-col h-[calc(100vh-7rem)]" style={{ backgroundColor: "hsl(var(--bg-content))" }}>
      {/* 消息区域 */}
      <div className="flex-1 overflow-y-auto">
        {isEmpty ? (
          /* 欢迎界面 - 居中，Claude 风格 */
          <div className="flex flex-col items-center justify-center h-full px-4">
            <div className="w-16 h-16 rounded-full flex items-center justify-center mb-5"
              style={{ backgroundColor: "hsl(var(--accent) / 0.1)" }}>
              <Sparkles size={32} style={{ color: "hsl(var(--accent))" }} />
            </div>
            <h2 className="text-[32px] font-medium mb-2 leading-tight" style={{ color: "hsl(var(--text-primary))", fontFamily: "'Times New Roman', serif" }}>
              有什么可以帮你的？
            </h2>
            {selectedConfig && (
              <div className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-[12px] mb-6"
                style={{ backgroundColor: "hsl(var(--bg-hover))", color: "hsl(var(--text-tertiary))" }}>
                <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: "hsl(var(--success))" }} />
                当前模型：{selectedConfig.name} · {selectedConfig.model_id}
              </div>
            )}
          </div>
        ) : (
          /* 消息列表 */
          <div className="max-w-3xl mx-auto px-4 py-8 space-y-8">
            {messages.map((msg, i) => (
              <div key={i} className="flex gap-4">
                {msg.role === "assistant" ? (
                  <div className="w-7 h-7 rounded-md flex items-center justify-center shrink-0 mt-0.5"
                    style={{ backgroundColor: "hsl(var(--accent))" }}>
                    <Bot size={14} className="text-white" />
                  </div>
                ) : (
                  <div className="w-7 h-7 rounded-md flex items-center justify-center shrink-0 mt-0.5"
                    style={{ backgroundColor: "hsl(var(--sidebar-bg))" }}>
                    <User size={14} className="text-white" />
                  </div>
                )}
                <div className="flex-1 min-w-0 pt-0.5">
                  <div className="text-sm font-medium mb-1" style={{ color: "hsl(var(--text-secondary))" }}>
                    {msg.role === "assistant" ? "AI 巡检助手" : "你"}
                  </div>
                  {msg.role === "assistant" ? (
                    <div className="prose prose-sm max-w-none leading-7" style={{ color: "hsl(var(--text-primary))" }}>
                      <ReactMarkdown remarkPlugins={[remarkGfm]}>{msg.content}</ReactMarkdown>
                    </div>
                  ) : (
                    <div className="text-[15px] leading-7 whitespace-pre-wrap" style={{ color: "hsl(var(--text-primary))" }}>
                      {msg.content}
                    </div>
                  )}
                </div>
              </div>
            ))}
            {loading && (
              <div className="flex gap-4">
                <div className="w-7 h-7 rounded-md flex items-center justify-center shrink-0 mt-0.5"
                  style={{ backgroundColor: "hsl(var(--accent))" }}>
                  <Bot size={14} className="text-white" />
                </div>
                <div className="flex-1 pt-0.5">
                  <div className="text-sm font-medium mb-1" style={{ color: "hsl(var(--text-secondary))" }}>
                    AI 巡检助手
                  </div>
                  <div className="flex items-center gap-2">
                    <Loader2 size={14} className="animate-spin" style={{ color: "hsl(var(--text-tertiary))" }} />
                    <span className="text-[15px]" style={{ color: "hsl(var(--text-tertiary))" }}>思考中...</span>
                  </div>
                </div>
              </div>
            )}
            <div ref={messagesEndRef} />
          </div>
        )}
      </div>

      {/* 输入区域 - Claude 风格大卡片 */}
      <div className="shrink-0 px-4 pb-6">
        <div className="max-w-3xl mx-auto">
          {/* 大圆角输入框 */}
          <div className="rounded-[24px] border transition-shadow focus-within:shadow-md"
            style={{
              backgroundColor: "hsl(var(--bg-card))",
              borderColor: "hsl(var(--border-light))",
            }}
          >
            <textarea
              ref={inputRef}
              value={input}
              onChange={e => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="给 AI 巡检助手发送消息..."
              disabled={loading}
              rows={1}
              className="w-full resize-none bg-transparent px-5 pt-4 pb-14 text-[15px] outline-none
                placeholder:text-[hsl(var(--text-tertiary))] disabled:opacity-50 max-h-48 rounded-[24px]"
              style={{ color: "hsl(var(--text-primary))", lineHeight: 1.6 }}
              onInput={(e) => {
                const target = e.target as HTMLTextAreaElement;
                target.style.height = "auto";
                target.style.height = Math.min(target.scrollHeight, 192) + "px";
              }}
            />
            {/* 底部工具栏 */}
            <div className="flex items-center justify-between px-3 pb-3 -mt-10">
              {/* 左侧：附件 + */}
              <button
                className="flex items-center justify-center w-8 h-8 rounded-lg transition-colors hover:bg-[hsl(var(--bg-hover))]"
                style={{ color: "hsl(var(--text-tertiary))" }}
                title="添加附件（暂未支持）"
              >
                <Plus size={18} />
              </button>
              {/* 右侧：模型选择 + 发送 */}
              <div className="flex items-center gap-2">
                <div ref={modelListRef} className="relative">
                  <button
                    onClick={() => setShowModelList(!showModelList)}
                    className="flex items-center gap-1 px-2 py-1 rounded-lg text-[13px] transition-colors hover:bg-[hsl(var(--bg-hover))]"
                    style={{ color: "hsl(var(--text-tertiary))" }}
                  >
                    {selectedConfig ? (
                      <span className="font-medium">{selectedConfig.name}</span>
                    ) : (
                      <span>选择模型</span>
                    )}
                    <ChevronDown size={14} className={`transition-transform ${showModelList ? "rotate-180" : ""}`} />
                  </button>
                  {showModelList && configs.length > 0 && (
                    <div
                      className="absolute bottom-full right-0 mb-2 w-56 rounded-xl border shadow-lg overflow-hidden z-50"
                      style={{ backgroundColor: "hsl(var(--bg-card))", borderColor: "hsl(var(--border))" }}
                    >
                      <div className="py-1">
                        {configs.map(c => (
                          <button
                            key={c.id}
                            onClick={() => { setSelectedId(c.id); setShowModelList(false); }}
                            className="flex items-center justify-between w-full px-3 py-2 text-left text-[13px] transition-colors hover:bg-[hsl(var(--bg-hover))]"
                            style={{
                              color: "hsl(var(--text-primary))",
                              backgroundColor: c.id === selectedId ? "hsl(var(--bg-hover))" : "transparent",
                            }}
                          >
                            <span className="font-medium">{c.name}</span>
                            <span className="text-[11px]" style={{ color: "hsl(var(--text-tertiary))" }}>{c.model_id}</span>
                            {c.id === selectedId && (
                              <Check size={14} style={{ color: "hsl(var(--accent))" }} />
                            )}
                          </button>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
                <button
                  onClick={() => handleSend()}
                  disabled={loading || !input.trim()}
                  className="flex items-center justify-center w-8 h-8 rounded-full transition-all disabled:opacity-30"
                  style={{
                    backgroundColor: input.trim() ? "hsl(var(--text-primary))" : "hsl(var(--text-tertiary) / 0.15)",
                    color: input.trim() ? "white" : "hsl(var(--text-tertiary))",
                  }}
                >
                  <ArrowUp size={16} />
                </button>
              </div>
            </div>
          </div>

          {/* 建议胶囊 - 仅在空状态时显示 */}
          {isEmpty && (
            <div className="flex flex-wrap justify-center gap-2 mt-4">
              {SUGGESTIONS.map((s, i) => {
                const Icon = s.icon;
                return (
                  <button
                    key={i}
                    onClick={() => handleSend(s.prompt)}
                    className="flex items-center gap-2 px-4 py-2.5 rounded-full text-[13px] transition-all hover:shadow-sm"
                    style={{
                      border: "1px solid hsl(var(--border-light))",
                      color: "hsl(var(--text-primary))",
                      backgroundColor: "hsl(var(--bg-card))",
                    }}
                    onMouseEnter={e => (e.currentTarget.style.borderColor = "hsl(var(--border))")}
                    onMouseLeave={e => (e.currentTarget.style.borderColor = "hsl(var(--border-light))")}
                  >
                    <Icon size={14} style={{ color: "hsl(var(--text-tertiary))" }} />
                    <span>{s.text}</span>
                  </button>
                );
              })}
            </div>
          )}

          <p className="text-center text-[11px] mt-3" style={{ color: "hsl(var(--text-tertiary) / 0.5)" }}>
            AI 巡检助手可能会犯错，请核实重要信息
          </p>
        </div>
      </div>
    </div>
  );
}
