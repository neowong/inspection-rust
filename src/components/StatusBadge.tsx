type Status = "online" | "offline" | "unknown" | "ok" | "warning" | "critical" | "info" | "pending" | "running" | "completed" | "failed" | "stopped";

const STYLES: Record<Status, string> = {
  online: "bg-green-100 text-green-700 border-green-300",
  offline: "bg-red-100 text-red-700 border-red-300",
  unknown: "bg-gray-100 text-gray-500 border-gray-300",
  ok: "bg-green-100 text-green-700 border-green-300",
  warning: "bg-yellow-100 text-yellow-700 border-yellow-300",
  critical: "bg-red-100 text-red-700 border-red-300",
  info: "bg-blue-100 text-blue-700 border-blue-300",
  pending: "bg-gray-100 text-gray-500 border-gray-300",
  running: "bg-blue-100 text-blue-700 border-blue-300 animate-pulse",
  completed: "bg-green-100 text-green-700 border-green-300",
  failed: "bg-red-100 text-red-700 border-red-300",
  stopped: "bg-yellow-100 text-yellow-700 border-yellow-300",
};

const LABELS: Record<Status, string> = {
  online: "在线", offline: "离线", unknown: "未知",
  ok: "正常", warning: "警告", critical: "严重", info: "信息",
  pending: "等待中", running: "执行中", completed: "已完成", failed: "失败", stopped: "已停止",
};

export default function StatusBadge({ status }: { status: Status }) {
  return (
    <span className={`inline-block px-1.5 py-0.5 rounded border text-[11px] font-medium ${STYLES[status] || STYLES.unknown}`}>
      {LABELS[status] || status}
    </span>
  );
}
