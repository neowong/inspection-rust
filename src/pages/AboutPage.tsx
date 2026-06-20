import { Heart, Network, Wrench } from "lucide-react";
import Card from "../components/ui/Card";

function WorkflowSvg() {
  const steps = [
    ["01", "配置 AI", "设置 AI 模型连接", "可选步骤；启用后自动生成巡检评判。"],
    ["02", "维护命令库", "录入厂商命令与中文说明", "命令说明会作为报告里的巡检项目名称。"],
    ["03", "设计报告模板", "配置 DOCX 样式和列定义", "右侧 A4 预览用于确认最终报告版式。"],
    ["04", "创建巡检模板", "选择巡检项与静态信息命令", "静态信息命令提取 sysname/SN/型号，不进报告明细。"],
    ["05", "添加设备", "录入 IP、厂商、SSH 和模板", "设备可自动检测型号、SN、出厂日期和 sysname。"],
    ["06", "执行巡检", "批量 SSH 执行命令", "保存命令输出和本次巡检静态信息快照。"],
    ["07", "AI 分析", "生成状态、发现和建议", "评判内容会合并到报告的“评判结论”列。"],
    ["08", "导出 DOCX", "生成 Word 巡检报告", "支持单设备、批量 ZIP 和合并 DOCX。"],
  ];

  return (
    <div className="overflow-x-auto rounded-xl border border-[hsl(var(--border))] bg-[hsl(var(--bg-card))] p-3">
      <svg viewBox="0 0 980 900" className="min-w-[880px] w-full" role="img" aria-label="AI巡检助手 使用流程图">
        <defs>
          <linearGradient id="flowNode" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor="#38BDF8" />
            <stop offset="1" stopColor="#22C55E" />
          </linearGradient>
          <marker id="arrow" markerWidth="12" markerHeight="12" refX="10" refY="6" orient="auto">
            <path d="M2,2 L10,6 L2,10 Z" fill="#64748B" />
          </marker>
          <filter id="shadow" x="-20%" y="-20%" width="140%" height="140%">
            <feDropShadow dx="0" dy="4" stdDeviation="4" floodColor="#020617" floodOpacity="0.16" />
          </filter>
        </defs>

        <rect x="20" y="20" width="940" height="860" rx="22" fill="#F8FAFC" />
        <text x="490" y="60" textAnchor="middle" fontFamily="Microsoft YaHei, PingFang SC, sans-serif" fontSize="26" fontWeight="700" fill="#0F172A">
          AI巡检助手 使用流程
        </text>
        <text x="490" y="88" textAnchor="middle" fontFamily="Microsoft YaHei, PingFang SC, sans-serif" fontSize="14" fill="#64748B">
          从模板准备到批量巡检，再到 AI 分析和 DOCX 报告交付
        </text>

        {/* 主流程连线 */}
        <path
          d="M190 145 L190 780"
          stroke="#94A3B8"
          strokeWidth="3"
          strokeDasharray="8 8"
          markerEnd="url(#arrow)"
          fill="none"
        />

        {steps.map(([no, title, desc, note], i) => {
          const y = 130 + i * 90;
          return (
            <g key={no}>
              {/* 节点 */}
              <rect x="80" y={y} width="220" height="62" rx="16" fill="white" stroke="#CBD5E1" strokeWidth="1.5" filter="url(#shadow)" />
              <circle cx="112" cy={y + 31} r="21" fill="url(#flowNode)" />
              <text x="112" y={y + 36} textAnchor="middle" fontFamily="Inter, Arial, sans-serif" fontSize="13" fontWeight="700" fill="white">
                {no}
              </text>
              <text x="145" y={y + 26} fontFamily="Microsoft YaHei, PingFang SC, sans-serif" fontSize="17" fontWeight="700" fill="#0F172A">
                {title}
              </text>
              <text x="145" y={y + 48} fontFamily="Microsoft YaHei, PingFang SC, sans-serif" fontSize="12" fill="#64748B">
                {desc}
              </text>

              {/* 注释连接线 */}
              <path d={`M300 ${y + 31} L365 ${y + 31}`} stroke="#94A3B8" strokeWidth="1.5" markerEnd="url(#arrow)" fill="none" />

              {/* 注释框 */}
              <rect x="375" y={y + 5} width="520" height="52" rx="12" fill="#FFFFFF" stroke="#E2E8F0" strokeWidth="1.2" />
              <text x="397" y={y + 28} fontFamily="Microsoft YaHei, PingFang SC, sans-serif" fontSize="13" fontWeight="700" fill="#334155">
                注释
              </text>
              <text x="397" y={y + 47} fontFamily="Microsoft YaHei, PingFang SC, sans-serif" fontSize="12" fill="#64748B">
                {note}
              </text>
            </g>
          );
        })}
      </svg>
    </div>
  );
}

function DonateQrCode({ title, subtitle, src }: { title: string; subtitle: string; src: string }) {
  return (
    <div className="rounded-xl border border-[hsl(var(--border))] bg-[hsl(var(--bg-card))] p-4 text-center">
      <img src={src} alt={title} className="mx-auto h-48 w-48 rounded-lg object-contain" />
      <div className="mt-3 text-sm font-medium text-[hsl(var(--text-primary))]">{title}</div>
      <div className="mt-1 text-xs text-[hsl(var(--text-tertiary))]">{subtitle}</div>
    </div>
  );
}

export default function AboutPage() {
  return (
    <div className="space-y-5">
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm relative">
        <h1 className="text-lg font-bold">关于</h1>
        <p className="text-xs text-[hsl(var(--text-tertiary))] mt-0.5">项目介绍、使用流程与支持作者</p>
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

      <Card>
        <div className="flex items-center gap-2 text-sm font-semibold text-[hsl(var(--text-primary))]">
          <Heart size={18} className="text-[hsl(var(--danger))]" />
          支持作者
        </div>
        <p className="mt-2 text-sm text-[hsl(var(--text-secondary))]">
          如果这个项目对你的网络巡检工作有帮助，可以通过扫码打赏支持后续维护。
        </p>
        <div className="mt-4 grid gap-4 sm:grid-cols-2">
          <DonateQrCode title="微信打赏" subtitle="扫码支持作者" src="/wx.png" />
          <DonateQrCode title="支付宝打赏" subtitle="扫码支持作者" src="/zfb.png" />
        </div>
      </Card>

      <Card>
        <div className="mb-5 flex items-center gap-2 text-sm font-semibold text-[hsl(var(--text-primary))]">
          <Wrench size={18} className="text-[hsl(var(--accent))]" />
          推荐使用流程
        </div>
        <WorkflowSvg />
      </Card>
    </div>
  );
}
