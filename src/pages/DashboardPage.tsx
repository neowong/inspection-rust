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
        <div className="overflow-x-auto">
          <svg viewBox="0 0 960 480" className="min-w-[860px] w-full" role="img" aria-label="AI巡检助手 使用流程图">
            <defs>
              <linearGradient id="g0" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stopColor="#818CF8" /><stop offset="1" stopColor="#6366F1" /></linearGradient>
              <linearGradient id="g1" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stopColor="#34D399" /><stop offset="1" stopColor="#10B981" /></linearGradient>
              <linearGradient id="g2" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stopColor="#60A5FA" /><stop offset="1" stopColor="#3B82F6" /></linearGradient>
              <linearGradient id="g3" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stopColor="#A78BFA" /><stop offset="1" stopColor="#8B5CF6" /></linearGradient>
              <linearGradient id="g4" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stopColor="#F472B6" /><stop offset="1" stopColor="#EC4899" /></linearGradient>
              <linearGradient id="g5" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stopColor="#FB923C" /><stop offset="1" stopColor="#F97316" /></linearGradient>
              <linearGradient id="g6" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stopColor="#38BDF8" /><stop offset="1" stopColor="#0EA5E9" /></linearGradient>
              <linearGradient id="g7" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stopColor="#4ADE80" /><stop offset="1" stopColor="#22C55E" /></linearGradient>
              <marker id="fa" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto"><path d="M1,1 L7,4 L1,7Z" fill="#94A3B8" /></marker>
            </defs>

            {[
              ["01", "配置 AI 模型", "添加 OpenAI / DeepSeek 等 AI 供应商", "可选，未启用时可人工评判"],
              ["02", "维护命令库", "按厂商录入巡检命令和中文说明", "支持 H3C / 华为 / 思科 / 服务器等"],
              ["03", "设计报告模板", "配置封面、列定义、页眉页脚", "右侧 A4 实时预览报告效果"],
              ["04", "创建巡检模板", "选择巡检项与静态信息命令", "静态信息提取 sysname / SN / 型号"],
              ["05", "添加设备", "录入 IP、SSH 凭据并绑定模板", "保存后自动检测连通性和型号"],
              ["06", "执行巡检", "多设备并发 SSH 执行命令", "支持暂停 / 停止 / 重试"],
              ["07", "AI 分析评判", "逐条命令评判：正常 / 注意 / 警告 / 严重", "结论自动合并到报告评判列"],
              ["08", "导出 DOCX", "生成可编辑 Word 巡检报告", "支持单设备 / 批量 ZIP / 合并"],
            ].map(([no, title, desc, note], i) => {
              const col = i % 4;
              const row = Math.floor(i / 4);
              const x = 20 + col * 235;
              const y = 20 + row * 230;
              return (
                <g key={no}>
                  {/* 卡片 */}
                  <rect x={x} y={y} width="215" height="200" rx="16" fill="var(--bg-card, #FFFFFF)" stroke="var(--border, #E2E8F0)" strokeWidth="1.5" />
                  {/* 顶部色条 */}
                  <rect x={x} y={y} width="215" height="6" rx="16" fill={`url(#g${i})`} />
                  <rect x={x} y={y + 3} width="215" height="3" fill={`url(#g${i})`} />
                  {/* 编号圆 */}
                  <circle cx={x + 30} cy={y + 40} r="18" fill={`url(#g${i})`} />
                  <text x={x + 30} y={y + 45} textAnchor="middle" fontFamily="Inter, monospace" fontSize="13" fontWeight="700" fill="white">{no}</text>
                  {/* 标题 */}
                  <text x={x + 56} y={y + 46} fontFamily="Microsoft YaHei, PingFang SC, sans-serif" fontSize="18" fontWeight="700" fill="var(--text-primary, #0F172A)">{title}</text>
                  {/* 分割线 */}
                  <line x1={x + 16} y1={y + 65} x2={x + 199} y2={y + 65} stroke="var(--border, #E2E8F0)" strokeWidth="1" />
                  {/* 描述 */}
                  <text x={x + 16} y={y + 92} fontFamily="Microsoft YaHei, PingFang SC, sans-serif" fontSize="13" fill="var(--text-secondary, #475569)">{desc}</text>
                  {/* 注释 */}
                  <rect x={x + 12} y={y + 115} width={191} height="52" rx="8" fill="var(--bg-hover, #F1F5F9)" />
                  <text x={x + 22} y={y + 138} fontFamily="Microsoft YaHei, PingFang SC, sans-serif" fontSize="12" fill="var(--text-tertiary, #64748B)">{note}</text>
                </g>
              );
            })}

            {/* 连线 */}
            <path d="M235 120 L255 120" stroke="#94A3B8" strokeWidth="2" markerEnd="url(#fa)" fill="none" />
            <path d="M470 120 L490 120" stroke="#94A3B8" strokeWidth="2" markerEnd="url(#fa)" fill="none" />
            <path d="M705 120 L725 120" stroke="#94A3B8" strokeWidth="2" markerEnd="url(#fa)" fill="none" />
            <path d="M825 220 L825 250" stroke="#94A3B8" strokeWidth="2" markerEnd="url(#fa)" fill="none" />
            <path d="M805 350 L785 350" stroke="#94A3B8" strokeWidth="2" markerEnd="url(#fa)" fill="none" />
            <path d="M570 350 L550 350" stroke="#94A3B8" strokeWidth="2" markerEnd="url(#fa)" fill="none" />
            <path d="M335 350 L315 350" stroke="#94A3B8" strokeWidth="2" markerEnd="url(#fa)" fill="none" />
          </svg>
        </div>
      </div>
    </div>
  );
}
