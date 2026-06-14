import { Github, Heart, ShieldCheck, Network } from "lucide-react";
import Card from "../components/ui/Card";

function DonatePlaceholder({ title, subtitle }: { title: string; subtitle: string }) {
  return (
    <div className="rounded-xl border border-dashed border-[hsl(var(--border))] bg-[hsl(var(--bg-card))] p-4 text-center">
      <div className="mx-auto flex h-36 w-36 items-center justify-center rounded-lg bg-[hsl(var(--bg-hover))] text-[hsl(var(--text-tertiary))]">
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
        <p className="text-xs text-[hsl(var(--text-tertiary))] mt-0.5">开源版本信息、项目说明与支持作者</p>
      </div>

      <Card>
        <div className="flex items-start gap-4">
          <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl bg-[hsl(var(--accent)_/_0.12)] text-[hsl(var(--accent))]">
            <Network size={30} />
          </div>
          <div className="min-w-0 flex-1">
            <h2 className="text-xl font-bold text-[hsl(var(--text-primary))]">OpenInspect</h2>
            <p className="mt-1 text-sm text-[hsl(var(--text-secondary))]">
              面向网络工程师的开源网络设备巡检工具，支持设备管理、巡检模板、批量巡检、AI 分析和 DOCX 报告生成。
            </p>
            <div className="mt-3 flex flex-wrap gap-2 text-xs text-[hsl(var(--text-secondary))]">
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">Rust + Tauri</span>
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">React + TypeScript</span>
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">SQLite</span>
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">DOCX 报告</span>
            </div>
          </div>
        </div>
      </Card>

      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <div className="flex items-center gap-2 text-sm font-semibold text-[hsl(var(--text-primary))]">
            <ShieldCheck size={18} className="text-[hsl(var(--success))]" />
            开源说明
          </div>
          <div className="mt-3 space-y-2 text-sm text-[hsl(var(--text-secondary))]">
            <p>本版本用于开源发布，移除了内部品牌 Logo，使用通用 OpenInspect 标识。</p>
            <p>项目适合网络设备巡检、巡检报告自动化、网络工具集成等场景。</p>
            <p className="text-xs text-[hsl(var(--text-tertiary))]">许可证信息可在项目仓库中补充或调整。</p>
          </div>
        </Card>

        <Card>
          <div className="flex items-center gap-2 text-sm font-semibold text-[hsl(var(--text-primary))]">
            <Github size={18} className="text-[hsl(var(--text-secondary))]" />
            项目地址
          </div>
          <div className="mt-3 space-y-2 text-sm text-[hsl(var(--text-secondary))]">
            <p>GitHub 仓库：</p>
            <code className="block rounded-lg bg-[hsl(var(--bg-hover))] px-3 py-2 text-xs text-[hsl(var(--accent))]">
              https://github.com/neowong/inspection-rust
            </code>
            <p className="text-xs text-[hsl(var(--text-tertiary))]">欢迎提交 Issue、建议和改进。</p>
          </div>
        </Card>
      </div>

      <Card>
        <div className="flex items-center gap-2 text-sm font-semibold text-[hsl(var(--text-primary))]">
          <Heart size={18} className="text-[hsl(var(--danger))]" />
          支持作者
        </div>
        <p className="mt-2 text-sm text-[hsl(var(--text-secondary))]">
          如果这个项目对你有帮助，可以通过打赏支持后续维护。当前二维码为占位，后续将替换为正式收款码。
        </p>
        <div className="mt-4 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          <DonatePlaceholder title="微信打赏" subtitle="待替换为微信收款码" />
          <DonatePlaceholder title="支付宝打赏" subtitle="待替换为支付宝收款码" />
          <DonatePlaceholder title="其它方式" subtitle="可替换为赞助链接或二维码" />
        </div>
      </Card>
    </div>
  );
}
