import { useState, useEffect } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import {
  Server, Wifi, WifiOff, FileText, Zap, Package, Clock, CheckCircle2,
  Monitor, ShieldCheck, HardDrive, Database, FileCheck,
} from "lucide-react";
import type { Stats } from "../types";

interface StatCard {
  label: string;
  key: keyof Stats;
  color: string;
  Icon: typeof Server;
  path: string;
}

const SUMMARY: StatCard[] = [
  { label: "设备总数", key: "device_count", color: "accent", Icon: Server, path: "/devices" },
  { label: "在线设备", key: "online_device_count", color: "success", Icon: Wifi, path: "/devices" },
  { label: "离线设备", key: "offline_device_count", color: "danger", Icon: WifiOff, path: "/devices" },
  { label: "报告总数", key: "report_count", color: "accent", Icon: FileCheck, path: "/reports" },
];

const DEVICE_TYPES: StatCard[] = [
  { label: "网络设备", key: "network_device_count", color: "accent", Icon: Monitor, path: "/devices" },
  { label: "安全设备", key: "security_device_count", color: "warning", Icon: ShieldCheck, path: "/devices" },
  { label: "服务器", key: "server_count", color: "success", Icon: HardDrive, path: "/devices" },
  { label: "数据库", key: "database_count", color: "accent", Icon: Database, path: "/devices" },
];

const DETAILS: StatCard[] = [
  { label: "巡检模板", key: "template_count", color: "accent", Icon: FileText, path: "/templates" },
  { label: "命令库", key: "command_count", color: "accent", Icon: Zap, path: "/templates" },
  { label: "巡检任务", key: "batch_count", color: "accent", Icon: Package, path: "/inspection" },
  { label: "进行中", key: "pending_batch_count", color: "warning", Icon: Clock, path: "/inspection" },
  { label: "已完成", key: "completed_batch_count", color: "success", Icon: CheckCircle2, path: "/inspection" },
];

function colorVar(name: string) {
  return `hsl(var(--${name}))`;
}

function SummaryCard({ card, value, muted }: { card: StatCard; value: string; muted: boolean }) {
  const navigate = useNavigate();
  const c = colorVar(card.color);
  return (
    <div
      onClick={() => navigate(card.path)}
      className="relative overflow-hidden rounded-lg border bg-[hsl(var(--bg-card))] px-5 py-4 shadow-sm cursor-pointer hover:shadow-md transition-shadow"
    >
      <div className="absolute left-0 top-0 bottom-0 w-1"
        style={{ background: muted ? colorVar("text-tertiary") : c }} />
      <div className="flex items-start justify-between">
        <div>
          <p className="text-xs font-medium text-[hsl(var(--text-tertiary))] uppercase tracking-wide">{card.label}</p>
          <p className="mt-1.5 text-3xl font-bold tabular-nums"
            style={{ color: muted ? colorVar("text-tertiary") : c }}>{value}</p>
        </div>
        <card.Icon size={22} style={{ color: muted ? colorVar("text-tertiary") : c }} className="opacity-50 mt-0.5" />
      </div>
    </div>
  );
}

function SmallCard({ card, value }: { card: StatCard; value: string }) {
  const navigate = useNavigate();
  const c = colorVar(card.color);
  return (
    <div
      onClick={() => navigate(card.path)}
      className="rounded-lg border bg-[hsl(var(--bg-card))] px-4 py-3.5 shadow-sm hover:shadow-md transition-shadow cursor-pointer"
    >
      <div className="flex items-center gap-2.5">
        <span className="flex items-center justify-center w-8 h-8 rounded-md"
          style={{ background: `hsl(var(--${card.color}) / 0.1)`, color: c }}>
          <card.Icon size={16} />
        </span>
        <div>
          <p className="text-lg font-semibold tabular-nums text-[hsl(var(--text-primary))]">{value}</p>
          <p className="text-[11px] text-[hsl(var(--text-tertiary))]">{card.label}</p>
        </div>
      </div>
    </div>
  );
}

