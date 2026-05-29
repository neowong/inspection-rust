type Status =
  | "online"
  | "offline"
  | "unknown"
  | "ok"
  | "warning"
  | "critical"
  | "info"
  | "pending"
  | "running"
  | "completed"
  | "failed"
  | "stopped";

const STYLES: Record<string, string> = {
  online: "bg-[hsl(var(--success)/0.1)] text-[hsl(var(--success))] border border-[hsl(var(--success)/0.25)]",
  offline: "bg-[hsl(var(--danger)/0.1)] text-[hsl(var(--danger))] border border-[hsl(var(--danger)/0.25)]",
  unknown: "bg-[hsl(var(--text-tertiary)/0.1)] text-[hsl(var(--text-secondary))] border border-[hsl(var(--text-tertiary)/0.25)]",
  ok: "bg-[hsl(var(--success)/0.1)] text-[hsl(var(--success))] border border-[hsl(var(--success)/0.25)]",
  warning: "bg-[hsl(var(--warning)/0.1)] text-[hsl(var(--warning))] border border-[hsl(var(--warning)/0.25)]",
  critical: "bg-[hsl(var(--danger)/0.1)] text-[hsl(var(--danger))] border border-[hsl(var(--danger)/0.25)]",
  info: "bg-[hsl(var(--info)/0.1)] text-[hsl(var(--info))] border border-[hsl(var(--info)/0.25)]",
  pending: "bg-[hsl(var(--text-tertiary)/0.1)] text-[hsl(var(--text-secondary))] border border-[hsl(var(--text-tertiary)/0.25)]",
  running: "bg-[hsl(var(--info)/0.1)] text-[hsl(var(--info))] border border-[hsl(var(--info)/0.25)]",
  completed: "bg-[hsl(var(--success)/0.1)] text-[hsl(var(--success))] border border-[hsl(var(--success)/0.25)]",
  failed: "bg-[hsl(var(--danger)/0.1)] text-[hsl(var(--danger))] border border-[hsl(var(--danger)/0.25)]",
  stopped: "bg-[hsl(var(--warning)/0.1)] text-[hsl(var(--warning))] border border-[hsl(var(--warning)/0.25)]",
};

const LABELS: Record<string, string> = {
  online: "在线",
  offline: "离线",
  unknown: "未知",
  ok: "正常",
  warning: "警告",
  critical: "严重",
  info: "信息",
  pending: "等待中",
  running: "运行中",
  completed: "已完成",
  failed: "失败",
  stopped: "已停止",
};

const DOT_COLORS: Record<string, string> = {
  online: "bg-[hsl(var(--success))]",
  offline: "bg-[hsl(var(--danger))]",
  unknown: "bg-[hsl(var(--text-tertiary))]",
  ok: "bg-[hsl(var(--success))]",
  warning: "bg-[hsl(var(--warning))]",
  critical: "bg-[hsl(var(--danger))]",
  info: "bg-[hsl(var(--info))]",
  pending: "bg-[hsl(var(--text-tertiary))]",
  running: "bg-[hsl(var(--info))]",
  completed: "bg-[hsl(var(--success))]",
  failed: "bg-[hsl(var(--danger))]",
  stopped: "bg-[hsl(var(--warning))]",
};

export default function StatusBadge({ status }: { status: Status }) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[11px] font-medium ${STYLES[status]}`}
    >
      <span
        className={`w-1.5 h-1.5 rounded-full ${DOT_COLORS[status]}`}
      />
      {LABELS[status]}
    </span>
  );
}
