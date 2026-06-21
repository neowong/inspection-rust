import { useState, useEffect } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import {
  Monitor, Wifi, WifiOff, FileText, Zap, Package, Clock, CheckCircle2,
  Server, Router, ShieldCheck, Database, FileCheck, ArrowRight,
} from "lucide-react";
import type { Stats } from "../types";

function colorVar(name: string) {
  return `hsl(var(--${name}))`;
}

function SummaryCard({ label, value, icon: Icon, color, muted, onClick }: {
  label: string; value: string; icon: typeof Server; color: string; muted?: boolean; onClick: () => void;
}) {
  const c = colorVar(color);
  return (
    <div onClick={onClick}
      className="relative overflow-hidden rounded-xl border bg-[hsl(var(--bg-card))] px-5 py-5 cursor-pointer hover:shadow-lg transition-all group">
      <div className="absolute right-3 top-3 opacity-[0.07] group-hover:opacity-[0.12] transition-opacity">
        <Icon size={64} />
      </div>
      <p className="text-xs font-medium text-[hsl(var(--text-tertiary))] uppercase tracking-wider">{label}</p>
      <p className="mt-2 text-4xl font-bold tabular-nums" style={{ color: muted ? colorVar("text-tertiary") : c }}>{value}</p>
    </div>
  );
}

function StatRow({ label, value, icon: Icon, color, onClick }: {
  label: string; value: string; icon: typeof Server; color: string; onClick: () => void;
}) {
  const c = colorVar(color);
  return (
    <div onClick={onClick}
      className="flex items-center gap-3 rounded-lg px-3 py-2.5 cursor-pointer hover:bg-[hsl(var(--bg-hover))] transition-colors group">
      <span className="flex items-center justify-center w-9 h-9 rounded-lg shrink-0"
        style={{ background: `hsl(var(--${color}) / 0.1)`, color: c }}>
        <Icon size={18} />
      </span>
      <span className="flex-1 text-sm text-[hsl(var(--text-primary))]">{label}</span>
      <span className="text-lg font-semibold tabular-nums text-[hsl(var(--text-primary))]">{value}</span>
      <ArrowRight size={14} className="text-[hsl(var(--text-tertiary))] opacity-0 group-hover:opacity-100 transition-opacity" />
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="rounded-xl border bg-[hsl(var(--bg-card))] overflow-hidden">
      <div className="px-4 py-3 border-b border-[hsl(var(--border))]">
        <h2 className="text-xs font-semibold uppercase tracking-wider text-[hsl(var(--text-tertiary))]">{title}</h2>
      </div>
      <div className="p-2">{children}</div>
    </div>
  );
}

export default function DashboardPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const [stats, setStats] = useState<Stats | null>(null);

  const loadStats = () => invoke<Stats>("get_stats").then(setStats).catch(console.error);
  useEffect(() => { loadStats(); }, [location.key]);
  useEffect(() => {
    window.addEventListener("focus", loadStats);
    return () => window.removeEventListener("focus", loadStats);
  }, []);

  const v = (key: keyof Stats) => stats ? String(stats[key] ?? 0) : "...";

  return (
    <div className="space-y-5 animate-in">
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm">
        <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">仪表盘</h1>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1">AI巡检助手概览</p>
      </div>

      {/* 核心指标 */}
      <div className="grid grid-cols-4 gap-4">
        <SummaryCard label="设备总数" value={v("device_count")} icon={Monitor} color="accent" onClick={() => navigate("/devices")} />
        <SummaryCard label="在线设备" value={v("online_device_count")} icon={Wifi} color="success" onClick={() => navigate("/devices")} />
        <SummaryCard label="离线设备" value={v("offline_device_count")} icon={WifiOff} color="danger" muted={v("offline_device_count") === "0"} onClick={() => navigate("/devices")} />
        <SummaryCard label="报告总数" value={v("report_count")} icon={FileCheck} color="accent" onClick={() => navigate("/reports")} />
      </div>

      {/* 下方两列 */}
      <div className="grid grid-cols-2 gap-4">
        <Section title="设备分类">
          <StatRow label="网络设备" value={v("network_device_count")} icon={Router} color="accent" onClick={() => navigate("/devices")} />
          <StatRow label="安全设备" value={v("security_device_count")} icon={ShieldCheck} color="warning" onClick={() => navigate("/devices")} />
          <StatRow label="服务器" value={v("server_count")} icon={Server} color="success" onClick={() => navigate("/devices")} />
          <StatRow label="数据库" value={v("database_count")} icon={Database} color="accent" onClick={() => navigate("/devices")} />
        </Section>

        <Section title="巡检任务">
          <StatRow label="巡检模板" value={v("template_count")} icon={FileText} color="accent" onClick={() => navigate("/templates")} />
          <StatRow label="命令库" value={v("command_count")} icon={Zap} color="accent" onClick={() => navigate("/templates")} />
          <StatRow label="巡检任务" value={v("batch_count")} icon={Package} color="accent" onClick={() => navigate("/inspection")} />
          <StatRow label="进行中" value={v("pending_batch_count")} icon={Clock} color="warning" onClick={() => navigate("/inspection")} />
          <StatRow label="已完成" value={v("completed_batch_count")} icon={CheckCircle2} color="success" onClick={() => navigate("/inspection")} />
        </Section>
      </div>
    </div>
  );
}
