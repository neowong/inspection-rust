import { useState, useEffect } from "react";
import { Outlet, useNavigate, useLocation } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";

type PageKey = "dashboard" | "devices" | "templates" | "inspection" | "reports" | "ai-config" | "settings";

const NAV_ITEMS: { key: PageKey; label: string; icon: string; path: string }[] = [
  { key: "dashboard",   label: "仪表盘",   icon: "◫", path: "/" },
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
  const appWindow = getCurrentWindow();

  const pathSegments = location.pathname.split("/").filter(Boolean);
  const activeNav = NAV_ITEMS.find(item => item.path === location.pathname);

  useEffect(() => {
    const handler = (e: Event) => setStatusMsg((e as CustomEvent).detail);
    window.addEventListener("statusbar-message", handler);
    return () => window.removeEventListener("statusbar-message", handler);
  }, []);

  return (
    <div className="h-screen flex flex-col overflow-hidden" style={{ background: "#f0f2f5" }}>
      {/* ========== 自定义标题栏 ========== */}
      <header
        className="titlebar-drag h-9 shrink-0 flex items-center px-3 gap-2 select-none"
        style={{
          background: "linear-gradient(135deg, #0f172a 0%, #1e293b 100%)",
          color: "#e2e8f0",
        }}
      >
        {/* App icon + name */}
        <div className="flex items-center gap-2 titlebar-no-drag">
          <span className="text-sm font-bold tracking-wide text-blue-400">INSPECT</span>
          <span className="text-[11px] text-slate-400">|</span>
          <span className="text-[11px] text-slate-300">网络设备巡检系统</span>
        </div>

        {/* Menu area */}
        <div className="flex-1 flex items-center gap-0.5 ml-6 titlebar-no-drag">
          {["设备", "巡检", "报告", "帮助"].map(label => (
            <button
              key={label}
              className="px-3 py-1 text-[11px] text-slate-400 hover:text-white hover:bg-white/10 rounded transition-colors"
            >
              {label}
            </button>
          ))}
        </div>

        {/* Window controls */}
        <div className="flex items-center titlebar-no-drag">
          <button
            onClick={() => appWindow.minimize()}
            className="w-8 h-8 flex items-center justify-center text-slate-400 hover:text-white hover:bg-white/10 transition-colors"
          >
            <svg width="10" height="1"><line x1="0" y1="0.5" x2="10" y2="0.5" stroke="currentColor" strokeWidth="1.2"/></svg>
          </button>
          <button
            onClick={() => appWindow.toggleMaximize()}
            className="w-8 h-8 flex items-center justify-center text-slate-400 hover:text-white hover:bg-white/10 transition-colors"
          >
            <svg width="10" height="10"><rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" strokeWidth="1.2"/></svg>
          </button>
          <button
            onClick={() => appWindow.close()}
            className="w-8 h-8 flex items-center justify-center text-slate-400 hover:text-white hover:bg-red-500/80 transition-colors"
          >
            <svg width="10" height="10"><line x1="0" y1="0" x2="10" y2="10" stroke="currentColor" strokeWidth="1.2"/><line x1="10" y1="0" x2="0" y2="10" stroke="currentColor" strokeWidth="1.2"/></svg>
          </button>
        </div>
      </header>

      <div className="flex flex-1 overflow-hidden">
        {/* ========== 侧边栏 ========== */}
        <nav
          className={`${collapsed ? "w-14" : "w-52"} shrink-0 flex flex-col transition-all duration-200`}
          style={{
            background: "linear-gradient(180deg, #0f172a 0%, #1e293b 50%, #0f172a 100%)",
          }}
        >
          {/* Nav header */}
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

          {/* Sidebar footer */}
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
          {/* Breadcrumb */}
          {activeNav && (
            <div className="flex items-center gap-1.5 mb-3 text-[11px] text-slate-400">
              <span className="text-slate-300">◫</span>
              <span>/</span>
              <span className="text-slate-600 font-medium">{activeNav.label}</span>
              {pathSegments.length > 1 && (
                <>
                  <span>/</span>
                  <span className="text-slate-500">{pathSegments[pathSegments.length - 1]}</span>
                </>
              )}
            </div>
          )}
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
