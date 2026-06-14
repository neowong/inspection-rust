interface StatBadgeProps {
  label: string;
  value: string | number;
  color: string;
}

export default function StatBadge({ label, value, color }: StatBadgeProps) {
  const c = `hsl(var(--${color}))`;
  const bg = color.startsWith("text") ? "bg-[hsl(var(--bg-hover))]" : `bg-[hsl(var(--${color})_/_0.1)]`;
  return (
    <div className={`rounded-lg ${bg} px-3 py-2 text-center`}>
      <div className="text-lg font-bold" style={{ color: c }}>{value}</div>
      <div className="text-[10px] uppercase tracking-wide" style={{ color: c, opacity: 0.7 }}>{label}</div>
    </div>
  );
}
