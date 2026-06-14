import { BrainCircuit, FileText, Heart, Network, Server, Settings2, TerminalSquare, Wrench } from "lucide-react";
import Card from "../components/ui/Card";

function FeatureItem({ icon: Icon, title, desc }: { icon: typeof Network; title: string; desc: string }) {
  return (
    <div className="rounded-xl border border-[hsl(var(--border))] bg-[hsl(var(--bg-card))] p-4">
      <div className="flex items-center gap-2 text-sm font-semibold text-[hsl(var(--text-primary))]">
        <span className="flex h-8 w-8 items-center justify-center rounded-lg bg-[hsl(var(--accent)_/_0.1)] text-[hsl(var(--accent))]">
          <Icon size={17} />
        </span>
        {title}
      </div>
      <p className="mt-2 text-xs leading-relaxed text-[hsl(var(--text-secondary))]">{desc}</p>
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
        <p className="text-xs text-[hsl(var(--text-tertiary))] mt-0.5">项目介绍、功能说明与支持作者</p>
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
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">AI 分析</span>
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">DOCX 报告</span>
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">网络工具箱</span>
            </div>
          </div>
        </div>
      </Card>

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        <FeatureItem icon={Server} title="设备资产管理" desc="维护设备名称、IP、厂商、型号、SN、出厂日期、Sysname 和 SSH 登录信息，支持状态检测与批量管理。" />
        <FeatureItem icon={TerminalSquare} title="巡检模板与静态信息" desc="按厂商维护巡检命令，支持拖拽排序；静态信息命令可提取 sysname、型号、SN、出厂日期，且不显示在报告明细中。" />
        <FeatureItem icon={BrainCircuit} title="AI 巡检评判" desc="对巡检命令输出进行 AI 分析，生成状态、发现和建议，并整合到报告的评判结论列中。" />
        <FeatureItem icon={FileText} title="DOCX 报告生成" desc="在线配置封面、设备信息、巡检明细列、页眉页脚和总结区，生成可编辑 Word 报告，支持单设备、ZIP 和合并报告。" />
        <FeatureItem icon={Wrench} title="网工工具箱" desc="内置存活扫描、TCP/UDP 端口扫描、WEB 检测、SNMP v2c/v3 和 Zabbix Agent 探测等常用工具。" />
        <FeatureItem icon={Settings2} title="本地桌面运行" desc="基于 Rust + Tauri 构建，数据保存在本地 SQLite，适合离线环境、内网环境和现场运维场景。" />
      </div>

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
