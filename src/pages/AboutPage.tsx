import { useState } from "react";
import { Network, Mail, Copy, Check, Send } from "lucide-react";
import Card from "../components/ui/Card";
import Button from "../components/ui/Button";
import Input, { Select } from "../components/ui/Input";

function FeedbackSection() {
  const [category, setCategory] = useState("bug");
  const [title, setTitle] = useState("");
  const [desc, setDesc] = useState("");
  const [copied, setCopied] = useState(false);

  const EMAIL = "neowong2005@gmail.com";

  const buildEmailBody = () => {
    const lines = [
      `反馈类型: ${category === "bug" ? "Bug 报告" : category === "feature" ? "功能建议" : "其它反馈"}`,
      `标题: ${title || "(未填写)"}`,
      "",
      "描述:",
      desc || "(未填写)",
      "",
      "--- 环境信息 ---",
      `App 版本: v3.2.0`,
      `平台: ${navigator.platform}`,
      `User-Agent: ${navigator.userAgent}`,
    ];
    return lines.join("\n");
  };

  const handleCopy = async () => {
    await navigator.clipboard.writeText(buildEmailBody());
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleSendEmail = () => {
    const subject = `[${category === "bug" ? "Bug" : "建议"}] ${title || "问题反馈"}`;
    const mailtoUrl = `mailto:${EMAIL}?subject=${encodeURIComponent(subject)}&body=${encodeURIComponent(buildEmailBody())}`;
    window.open(mailtoUrl, "_blank");
  };

  return (
    <Card>
      <div className="flex items-center gap-2 text-sm font-semibold text-[hsl(var(--text-primary))]">
        <Mail size={18} className="text-[hsl(var(--accent))]" />
        问题反馈
      </div>
      <p className="mt-1.5 text-xs text-[hsl(var(--text-tertiary))]">
        遇到 Bug 或有功能建议？欢迎通过邮件反馈，帮助我们改进产品。
      </p>

      <div className="mt-4 space-y-3">
        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">反馈类型</label>
            <Select value={category} onChange={(e) => setCategory(e.target.value)}>
              <option value="bug">Bug 报告</option>
              <option value="feature">功能建议</option>
              <option value="other">其它反馈</option>
            </Select>
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">标题</label>
            <Input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="简短描述问题或建议" />
          </div>
        </div>
        <div>
          <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">详细描述</label>
          <textarea
            value={desc}
            onChange={(e) => setDesc(e.target.value)}
            placeholder="请描述问题的复现步骤、期望行为、实际行为等..."
            className="w-full h-24 rounded-md border border-[hsl(var(--border))] bg-[hsl(var(--bg-card))] px-3 py-2 text-sm text-[hsl(var(--text-primary))] placeholder:text-[hsl(var(--text-tertiary))] outline-none focus:border-[hsl(var(--accent))] focus:ring-2 focus:ring-[hsl(var(--accent)/0.2)] resize-none"
          />
        </div>

        <div className="flex gap-2">
          <Button size="sm" variant="ghost" onClick={handleCopy}>
            {copied ? <Check size={14} /> : <Copy size={14} />}
            {copied ? "已复制" : "复制内容"}
          </Button>
          <Button size="sm" variant="primary" onClick={handleSendEmail}>
            <Send size={14} />
            发送邮件
          </Button>
        </div>

        <div className="rounded-lg bg-[hsl(var(--bg-hover))] px-3 py-2 text-xs text-[hsl(var(--text-tertiary))] space-y-1">
          <p>反馈内容会自动附带系统环境信息（版本、平台、浏览器），无需手动填写。</p>
          <p>收件邮箱：<span className="text-[hsl(var(--text-primary))] font-medium">{EMAIL}</span></p>
        </div>
      </div>
    </Card>
  );
}

export default function AboutPage() {
  return (
    <div className="space-y-5">
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm relative">
        <h1 className="text-lg font-bold">关于</h1>
        <p className="text-xs text-[hsl(var(--text-tertiary))] mt-0.5">项目介绍与问题反馈</p>
      </div>

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

      <FeedbackSection />
    </div>
  );
}
