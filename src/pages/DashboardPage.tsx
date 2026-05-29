import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Stats {
  device_count: number;
  online_device_count: number;
  offline_device_count: number;
  template_count: number;
  command_count: number;
  batch_count: number;
  pending_batch_count: number;
  completed_batch_count: number;
}

const CARDS: { label: string; key: keyof Stats; gradient: string; icon: string }[] = [
  { label: "设备总数", key: "device_count",    gradient: "from-blue-500 to-blue-600",      icon: "⊞" },
  { label: "在线设备", key: "online_device_count", gradient: "from-emerald-500 to-emerald-600", icon: "●" },
  { label: "离线设备", key: "offline_device_count", gradient: "from-red-400 to-rose-500",       icon: "○" },
  { label: "巡检模板", key: "template_count",  gradient: "from-violet-500 to-purple-600",  icon: "⊟" },
  { label: "命令库",   key: "command_count",   gradient: "from-amber-500 to-orange-500",   icon: "▤" },
  { label: "巡检批次", key: "batch_count",     gradient: "from-cyan-500 to-teal-600",      icon: "▶" },
  { label: "进行中",   key: "pending_batch_count", gradient: "from-yellow-400 to-amber-500", icon: "◷" },
  { label: "已完成",   key: "completed_batch_count", gradient: "from-green-500 to-emerald-600", icon: "✓" },
];

export default function DashboardPage() {
  const [stats, setStats] = useState<Stats | null>(null);

  useEffect(() => {
    invoke<Stats>("get_stats").then(setStats).catch(console.error);
  }, []);

  if (!stats) {
    return (
      <div className="flex items-center justify-center h-64 text-slate-400">
        <div className="text-center">
          <div className="text-3xl mb-2 animate-pulse">◷</div>
          <div className="text-sm">加载仪表盘数据...</div>
        </div>
      </div>
    );
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <div>
          <h1 className="text-base font-semibold text-slate-800">仪表盘</h1>
          <p className="text-[11px] text-slate-400 mt-0.5">网络设备巡检概览</p>
        </div>
        <button
          onClick={() => invoke<Stats>("get_stats").then(setStats)}
          className="btn btn-secondary btn-sm"
        >
          ↻ 刷新
        </button>
      </div>

      <div className="grid grid-cols-4 gap-3">
        {CARDS.map(card => (
          <StatCard
            key={card.key}
            label={card.label}
            value={stats[card.key]}
            gradient={card.gradient}
            icon={card.icon}
          />
        ))}
      </div>
    </div>
  );
}

function StatCard({ label, value, gradient, icon }: {
  label: string;
  value: number;
  gradient: string;
  icon: string;
}) {
  return (
    <div
      className="relative overflow-hidden rounded-xl p-4 cursor-default group transition-all duration-200 hover:-translate-y-0.5 hover:shadow-lg"
      style={{ background: "#fff", boxShadow: "0 1px 3px rgba(0,0,0,.06), 0 1px 2px rgba(0,0,0,.04)" }}
    >
      {/* Gradient strip */}
      <div className={`absolute top-0 left-0 right-0 h-0.5 bg-gradient-to-r ${gradient}`} />

      <div className="flex items-start justify-between">
        <div>
          <div className="text-[11px] text-slate-400 font-medium mb-1">{label}</div>
          <div className={`text-3xl font-bold bg-gradient-to-r ${gradient} bg-clip-text text-transparent`}>
            {value}
          </div>
        </div>
        <div className={`w-8 h-8 rounded-lg flex items-center justify-center text-sm bg-gradient-to-br ${gradient} text-white/90 shadow-sm`}>
          {icon}
        </div>
      </div>
    </div>
  );
}
