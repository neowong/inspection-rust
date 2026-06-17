import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { Server, Wifi, WifiOff, FileText, Zap, Package, Clock, CheckCircle2 } from "lucide-react";
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
];

const DETAILS: StatCard[] = [
  { label: "巡检模板", key: "template_count", color: "accent", Icon: FileText, path: "/templates" },
  { label: "命令库", key: "command_count", color: "accent", Icon: Zap, path: "/templates" },
  { label: "巡检批次", key: "batch_count", color: "accent", Icon: Package, path: "/inspection" },
  { label: "进行中", key: "pending_batch_count", color: "warning", Icon: Clock, path: "/inspection" },
  { label: "已完成", key: "completed_batch_count", color: "success", Icon: CheckCircle2, path: "/inspection" },
];

function colorVar(name: string) {
  return `hsl(var(--${name}))`;
}

export default function DashboardPage() {
  const navigate = useNavigate();
  const [stats, setStats] = useState<Stats | null>(null);

  useEffect(() => {
    invoke<Stats>("get_stats").then(setStats).catch(console.error);
  }, []);

  const val = (key: keyof Stats) =>
    stats ? String(stats[key] ?? 0) : "...";

  return (
    <div className="space-y-6 animate-in">
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm relative">
        <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">仪表盘</h1>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1">
          AI巡检助手概览
        </p>
      </div>

      {/* 摘要栏 — 设备核心指标 */}
      <div className="grid grid-cols-3 gap-4">
        {SUMMARY.map((card) => {
          const value = val(card.key);
          const c = colorVar(card.color);
          const muted = card.key === "offline_device_count" && value === "0";
          return (
            <div
              key={card.key}
              onClick={() => navigate(card.path)}
              className="relative overflow-hidden rounded-lg border bg-[hsl(var(--bg-card))] px-5 py-4 shadow-sm cursor-pointer hover:shadow-md transition-shadow"
            >
              {/* left color bar */}
              <div
                className="absolute left-0 top-0 bottom-0 w-1"
                style={{ background: muted ? colorVar("text-tertiary") : c }}
              />
              <div className="flex items-start justify-between">
                <div>
                  <p className="text-xs font-medium text-[hsl(var(--text-tertiary))] uppercase tracking-wide">
                    {card.label}
                  </p>
                  <p
                    className="mt-1.5 text-3xl font-bold tabular-nums"
                    style={{ color: muted ? colorVar("text-tertiary") : c }}
                  >
                    {value}
                  </p>
                </div>
                <card.Icon
                  size={22}
                  style={{ color: muted ? colorVar("text-tertiary") : c }}
                  className="opacity-50 mt-0.5"
                />
              </div>
            </div>
          );
        })}
      </div>

      {/* 二级指标 */}
      <div>
        <h2 className="text-xs font-semibold uppercase tracking-wider text-[hsl(var(--text-tertiary))] mb-3">
          其他统计
        </h2>
        <div className="grid grid-cols-5 gap-3">
          {DETAILS.map((card) => {
            const value = val(card.key);
            const c = colorVar(card.color);
            return (
              <div
                key={card.key}
                onClick={() => navigate(card.path)}
                className="rounded-lg border bg-[hsl(var(--bg-card))] px-4 py-3.5 shadow-sm hover:shadow-md transition-shadow cursor-pointer"
              >
                <div className="flex items-center gap-2.5">
                  <span
                    className="flex items-center justify-center w-8 h-8 rounded-md"
                    style={{
                      background: `hsl(var(--${card.color}) / 0.1)`,
                      color: c,
                    }}
                  >
                    <card.Icon size={16} />
                  </span>
                  <div>
                    <p className="text-lg font-semibold tabular-nums text-[hsl(var(--text-primary))]">
                      {value}
                    </p>
                    <p className="text-[11px] text-[hsl(var(--text-tertiary))]">
                      {card.label}
                    </p>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
