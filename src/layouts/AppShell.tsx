import { useState, useEffect } from "react";
import { Outlet, useNavigate, useLocation } from "react-router-dom";

type PageKey = "devices" | "templates" | "inspection" | "reports" | "ai-config" | "settings";

const NAV_ITEMS: { key: PageKey; label: string; icon: string; path: string }[] = [
  { key: "devices",     label: "设备管理", icon: "⊞", path: "/devices" },
  { key: "templates",   label: "巡检模板", icon: "⊟", path: "/templates" },
  { key: "inspection",  label: "执行巡检", icon: "▶", path: "/inspection" },
  { key: "reports",     label: "巡检报告", icon: "▤", path: "/reports" },
  { key: "ai-config",   label: "AI 配置",  icon: "◆", path: "/ai-config" },
  { key: "settings",    label: "系统设置", icon: "⚙", path: "/settings" },
];

export default function AppShell() {
  const navigate = useNavigate();
  const location = useLocation();
  const [collapsed, setCollapsed] = useState(false);
  const [statusMsg, setStatusMsg] = useState("就绪");

  const activeNav = NAV_ITEMS.find(item => location.pathname.startsWith(item.path));

  useEffect(() => {
    const handler = (e: Event) => setStatusMsg((e as CustomEvent).detail);
    window.addEventListener("statusbar-message", handler);
    return () => window.removeEventListener("statusbar-message", handler);
  }, []);

  return (
    <div className="h-screen flex flex-col overflow-hidden" style={{ background: "#f0f2f5" }}>
      <div className="flex flex-1 overflow-hidden">
        {/* ========== 侧边栏 ========== */}
        <nav
          className={`${collapsed ? "w-14" : "w-52"} shrink-0 flex flex-col transition-all duration-200`}
          style={{
            background: "linear-gradient(180deg, #0f172a 0%, #1e293b 50%, #0f172a 100%)",
          }}
        >
          {/* Header */}
          <div className={`px-4 py-4 ${collapsed ? "text-center" : ""}`}>
            {collapsed ? (
              <span className="text-blue-400 font-bold text-sm">IN</span>
            ) : (
              <div>
                <div className="text-[10px] uppercase tracking-widest text-slate-500 mb-1">Navigation</div>
                <div className="text-xs text-slate-400">巡检管理</div>
              </div>
            )}
          </div>

          {/* Nav items */}
          <div className="flex-1 flex flex-col gap-0.5 px-2">
            {NAV_ITEMS.map(item => {
              const isActive = activeNav?.key === item.key;
              return (
                <button
                  key={item.key}
                  onClick={() => navigate(item.path)}
                  title={collapsed ? item.label : undefined}
                  className={`
                    flex items-center gap-3 px-3 py-2 text-[12px] rounded-lg transition-all duration-150
                    ${isActive
                      ? "bg-gradient-to-r from-blue-600/30 to-blue-500/10 text-white shadow-sm shadow-blue-500/10 border border-blue-500/20"
                      : "text-slate-400 hover:text-slate-200 hover:bg-white/5 border border-transparent"
                    }
                  `}
                >
                  <span className={`text-sm shrink-0 ${isActive ? "text-blue-400" : "text-slate-500"}`}>
                    {item.icon}
                  </span>
                  {!collapsed && <span className="truncate">{item.label}</span>}
                  {!collapsed && isActive && (
                    <span className="ml-auto w-1.5 h-1.5 rounded-full bg-blue-400 shadow-sm shadow-blue-400/50" />
                  )}
                </button>
              );
            })}
          </div>

          {/* Footer */}
          <div className={`px-3 py-3 border-t border-slate-800 ${collapsed ? "text-center" : ""}`}>
            <button
              onClick={() => setCollapsed(!collapsed)}
              className={`w-full flex items-center gap-2 px-2 py-1.5 text-[11px] text-slate-500 hover:text-slate-300 rounded transition-colors ${collapsed ? "justify-center" : ""}`}
            >
              <span className="text-sm">{collapsed ? "▸" : "◂"}</span>
              {!collapsed && <span>收起菜单</span>}
            </button>
          </div>
        </nav>

        {/* ========== 内容区域 ========== */}
        <main className="flex-1 overflow-auto p-4" style={{ background: "#f0f2f5" }}>
          <div className="animate-in">
            <Outlet />
          </div>
        </main>
      </div>

      {/* ========== 状态栏 ========== */}
      <footer
        className="h-7 shrink-0 flex items-center px-4 text-[11px] gap-4 select-none border-t"
        style={{ background: "#f8fafc", borderColor: "#e2e8f0", color: "#64748b" }}
      >
        <div className="flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
          <span>{statusMsg}</span>
        </div>
        <span className="flex-1" />
        <span className="text-slate-400">v3.1.0</span>
      </footer>
    </div>
  );
}
