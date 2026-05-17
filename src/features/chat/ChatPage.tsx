import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Send, Bot, User } from "lucide-react";

interface Message { role: string; content: string; }

export default function ChatPage() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => { endRef.current?.scrollIntoView({ behavior: "smooth" }); }, [messages]);

  const send = async () => {
    if (!input.trim() || loading) return;
    const userMsg = { role: "user", content: input };
    setMessages(m => [...m, userMsg]);
    setInput(""); setLoading(true);

    try {
      const reply = await invoke<string>("chat_stream", { messages: [...messages, userMsg] });
      setMessages(m => [...m, { role: "assistant", content: reply }]);
    } catch (e) {
      setMessages(m => [...m, { role: "assistant", content: `错误: ${e}` }]);
    }
    setLoading(false);
  };

  return (
    <div className="flex flex-col h-[calc(100vh-6rem)]">
      <h2 className="text-2xl font-bold mb-4">AI 对话助手</h2>

      <div className="flex-1 border rounded-lg bg-card overflow-auto p-4 space-y-3">
        {messages.length === 0 && (
          <div className="text-center text-muted-foreground py-16">
            <Bot className="h-12 w-12 mx-auto mb-3 opacity-30" />
            <p className="text-sm">询问关于设备、巡检、报告的问题</p>
            <p className="text-xs mt-1">支持的操作：查询设备状态、创建巡检批次、分析结果、生成报告等</p>
          </div>
        )}
        {messages.map((m, i) => (
          <div key={i} className={`flex gap-2 ${m.role === "user" ? "justify-end" : ""}`}>
            {m.role === "assistant" && <Bot className="h-5 w-5 mt-0.5 shrink-0 text-primary" />}
            <div className={`max-w-[80%] rounded-lg px-3 py-2 text-sm whitespace-pre-wrap ${m.role === "user" ? "bg-primary text-primary-foreground" : "bg-muted"}`}>{m.content}</div>
            {m.role === "user" && <User className="h-5 w-5 mt-0.5 shrink-0 text-muted-foreground" />}
          </div>
        ))}
        {loading && <div className="text-xs text-muted-foreground italic">AI 正在思考...</div>}
        <div ref={endRef} />
      </div>

      <div className="flex gap-2 mt-3">
        <input className="flex-1 border rounded-md px-3 py-2 text-sm" value={input} onChange={e=>setInput(e.target.value)}
          onKeyDown={e => e.key === "Enter" && send()} placeholder="输入问题，回车发送..." />
        <button onClick={send} disabled={loading} className="bg-primary text-primary-foreground px-4 py-2 rounded-md"><Send className="h-4 w-4"/></button>
      </div>
    </div>
  );
}
