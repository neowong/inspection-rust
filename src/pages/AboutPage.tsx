import { BrainCircuit, FileText, Heart, Network, Play, Server, TerminalSquare, Wrench } from "lucide-react";
import Card from "../components/ui/Card";

const FLOW_STEPS = [
  { title: "配置 AI", desc: "在系统设置中添加并激活模型配置", icon: BrainCircuit },
  { title: "维护命令库", desc: "按厂商维护巡检命令与中文说明", icon: TerminalSquare },
  { title: "设计报告模板", desc: "配置 DOCX 样式、列定义和实时预览", icon: FileText },
  { title: "创建巡检模板", desc: "选择巡检项与静态信息采集命令", icon: Network },
  { title: "添加设备", desc: "录入 SSH 信息并绑定巡检模板", icon: Server },
  { title: "执行巡检", desc: "批量 SSH 执行并保存本次静态快照", icon: Play },
  { title: "AI 分析", desc: "生成状态、发现和建议", icon: BrainCircuit },
  { title: "导出 DOCX", desc: "下载单设备、ZIP 或合并 Word 报告", icon: FileText },
];

function FlowStep({ step, index }: { step: typeof FLOW_STEPS[number]; index: number }) {
  const Icon = step.icon;
  return (
    <div className="relative rounded-xl border border-[hsl(var(--border))] bg-[hsl(var(--bg-card))] p-4">
      <div className="flex items-start gap-3">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-[hsl(var(--accent)_/_0.12)] text-[hsl(var(--accent))]">
          <Icon size={18} />
        </div>
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-[11px] font-semibold text-[hsl(var(--accent))]">{String(index + 1).padStart(2, "0")}</span>
            <h3 className="text-sm font-semibold text-[hsl(var(--text-primary))]">{step.title}</h3>
          </div>
          <p className="mt-1 text-xs leading-relaxed text-[hsl(var(--text-secondary))]">{step.desc}</p>
        </div>
      </div>
    </div>
  );
}

function DonatePlaceholder({ title, subtitle }: { title: string; subtitle: string }) {
  return (
    <div className="rounded-xl border border-dashed border-[hsl(var(--border))] bg-[hsl(var(--bg-card))] p-4 text-center">
      <div className="mx-auto flex h-40 w-40 items-center justify-center rounded-lg bg-[hsl(var(--bg-hover))] text-[hsl(var(--text-tertiary))]">
        <div>
          <div className="text-xs font-medium">二维码占位</div>
          <div className="mt-1 text-[10px]">后续替换图片</div>
        </div>
      </div>
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
            <h2 className="text-xl font-bold text-[hsl(var(--text-primary))]">OpenInspect</h2>
            <p className="mt-1 text-sm leading-relaxed text-[hsl(var(--text-secondary))]">
              OpenInspect 是面向网络工程师的桌面巡检工具，用于集中管理网络设备、维护巡检命令模板、批量执行 SSH 巡检、调用 AI 生成评判结论，并输出可编辑的 DOCX 巡检报告。
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
        <div className="mb-4 flex items-center gap-2 text-sm font-semibold text-[hsl(var(--text-primary))]">
          <Wrench size={18} className="text-[hsl(var(--accent))]" />
          推荐使用流程
        </div>
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
          {FLOW_STEPS.map((step, index) => (
            <FlowStep key={step.title} step={step} index={index} />
          ))}
        </div>
      </Card>

      <Card>
        <div className="flex items-center gap-2 text-sm font-semibold text-[hsl(var(--text-primary))]">
          <Heart size={18} className="text-[hsl(var(--danger))]" />
          支持作者
        </div>
        <p className="mt-2 text-sm text-[hsl(var(--text-secondary))]">
          如果这个项目对你的网络巡检工作有帮助，可以通过打赏支持后续维护。当前二维码为占位，后续可替换为正式收款码图片。
        </p>
        <div className="mt-4 grid gap-4 sm:grid-cols-2">
          <DonatePlaceholder title="微信打赏" subtitle="待替换为微信收款码" />
          <DonatePlaceholder title="支付宝打赏" subtitle="待替换为支付宝收款码" />
        </div>
      </Card>
    </div>
  );
}
