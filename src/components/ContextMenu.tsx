import { useEffect, useRef } from "react";

export interface ContextMenuItem {
  label: string;
  separator?: boolean;
  danger?: boolean;
  disabled?: boolean;
  action?: () => void;
}

interface Props {
  items: ContextMenuItem[];
  visible: boolean;
  x: number;
  y: number;
  onClose: () => void;
}

export default function ContextMenu({ items, visible, x, y, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    if (visible) {
      document.addEventListener("mousedown", handler);
      return () => document.removeEventListener("mousedown", handler);
    }
  }, [visible, onClose]);

  if (!visible) return null;

  return (
    <div
      ref={ref}
      className="fixed z-50 bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg shadow-2xl py-1 min-w-[150px] text-sm"
      style={{ left: x, top: y }}
    >
      {items.map((item, i) =>
        item.separator ? (
          <div key={i} className="border-t border-[hsl(var(--border-light))] my-1" />
        ) : (
          <button
            key={i}
            disabled={item.disabled}
            className={`w-full text-left px-3 py-1.5 transition-colors disabled:text-[hsl(var(--text-tertiary))] disabled:cursor-not-allowed ${
              item.danger
                ? "text-[hsl(var(--danger))] hover:bg-[hsl(var(--danger)/0.1)]"
                : "text-[hsl(var(--text-primary))] hover:bg-[hsl(var(--bg-hover))]"
            }`}
            onClick={() => { item.action?.(); onClose(); }}
          >
            {item.label}
          </button>
        )
      )}
    </div>
  );
}
