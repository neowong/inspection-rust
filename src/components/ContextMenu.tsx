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

export default function ContextMenu({
  items,
  visible,
  x,
  y,
  onClose,
}: Props) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!visible) return;

    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onClose();
      }
    };

    // Use setTimeout to avoid the same click that opened the menu from closing it
    const timer = setTimeout(() => {
      document.addEventListener("mousedown", handler);
    }, 0);

    return () => {
      clearTimeout(timer);
      document.removeEventListener("mousedown", handler);
    };
  }, [visible, onClose]);

  useEffect(() => {
    if (!visible) return;

    const escapeHandler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", escapeHandler);
    return () => document.removeEventListener("keydown", escapeHandler);
  }, [visible, onClose]);

  if (!visible) return null;

  return (
    <div
      ref={ref}
      className="fixed z-50 min-w-[160px] bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg shadow-xl py-1 animate-in"
      style={{ left: x, top: y }}
    >
      {items.map((item, i) => {
        if (item.separator) {
          return (
            <div
              key={i}
              className="my-1 border-t border-[hsl(var(--border-light))]"
            />
          );
        }
        return (
          <button
            key={i}
            disabled={item.disabled}
            onClick={() => {
              if (!item.disabled) {
                item.action?.();
                onClose();
              }
            }}
            className={`w-full text-left px-3 py-1.5 text-[13px] transition-colors ${
              item.danger
                ? "text-[hsl(var(--danger))] hover:bg-[hsl(var(--danger)/0.08)]"
                : "text-[hsl(var(--text-primary))] hover:bg-[hsl(var(--bg-hover))]"
            } ${item.disabled ? "opacity-40 cursor-not-allowed" : "cursor-pointer"}`}
          >
            {item.label}
          </button>
        );
      })}
    </div>
  );
}
