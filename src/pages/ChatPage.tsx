import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Bot, User, Loader2, Sparkles, Server, Play, Search, BarChart3, ChevronDown, Check, ArrowUp } from "lucide-react";

interface Message {
  role: "user" | "assistant";
  content: string;
}

interface AiConfig {
  id: number;
  name: string;
  model: string;
  is_active: boolean;
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
  { icon: BarChart3, text: "查看系统状态" },
  { icon: Server, text: "添加设备" },
  { icon: Play, text: "执行巡检" },
  { icon: Search, text: "扫描网络" },
];

const PROMPT_MAP: Record<string, string> = {
  "查看系统状态": "帮我查看一下系统当前的状态概览",
  "添加设备": "我想添加一台网络设备",
  "执行巡检": "帮我执行一次巡检任务",
  "扫描网络": "帮我扫描一下网段内的存活主机",
};

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
      setMessages(prev => [...prev, { role: "user", content: msg }, { role: "assistant", content: "请先在输入框右下角选择一个 AI 模型，或在「系统设置」中添加模型配置。" }]);
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
    <div className="flex flex-col h-[calc(100vh-7rem)]">
      {/* 消息区域 */}
      <div className="flex-1 overflow-y-auto">
        {isEmpty ? (
          /* 欢迎界面 */
          <div className="flex flex-col items-center justify-center h-full px-4">
            <div className="w-16 h-16 rounded-2xl flex items-center justify-center mb-6"
              style={{ backgroundColor: "hsl(var(--accent) / 0.1)" }}>
              <Sparkles size={32} style={{ color: "hsl(var(--accent))" }} />
            </div>
            <h2 className="text-2xl font-semibold mb-3" style={{ color: "hsl(var(--text-primary))" }}>
              有什么可以帮你的？
            </h2>
            <p className="text-sm mb-10" style={{ color: "hsl(var(--text-tertiary))" }}>
              我可以帮你管理设备、执行巡检、扫描网络等操作
            </p>
            <div className="flex flex-wrap justify-center gap-2 max-w-lg">
              {SUGGESTIONS.map((s, i) => {
                const Icon = s.icon;
                return (
                  <button
                    key={i}
                    onClick={() => handleSend(PROMPT_MAP[s.text] || s.text)}
                    className="inline-flex items-center gap-2 px-4 py-2.5 rounded-full text-[13px] transition-colors cursor-pointer"
                    style={{
                      border: "1px solid hsl(var(--border))",
                      color: "hsl(var(--text-primary))",
                      backgroundColor: "transparent",
                    }}
                    onMouseEnter={e => (e.currentTarget.style.backgroundColor = "hsl(var(--bg-hover))")}
                    onMouseLeave={e => (e.currentTarget.style.backgroundColor = "transparent")}
                  >
                    <Icon size={14} style={{ opacity: 0.5 }} />
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
              <div key={i} className="flex gap-4">
                {msg.role === "assistant" ? (
                  <div className="w-7 h-7 rounded-md flex items-center justify-center shrink-0 mt-1"
                    style={{ backgroundColor: "hsl(var(--accent))" }}>
                    <Bot size={14} className="text-white" />
                  </div>
                ) : (
                  <div className="w-7 h-7 rounded-md flex items-center justify-center shrink-0 mt-1"
                    style={{ backgroundColor: "hsl(var(--sidebar-bg))" }}>
                    <User size={14} className="text-white" />
                  </div>
                )}
                <div className="flex-1 min-w-0">
                  <div className="text-[13px] font-medium mb-1" style={{ color: "hsl(var(--text-secondary))" }}>
                    {msg.role === "assistant" ? "AI 巡检助手" : "你"}
                  </div>
                  <div className="text-sm leading-relaxed whitespace-pre-wrap" style={{ color: "hsl(var(--text-primary))" }}>
                    {msg.content}
                  </div>
                </div>
              </div>
            ))}
            {loading && (
              <div className="flex gap-4">
                <div className="w-7 h-7 rounded-md flex items-center justify-center shrink-0 mt-1"
                  style={{ backgroundColor: "hsl(var(--accent))" }}>
                  <Bot size={14} className="text-white" />
                </div>
                <div className="flex-1">
                  <div className="text-[13px] font-medium mb-1" style={{ color: "hsl(var(--text-secondary))" }}>
                    AI 巡检助手
                  </div>
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
      <div className="shrink-0 pb-5 pt-2">
        <div className="max-w-3xl mx-auto px-4">
          <div className="relative rounded-2xl border shadow-sm transition-all
            focus-within:border-[hsl(var(--accent) / 0.4)] focus-within:shadow-md"
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
              placeholder="给 AI 巡检助手发送消息..."
              disabled={loading}
              rows={1}
              className="w-full resize-none bg-transparent px-4 pt-3.5 pb-12 text-sm outline-none
                placeholder:text-[hsl(var(--text-tertiary))] disabled:opacity-50 max-h-48"
              style={{ color: "hsl(var(--text-primary))" }}
              onInput={(e) => {
                const target = e.target as HTMLTextAreaElement;
                target.style.height = "auto";
                target.style.height = Math.min(target.scrollHeight, 192) + "px";
              }}
            />

            {/* 底部工具栏 */}
            <div className="absolute bottom-0 left-0 right-0 flex items-center justify-between px-3 py-2.5">
              {/* 左侧：模型选择 */}
              <div ref={modelListRef} className="relative">
                <button
                  onClick={() => setShowModelList(!showModelList)}
                  className="flex items-center gap-1 px-2 py-1 rounded-md text-[12px] transition-colors
                    hover:bg-[hsl(var(--bg-hover))]"
                  style={{ color: "hsl(var(--text-tertiary))" }}
                >
                  {selectedConfig ? (
                    <span>{selectedConfig.model}</span>
                  ) : (
                    <span>选择模型</span>
                  )}
                  <ChevronDown size={11} className={`transition-transform ${showModelList ? "rotate-180" : ""}`} />
                </button>

                {showModelList && configs.length > 0 && (
                  <div
                    className="absolute bottom-full left-0 mb-1 w-56 rounded-xl border shadow-lg overflow-hidden z-50"
                    style={{ backgroundColor: "hsl(var(--bg-card))", borderColor: "hsl(var(--border))" }}
                  >
                    <div className="py-1">
                      {configs.map(c => (
                        <button
                          key={c.id}
                          onClick={() => { setSelectedId(c.id); setShowModelList(false); }}
                          className="flex items-center justify-between w-full px-3 py-2 text-left text-sm transition-colors
                            hover:bg-[hsl(var(--bg-hover))]"
                          style={{ color: "hsl(var(--text-primary))" }}
                        >
                          <div className="flex items-center gap-2">
                            <span className="font-medium">{c.model}</span>
                          </div>
                          {c.id === selectedId && (
                            <Check size={14} style={{ color: "hsl(var(--accent))" }} />
                          )}
                        </button>
                      ))}
                    </div>
                  </div>
                )}
              </div>

              {/* 右侧：发送按钮 */}
              <button
                onClick={() => handleSend()}
                disabled={loading || !input.trim()}
                className="flex items-center justify-center w-7 h-7 rounded-lg transition-all
                  disabled:opacity-20"
                style={{
                  backgroundColor: input.trim() ? "hsl(var(--accent))" : "hsl(var(--text-tertiary) / 0.2)",
                  color: input.trim() ? "white" : "hsl(var(--text-tertiary))",
                }}
              >
                <ArrowUp size={14} />
              </button>
            </div>
          </div>

          <p className="text-center text-[11px] mt-2.5" style={{ color: "hsl(var(--text-tertiary) / 0.6)" }}>
            AI 巡检助手可能会犯错，请核实重要信息
          </p>
        </div>
      </div>
    </div>
  );
}
