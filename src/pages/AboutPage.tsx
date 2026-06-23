import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Network, Mail, Send, CheckCircle2 } from "lucide-react";
import Card from "../components/ui/Card";

const FEEDBACK_TYPES = [
  { value: "bug", label: "问题反馈" },
  { value: "feature", label: "功能需求" },
  { value: "other", label: "其他" },
];

export default function AboutPage() {
  const [feedbackType, setFeedbackType] = useState("bug");
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [contact, setContact] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  const [error, setError] = useState("");

  const handleSubmit = async () => {
    if (!title.trim()) { setError("请填写标题"); return; }
    if (!content.trim()) { setError("请填写详细描述"); return; }

    setSubmitting(true);
    setError("");
    try {
      await invoke("submit_feedback", {
        feedbackType,
        title: title.trim(),
        content: content.trim(),
        contact: contact.trim() || null,
        version: "3.40.20",
      });
      setSubmitted(true);
      setTitle("");
      setContent("");
      setContact("");
    } catch (e: any) {
      setError(typeof e === "string" ? e : e?.message || "提交失败");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="space-y-5">
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm relative">
        <h1 className="text-lg font-bold">关于</h1>
        <p className="text-xs text-[hsl(var(--text-tertiary))] mt-0.5">项目介绍与问题反馈</p>
      </div>

      {/* 项目信息 */}
      <Card>
        <div className="flex items-start gap-4">
          <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl bg-[hsl(var(--accent)_/_0.12)] text-[hsl(var(--accent))]">
            <Network size={30} />
          </div>
          <div className="min-w-0 flex-1">
            <h2 className="text-xl font-bold text-[hsl(var(--text-primary))]">AI巡检助手</h2>
            <p className="mt-1 text-sm leading-relaxed text-[hsl(var(--text-secondary))]">
              AI巡检助手 是面向运维工程师的桌面巡检工具，用于集中管理网络设备与服务器、维护巡检命令模板、批量执行 SSH 巡检、调用 AI 生成评判结论，并输出可编辑的 DOCX 巡检报告。
            </p>
            <div className="mt-3 flex flex-wrap gap-2 text-xs text-[hsl(var(--text-secondary))]">
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">设备巡检</span>
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">静态信息采集</span>
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">AI 分析</span>
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">DOCX 报告</span>
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">网络工具箱</span>
            </div>
          </div>
        </div>
      </Card>

      {/* 问题反馈表单 */}
      <Card>
        <div className="flex items-center gap-2 text-sm font-semibold text-[hsl(var(--text-primary))]">
          <Mail size={18} className="text-[hsl(var(--accent))]" />
          问题反馈
        </div>

        {submitted ? (
          <div className="mt-4 flex flex-col items-center gap-3 py-6">
            <CheckCircle2 size={48} className="text-[hsl(var(--success))]" />
            <p className="text-sm font-medium text-[hsl(var(--text-primary))]">感谢您的反馈！</p>
            <p className="text-xs text-[hsl(var(--text-tertiary))]">我们会认真处理每一条反馈</p>
            <button
              onClick={() => setSubmitted(false)}
              className="mt-2 text-sm text-[hsl(var(--accent))] hover:underline"
            >
              继续反馈
            </button>
          </div>
        ) : (
          <div className="mt-3 space-y-3">
            {/* 反馈类型 */}
            <div className="flex gap-2">
              {FEEDBACK_TYPES.map(t => (
                <button
                  key={t.value}
                  onClick={() => setFeedbackType(t.value)}
                  className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-colors ${
                    feedbackType === t.value
                      ? "bg-[hsl(var(--accent))] text-white"
                      : "bg-[hsl(var(--bg-hover))] text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))]"
                  }`}
                >
                  {t.label}
                </button>
              ))}
            </div>

            {/* 标题 */}
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">标题</label>
              <input
                type="text"
                value={title}
                onChange={(e) => { setTitle(e.target.value); setError(""); }}
                placeholder="简要描述问题或需求"
                className="w-full px-3 py-2 text-sm rounded-lg border border-[hsl(var(--border))] bg-[hsl(var(--bg-input))] focus:outline-none focus:ring-2 focus:ring-[hsl(var(--accent)_/_0.4)]"
              />
            </div>

            {/* 详细描述 */}
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">详细描述</label>
              <textarea
                value={content}
                onChange={(e) => { setContent(e.target.value); setError(""); }}
                placeholder={"请详细描述问题的复现步骤、期望行为、实际行为等\n或描述您希望新增的功能"}
                rows={5}
                className="w-full px-3 py-2 text-sm rounded-lg border border-[hsl(var(--border))] bg-[hsl(var(--bg-input))] focus:outline-none focus:ring-2 focus:ring-[hsl(var(--accent)_/_0.4)] resize-none"
              />
            </div>

            {/* 联系方式 */}
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">联系方式（可选）</label>
              <input
                type="text"
                value={contact}
                onChange={(e) => setContact(e.target.value)}
                placeholder="邮箱或微信号，方便我们联系您"
                className="w-full px-3 py-2 text-sm rounded-lg border border-[hsl(var(--border))] bg-[hsl(var(--bg-input))] focus:outline-none focus:ring-2 focus:ring-[hsl(var(--accent)_/_0.4)]"
              />
            </div>

            {error && (
              <div className="rounded-lg bg-[hsl(var(--danger)_/_0.1)] border border-[hsl(var(--danger)_/_0.3)] px-3 py-2 text-xs text-[hsl(var(--danger))]">
                {error}
              </div>
            )}

            <button
              onClick={handleSubmit}
              disabled={submitting}
              className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-[hsl(var(--accent))] rounded-lg hover:opacity-90 transition-opacity disabled:opacity-50"
            >
              <Send size={14} />
              {submitting ? "提交中..." : "提交反馈"}
            </button>
          </div>
        )}
      </Card>

      {/* 联系方式 */}
      <Card>
        <p className="text-sm text-[hsl(var(--text-secondary))]">
          也可以通过以下方式联系我们：
        </p>
        <p className="mt-2 flex items-center gap-1.5 text-sm text-[hsl(var(--accent))]">
          <Mail size={14} />
          neowong2005@gmail.com
        </p>
        <div className="mt-4 flex items-start gap-6">
          <div className="text-center">
            <img src="/weixin.png" alt="微信二维码" className="h-36 w-36 rounded-lg border border-[hsl(var(--border))] object-contain" />
            <p className="mt-2 text-xs text-[hsl(var(--text-tertiary))]">扫码添加微信</p>
          </div>
        </div>
      </Card>
    </div>
  );
}
