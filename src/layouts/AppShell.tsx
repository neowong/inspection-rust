import { useState, useEffect, useMemo } from "react";
import { Outlet, useNavigate, useLocation } from "react-router-dom";
import { LayoutDashboard, FolderTree, Server, Play, Settings, FileSearch, FileText, Wrench, Info, MessageSquare, PanelLeftClose, PanelLeft } from "lucide-react";

type PageKey = "dashboard" | "templates" | "devices" | "inspection" | "reports" | "tools" | "logs" | "settings" | "about" | "chat";

interface NavItem {
  key: PageKey;
  label: string;
  path: string;
  icon: typeof FolderTree;
}

const NAV_GROUPS: { label?: string; items: NavItem[] }[] = [
  {
    label: "信息概览",
    items: [
      { key: "dashboard",  label: "仪表盘",   path: "/dashboard",  icon: LayoutDashboard },
    ],
  },
  {
    label: "巡检工作流",
    items: [
      { key: "templates",  label: "巡检模板", path: "/templates",  icon: FolderTree },
      { key: "devices",    label: "设备管理", path: "/devices",    icon: Server },
      { key: "inspection", label: "执行巡检", path: "/inspection", icon: Play },
      { key: "reports",    label: "报告管理", path: "/reports",    icon: FileText },
    ],
  },
  {
    label: "运维工具",
    items: [
      { key: "tools",      label: "工具箱", path: "/tools",      icon: Wrench },
      { key: "logs",       label: "日志分析",   path: "/logs",       icon: FileSearch },
    ],
  },
  {
    label: "系统",
    items: [
      { key: "settings",  label: "系统设置", path: "/settings",  icon: Settings },
      { key: "about",     label: "关于",     path: "/about",     icon: Info },
    ],
  },
];

const FLAT_ITEMS = [
  ...NAV_GROUPS.flatMap(g => g.items),
  { key: "chat" as PageKey, label: "对话模式", path: "/chat", icon: MessageSquare },
];