export default function DashboardPage() {
  const location = useLocation();
  const [stats, setStats] = useState<Stats | null>(null);

  // 每次进入仪表盘都刷新数据（从其它页面切换回来时也能拿到最新统计）
  useEffect(() => {
    invoke<Stats>("get_stats").then(setStats).catch(console.error);
  }, [location.pathname]);

  const val = (key: keyof Stats) => stats ? String(stats[key] ?? 0) : "...";

  return (
    <div className="space-y-6 animate-in">
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm relative">
        <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">仪表盘</h1>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1">AI巡检助手概览</p>
      </div>

      {/* 摘要栏 — 核心指标 */}
      <div className="grid grid-cols-4 gap-4">
        {SUMMARY.map((card) => {
          const value = val(card.key);
          const muted = card.key === "offline_device_count" && value === "0";
          return <SummaryCard key={card.key} card={card} value={value} muted={muted} />;
        })}
      </div>

      {/* 设备分类 */}
      <div>
        <h2 className="text-xs font-semibold uppercase tracking-wider text-[hsl(var(--text-tertiary))] mb-3">
          设备分类
        </h2>
        <div className="grid grid-cols-4 gap-3">
          {DEVICE_TYPES.map((card) => <SmallCard key={card.key} card={card} value={val(card.key)} />)}
        </div>
      </div>

      {/* 其他统计 */}
      <div>
        <h2 className="text-xs font-semibold uppercase tracking-wider text-[hsl(var(--text-tertiary))] mb-3">
          其他统计
        </h2>
        <div className="grid grid-cols-5 gap-3">
          {DETAILS.map((card) => <SmallCard key={card.key} card={card} value={val(card.key)} />)}
        </div>
      </div>

      {/* 使用流程 */}
      <div>
        <h2 className="text-xs font-semibold uppercase tracking-wider text-[hsl(var(--text-tertiary))] mb-3">
          使用流程
        </h2>
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

            <rect x="20" y="20" width="940" height="860" rx="22" fill="var(--bg-card, #F8FAFC)" />
            <text x="490" y="60" textAnchor="middle" fontFamily="Microsoft YaHei, PingFang SC, sans-serif" fontSize="26" fontWeight="700" fill="var(--text-primary, #0F172A)">
              AI巡检助手 使用流程
            </text>
            <text x="490" y="88" textAnchor="middle" fontFamily="Microsoft YaHei, PingFang SC, sans-serif" fontSize="14" fill="var(--text-tertiary, #64748B)">
              从模板准备到批量巡检，再到 AI 分析和 DOCX 报告交付
            </text>

            {/* 主流程连线 */}
            <path d="M190 145 L190 780" stroke="var(--text-tertiary, #94A3B8)" strokeWidth="3" strokeDasharray="8 8" markerEnd="url(#arrow)" fill="none" />

            {[
              ["01", "配置 AI 模型", "在系统设置中添加并激活 AI 供应商（OpenAI/DeepSeek 等）", "可选步骤；未启用时可人工评判，不影响巡检执行。"],
              ["02", "维护命令库", "按厂商分类录入巡检命令，支持 H3C/华为/思科/锐捷/飞塔/服务器", "命令说明会作为报告里的巡检项目名称，支持拖拽排序。"],
              ["03", "设计报告模板", "配置封面、设备信息、巡检明细列、页眉页脚，右侧 A4 实时预览", "DOCX 模板决定最终报告的版式，可按厂商创建多套模板。"],
              ["04", "创建巡检模板", "从命令库选择巡检项，标记静态信息命令（提取 sysname/SN/型号）", "静态信息命令执行后不进报告明细，仅提取字段写入设备表。"],
              ["05", "添加设备", "录入 IP、SSH 凭据，绑定巡检模板；支持自动检测型号、SN、sysname", "设备保存后自动触发连通性检测和静态信息采集。"],
              ["06", "执行巡检", "创建批次并运行，多设备并发 SSH 执行命令，实时进度追踪", "支持暂停/停止/重试，Linux 服务器使用 exec 通道并行执行。"],
              ["07", "AI 分析评判", "调用 AI 对每条命令输出逐条评判：正常/注意/警告/严重", "评判结论自动合并到报告的评判结论列，生成巡检总结。"],
              ["08", "导出 DOCX 报告", "生成可编辑的 Word 巡检报告，支持单设备/批量 ZIP/合并 DOCX", "合并报告每台设备从新页开始，内置目录和页码。"],
            ].map(([no, title, desc, note], i) => {
              const y = 130 + i * 90;
              return (
                <g key={no}>
                  <rect x="80" y={y} width="220" height="62" rx="16" fill="var(--bg-card, white)" stroke="var(--border, #CBD5E1)" strokeWidth="1.5" filter="url(#shadow)" />
                  <circle cx="112" cy={y + 31} r="21" fill="url(#flowNode)" />
                  <text x="112" y={y + 36} textAnchor="middle" fontFamily="Inter, Arial, sans-serif" fontSize="13" fontWeight="700" fill="white">{no}</text>
                  <text x="145" y={y + 26} fontFamily="Microsoft YaHei, PingFang SC, sans-serif" fontSize="17" fontWeight="700" fill="var(--text-primary, #0F172A)">{title}</text>
                  <text x="145" y={y + 48} fontFamily="Microsoft YaHei, PingFang SC, sans-serif" fontSize="12" fill="var(--text-tertiary, #64748B)">{desc}</text>

                  <path d={`M300 ${y + 31} L365 ${y + 31}`} stroke="var(--text-tertiary, #94A3B8)" strokeWidth="1.5" markerEnd="url(#arrow)" fill="none" />
                  <rect x="375" y={y + 5} width="520" height="52" rx="12" fill="var(--bg-card, #FFFFFF)" stroke="var(--border, #E2E8F0)" strokeWidth="1.2" />
                  <text x="397" y={y + 28} fontFamily="Microsoft YaHei, PingFang SC, sans-serif" fontSize="13" fontWeight="700" fill="var(--text-primary, #334155)">注释</text>
                  <text x="397" y={y + 47} fontFamily="Microsoft YaHei, PingFang SC, sans-serif" fontSize="12" fill="var(--text-tertiary, #64748B)">{note}</text>
                </g>
              );
            })}
          </svg>
        </div>
      </div>
    </div>
  );
}
