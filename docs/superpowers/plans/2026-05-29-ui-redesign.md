# UI 全面重设计 — 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将整个前端从浅色杂乱的工具类样式升级为统一暗色专业工具风

**Architecture:** 自底向上重构 — 先更新 CSS 变量和基础组件，再逐层推进到布局和页面。组件用 React + Tailwind + class-variance-authority，lucide-react 替代手写 SVG 和 emoji。

**Tech Stack:** React 18, TypeScript, TailwindCSS 3, lucide-react, class-variance-authority, clsx + tailwind-merge

---

### Task 1: 更新 CSS 基础 — 配色变量、重置、滚动条

**Files:**
- Modify: `src/index.css`

- [ ] **Step 1: 替换全局 CSS 变量和基础样式**

用以下内容替换 `src/index.css` 中 `:root` 块及 body 样式：

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

@layer base {
  :root {
    --bg-app: 215 28% 7%;
    --bg-content: 217 19% 11%;
    --bg-card: 217 19% 14%;
    --bg-hover: 217 19% 18%;
    --bg-active: 217 19% 22%;
    --text-primary: 210 20% 95%;
    --text-secondary: 215 12% 65%;
    --text-tertiary: 215 12% 45%;
    --accent: 217 91% 60%;
    --accent-subtle: 217 91% 60% / 0.12;
    --success: 142 71% 45%;
    --warning: 38 92% 50%;
    --danger: 0 72% 51%;
    --info: 217 91% 60%;
    --border: 217 15% 20%;
    --border-light: 217 15% 15%;
    --radius: 0.5rem;
  }

  * { border-color: hsl(var(--border)); }

  body {
    background-color: hsl(var(--bg-content));
    color: hsl(var(--text-primary));
    font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont,
      "Segoe UI", Roboto, "Helvetica Neue", Arial, "Noto Sans",
      "Microsoft YaHei", "PingFang SC", sans-serif;
    font-size: 14px;
    line-height: 1.5;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
  }
}
```

- [ ] **Step 2: 更新滚动条样式为暗色主题**

```css
::-webkit-scrollbar { width: 5px; height: 5px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: hsl(var(--text-tertiary) / 0.4); border-radius: 3px; }
::-webkit-scrollbar-thumb:hover { background: hsl(var(--text-secondary) / 0.6); }
```

- [ ] **Step 3: 删除旧的 CSS 组件类**

删除 `.btn`、`.btn-primary`、`.btn-outline`、`.btn-danger`、`.btn-ghost`、`.btn-sm`、`.input`、`.select`、`.card`、`.page-header`、`.page-title`、`.page-desc`、`.empty-state` 及其所有子样式。保留 `.animate-in` 和 `@keyframes fadeIn`，但将 duration 改为 200ms。

- [ ] **Step 4: Commit**

```bash
git add src/index.css
git commit -m "style: 暗色主题 CSS 变量 + 移除旧组件类"
```

---

### Task 2: 创建 Button 组件

**Files:**
- Create: `src/components/ui/Button.tsx`

- [ ] **Step 1: 编写 Button 组件**

```tsx
import React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/utils";
import { Loader2 } from "lucide-react";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-1.5 rounded-md text-sm font-medium transition-all duration-150 whitespace-nowrap focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[hsl(var(--accent))] focus-visible:ring-offset-1 focus-visible:ring-offset-[hsl(var(--bg-content))] disabled:opacity-50 disabled:cursor-not-allowed select-none",
  {
    variants: {
      variant: {
        primary:
          "bg-[hsl(var(--accent))] text-white hover:bg-[hsl(var(--accent)/0.9)] shadow-sm shadow-[hsl(var(--accent)/0.25)]",
        secondary:
          "bg-transparent text-[hsl(var(--text-primary))] border border-[hsl(var(--border))] hover:bg-[hsl(var(--bg-hover))]",
        ghost:
          "bg-transparent text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))] hover:bg-[hsl(var(--bg-hover))]",
        danger:
          "bg-[hsl(var(--danger))] text-white hover:bg-[hsl(var(--danger)/0.9)] shadow-sm shadow-[hsl(var(--danger)/0.25)]",
      },
      size: {
        sm: "h-7 px-2.5 text-xs rounded",
        md: "h-8 px-3.5 text-sm rounded-md",
        icon: "h-8 w-8 p-0",
      },
    },
    defaultVariants: {
      variant: "primary",
      size: "md",
    },
  }
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  loading?: boolean;
}

