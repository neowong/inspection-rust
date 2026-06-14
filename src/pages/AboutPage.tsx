import { BrainCircuit, FileText, Heart, Network, Play, Server, TerminalSquare, Wrench } from "lucide-react";
import Card from "../components/ui/Card";

const FLOW_STEPS = [
  { title: "配置 AI", desc: "设置 OpenAI / Anthropic / DeepSeek 等模型连接", note: "没有 AI 也能巡检；启用后可自动生成评判结论。", icon: BrainCircuit },
  { title: "维护命令库", desc: "按厂商维护命令文本、分类和中文说明", note: "命令说明会成为报告中的“项目”名称。", icon: TerminalSquare },
  { title: "设计报告模板", desc: "在线配置 DOCX 封面、表格列和页眉页脚", note: "右侧 A4 预览可实时查看报告排版效果。", icon: FileText },
  { title: "创建巡检模板", desc: "选择巡检项和静态信息采集命令", note: "静态信息命令可提取 sysname、SN、型号等，但不进入报告明细。", icon: Network },
  { title: "添加设备", desc: "录入设备 IP、厂商、SSH 凭据并绑定模板", note: "H3C 设备可自动检测型号、SN、出厂日期和 sysname。", icon: Server },
  { title: "执行巡检", desc: "批量 SSH 执行命令并保存本次巡检快照", note: "巡检结果按设备保存，静态信息会同步为本次报告快照。", icon: Play },
  { title: "AI 分析", desc: "对巡检输出生成状态、发现和建议", note: "AI 评判会整合到报告“评判结论”列。", icon: BrainCircuit },
  { title: "导出 DOCX", desc: "生成单设备、ZIP 或合并 Word 报告", note: "DOCX 可继续编辑，适合交付、归档和二次整理。", icon: FileText },
];

function FlowDiagram() {
  return (
    <div className="overflow-x-auto pb-2">
      <div className="min-w-[900px]">
        <div className="grid grid-cols-[1fr_34px_1fr_34px_1fr_34px_1fr] items-stretch gap-0">
          {FLOW_STEPS.slice(0, 4).map((step, index) => (
            <FlowDiagramItem key={step.title} step={step} index={index} showArrow={index < 3} />
          ))}
        </div>
        <div className="my-4 flex justify-end pr-[12.5%]">
          <div className="flex items-center gap-2 text-[11px] text-[hsl(var(--text-tertiary))]">
            <span className="h-px w-20 bg-[hsl(var(--border))]" />
            <span>继续执行</span>
            <span className="text-[hsl(var(--accent))]">↓</span>
          </div>
        </div>
        <div className="grid grid-cols-[1fr_34px_1fr_34px_1fr_34px_1fr] items-stretch gap-0">
          {FLOW_STEPS.slice(4).map((step, idx) => {
            const index = idx + 4;
            return <FlowDiagramItem key={step.title} step={step} index={index} showArrow={idx < 3} />;
          })}
        </div>
      </div>
    </div>
  );
}

function FlowDiagramItem({
  step,
  index,
  showArrow,
}: {
  step: typeof FLOW_STEPS[number];
  index: number;
  showArrow: boolean;
}) {
  const Icon = step.icon;
  return (
    <>
      <div className="relative rounded-2xl border border-[hsl(var(--border))] bg-[hsl(var(--bg-card))] p-4 shadow-sm">
        <div className="absolute -top-2 left-4 rounded-full bg-[hsl(var(--accent))] px-2 py-0.5 text-[10px] font-bold text-white">
          {String(index + 1).padStart(2, "0")}
        </div>
        <div className="flex items-start gap-3">
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-[hsl(var(--accent)_/_0.12)] text-[hsl(var(--accent))]">
            <Icon size={19} />
          </div>
          <div className="min-w-0">
            <h3 className="text-sm font-semibold text-[hsl(var(--text-primary))]">{step.title}</h3>
            <p className="mt-1 text-xs leading-relaxed text-[hsl(var(--text-secondary))]">{step.desc}</p>
          </div>
        </div>
        <div className="mt-3 rounded-lg bg-[hsl(var(--bg-hover))] px-3 py-2 text-[11px] leading-relaxed text-[hsl(var(--text-tertiary))]">
          注：{step.note}
        </div>
      </div>
      {showArrow && (
        <div className="flex items-center justify-center">
          <div className="relative h-px w-full bg-[hsl(var(--border))]">
            <span className="absolute -right-1.5 -top-[5px] text-[hsl(var(--border))]">▶</span>
          </div>
        </div>
      )}
    </>
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
        <div className="mb-5 flex items-center gap-2 text-sm font-semibold text-[hsl(var(--text-primary))]">
          <Wrench size={18} className="text-[hsl(var(--accent))]" />
          推荐使用流程
        </div>
        <FlowDiagram />
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
