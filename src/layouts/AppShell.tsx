import { useState, useEffect, useMemo } from "react";
import { Outlet, useNavigate, useLocation } from "react-router-dom";

type PageKey = "devices" | "templates" | "inspection" | "reports" | "ai-config" | "settings";

const NAV_ITEMS = [
  { key: "devices" as const,    label: "设备管理", path: "/devices" },
  { key: "templates" as const,  label: "巡检模板", path: "/templates" },
  { key: "inspection" as const, label: "执行巡检", path: "/inspection" },
  { key: "reports" as const,    label: "巡检报告", path: "/reports" },
  { key: "ai-config" as const,  label: "AI 配置",  path: "/ai-config" },
  { key: "settings" as const,   label: "系统设置", path: "/settings" },
];

/* ---- Inline SVG icons (16x16, currentColor) ---- */
const Icon = ({ children }: { children: React.ReactNode }) => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
    {children}
  </svg>
);

const NAV_ICONS: Record<PageKey, React.ReactNode> = {
  devices:    <Icon><rect x="1" y="2" width="6" height="5" rx="0.8"/><rect x="9" y="2" width="6" height="5" rx="0.8"/><rect x="1" y="9" width="6" height="5" rx="0.8"/><rect x="9" y="9" width="6" height="5" rx="0.8"/></Icon>,
  templates:  <Icon><path d="M2 3h5l2 3h5v7H2V3z"/></Icon>,
  inspection: <Icon><polygon points="4,2 14,8 4,14"/></Icon>,
  reports:    <Icon><rect x="2" y="1" width="9" height="11" rx="1"/><line x1="5" y1="5" x2="8" y2="5"/><line x1="5" y1="8" x2="8" y2="8"/></Icon>,
  "ai-config": <Icon><circle cx="8" cy="8" r="3"/><line x1="8" y1="1" x2="8" y2="5"/><line x1="8" y1="11" x2="8" y2="15"/><line x1="1" y1="8" x2="5" y2="8"/><line x1="11" y1="8" x2="15" y2="8"/></Icon>,
  settings:   <Icon><circle cx="8" cy="8" r="2.5"/><path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4"/></Icon>,
};

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
    <div className="h-screen flex flex-col overflow-hidden bg-[#f5f6f8]">
      <div className="flex flex-1 overflow-hidden">
        {/* ====== Sidebar ====== */}
        <aside
          className={`${collapsed ? "w-[56px]" : "w-[212px]"} shrink-0 flex flex-col transition-[width] duration-200 ease-out relative`}
          style={{ background: "linear-gradient(180deg, #111827 0%, #1f2937 100%)" }}
        >
          {/* Brand */}
          <div className={`flex items-center gap-3 px-4 h-12 border-b border-white/[0.06] ${collapsed ? "justify-center" : ""}`}>
            <div className="w-7 h-7 rounded-md bg-gradient-to-br from-blue-500 to-blue-600 flex items-center justify-center shrink-0 shadow-sm shadow-blue-500/25">
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="white" strokeWidth="2" strokeLinecap="round">
                <circle cx="4" cy="7" r="2"/><circle cx="10" cy="7" r="2"/><line x1="6" y1="7" x2="8" y2="7"/>
              </svg>
            </div>
            {!collapsed && <span className="text-sm font-semibold text-white tracking-tight">Inspect</span>}
          </div>

          {/* Nav */}
          <nav className="flex-1 py-3 px-2 space-y-0.5">
            {NAV_ITEMS.map(item => {
              const active = activeKey === item.key;
              return (
                <button
                  key={item.key}
                  onClick={() => navigate(item.path)}
                  title={collapsed ? item.label : undefined}
                  className={`
                    flex items-center gap-3 w-full rounded-md transition-all duration-150 select-none
                    ${collapsed ? "px-0 justify-center h-9" : "px-3 h-8"}
                    ${active
                      ? "bg-white/[0.08] text-white"
                      : "text-white/40 hover:text-white/70 hover:bg-white/[0.04]"
                    }
                  `}
                >
                  <span className={`shrink-0 ${active ? "text-blue-400" : ""}`}>
                    {NAV_ICONS[item.key]}
                  </span>
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
          <div className="border-t border-white/[0.06] p-2">
            <button
              onClick={() => setCollapsed(!collapsed)}
              className={`w-full flex items-center gap-2 text-white/30 hover:text-white/60 hover:bg-white/[0.04] rounded-md transition-colors ${collapsed ? "justify-center h-9" : "px-3 h-8"}`}
            >
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"
                style={{ transform: collapsed ? "rotate(180deg)" : "none", transition: "transform 0.2s" }}>
                <polyline points="9,2 4,7 9,12"/>
              </svg>
              {!collapsed && <span className="text-[12px]">收起菜单</span>}
            </button>
          </div>
        </aside>

        {/* ====== Content ====== */}
        <main className="flex-1 overflow-auto">
          <div className="animate-in p-5">
            <Outlet />
          </div>
        </main>
      </div>

      {/* ====== Status bar ====== */}
      <footer className="h-[26px] shrink-0 flex items-center px-4 text-[11px] gap-3 select-none bg-white border-t border-gray-200 text-gray-400">
        <span className="flex items-center gap-1.5">
          <span className="w-[6px] h-[6px] rounded-full bg-emerald-400 shadow-sm shadow-emerald-400/50" />
          {statusMsg}
        </span>
        <span className="flex-1" />
        <span className="text-gray-300">v3.1</span>
      </footer>
    </div>
  );
}
