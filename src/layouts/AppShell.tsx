import { useState, useEffect, useMemo } from "react";
import { Outlet, useNavigate, useLocation } from "react-router-dom";
import {
  FolderTree, Server, Play, FileText, Bot, Settings,
  ChevronLeft, Gauge,
} from "lucide-react";

type PageKey = "templates" | "devices" | "inspection" | "reports" | "ai-config" | "settings";

interface NavItem {
  key: PageKey;
  label: string;
  path: string;
  icon: typeof FolderTree;
}

const NAV_GROUPS: { label?: string; items: NavItem[] }[] = [
  {
    label: "巡检工作流",
    items: [
      { key: "templates",  label: "巡检模板", path: "/templates",  icon: FolderTree },
      { key: "devices",    label: "设备管理", path: "/devices",    icon: Server },
      { key: "inspection", label: "执行巡检", path: "/inspection", icon: Play },
      { key: "reports",    label: "巡检报告", path: "/reports",    icon: FileText },
    ],
  },
  {
    label: "系统",
    items: [
      { key: "ai-config", label: "AI 配置",  path: "/ai-config", icon: Bot },
      { key: "settings",  label: "系统设置", path: "/settings",  icon: Settings },
    ],
  },
];

const FLAT_ITEMS = NAV_GROUPS.flatMap(g => g.items);

export default function AppShell() {
  const navigate = useNavigate();
  const location = useLocation();
  const [collapsed, setCollapsed] = useState(false);
  const [statusMsg, setStatusMsg] = useState("就绪");

  const activeKey = useMemo(
    () => FLAT_ITEMS.find(item => location.pathname.startsWith(item.path))?.key ?? "templates",
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
        {/* ====== Sidebar ====== */}
        <aside
          className={`${collapsed ? "w-[56px]" : "w-[224px]"} shrink-0 flex flex-col transition-[width] duration-200 ease-out border-r border-[hsl(var(--border-light))]`}
          style={{ backgroundColor: "hsl(var(--bg-app))" }}
        >
          {/* Brand */}
          <div className={`flex items-center gap-3 h-12 border-b border-[hsl(var(--border-light))] ${collapsed ? "px-0 justify-center" : "px-4"}`}>
            <div className="w-7 h-7 rounded-lg bg-[hsl(var(--accent))] flex items-center justify-center shrink-0 shadow-sm">
              <Gauge size={16} className="text-white" />
            </div>
            {!collapsed && (
              <span className="text-[15px] font-semibold text-[hsl(var(--text-primary))] tracking-tight">
                NetInspect
              </span>
            )}
          </div>

          {/* Nav groups */}
          <nav className="flex-1 py-2 overflow-y-auto">
            {NAV_GROUPS.map((group, gi) => (
              <div key={gi} className={gi > 0 ? "mt-2" : ""}>
                {!collapsed && group.label && (
                  <div className="px-3 pt-2 pb-1 text-[10px] font-medium uppercase tracking-widest text-[hsl(var(--text-tertiary))]">
                    {group.label}
                  </div>
                )}
                <div className="px-2 space-y-0.5">
                  {group.items.map(item => {
                    const active = activeKey === item.key;
                    const Icon = item.icon;
                    return (
                      <button
                        key={item.key}
                        onClick={() => navigate(item.path)}
                        title={collapsed ? item.label : undefined}
                        className={`
                          flex items-center gap-3 w-full select-none transition-all duration-150
                          border-l-[3px] rounded-r-md
                          ${collapsed ? "px-0 justify-center h-9 rounded-l-md border-l-0" : "px-3 h-8 rounded-l-[2px]"}
                          ${active
                            ? "bg-[hsl(var(--accent-subtle))] text-[hsl(var(--accent))] font-medium border-l-[hsl(var(--accent))]"
                            : "text-[hsl(var(--text-tertiary))] border-l-transparent hover:text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--bg-hover))]"
                          }
                        `}
                      >
                        <Icon size={17} className={`shrink-0 ${active ? "opacity-100" : "opacity-70"}`} />
                        {!collapsed && (
                          <span className="text-[13px] truncate">
                            {item.label}
                          </span>
                        )}
                      </button>
                    );
                  })}
                </div>
              </div>
            ))}
          </nav>

          {/* Collapse toggle */}
          <div className="border-t border-[hsl(var(--border-light))] p-2">
            <button
              onClick={() => setCollapsed(!collapsed)}
              className={`w-full flex items-center gap-2 text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--bg-hover))] rounded-md transition-colors ${collapsed ? "justify-center h-9" : "px-3 h-8"}`}
            >
              <ChevronLeft
                size={14}
                style={{ transform: collapsed ? "rotate(180deg)" : "none", transition: "transform 0.2s" }}
              />
              {!collapsed && <span className="text-[12px]">收起菜单</span>}
            </button>
          </div>
        </aside>

        {/* ====== Content ====== */}
        <main className="flex-1 overflow-auto" style={{ backgroundColor: "hsl(var(--bg-content))" }}>
          <div className="animate-in p-6">
            <Outlet />
          </div>
        </main>
      </div>

      {/* ====== Status bar ====== */}
      <footer
        className="h-7 shrink-0 flex items-center px-4 text-[11px] gap-3 select-none border-t border-[hsl(var(--border-light))] text-[hsl(var(--text-tertiary))]"
        style={{ backgroundColor: "hsl(var(--bg-app))" }}
      >
        <span className="flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 shadow-sm shadow-emerald-500/40" />
          {statusMsg}
        </span>
        <span className="flex-1" />
        <span>v3.1</span>
      </footer>
    </div>
  );
}
