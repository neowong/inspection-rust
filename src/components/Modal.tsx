import { useEffect } from "react";
import { X } from "lucide-react";

interface Props {
  open: boolean;
  title: string;
  width?: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  onClose: () => void;
}

const WIDTH_MAP: Record<string, number> = {
  "max-w-sm": 384,
  "max-w-md": 448,
  "max-w-lg": 512,
  "max-w-xl": 576,
  "max-w-2xl": 672,
  "max-w-3xl": 768,
  "max-w-4xl": 896,
  "max-w-5xl": 1024,
  "max-w-6xl": 1152,
};

function responsiveWidth(twClass: string): string {
  const basePx = WIDTH_MAP[twClass] ?? 576;
  const vwPct = Math.round(basePx / 13);
  const capPx = Math.round(basePx * 1.6);
  return `min(${vwPct}vw, ${capPx}px)`;
}

export default function Modal({
  open,
  title,
  width = "max-w-xl",
  children,
  footer,
  onClose,
}: Props) {
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-[999] flex items-center justify-center bg-black/50"
      style={{ animation: "fadeInOnly 0.15s ease-out" }}
      onClick={onClose}
    >
      <div
        className="relative bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-xl shadow-2xl w-full mx-4 max-h-[80vh] flex flex-col animate-in"
        style={{ animationDuration: "150ms", maxWidth: responsiveWidth(width) }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-5 py-3 border-b border-[hsl(var(--border-light))]">
          <h2 className="text-lg font-semibold text-[hsl(var(--text-primary))]">
            {title}
          </h2>
          <button
            onClick={onClose}
            className="h-7 w-7 inline-flex items-center justify-center rounded-md text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-primary))] hover:bg-[hsl(var(--bg-hover))] transition-colors"
          >
            <X size={16} />
          </button>
        </div>
        <div className="flex-1 overflow-auto p-5 text-sm text-[hsl(var(--text-primary))]">
          {children}
        </div>
        {footer && (
          <div className="flex justify-end gap-2 px-5 py-3 border-t border-[hsl(var(--border-light))] bg-[hsl(var(--bg-app))]">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}
