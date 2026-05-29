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
