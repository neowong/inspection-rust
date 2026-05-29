import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Stats } from "../types";

interface CardDef {
  label: string;
  key: keyof Stats;
  gradient: string;
  icon: string;
}

const CARDS: CardDef[] = [
  { label: "设备总数", key: "device_count", gradient: "linear-gradient(135deg, #667eea 0%, #764ba2 100%)", icon: "🖥" },
  { label: "在线设备", key: "online_device_count", gradient: "linear-gradient(135deg, #11998e 0%, #38ef7d 100%)", icon: "✅" },
  { label: "离线设备", key: "offline_device_count", gradient: "linear-gradient(135deg, #eb3349 0%, #f45c43 100%)", icon: "❌" },
  { label: "巡检模板", key: "template_count", gradient: "linear-gradient(135deg, #f093fb 0%, #f5576c 100%)", icon: "📋" },
  { label: "命令库", key: "command_count", gradient: "linear-gradient(135deg, #4facfe 0%, #00f2fe 100%)", icon: "⚡" },
  { label: "巡检批次", key: "batch_count", gradient: "linear-gradient(135deg, #43e97b 0%, #38f9d7 100%)", icon: "📦" },
  { label: "进行中", key: "pending_batch_count", gradient: "linear-gradient(135deg, #fa709a 0%, #fee140 100%)", icon: "⏳" },
  { label: "已完成", key: "completed_batch_count", gradient: "linear-gradient(135deg, #a18cd1 0%, #fbc2eb 100%)", icon: "✅" },
];

export default function DashboardPage() {
  const [stats, setStats] = useState<Stats | null>(null);

  useEffect(() => {
    invoke<Stats>("get_stats").then(setStats).catch(console.error);
  }, []);

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">仪表盘</h1>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1">网络设备巡检系统概览</p>
      </div>
      <div className="grid grid-cols-4 gap-4">
        {CARDS.map((card) => {
          const value = stats ? String(stats[card.key] ?? 0) : "...";
          return (
            <div
              key={card.key}
              className="rounded-xl p-5 text-white shadow-lg"
              style={{ background: card.gradient }}
            >
              <div className="flex items-center justify-between mb-3">
                <span className="text-2xl opacity-90">{card.icon}</span>
              </div>
              <div className="text-3xl font-bold mb-1">{value}</div>
              <div className="text-sm opacity-80">{card.label}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
