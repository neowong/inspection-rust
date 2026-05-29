type Status = "online" | "offline" | "unknown" | "ok" | "warning" | "critical" | "info" | "pending" | "running" | "completed" | "failed" | "stopped";

const STYLES: Record<Status, string> = {
  online: "bg-emerald-500/10 text-emerald-700 border-emerald-500/25",
  offline: "bg-red-500/10 text-red-700 border-red-500/25",
  unknown: "bg-gray-500/10 text-gray-600 border-gray-500/25",
  ok: "bg-emerald-500/10 text-emerald-700 border-emerald-500/25",
  warning: "bg-amber-500/10 text-amber-700 border-amber-500/25",
  critical: "bg-red-500/10 text-red-700 border-red-500/25",
  info: "bg-blue-500/10 text-blue-700 border-blue-500/25",
  pending: "bg-gray-500/10 text-gray-600 border-gray-500/25",
  running: "bg-blue-500/10 text-blue-700 border-blue-500/25 animate-pulse",
  completed: "bg-emerald-500/10 text-emerald-700 border-emerald-500/25",
  failed: "bg-red-500/10 text-red-700 border-red-500/25",
  stopped: "bg-amber-500/10 text-amber-700 border-amber-500/25",
};

const LABELS: Record<Status, string> = {
  online: "在线", offline: "离线", unknown: "未知",
  ok: "正常", warning: "警告", critical: "严重", info: "信息",
  pending: "等待中", running: "执行中", completed: "已完成", failed: "失败", stopped: "已停止",
};

const DOT_COLORS: Record<Status, string> = {
  online: "bg-emerald-500", offline: "bg-red-500", unknown: "bg-gray-400",
  ok: "bg-emerald-500", warning: "bg-amber-500", critical: "bg-red-500", info: "bg-blue-500",
  pending: "bg-gray-400", running: "bg-blue-500", completed: "bg-emerald-500", failed: "bg-red-500", stopped: "bg-amber-500",
};

export default function StatusBadge({ status }: { status: Status }) {
  const style = STYLES[status] || STYLES.unknown;
  const dot = DOT_COLORS[status] || DOT_COLORS.unknown;
  return (
    <span className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded border text-[11px] font-medium ${style}`}>
      <span className={`w-1.5 h-1.5 rounded-full ${dot}`} />
      {LABELS[status] || status}
    </span>
  );
}
