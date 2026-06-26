import { useState, useRef, useEffect } from "react";
import { useSearchParams } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { Bot, User, Loader2, ChevronDown, Check, ArrowUp, Plus, Copy, CheckCheck } from "lucide-react";
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

export interface ChatSession {
  id: string;
  title: string;
  messages: Message[];
  createdAt: string;
  updatedAt: string;
}

const SYSTEM_PROMPT = `你是 AI 巡检助手的智能对话助手，可以实际调用系统工具来完成操作，不只是文字回复。

## 可用工具及参数

### 统计概览
**get_stats()** — 获取系统统计概览。无参数。
返回示例：{"device_count":10,"online_device_count":7,"offline_device_count":3,"template_count":5,...}

### 设备管理

**list_devices(vendor?, device_type?, status?)** — 查询设备列表。
- vendor: 按厂商名筛选，如 "H3C","华为","Cisco"
- device_type: 设备类型。可选值：switch, router, firewall, loadbalancer, server, database。支持逗号多选如 "switch,router"。传 "other" 表示其他类型。
- status: "online" 或 "offline"
返回数组，每项含 id,name,ip,device_type,vendor,status。

**create_device(name, ip, device_type, vendor)** — 添加新设备。
- name: 设备名称（必需）
- ip: IP地址（必需）
- device_type: 设备类型（必需），可选 switch/router/firewall/loadbalancer/server/database
- vendor: 厂商名称（必需），常见值：H3C、华为、思科、锐捷、飞塔、Linux、MySQL 等
返回新设备的完整信息。

**update_device(device_id, name?, ip?, device_type?, vendor?)** — 修改设备信息。
- device_id: 设备ID（必需，整数）
- 其他字段均为可选，只传要修改的字段即可
返回修改后的设备信息。

**check_device_status(device_id)** — 检测单台设备在线状态。
- device_id: 设备ID（必需，整数）
返回 {"device_id":1,"old_status":"offline","new_status":"online"}。

**check_all_devices_status()** — 批量检测所有设备在线状态。无参数。
返回 {"total":10,"online":7,"offline":3}。

### 巡检管理

**list_templates()** — 查询巡检模板列表。无参数。
返回数组，每项含 id,name,vendor。

**update_template(template_id, name?, vendor?, device_type?, description?)** — 修改巡检模板。
- template_id: 模板ID（必需，整数）
- 其他字段均为可选
返回修改后的模板信息。

**list_batches(status?)** — 查询巡检任务列表。
- status: 任务状态过滤，可选值：pending, running, paused, completed, partially_completed, stopped, failed。支持逗号多选。
返回数组，每项含 id,name,status,records 等。

**run_batch(batch_id)** — 执行巡检任务，对批次中所有设备发起 SSH 巡检。
- batch_id: 批次ID（必需，整数）
返回成功后任务状态变为 running。

**analyze_batch(batch_id, force?)** — AI 分析巡检结果。
- batch_id: 批次ID（必需，整数）
- force: 设为 true 可强制重新分析已完成的批次
返回分析结果 JSON。

### 工具箱

**scan_live_hosts(subnet, timeout_ms?)** — 存活主机扫描（ICMP ping + TCP 回退）。
- subnet: CIDR 网段，如 "192.168.1.0/24"（必需）
- timeout_ms: 每台超时毫秒，默认 3000
返回数组 [{ip:"192.168.1.1",alive:true,response_time_ms:5.2},...]。

## 回复规则
1. 用中文回复，不要用英文
2. **所有操作必须实际调用工具执行，不能只是文字回复说「已修改」「已完成」**
3. 需要多步骤时（如先查询 ID 再修改），在同一轮连续调用 2 个工具完成
4. 关键号码/数量用加粗展示，IP 和名称用代码块
5. 用户信息不够时主动询问，但不要反复确认——查一次就知道的别问
6. 如果工具调用失败，如实告诉用户失败原因`;

const EXAMPLES = [
  "查看系统当前的状态概览",
  "添加一台 H3C 交换机 192.168.1.100",
  "帮我扫描一下 192.168.31.0/24 网段",
  "把设备 fg 的名称修改为 飞塔防火墙",
  "帮我执行一次巡检任务",
  "生成最新的巡检报告",
];

