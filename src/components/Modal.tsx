import { useEffect, useRef } from "react";

interface Props {
  open: boolean;
  title: string;
  width?: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  onClose: () => void;
}

export default function Modal({ open, title, width = "max-w-lg", children, footer, onClose }: Props) {
  const overlayRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    if (open) { document.addEventListener("keydown", handler); return () => document.removeEventListener("keydown", handler); }
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div ref={overlayRef} className="fixed inset-0 z-40 flex items-center justify-center bg-black/30" onClick={e => { if (e.target === overlayRef.current) onClose(); }}>
      <div className={`bg-white rounded shadow-xl ${width} w-full mx-4 max-h-[80vh] flex flex-col`}>
        <div className="flex items-center justify-between px-4 py-2 border-b">
          <h2 className="text-sm font-semibold">{title}</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-600 text-lg leading-none">&times;</button>
        </div>
        <div className="flex-1 overflow-auto p-4 text-sm">{children}</div>
        {footer && <div className="flex justify-end gap-2 px-4 py-2 border-t bg-gray-50">{footer}</div>}
      </div>
    </div>
  );
}
