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

export default function DashboardPage() {
  const [stats, setStats] = useState<Stats | null>(null);

  useEffect(() => {
    invoke<Stats>("get_stats").then(setStats).catch(console.error);
  }, []);

  if (!stats) return <div className="p-4 text-gray-500">加载中...</div>;

  return (
    <div>
      <h1 className="text-lg font-bold mb-3">仪表盘</h1>
      <div className="grid grid-cols-4 gap-3 mb-4">
        <StatCard label="设备总数" value={stats.device_count} color="text-blue-600" />
        <StatCard label="在线设备" value={stats.online_device_count} color="text-green-600" />
        <StatCard label="离线设备" value={stats.offline_device_count} color="text-red-600" />
        <StatCard label="巡检模板" value={stats.template_count} color="text-purple-600" />
        <StatCard label="命令库" value={stats.command_count} color="text-orange-600" />
        <StatCard label="巡检批次" value={stats.batch_count} color="text-teal-600" />
        <StatCard label="进行中" value={stats.pending_batch_count} color="text-yellow-600" />
        <StatCard label="已完成" value={stats.completed_batch_count} color="text-green-700" />
      </div>
    </div>
  );
}

function StatCard({ label, value, color }: { label: string; value: number; color: string }) {
  return (
    <div className="bg-white rounded border border-gray-200 p-3 text-center">
      <div className={`text-2xl font-bold ${color}`}>{value}</div>
      <div className="text-xs text-gray-500 mt-1">{label}</div>
    </div>
  );
}
