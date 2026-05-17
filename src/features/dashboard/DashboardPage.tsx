import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Link } from "react-router-dom";
import { Server, FileText, Terminal, ClipboardList, TrendingUp } from "lucide-react";
import type { Stats } from "@/types";

const cards = [
  { key: "device_count", label: "设备总数", icon: Server, color: "text-blue-600", to: "/devices" },
  { key: "template_count", label: "巡检模板", icon: FileText, color: "text-green-600", to: "/templates" },
  { key: "command_count", label: "命令库", icon: Terminal, color: "text-purple-600", to: "/commands" },
  { key: "batch_count", label: "巡检批次", icon: ClipboardList, color: "text-orange-600", to: "/batches" },
] as const;

export default function DashboardPage() {
  const [stats, setStats] = useState<Stats | null>(null);

  useEffect(() => {
    invoke<Stats>("get_stats").then(setStats).catch(console.error);
  }, []);

  if (!stats) return <div className="text-muted-foreground">加载中...</div>;

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-2xl font-bold tracking-tight">仪表盘</h2>
        <p className="text-muted-foreground">系统运行概览</p>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {cards.map(({ key, label, icon: Icon, color, to }) => (
          <Link key={key} to={to} className="bg-card border rounded-lg p-4 hover:shadow-md transition-shadow">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-muted-foreground">{label}</p>
                <p className="text-2xl font-bold mt-1">{stats[key as keyof Stats] as number}</p>
              </div>
              <Icon className={`h-8 w-8 ${color}`} />
            </div>
          </Link>
        ))}
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div className="bg-card border rounded-lg p-4">
          <h3 className="font-semibold mb-4 flex items-center gap-2"><Server className="h-4 w-4" />设备状态</h3>
          <div className="space-y-2">
            <div className="flex justify-between"><span>在线</span><span className="text-green-600 font-bold">{stats.online_device_count}</span></div>
            <div className="flex justify-between"><span>离线</span><span className="text-red-600 font-bold">{stats.offline_device_count}</span></div>
            <div className="flex justify-between"><span>未知</span><span className="text-muted-foreground font-bold">{stats.device_count - stats.online_device_count - stats.offline_device_count}</span></div>
          </div>
        </div>
        <div className="bg-card border rounded-lg p-4">
          <h3 className="font-semibold mb-4 flex items-center gap-2"><TrendingUp className="h-4 w-4" />巡检概况</h3>
          <div className="space-y-2">
            <div className="flex justify-between"><span>已完成批次</span><span className="text-green-600 font-bold">{stats.completed_batch_count}</span></div>
            <div className="flex justify-between"><span>待执行批次</span><span className="text-orange-600 font-bold">{stats.pending_batch_count}</span></div>
            <div className="flex justify-between"><span>报告模板</span><span>{stats.report_template_count}</span></div>
          </div>
        </div>
      </div>
    </div>
  );
}
