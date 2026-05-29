type Status = "online" | "offline" | "unknown" | "ok" | "warning" | "critical" | "info" | "pending" | "running" | "completed" | "failed" | "stopped";

const STYLES: Record<Status, string> = {
  online: "bg-emerald-500/15 text-emerald-400 border-emerald-500/30",
  offline: "bg-red-500/15 text-red-400 border-red-500/30",
  unknown: "bg-gray-500/15 text-gray-400 border-gray-500/30",
  ok: "bg-emerald-500/15 text-emerald-400 border-emerald-500/30",
  warning: "bg-amber-500/15 text-amber-400 border-amber-500/30",
  critical: "bg-red-500/15 text-red-400 border-red-500/30",
  info: "bg-blue-500/15 text-blue-400 border-blue-500/30",
  pending: "bg-gray-500/15 text-gray-400 border-gray-500/30",
  running: "bg-blue-500/15 text-blue-400 border-blue-500/30 animate-pulse",
  completed: "bg-emerald-500/15 text-emerald-400 border-emerald-500/30",
  failed: "bg-red-500/15 text-red-400 border-red-500/30",
  stopped: "bg-amber-500/15 text-amber-400 border-amber-500/30",
};

const LABELS: Record<Status, string> = {
  online: "在线", offline: "离线", unknown: "未知",
  ok: "正常", warning: "警告", critical: "严重", info: "信息",
  pending: "等待中", running: "执行中", completed: "已完成", failed: "失败", stopped: "已停止",
};

const DOT_COLORS: Record<Status, string> = {
  online: "bg-emerald-400", offline: "bg-red-400", unknown: "bg-gray-400",
  ok: "bg-emerald-400", warning: "bg-amber-400", critical: "bg-red-400", info: "bg-blue-400",
  pending: "bg-gray-400", running: "bg-blue-400", completed: "bg-emerald-400", failed: "bg-red-400", stopped: "bg-amber-400",
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
