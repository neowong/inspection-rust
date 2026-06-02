import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X } from "lucide-react";

interface VariableDef {
  name: string;
  category: string;
  description: string;
  example: string;
}

const CATEGORY_LABELS: Record<string, string> = {
  device: "设备信息",
  command_output: "命令输出",
  judgment: "逐项判断",
  ai: "AI 分析",
  meta: "报告元信息",
};

export default function VariablePicker({ onSelect, onClose }: { onSelect: (name: string) => void; onClose: () => void }) {
  const [variables, setVariables] = useState<VariableDef[]>([]);
  const [activeCategory, setActiveCategory] = useState<string>("");
  const [search, setSearch] = useState("");

  useEffect(() => {
    invoke<VariableDef[]>("get_available_variables")
      .then((vars) => {
        setVariables(vars);
        const cats = [...new Set(vars.map(v => v.category))];
        if (cats.length > 0) setActiveCategory(cats[0] ?? "");
      })
      .catch(console.error);
  }, []);

  const categories = [...new Set(variables.map(v => v.category))];

  const filtered = variables.filter((v) => {
    if (activeCategory && v.category !== activeCategory) return false;
    if (search && !v.name.includes(search.toLowerCase()) && !v.description.includes(search.toLowerCase())) return false;
    return true;
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30" onClick={onClose}>
      <div
        className="bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg shadow-xl w-[480px] max-h-[400px] flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-3 py-2 border-b border-[hsl(var(--border-light))]">
          <span className="text-sm font-medium text-[hsl(var(--text-primary))]">插入变量</span>
          <button onClick={onClose} className="text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-primary))]"><X size={16} /></button>
        </div>

        {/* Category tabs */}
        <div className="flex gap-1 px-3 pt-2 pb-1 border-b border-[hsl(var(--border-light))] overflow-x-auto">
          {categories.map((cat) => (
            <button
              key={cat}
              onClick={() => setActiveCategory(cat)}
              className={`px-2.5 py-1 rounded-md text-[11px] font-medium whitespace-nowrap transition-colors ${
                activeCategory === cat
                  ? "bg-[hsl(var(--accent)_/_0.1)] text-[hsl(var(--accent))]"
                  : "text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-secondary))]"
              }`}
            >
              {CATEGORY_LABELS[cat] || cat}
            </button>
          ))}
        </div>

        {/* Search */}
        <div className="px-3 pt-2">
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="搜索变量..."
            className="w-full h-7 text-xs px-2 rounded border border-[hsl(var(--border))] bg-[hsl(var(--bg-app))] text-[hsl(var(--text-primary))] focus:outline-none focus:border-[hsl(var(--accent))]"
          />
        </div>

        {/* Variable list */}
        <div className="flex-1 overflow-y-auto px-3 py-2 space-y-0.5">
          {filtered.length === 0 && (
            <p className="text-xs text-[hsl(var(--text-tertiary))] text-center py-4">无匹配变量</p>
          )}
          {filtered.map((v) => (
            <button
              key={v.name}
              onClick={() => onSelect(v.name)}
              className="w-full text-left px-2 py-1.5 rounded-md hover:bg-[hsl(var(--bg-hover))] transition-colors group"
            >
              <div className="flex items-center justify-between">
                <code className="text-[11px] text-[hsl(var(--accent))] font-mono">{`{{${v.name}}}`}</code>
                <span className="text-[10px] text-[hsl(var(--text-tertiary))] opacity-0 group-hover:opacity-100 transition-opacity">点击插入</span>
              </div>
              <div className="text-[10px] text-[hsl(var(--text-secondary))] mt-0.5">{v.description}</div>
              {v.example && <div className="text-[10px] text-[hsl(var(--text-tertiary))]">示例: {v.example}</div>}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