// ── 会话持久化工具 ──

function loadSessions(): ChatSession[] {
  try {
    return JSON.parse(localStorage.getItem("chat_sessions") || "[]");
  } catch { return []; }
}

function saveSessions(sessions: ChatSession[]) {
  localStorage.setItem("chat_sessions", JSON.stringify(sessions));
}

function loadSessionById(id: string): ChatSession | undefined {
  return loadSessions().find(s => s.id === id);
}

function upsertSession(session: ChatSession) {
  const sessions = loadSessions();
  const idx = sessions.findIndex(s => s.id === session.id);
  if (idx >= 0) sessions[idx] = session;
  else sessions.unshift(session);
  saveSessions(sessions);
}

function deleteSession(id: string) {
  const sessions = loadSessions().filter(s => s.id !== id);
  saveSessions(sessions);
}

function generateId(): string {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 8);
}

// ── 暴露给 AppShell 的 API ──

export { loadSessions, deleteSession };

export default function ChatPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const chatId = searchParams.get("id") || "";

  // 当前会话
  const [session, setSession] = useState<ChatSession>(() => {
    const s = chatId ? loadSessionById(chatId) : null;
    if (s) return s;
    return { id: generateId(), title: "新对话", messages: [], createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() };
  });

  // chatId 变化时重新加载会话（跳过初始渲染和相同 ID）
  const prevChatIdRef = useRef(chatId);
  useEffect(() => {
    if (chatId === prevChatIdRef.current) return;
    prevChatIdRef.current = chatId;
    const saved = chatId ? loadSessionById(chatId) : null;
    if (saved) {
      setSession(saved);
    } else if (!chatId) {
      setSession({ id: generateId(), title: "新对话", messages: [], createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() });
    }
  }, [chatId]);

  const messages = session.messages;

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
      const saved = localStorage.getItem("chat_model_id");
      if (saved) setSelectedId(Number(saved));
      else {
        const active = list.find(c => c.is_active);
        if (active) setSelectedId(active.id);
        else if (list.length > 0) setSelectedId(list[0]!.id);
      }
    }).catch(() => {});
  }, []);

  // 保存模型选择
  useEffect(() => {
    if (selectedId) localStorage.setItem("chat_model_id", String(selectedId));
  }, [selectedId]);

  // 点击外部关闭模型列表
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

  const sessionRef = useRef(session);
  sessionRef.current = session;

  const selectedConfig = configs.find(c => c.id === selectedId);

  const updateMessages = (newMessages: Message[], userMsg?: string) => {
    // 始终基于最新 session 而不是闭包中的旧值
    const s = { ...sessionRef.current };
    if (s.title === "新对话" && userMsg) {
      s.title = userMsg.length > 30 ? userMsg.slice(0, 30) + "…" : userMsg;
    }
    s.messages = newMessages;
    s.updatedAt = new Date().toISOString();
    if (!chatId) {
      setSearchParams({ id: s.id }, { replace: true });
    }
    upsertSession(s);
    setSession(s);
  };

  const handleSend = async (text?: string) => {
    const msg = (text || input).trim();
    if (!msg || loading) return;

    if (!selectedId) {
      const newMsgs = [...messages, { role: "user" as const, content: msg }, { role: "assistant" as const, content: "请先在系统设置中添加并激活一个 AI 模型。" }];
      updateMessages(newMsgs, msg);
      return;
    }

    setInput("");
    const newMsgs = [...messages, { role: "user" as const, content: msg }];
    updateMessages(newMsgs, msg);
    setLoading(true);

    try {
      const result = await invoke<string>("chat_with_ai", {
        configId: selectedId,
        systemPrompt: SYSTEM_PROMPT,
        messages: newMsgs,
      });
      const finalMsgs = [...newMsgs, { role: "assistant" as const, content: result }];
      updateMessages(finalMsgs);
    } catch (e) {
      const errMsgs = [...newMsgs, { role: "assistant" as const, content: `抱歉，出现了错误：${e}` }];
      updateMessages(errMsgs);
    } finally {
      setLoading(false);
    }
  };

  const fillInput = (text: string) => {
    setInput(text);
    inputRef.current?.focus();
  };

  const [copiedId, setCopiedId] = useState<string | null>(null);

  const copyContent = async (text: string, id: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedId(id);
      setTimeout(() => setCopiedId(null), 2000);
    } catch { /* ignore */ }
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
      {/* 开发中提示 */}
      <div className="shrink-0 flex items-center justify-center gap-1.5 px-4 py-1.5 text-[12px]"
        style={{ backgroundColor: "hsl(var(--warning) / 0.08)", color: "hsl(var(--warning))" }}>
        此功能还在完善中，可能存在不完善之处，欢迎反馈
      </div>
      {/* 消息区域 */}
      <div className="flex-1 overflow-y-auto">
        {isEmpty ? (
          /* 欢迎界面 */
          <div className="flex flex-col items-center justify-center h-full px-4">
            <div className="w-16 h-16 rounded-full flex items-center justify-center mb-5"
              style={{ backgroundColor: "hsl(var(--accent) / 0.1)" }}>
              <Bot size={32} style={{ color: "hsl(var(--accent))" }} />
            </div>
            <h2 className="text-[32px] font-medium mb-8 leading-tight" style={{ color: "hsl(var(--text-primary))", fontFamily: "'Times New Roman', serif" }}>
              有什么可以帮你的？
            </h2>
            <div className="flex flex-wrap justify-center gap-2 max-w-lg">
              {EXAMPLES.map((text, i) => (
                <button
                  key={i}
                  onClick={() => fillInput(text)}
                  className="px-4 py-2 rounded-full text-[13px] transition-all hover:shadow-sm"
                  style={{
                    border: "1px solid hsl(var(--border-light))",
                    color: "hsl(var(--text-primary))",
                    backgroundColor: "hsl(var(--bg-card))",
                  }}
                  onMouseEnter={e => (e.currentTarget.style.borderColor = "hsl(var(--border))")}
                  onMouseLeave={e => (e.currentTarget.style.borderColor = "hsl(var(--border-light))")}
                >
                  {text}
                </button>
              ))}
            </div>
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
                <div className="flex-1 min-w-0 pt-0.5 group">
                  <div className="flex items-center justify-between mb-1">
                    <div className="text-sm font-medium" style={{ color: "hsl(var(--text-secondary))" }}>
                      {msg.role === "assistant" ? "AI 巡检助手" : "你"}
                    </div>
                    <button
                      onClick={() => copyContent(msg.content, `msg-${i}`)}
                      className="opacity-0 group-hover:opacity-100 transition-opacity p-1 rounded hover:bg-[hsl(var(--bg-hover))]"
                      style={{ color: "hsl(var(--text-tertiary))" }}
                      title="复制"
                    >
                      {copiedId === `msg-${i}` ? <CheckCheck size={14} /> : <Copy size={14} />}
                    </button>
                  </div>
                  {msg.role === "assistant" ? (
                    <div className="prose prose-sm max-w-none leading-7" style={{ color: "hsl(var(--text-primary))" }}>
                      <ReactMarkdown remarkPlugins={[remarkGfm]}>{msg.content}</ReactMarkdown>
                    </div>
                  ) : (
                    <div className="text-[15px] leading-7 whitespace-pre-wrap select-all" style={{ color: "hsl(var(--text-primary))" }}>
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

      {/* 输入区域 */}
      <div className="shrink-0 px-4 pb-6">
        <div className="max-w-3xl mx-auto">
          <div className="rounded-[24px] border transition-shadow focus-within:shadow-md"
            style={{ backgroundColor: "hsl(var(--bg-card))", borderColor: "hsl(var(--border-light))" }}>
            <textarea
              ref={inputRef}
              value={input}
              onChange={e => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="输入问题，AI 帮你操作..."
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
            <div className="flex items-center justify-between px-3 pb-3 -mt-10">
              <button
                className="flex items-center justify-center w-8 h-8 rounded-lg transition-colors hover:bg-[hsl(var(--bg-hover))]"
                style={{ color: "hsl(var(--text-tertiary))" }}
                title="添加附件（暂未支持）"
              >
                <Plus size={18} />
              </button>
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

          <p className="text-center text-[11px] mt-3" style={{ color: "hsl(var(--text-tertiary) / 0.5)" }}>
            AI 巡检助手可能会犯错，请核实重要信息
          </p>
        </div>
      </div>
    </div>
  );
}
