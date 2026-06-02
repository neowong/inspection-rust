import { useEffect } from "react";

type ShortcutHandler = () => void;
const shortcuts = new Map<string, ShortcutHandler>();

export function useGlobalShortcuts() {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey;
      const key = `${ctrl ? "Ctrl+" : ""}${e.key}`;
      if (key === "Ctrl+f") e.preventDefault();
      if (key === "Ctrl+s") e.preventDefault();
      const fn = shortcuts.get(key);
      if (fn) { e.preventDefault(); fn(); }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);
}
