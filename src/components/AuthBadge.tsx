/** 显示 SSH 账号验证状态的徽章。配合 StatusBadge 一起在设备列表的状态列展示。 */

const STYLES: Record<string, string> = {
  ok:            "bg-[hsl(var(--success)/0.1)] text-[hsl(var(--success))] border border-[hsl(var(--success)/0.25)]",
  auth_failed:   "bg-[hsl(var(--danger)/0.1)] text-[hsl(var(--danger))] border border-[hsl(var(--danger)/0.25)]",
  unreachable:   "bg-[hsl(var(--danger)/0.1)] text-[hsl(var(--danger))] border border-[hsl(var(--danger)/0.25)]",
  timeout:       "bg-[hsl(var(--warning)/0.1)] text-[hsl(var(--warning))] border border-[hsl(var(--warning)/0.25)]",
  dns_fail:      "bg-[hsl(var(--danger)/0.1)] text-[hsl(var(--danger))] border border-[hsl(var(--danger)/0.25)]",
  no_credential: "bg-[hsl(var(--warning)/0.1)] text-[hsl(var(--warning))] border border-[hsl(var(--warning)/0.25)]",
  error:         "bg-[hsl(var(--danger)/0.1)] text-[hsl(var(--danger))] border border-[hsl(var(--danger)/0.25)]",
  unknown:       "bg-[hsl(var(--text-tertiary)/0.1)] text-[hsl(var(--text-secondary))] border border-[hsl(var(--text-tertiary)/0.25)]",
};

const LABELS: Record<string, string> = {
  ok:            "账号正常",
  auth_failed:   "账号错误",
  unreachable:   "无法连接",
  timeout:       "连接超时",
  dns_fail:      "解析失败",
  no_credential: "缺少凭据",
  error:         "检测失败",
  unknown:       "账号未验证",
};

interface Props {
  status: string | null | undefined;
  message?: string | null;
}

export default function AuthBadge({ status, message }: Props) {
  const key = status && STYLES[status] ? status : "unknown";
  const label = LABELS[key];
  // 详细错误信息作为 tooltip
  const title = message ? `${label}: ${message}` : label;
  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium whitespace-nowrap ${STYLES[key]}`}
      title={title}
    >
      {label}
    </span>
  );
}
