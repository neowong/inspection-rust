import { useState, useEffect } from "react";
import { Outlet, useNavigate, useLocation } from "react-router-dom";

type PageKey = "dashboard" | "devices" | "templates" | "inspection" | "reports" | "ai-config" | "settings";

const NAV_ITEMS: { key: PageKey; label: string; icon: string; path: string }[] = [
  { key: "dashboard",   label: "仪表盘",   icon: "📊", path: "/" },
  { key: "devices",     label: "设备管理", icon: "📡", path: "/devices" },
  { key: "templates",   label: "巡检模板", icon: "📋", path: "/templates" },
  { key: "inspection",  label: "执行巡检", icon: "🔍", path: "/inspection" },
  { key: "reports",     label: "巡检报告", icon: "📄", path: "/reports" },
  { key: "ai-config",   label: "AI 配置",  icon: "🤖", path: "/ai-config" },
  { key: "settings",    label: "系统设置", icon: "⚙️", path: "/settings" },
];

export default function AppShell() {
  const navigate = useNavigate();
  const location = useLocation();
  const [collapsed, setCollapsed] = useState(false);
  const [statusMsg, setStatusMsg] = useState("就绪");

  const activeKey = NAV_ITEMS.find(item => item.path === location.pathname)?.key ?? "dashboard";

  useEffect(() => {
    const handler = (e: Event) => setStatusMsg((e as CustomEvent).detail);
    window.addEventListener("statusbar-message", handler);
    return () => window.removeEventListener("statusbar-message", handler);
  }, []);

  return (
    <div className="h-screen flex flex-col bg-gray-100 text-gray-900 select-none">
      {/* 菜单栏 */}
      <header className="h-7 bg-gray-200 border-b border-gray-300 flex items-center px-2 text-xs gap-1 shrink-0">
        <span className="font-semibold mr-2">网络设备巡检系统</span>
        <button className="px-2 hover:bg-gray-300 rounded" onClick={() => navigate("/devices")}>设备</button>
        <button className="px-2 hover:bg-gray-300 rounded" onClick={() => navigate("/inspection")}>巡检</button>
        <button className="px-2 hover:bg-gray-300 rounded" onClick={() => navigate("/reports")}>报告</button>
        <span className="flex-1" />
        <button className="px-2 hover:bg-gray-300 rounded" onClick={() => setCollapsed(!collapsed)}>
          {collapsed ? "☰" : "☰"}
        </button>
      </header>

      <div className="flex flex-1 overflow-hidden">
        {/* 侧边栏 */}
        <nav className={`${collapsed ? "w-12" : "w-40"} bg-gray-800 text-gray-200 shrink-0 transition-all flex flex-col pt-1`}>
          {NAV_ITEMS.map(item => (
            <button
              key={item.key}
              onClick={() => navigate(item.path)}
              className={`flex items-center gap-2 px-3 py-2 text-xs hover:bg-gray-700 transition-colors
                ${activeKey === item.key ? "bg-gray-700 text-white border-l-2 border-blue-400" : ""}`}
            >
              <span className="text-base shrink-0">{item.icon}</span>
              {!collapsed && <span className="truncate">{item.label}</span>}
            </button>
          ))}
        </nav>

        {/* 内容区域 */}
        <main className="flex-1 overflow-auto p-3">
          <Outlet />
        </main>
      </div>

      {/* 状态栏 */}
      <footer className="h-6 bg-gray-200 border-t border-gray-300 flex items-center px-3 text-xs text-gray-600 shrink-0 gap-3">
        <span>✅ {statusMsg}</span>
        <span className="flex-1" />
        <span>v3.1.0</span>
      </footer>
    </div>
  );
}