export default function AppShell() {
  const navigate = useNavigate();
  const location = useLocation();
  const [collapsed, setCollapsed] = useState(false);
  const [hint, setHint] = useState<{ text: string; level: "info" | "warn" | "error" | "success" } | null>(null);
  const [updateVersion, setUpdateVersion] = useState<string | null>(null);

  const activeKey = useMemo(
    () => FLAT_ITEMS.find(item => location.pathname.startsWith(item.path))?.key ?? null,
    [location.pathname]
  );

  // 启动时获取版本号并检查更新
  useEffect(() => {
    const checkUpdate = async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const currentVersion = await invoke<string>("get_app_version");
        const result = await invoke<{ version: string; url: string } | null>("check_update", {
          currentVersion,
        });
        if (result) {
          setUpdateVersion(result.version);
        }
      } catch {
        // 静默忽略
      }
    };
    checkUpdate();
  }, []);

  // 临时提示标签：8 秒后自动消失，level 决定颜色
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail as
        | string
        | { text: string; level?: "info" | "warn" | "error" | "success"; durationMs?: number };
      const data = typeof detail === "string" ? { text: detail } : detail;
      setHint({ text: data.text, level: data.level ?? "info" });
      if (timer) clearTimeout(timer);
      const dur = (typeof detail === "object" && detail.durationMs) || 8000;
      timer = setTimeout(() => setHint(null), dur);
    };
    window.addEventListener("statusbar-hint", handler);
    return () => {
      window.removeEventListener("statusbar-hint", handler);
      if (timer) clearTimeout(timer);
    };
  }, []);

  const sidebarBg = "hsl(var(--sidebar-bg))";
  const sidebarActive = "hsl(var(--sidebar-active))";

  return (
    <div className="h-screen flex flex-col overflow-hidden" style={{ backgroundColor: "hsl(var(--bg-content))" }}>
      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar — dark navy */}
        <aside
          className={`${collapsed ? "w-[56px]" : "w-[228px]"} shrink-0 flex flex-col transition-[width] duration-200 ease-out`}
          style={{ backgroundColor: sidebarBg }}
        >

          {/* Brand */}
          <div className={`flex items-center h-14 border-b px-3 gap-2 ${collapsed ? "justify-center" : ""}`}
            style={{ borderColor: "hsl(var(--sidebar-hover))" }}>
            <img
              src="/router.svg"
              alt="AI巡检助手"
              className="h-9 w-9 object-contain shrink-0"
            />
            {!collapsed && <span className="text-base font-bold text-white truncate">AI巡检助手</span>}
          </div>

          {/* Nav groups */}
          <nav className="flex-1 py-3 overflow-y-auto sidebar-scroll">
            {NAV_GROUPS.map((group, gi) => (
              <div key={gi} className={gi > 0 ? "mt-3" : ""}>
                {!collapsed && group.label && (
                  <div className="px-4 pt-2 pb-1.5 text-[10px] font-semibold uppercase tracking-widest"
                    style={{ color: "hsl(var(--sidebar-text-muted))" }}>
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
                        className={`flex items-center gap-3 w-full select-none transition-all duration-150 rounded-lg
                          ${collapsed ? "px-0 justify-center h-10" : "px-3 h-9"}
                          ${active
                            ? "font-medium"
                            : "hover:bg-[hsl(var(--sidebar-hover))]"
                          }`}
                        style={active
                          ? { backgroundColor: sidebarActive, color: "hsl(var(--accent-foreground))" }
                          : { color: "hsl(var(--sidebar-text-muted))" }
                        }
                      >
                        <Icon size={18} className="shrink-0" />
                        {!collapsed && <span className="text-[13px] truncate">{item.label}</span>}
                      </button>
                    );
                  })}
                </div>
              </div>
            ))}
          </nav>

          {/* 底部：对话模式 + 收起按钮 */}
          <div className="px-2 pb-2 space-y-1" style={{ borderColor: "hsl(var(--sidebar-hover))", borderTopWidth: 1 }}>
            {/* 对话模式 */}
            <button
              onClick={() => navigate("/chat")}
              title={collapsed ? "对话模式" : undefined}
              className={`flex items-center gap-3 w-full select-none transition-all duration-150 rounded-lg hover:bg-[hsl(var(--sidebar-hover))]
                ${collapsed ? "px-0 justify-center h-10" : "px-3 h-9 mt-2"}`}
              style={{ color: "hsl(var(--sidebar-text-muted))" }}
            >
              <MessageSquare size={18} className="shrink-0" />
              {!collapsed && <span className="text-[13px] truncate">对话模式</span>}
            </button>
            {/* 收起/展开 */}
            <button
              onClick={() => setCollapsed(!collapsed)}
              title={collapsed ? "展开菜单" : "收起菜单"}
              className="flex items-center justify-center w-full h-7 rounded-md transition-colors hover:bg-[hsl(var(--sidebar-hover))]"
              style={{ color: "hsl(var(--sidebar-text-muted))" }}
            >
              {collapsed ? <PanelLeft size={15} /> : <PanelLeftClose size={15} />}
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
      <footer
        className="h-7 shrink-0 flex items-center px-4 text-[11px] gap-3 select-none"
        style={{ backgroundColor: sidebarBg, color: "hsl(var(--sidebar-text-muted))", borderColor: "hsl(var(--sidebar-hover))", borderTopWidth: 1 }}
      >
        <span className="flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full shadow-sm" style={{ backgroundColor: "hsl(var(--success))" }} />
          就绪
        </span>
        {hint && (
          <span
            className="px-2 py-[1px] rounded text-[10px] font-medium animate-in cursor-pointer"
            style={{
              backgroundColor:
                hint.level === "error"   ? "hsl(var(--danger) / 0.18)" :
                hint.level === "warn"    ? "hsl(45 93% 50% / 0.18)"   :
                hint.level === "success" ? "hsl(var(--success) / 0.18)" :
                                           "hsl(var(--accent) / 0.18)",
              color:
                hint.level === "error"   ? "hsl(var(--danger))" :
                hint.level === "warn"    ? "hsl(45 93% 65%)"    :
                hint.level === "success" ? "hsl(var(--success))" :
                                           "hsl(var(--accent))",
            }}
            title="点击关闭"
            onClick={() => setHint(null)}
          >
            {hint.text}
          </span>
        )}
        <span className="flex-1" />
        {updateVersion && (
          <button
            onClick={() => navigate("/about")}
            className="flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-medium animate-in cursor-pointer hover:opacity-80 transition-opacity"
            style={{ backgroundColor: "hsl(var(--accent) / 0.15)", color: "hsl(var(--accent))" }}
            title="点击查看详情"
          >
            🆕 v{updateVersion}
          </button>
        )}
      </footer>
    </div>
  );
}
