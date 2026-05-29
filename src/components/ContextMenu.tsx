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
      className="fixed z-50 bg-white border border-gray-300 rounded shadow-lg py-1 min-w-[140px] text-xs"
      style={{ left: x, top: y }}
    >
      {items.map((item, i) =>
        item.separator ? (
          <div key={i} className="border-t border-gray-200 my-1" />
        ) : (
          <button
            key={i}
            disabled={item.disabled}
            className={`w-full text-left px-3 py-1.5 hover:bg-blue-50 disabled:text-gray-400 disabled:hover:bg-white ${
              item.danger ? "text-red-600 hover:bg-red-50" : ""
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