export default function Button({
  className,
  variant,
  size,
  loading,
  disabled,
  children,
  ...props
}: ButtonProps) {
  return (
    <button
      className={cn(buttonVariants({ variant, size, className }))}
      disabled={disabled || loading}
      {...props}
    >
      {loading && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
      {children}
    </button>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/components/ui/Button.tsx
git commit -m "feat: 添加暗色主题 Button 组件 (cva)"
```

---

### Task 3: 创建 Input 组件

**Files:**
- Create: `src/components/ui/Input.tsx`

- [ ] **Step 1: 编写 Input 组件**

```tsx
import React from "react";
import { cn } from "../../lib/utils";

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  size?: "sm" | "md";
}

const sizeClasses = {
  sm: "h-7 text-xs px-2",
  md: "h-8 text-sm px-2.5",
};

export default function Input({ className, size = "md", ...props }: InputProps) {
  return (
    <input
      className={cn(
        "w-full rounded-md bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] text-[hsl(var(--text-primary))] placeholder:text-[hsl(var(--text-tertiary))] outline-none transition-colors duration-150",
        "focus:border-[hsl(var(--accent))] focus:ring-2 focus:ring-[hsl(var(--accent)/0.2)]",
        "disabled:opacity-50 disabled:cursor-not-allowed",
        sizeClasses[size],
        className
      )}
      {...props}
    />
  );
}
```

- [ ] **Step 2: 同样导出 Select 组件（同文件）**

```tsx
export interface SelectProps extends React.SelectHTMLAttributes<HTMLSelectElement> {
  size?: "sm" | "md";
}

export function Select({ className, size = "md", children, ...props }: SelectProps) {
  return (
    <select
      className={cn(
        "w-full rounded-md bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] text-[hsl(var(--text-primary))] outline-none transition-colors duration-150 cursor-pointer",
        "focus:border-[hsl(var(--accent))] focus:ring-2 focus:ring-[hsl(var(--accent)/0.2)]",
        "disabled:opacity-50 disabled:cursor-not-allowed",
        sizeClasses[size],
        className
      )}
      {...props}
    >
      {children}
    </select>
  );
}
```

- [ ] **Step 3: Commit**

```bash
git add src/components/ui/Input.tsx
git commit -m "feat: 添加暗色主题 Input/Select 组件"
```

---

### Task 4: 创建 Card 组件

**Files:**
- Create: `src/components/ui/Card.tsx`

- [ ] **Step 1: 编写 Card 组件**

```tsx
import React from "react";
import { cn } from "../../lib/utils";

interface CardProps {
  className?: string;
  padding?: boolean;
  children: React.ReactNode;
}

export default function Card({ className, padding = true, children }: CardProps) {
  return (
    <div
      className={cn(
        "bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg",
        padding && "p-4",
        className
      )}
    >
      {children}
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/components/ui/Card.tsx
git commit -m "feat: 添加暗色主题 Card 组件"
```

---

### Task 5: 重构 Modal 组件

**Files:**
- Modify: `src/components/Modal.tsx`

- [ ] **Step 1: 替换 Modal 内容**

```tsx
import { useEffect, useRef } from "react";
import { X } from "lucide-react";

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
    <div className="fixed inset-0 z-40 flex items-center justify-center">
      <div
        ref={overlayRef}
        className="absolute inset-0 bg-black/60 backdrop-blur-sm animate-in"
        onClick={e => { if (e.target === overlayRef.current) onClose(); }}
      />
      <div
        className={`relative bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-xl shadow-2xl ${width} w-full mx-4 max-h-[80vh] flex flex-col animate-in`}
        style={{ animationDuration: "150ms" }}
      >
        <div className="flex items-center justify-between px-5 py-3 border-b border-[hsl(var(--border-light))]">
          <h2 className="text-lg font-semibold text-[hsl(var(--text-primary))]">{title}</h2>
          <button
            onClick={onClose}
            className="h-7 w-7 inline-flex items-center justify-center rounded-md text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-primary))] hover:bg-[hsl(var(--bg-hover))] transition-colors"
          >
            <X size={16} />
          </button>
        </div>
        <div className="flex-1 overflow-auto p-5 text-sm text-[hsl(var(--text-primary))]">{children}</div>
        {footer && (
          <div className="flex justify-end gap-2 px-5 py-3 border-t border-[hsl(var(--border-light))] bg-[hsl(var(--bg-app))]">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/components/Modal.tsx
git commit -m "style: 重构 Modal 为暗色主题 + lucide X 图标"
```

---

### Task 6: 重构 StatusBadge 组件

**Files:**
- Modify: `src/components/StatusBadge.tsx`

- [ ] **Step 1: 替换 StatusBadge 内容**

```tsx
type Status = "online" | "offline" | "unknown" | "ok" | "warning" | "critical" | "info" | "pending" | "running" | "completed" | "failed" | "stopped";

const STYLES: Record<Status, string> = {
  online: "bg-emerald-500/15 text-emerald-400 border-emerald-500/30",
  offline: "bg-red-500/15 text-red-400 border-red-500/30",
  unknown: "bg-gray-500/15 text-gray-400 border-gray-500/30",
  ok: "bg-emerald-500/15 text-emerald-400 border-emerald-500/30",
  warning: "bg-amber-500/15 text-amber-400 border-amber-500/30",
  critical: "bg-red-500/15 text-red-400 border-red-500/30",
  info: "bg-blue-500/15 text-blue-400 border-blue-500/30",
  pending: "bg-gray-500/15 text-gray-400 border-gray-500/30",
  running: "bg-blue-500/15 text-blue-400 border-blue-500/30 animate-pulse",
  completed: "bg-emerald-500/15 text-emerald-400 border-emerald-500/30",
  failed: "bg-red-500/15 text-red-400 border-red-500/30",
  stopped: "bg-amber-500/15 text-amber-400 border-amber-500/30",
};

const LABELS: Record<Status, string> = {
  online: "在线", offline: "离线", unknown: "未知",
  ok: "正常", warning: "警告", critical: "严重", info: "信息",
  pending: "等待中", running: "执行中", completed: "已完成", failed: "失败", stopped: "已停止",
};

const DOT_COLORS: Record<Status, string> = {
  online: "bg-emerald-400", offline: "bg-red-400", unknown: "bg-gray-400",
  ok: "bg-emerald-400", warning: "bg-amber-400", critical: "bg-red-400", info: "bg-blue-400",
  pending: "bg-gray-400", running: "bg-blue-400", completed: "bg-emerald-400", failed: "bg-red-400", stopped: "bg-amber-400",
};

export default function StatusBadge({ status }: { status: Status }) {
  const style = STYLES[status] || STYLES.unknown;
  const dot = DOT_COLORS[status] || DOT_COLORS.unknown;
  return (
    <span className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded border text-[11px] font-medium ${style}`}>
      <span className={`w-1.5 h-1.5 rounded-full ${dot}`} />
      {LABELS[status] || status}
    </span>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/components/StatusBadge.tsx
git commit -m "style: 重构 StatusBadge 为暗色半透明风格 + 状态圆点"
```

---

### Task 7: 重构 SearchInput 组件

**Files:**
- Modify: `src/components/SearchInput.tsx`

- [ ] **Step 1: 替换为 lucide 图标 + 暗色样式**

```tsx
import { useRef, useEffect } from "react";
import { Search, X } from "lucide-react";

interface Props {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
}

export default function SearchInput({ value, onChange, placeholder = "搜索..." }: Props) {
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === "f") {
        e.preventDefault();
        ref.current?.focus();
        ref.current?.select();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  return (
    <div className="relative">
      <Search size={13} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-[hsl(var(--text-tertiary))]" />
      <input
        ref={ref}
        type="text"
        value={value}
        onChange={e => onChange(e.target.value)}
        placeholder={placeholder}
        className="h-7 pl-7 pr-6 rounded-md border border-[hsl(var(--border))] bg-[hsl(var(--bg-card))] text-xs text-[hsl(var(--text-primary))] placeholder:text-[hsl(var(--text-tertiary))] w-48 outline-none focus:border-[hsl(var(--accent))] focus:ring-2 focus:ring-[hsl(var(--accent)/0.2)] transition-colors"
      />
      {value && (
        <button
          className="absolute right-1.5 top-1/2 -translate-y-1/2 text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-primary))]"
          onClick={() => onChange("")}
        >
          <X size={12} />
        </button>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/components/SearchInput.tsx
git commit -m "style: 重构 SearchInput — lucide 图标 + 暗色样式"
```

---

### Task 8: 重构 DataTable 组件

**Files:**
- Modify: `src/components/DataTable.tsx`

- [ ] **Step 1: 替换 DataTable 内容**

```tsx
import React from "react";

interface Column<T> {
  key: string;
  header: string;
  width?: string;
  render: (row: T) => React.ReactNode;
}

interface Props<T> {
  columns: Column<T>[];
  data: T[];
  rowKey: (row: T) => string | number;
  selected?: Set<string | number>;
  onSelect?: (keys: Set<string | number>) => void;
  onRowDoubleClick?: (row: T) => void;
  onContextMenu?: (e: React.MouseEvent, row: T) => void;
  emptyText?: string;
}

export default function DataTable<T>({ columns, data, rowKey, selected, onSelect, onRowDoubleClick, onContextMenu, emptyText = "暂无数据" }: Props<T>) {
  return (
    <div className="border border-[hsl(var(--border))] rounded-lg overflow-hidden">
      <div className="overflow-auto max-h-[60vh]">
        <table className="w-full text-sm">
          <thead>
            <tr className="bg-[hsl(var(--bg-hover))] sticky top-0 z-10">
              {columns.map(col => (
                <th
                  key={col.key}
                  className="text-left px-3 py-2 border-b border-[hsl(var(--border))] text-[11px] font-medium uppercase tracking-wide text-[hsl(var(--text-secondary))]"
                  style={{ width: col.width }}
                >
                  {col.header}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data.length === 0 ? (
              <tr>
                <td colSpan={columns.length} className="text-center py-12 text-[hsl(var(--text-tertiary))] text-sm">
                  {emptyText}
                </td>
              </tr>
            ) : (
              data.map(row => (
                <tr
                  key={rowKey(row)}
                  className="border-b border-[hsl(var(--border-light))] hover:bg-[hsl(var(--bg-hover))] transition-colors cursor-default"
                  onDoubleClick={() => onRowDoubleClick?.(row)}
                  onContextMenu={e => onContextMenu?.(e, row)}
                >
                  {columns.map(col => (
                    <td key={col.key} className="px-3 py-2 text-[hsl(var(--text-primary))]">
                      {col.render(row)}
                    </td>
                  ))}
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/components/DataTable.tsx
git commit -m "style: 重构 DataTable 为暗色主题"
```

---

### Task 9: 重构 ContextMenu 组件

**Files:**
- Modify: `src/components/ContextMenu.tsx`

- [ ] **Step 1: 替换 ContextMenu 内容**

```tsx
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
```

- [ ] **Step 2: Commit**

```bash
git add src/components/ContextMenu.tsx
git commit -m "style: 重构 ContextMenu 为暗色主题"
```

---

### Task 10: 重构 Toolbar 组件

**Files:**
- Modify: `src/components/Toolbar.tsx`

- [ ] **Step 1: 更新样式**

```tsx
interface Props {
  children: React.ReactNode;
}

export default function Toolbar({ children }: Props) {
  return (
    <div className="flex items-center gap-2 flex-wrap">
      {children}
    </div>
  );
}
```

（移除 `mb-2`，让使用者自行控制外边距）

- [ ] **Step 2: Commit**

```bash
git add src/components/Toolbar.tsx
git commit -m "style: 精简 Toolbar 组件样式"
```

---

### Task 11: 重构 AppShell 布局

**Files:**
- Modify: `src/layouts/AppShell.tsx`

- [ ] **Step 1: 重写 AppShell，使用 lucide 图标 + 暗色 sidebar**

```tsx
import { useState, useEffect, useMemo } from "react";
import { Outlet, useNavigate, useLocation } from "react-router-dom";
import {
  Server, FolderTree, Play, FileText, Bot, Settings,
  ChevronLeft,
} from "lucide-react";

type PageKey = "devices" | "templates" | "inspection" | "reports" | "ai-config" | "settings";

const NAV_ITEMS = [
  { key: "devices" as const,    label: "设备管理", path: "/devices",    icon: Server },
  { key: "templates" as const,  label: "巡检模板", path: "/templates",  icon: FolderTree },
  { key: "inspection" as const, label: "执行巡检", path: "/inspection", icon: Play },
  { key: "reports" as const,    label: "巡检报告", path: "/reports",    icon: FileText },
  { key: "ai-config" as const,  label: "AI 配置",  path: "/ai-config",  icon: Bot },
  { key: "settings" as const,   label: "系统设置", path: "/settings",   icon: Settings },
];

export default function AppShell() {
  const navigate = useNavigate();
  const location = useLocation();
  const [collapsed, setCollapsed] = useState(false);
  const [statusMsg, setStatusMsg] = useState("就绪");

  const activeKey = useMemo(
    () => NAV_ITEMS.find(item => location.pathname.startsWith(item.path))?.key ?? "devices",
    [location.pathname]
  );

  useEffect(() => {
    const handler = (e: Event) => setStatusMsg((e as CustomEvent).detail);
    window.addEventListener("statusbar-message", handler);
    return () => window.removeEventListener("statusbar-message", handler);
  }, []);

  return (
    <div className="h-screen flex flex-col overflow-hidden" style={{ backgroundColor: "hsl(var(--bg-app))" }}>
      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <aside
          className={`${collapsed ? "w-[56px]" : "w-[220px]"} shrink-0 flex flex-col transition-[width] duration-200 ease-out`}
          style={{ backgroundColor: "hsl(var(--bg-app))" }}
        >
          {/* Brand */}
          <div className={`flex items-center gap-3 px-4 h-12 border-b border-[hsl(var(--border-light))] ${collapsed ? "justify-center" : ""}`}>
            <div className="w-7 h-7 rounded-md bg-[hsl(var(--accent))] flex items-center justify-center shrink-0 shadow-sm shadow-[hsl(var(--accent)/0.3)]">
              <Server size={15} className="text-white" />
            </div>
            {!collapsed && <span className="text-sm font-semibold text-[hsl(var(--text-primary))] tracking-tight">Inspect</span>}
          </div>

          {/* Nav */}
          <nav className="flex-1 py-3 px-2 space-y-0.5">
            {NAV_ITEMS.map(item => {
              const active = activeKey === item.key;
              const Icon = item.icon;
              return (
                <button
                  key={item.key}
                  onClick={() => navigate(item.path)}
                  title={collapsed ? item.label : undefined}
                  className={`
                    flex items-center gap-3 w-full rounded-md transition-all duration-150 select-none
                    ${collapsed ? "px-0 justify-center h-9" : "px-3 h-8"}
                    ${active
                      ? "bg-[hsl(var(--accent-subtle))] text-[hsl(var(--accent))]"
                      : "text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--bg-hover))]"
                    }
                  `}
                >
                  <Icon size={18} className="shrink-0" />
                  {!collapsed && (
                    <span className={`text-[13px] truncate ${active ? "font-medium" : ""}`}>
                      {item.label}
                    </span>
                  )}
                </button>
              );
            })}
          </nav>

          {/* Collapse toggle */}
          <div className="border-t border-[hsl(var(--border-light))] p-2">
            <button
              onClick={() => setCollapsed(!collapsed)}
              className={`w-full flex items-center gap-2 text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--bg-hover))] rounded-md transition-colors ${collapsed ? "justify-center h-9" : "px-3 h-8"}`}
            >
              <ChevronLeft size={14}
                style={{ transform: collapsed ? "rotate(180deg)" : "none", transition: "transform 0.2s" }}
              />
              {!collapsed && <span className="text-[12px]">收起菜单</span>}
            </button>
          </div>
        </aside>

        {/* Content */}
        <main className="flex-1 overflow-auto" style={{ backgroundColor: "hsl(var(--bg-content))" }}>
          <div className="animate-in p-6">
            <Outlet />
          </div>
        </main>
      </div>

      {/* Status bar */}
      <footer className="h-7 shrink-0 flex items-center px-4 text-[11px] gap-3 select-none border-t border-[hsl(var(--border-light))] text-[hsl(var(--text-tertiary))]" style={{ backgroundColor: "hsl(var(--bg-app))" }}>
        <span className="flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shadow-sm shadow-emerald-400/50" />
          {statusMsg}
        </span>
        <span className="flex-1" />
        <span className="text-[hsl(var(--text-tertiary))]">v3.1</span>
      </footer>
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/layouts/AppShell.tsx
git commit -m "style: 重构 AppShell — lucide 图标 + 暗色 sidebar/状态栏"
```

---

### Task 12: 重构 DevicesPage

**Files:**
- Modify: `src/pages/DevicesPage.tsx`

- [ ] **Step 1: 更新 imports，引入新组件**

```tsx
import Button from "../components/ui/Button";
import Card from "../components/ui/Card";
import Input from "../components/ui/Input";
```

- [ ] **Step 2: 更新页面标题区域**

将 `.page-header` / `.page-title` / `.page-desc` 替换为：

```tsx
<div className="flex items-center justify-between mb-4">
  <div>
    <h1 className="text-xl font-semibold text-[hsl(var(--text-primary))]">设备管理</h1>
  </div>
  <div className="flex items-center gap-2">
    <Button onClick={openAdd}>+ 添加设备</Button>
    <Button variant="secondary" onClick={handleRefreshAll}>刷新状态</Button>
  </div>
</div>
```

- [ ] **Step 3: 更新卡片区域**

将 `.card` 替换为 `<Card>`：

```tsx
<Card className="flex-1 flex flex-col min-h-0" padding={false}>
  <div className="flex items-center justify-between px-4 py-2.5 border-b border-[hsl(var(--border-light))]">
    ...
  </div>
  <div className="flex-1 overflow-auto">
    <DataTable ... />
  </div>
  ...
</Card>
```

- [ ] **Step 4: 更新所有按钮**

- `className="btn btn-primary"` → `<Button>...</Button>`
- `className="btn btn-outline btn-sm"` → `<Button variant="secondary" size="sm">...</Button>`
- `className="btn btn-danger btn-sm"` → `<Button variant="danger" size="sm">...</Button>`
- 表格内操作按钮：`px-2 py-0.5 bg-blue-500 ...` → `<Button variant="primary" size="sm">...</Button>`
- 表格内编辑按钮：`px-2 py-0.5 border ...` → `<Button variant="secondary" size="sm">...</Button>`

- [ ] **Step 5: 更新 Input 和 Select**

将 `<input className="input" .../>` 替换为 `<Input ... />`，将 `<select className="select" ...>` 替换为 `<Select ...>`。

- [ ] **Step 6: 更新 Modal footer 内的按钮**

```tsx
footer={
  <>
    <Button variant="secondary" size="sm" onClick={() => setEditOpen(false)}>取消</Button>
    <Button size="sm" disabled={saving || !form.name.trim() || !form.ip.trim()} onClick={handleSave} loading={saving}>
      保存
    </Button>
  </>
}
```

- [ ] **Step 7: 更新 FormField 文字颜色**

将 `text-gray-600` 替换为 `text-[hsl(var(--text-secondary))]`

- [ ] **Step 8: 更新表格内 checkbox 样式**

给 checkbox 添加暗色 accent-color 样式：

```tsx
<input type="checkbox" className="w-3.5 h-3.5 rounded accent-[hsl(var(--accent))]" ... />
```

- [ ] **Step 9: 更新 loading 文字颜色**

```tsx
if (loading) return <div className="flex items-center justify-center h-64 text-[hsl(var(--text-tertiary))] text-sm">加载中...</div>;
```

- [ ] **Step 10: Commit**

```bash
git add src/pages/DevicesPage.tsx
git commit -m "style: 重构 DevicesPage 使用新组件系统"
```

---

### Task 13: 重构 TemplatesPage

**Files:**
- Modify: `src/pages/TemplatesPage.tsx`

- [ ] **Step 1: 添加 imports**

```tsx
import Button from "../components/ui/Button";
import Card from "../components/ui/Card";
import Input from "../components/ui/Input";
import { Select } from "../components/ui/Input";
```

- [ ] **Step 2: 更新左侧面板**

将 `bg-white rounded border border-gray-200` 替换为 Card 组件样式 `bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg`。

- [ ] **Step 3: 更新所有按钮为 Button 组件**
  - 新建模板按钮：`<Button size="sm" className="w-full">+ 新建模板</Button>`
  - 自动生成命令 / 生成报告模板：`<Button variant="secondary" size="sm">...</Button>`
  - 保存按钮：`<Button size="sm" loading={saving} ...>保存</Button>`

- [ ] **Step 4: 更新面板背景和边框**

将三列面板的 `bg-white border border-gray-200` 统一替换为 `bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg`

- [ ] **Step 5: 更新列表项颜色**

将选中态 `bg-blue-100 text-blue-800` 替换为 `bg-[hsl(var(--accent-subtle))] text-[hsl(var(--accent))]`，hover 态 `hover:bg-blue-50/50` 替换为 `hover:bg-[hsl(var(--bg-hover))]`，文字 `text-gray-700` 替换为 `text-[hsl(var(--text-primary))]`

- [ ] **Step 6: 将所有 `.form-input` 替换为 `<Input>` / `<Select>` 组件**

- [ ] **Step 7: 删除页面底部 `<style>{'.form-input {...}'}</style>` 块**

- [ ] **Step 8: 更新 FormField 文字色**

`text-gray-600` → `text-[hsl(var(--text-secondary))]`，`text-gray-400` → `text-[hsl(var(--text-tertiary))]`

- [ ] **Step 9: 更新元素中的硬编码颜色类名**
  - 命令列表中的 code 背景：`bg-gray-100` → `bg-[hsl(var(--bg-hover))]`
  - 移除按钮：`text-red-500 border-red-200` → 使用 Button variant="danger" size="sm"

- [ ] **Step 10: Commit**

```bash
git add src/pages/TemplatesPage.tsx
git commit -m "style: 重构 TemplatesPage 使用新组件系统"
```

---

### Task 14: 重构 InspectionPage

**Files:**
- Modify: `src/pages/InspectionPage.tsx`

- [ ] **Step 1: 添加 imports**

```tsx
import Button from "../components/ui/Button";
import Card from "../components/ui/Card";
import Input from "../components/ui/Input";
import { Select } from "../components/ui/Input";
```

- [ ] **Step 2: 更新面板背景和边框**

所有 `bg-white rounded border border-gray-200` → `bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg`

- [ ] **Step 3: 更新所有按钮为 Button 组件**
  - 创建批次：`<Button size="sm" disabled={selectedIds.size === 0 || creating} loading={creating}>创建批次</Button>`
  - 开始巡检：`<Button size="sm" className="bg-emerald-500 hover:bg-emerald-600">开始巡检</Button>` (用 variant="primary" 包一层或直接用样式)
  - 暂停：`<Button size="sm" className="bg-amber-500 hover:bg-amber-600">暂停</Button>`
  - 停止：`<Button variant="danger" size="sm">停止</Button>`
  - 重新开始：`<Button variant="secondary" size="sm">重新开始</Button>`
  - 查看报告：`<Button variant="ghost" size="sm">查看报告</Button>`
  - 返回列表：`<Button variant="ghost" size="sm">返回列表</Button>`
  - 重试：`<Button variant="secondary" size="sm">重试</Button>`

- [ ] **Step 4: 更新 Input 和 Select**
  - 批次名称 input → `<Input size="sm" className="w-48" placeholder="批次名称（可选）" ... />`
  - Mode select → `<Select size="sm" ...>`

- [ ] **Step 5: 更新进度条颜色**

```tsx
<div className="w-full h-2.5 bg-[hsl(var(--bg-hover))] rounded-full overflow-hidden">
  <div
    className={`h-full rounded-full transition-all duration-500 ${
      isCompleted ? "bg-emerald-500" : isRunning ? "bg-[hsl(var(--accent))] animate-pulse" : "bg-[hsl(var(--text-tertiary))]"
    }`}
    style={{ width: `${progressPercent}%` }}
  />
</div>
```

- [ ] **Step 6: 更新表格表头和行样式**

参考 DataTable 组件样式：表头 `bg-[hsl(var(--bg-hover))]`，文字 `text-[hsl(var(--text-secondary))]`，行 hover `hover:bg-[hsl(var(--bg-hover))]`，border `border-[hsl(var(--border-light))]`

- [ ] **Step 7: 更新设备详情展开区域**

`bg-gray-50` → `bg-[hsl(var(--bg-app))]`

- [ ] **Step 8: 更新命令输出代码块的暗色样式**

```tsx
<pre className="bg-[hsl(var(--bg-app))] text-emerald-400 p-2 rounded overflow-auto max-h-48 text-[11px] whitespace-pre-wrap">
```

- [ ] **Step 9: 更新 AiStatusLabel 颜色类名**

`text-green-600` → `text-emerald-400`，`text-red-600` → `text-red-400`，`text-blue-600` → `text-blue-400`，`text-gray-400` → `text-[hsl(var(--text-tertiary))]`

- [ ] **Step 10: 删除页面底部 `<style>{'.form-input {...}'}</style>` 块**

- [ ] **Step 11: Commit**

```bash
git add src/pages/InspectionPage.tsx
git commit -m "style: 重构 InspectionPage 使用新组件系统"
```

---

### Task 15: 重构 ReportsPage

**Files:**
- Modify: `src/pages/ReportsPage.tsx`

- [ ] **Step 1: 添加 imports**

```tsx
import Button from "../components/ui/Button";
```

- [ ] **Step 2: 更新所有面板背景**
  - `bg-white border border-gray-200` → `bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg`

- [ ] **Step 3: 更新所有按钮为 Button 组件**
  - 返回列表 → `<Button variant="secondary" size="sm">← 返回列表</Button>`
  - 导出 PDF → `<Button variant="primary" size="sm">...</Button>`
  - 打印 → `<Button variant="secondary" size="sm">...</Button>`
  - 导出 Markdown → `<Button variant="secondary" size="sm">...</Button>`
  - 生成报告 → `<Button variant="ghost" size="sm">...</Button>`
  - 批量生成报告 → `<Button variant="secondary" size="sm">批量生成报告</Button>`
  - 删除选中 → `<Button variant="danger" size="sm">删除选中 ({n})</Button>`

- [ ] **Step 4: 更新表格样式**

表头 `bg-gray-50` → `bg-[hsl(var(--bg-hover))]`，文字 `text-gray-500` → `text-[hsl(var(--text-secondary))]`，行 hover 和 border 同步更新。

- [ ] **Step 5: 更新 AiJudgmentBadge 颜色为暗色半透明**

参考 StatusBadge 风格：`bg-red-500/15 text-red-400 border-red-500/30` 等

- [ ] **Step 6: 更新预览内容区背景**

`bg-white` → `bg-[hsl(var(--bg-card))]`，prose 样式：由于是暗色背景，需要确保 prose 的标题/文字对比度足够。在暗色卡片中，Markdown 内容以浅色呈现。

- [ ] **Step 7: 更新 ContextMenu 为封装组件** (如果已有独立 ContextMenu.tsx，确保已导入并使用)

- [ ] **Step 8: Commit**

```bash
git add src/pages/ReportsPage.tsx
git commit -m "style: 重构 ReportsPage 使用新组件系统"
```

---

### Task 16: 重构 AiConfigPage

**Files:**
- Modify: `src/pages/AiConfigPage.tsx`

- [ ] **Step 1: 添加 imports**

```tsx
import Button from "../components/ui/Button";
import Card from "../components/ui/Card";
import Input from "../components/ui/Input";
import { Select } from "../components/ui/Input";
```

- [ ] **Step 2: 更新页面标题和面板**

标题改为一致格式，面板 `bg-white border border-gray-200` → Card 组件样式 `bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg`

- [ ] **Step 3: 更新所有按钮为 Button 组件**
  - 测试连接 → `<Button variant="secondary" size="sm" loading={testing} ...>测试连接</Button>`
  - 保存配置 → `<Button size="sm" loading={saving} ...>保存配置</Button>`
  - 启用 → `<Button size="sm" className="bg-emerald-500 hover:bg-emerald-600">启用</Button>`
  - 停用 → `<Button variant="secondary" size="sm">停用</Button>`
  - 删除 → `<Button variant="danger" size="sm">删除</Button>`

- [ ] **Step 4: 替换所有 `.form-input` 为 `<Input>` / `<Select>` 组件**

- [ ] **Step 5: 更新 DataTable 用法**

DataTable 已在 Task 8 重构，这里只需确认引用即可。

- [ ] **Step 6: 更新 provider badge 颜色为暗色半透明**

`bg-green-100 text-green-700` → `bg-emerald-500/15 text-emerald-400` 等

- [ ] **Step 7: 删除页面底部 `<style>{'.form-input {...}'}</style>` 块**

- [ ] **Step 8: 更新 FormField 文字色**

`text-gray-600` → `text-[hsl(var(--text-secondary))]`

- [ ] **Step 9: 更新测试结果消息区域的背景色**

`bg-green-50 border-green-200` → `bg-emerald-500/10 border-emerald-500/30 text-emerald-400`
`bg-red-50 border-red-200` → `bg-red-500/10 border-red-500/30 text-red-400`

- [ ] **Step 10: Commit**

```bash
git add src/pages/AiConfigPage.tsx
git commit -m "style: 重构 AiConfigPage 使用新组件系统"
```

---

### Task 17: 重构 SettingsPage

**Files:**
- Modify: `src/pages/SettingsPage.tsx`

- [ ] **Step 1: 添加 imports**

```tsx
import Button from "../components/ui/Button";
import Card from "../components/ui/Card";
import Input from "../components/ui/Input";
```

- [ ] **Step 2: 更新页面标题和面板**

面板 `bg-white border border-gray-200` → `bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg`

- [ ] **Step 3: 更新 tabs 样式**

```tsx
<button
  className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
    activeTab === tab.key
      ? "border-[hsl(var(--accent))] text-[hsl(var(--accent))]"
      : "border-transparent text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))] hover:border-[hsl(var(--border))]"
  }`}
>
  {tab.label}
</button>
```

- [ ] **Step 4: 更新所有按钮为 Button 组件**
  - 保存设置 → `<Button size="sm" loading={saving} ...>保存设置</Button>`
  - 浏览 → `<Button variant="secondary" size="sm">浏览</Button>`

- [ ] **Step 5: 替换所有 `.form-input` 为 `<Input>` 组件**

- [ ] **Step 6: 更新关于页面的标签颜色**

`bg-gray-100 border-gray-200 text-gray-600` → `bg-[hsl(var(--bg-hover))] border-[hsl(var(--border))] text-[hsl(var(--text-secondary))]`

- [ ] **Step 7: 删除页面底部 `<style>{'.form-input {...}'}</style>` 块**

- [ ] **Step 8: 更新 FormField 文字色**

`text-gray-600` → `text-[hsl(var(--text-secondary))]`，辅助文字 `text-gray-400` → `text-[hsl(var(--text-tertiary))]`

- [ ] **Step 9: Commit**

```bash
git add src/pages/SettingsPage.tsx
git commit -m "style: 重构 SettingsPage 使用新组件系统"
```

---

### Task 18: 最终清理和验证

**Files:**
- Modify: `src/index.css` (移除可能遗留的旧样式)
- Modify: `tailwind.config.js` (如有需要)

- [ ] **Step 1: 全局搜索遗留的硬编码亮色类名**

```bash
grep -r "bg-white\|bg-gray-100\|border-gray-\|text-gray-\|bg-blue-100\|bg-green-100\|bg-red-100\|bg-yellow-100" src/ --include="*.tsx" --include="*.ts" --include="*.css" | grep -v node_modules
```

- [ ] **Step 2: 全局搜索 emoji**

```bash
grep -r "🔍\|✕\|🔴\|⚠️\|✅\|🔴\|🚀\|⚙️\|🎨" src/ --include="*.tsx"
```

- [ ] **Step 3: 运行前端构建确认无错误**

```bash
npm run build
```

Expected: 成功构建，无 TypeScript 错误

- [ ] **Step 4: 启动开发服务器确认 UI 正常**

```bash
cargo tauri dev
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: 最终清理 — 移除残余旧样式和 emoji"
```
