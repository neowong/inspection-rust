import { useState, useEffect, useMemo } from "react";
import { Outlet, useNavigate, useLocation } from "react-router-dom";
import {
  LayoutDashboard, FolderTree, Server, Play, Settings, FileSearch, FileText, Wrench, Info,
  Bot, Plus, MessageCircle, Trash2, RotateCw,
} from "lucide-react";
import { loadSessions, deleteSession } from "../pages/ChatPage";
import type { ChatSession } from "../pages/ChatPage";

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

const FLAT_ITEMS = NAV_GROUPS.flatMap(g => g.items);

// ── 按日期分组 ──
function groupByDate(sessions: ChatSession[]): { label: string; items: ChatSession[] }[] {
  const today = new Date().toISOString().slice(0, 10);
  const yesterday = new Date(Date.now() - 86400000).toISOString().slice(0, 10);

  const byLabel: Record<string, ChatSession[]> = { "今天": [], "昨天": [], "更早": [] };

  for (const s of sessions) {
    const d = (s.updatedAt || s.createdAt || "").slice(0, 10);
    if (d === today) byLabel["今天"]!.push(s);
    else if (d === yesterday) byLabel["昨天"]!.push(s);
    else byLabel["更早"]!.push(s);
  }

  return ["今天", "昨天", "更早"]
    .filter(l => byLabel[l]!.length > 0)
    .map(l => ({ label: l, items: byLabel[l]! }));
}

export default function AppShell() {
  const navigate = useNavigate();
  const location = useLocation();
  const [collapsed, setCollapsed] = useState(false);
  const [navMode, setNavMode] = useState(() => {
    // 默认传统模式，仅当用户之前明确选择过 AI 模式时恢复
    return localStorage.getItem("sidebar_mode") !== "chat";
  });
  const [hint, setHint] = useState<{ text: string; level: "info" | "warn" | "error" | "success" } | null>(null);
  const [updateVersion, setUpdateVersion] = useState<string | null>(null);
  const [sessions, setSessions] = useState<ChatSession[]>([]);

  // 持久化 navMode
  useEffect(() => {
    localStorage.setItem("sidebar_mode", navMode ? "nav" : "chat");
  }, [navMode]);

  // 刷新会话列表
  const refreshSessions = () => setSessions(loadSessions());
  useEffect(() => { refreshSessions(); }, [location.pathname, location.search]);

  // 版本更新检查
  useEffect(() => {
    const checkUpdate = async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const currentVersion = await invoke<string>("get_app_version");
        const result = await invoke<{ version: string; url: string } | null>("check_update", { currentVersion });
        if (result) setUpdateVersion(result.version);
      } catch { /* ignore */ }
    };
    checkUpdate();
  }, []);

  // 状态栏提示
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail as string | { text: string; level?: string; durationMs?: number };
      const data = typeof detail === "string" ? { text: detail } : detail;
      setHint({ text: data.text, level: (data.level as any) || "info" });
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => setHint(null), data.durationMs || 8000);
    };
    window.addEventListener("statusbar-hint", handler);
    return () => { window.removeEventListener("statusbar-hint", handler); if (timer) clearTimeout(timer); };
  }, []);

  const activeKey = useMemo(
    () => FLAT_ITEMS.find(item => location.pathname.startsWith(item.path))?.key ?? null,
    [location.pathname]
  );

  const sidebarBg = "hsl(var(--sidebar-bg))";
  const sidebarActive = "hsl(var(--sidebar-active))";
  const groupedSessions = useMemo(() => groupByDate(sessions), [sessions]);
  const currentChatId = location.pathname.startsWith("/chat") ? new URLSearchParams(location.search).get("id") : null;

  const newChat = () => {
    navigate("/chat");
    refreshSessions();
  };

  return (
    <div className="h-screen flex flex-col overflow-hidden" style={{ backgroundColor: "hsl(var(--bg-content))" }}>
      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <aside
          className={`${collapsed ? "w-[56px]" : "w-[260px]"} shrink-0 flex flex-col transition-[width] duration-200 ease-out`}
          style={{ backgroundColor: sidebarBg }}
        >
          {/* Brand — 点击收起 */}
          <div
            onClick={() => setCollapsed(!collapsed)}
            className={`flex items-center h-14 border-b px-3 gap-2 cursor-pointer select-none transition-colors hover:bg-[hsl(var(--sidebar-hover))]
              ${collapsed ? "justify-center" : ""}`}
            style={{ borderColor: "hsl(var(--sidebar-hover))" }}
            title={collapsed ? "展开菜单" : "收起菜单"}
          >
            <img src="/router.svg" alt="AI巡检助手" className="h-9 w-9 object-contain shrink-0" />
            {!collapsed && <span className="text-base font-bold text-white truncate">AI巡检助手</span>}
          </div>

          {collapsed ? (
            /* ────── 收起模式：根据模式显示图标 ────── */
            navMode ? (
              <nav className="flex flex-col items-center py-3 gap-2 flex-1">
                {FLAT_ITEMS.map(item => {
                  const active = activeKey === item.key;
                  const Icon = item.icon;
                  return (
                    <button key={item.key} onClick={() => navigate(item.path)}
                      className="w-10 h-10 rounded-lg flex items-center justify-center hover:bg-[hsl(var(--sidebar-hover))]"
                      style={{ color: active ? "hsl(var(--accent))" : "hsl(var(--sidebar-text-muted))" }}
                      title={item.label}>
                      <Icon size={20} />
                    </button>
                  );
                })}
                <div className="flex-1" />
                <button onClick={() => { setNavMode(false); navigate("/chat"); }}
                  className="w-10 h-10 rounded-lg flex items-center justify-center hover:bg-[hsl(var(--sidebar-hover))]"
                  style={{ color: "hsl(var(--sidebar-text-muted))" }} title="AI模式">
                  <Bot size={18} />
                </button>
              </nav>
            ) : (
              <div className="flex flex-col items-center py-3 gap-1.5 flex-1">
                <button onClick={newChat} className="w-10 h-10 rounded-lg flex items-center justify-center hover:bg-[hsl(var(--sidebar-hover))]"
                  style={{ color: "hsl(var(--sidebar-text-muted))" }} title="新对话">
                  <Plus size={20} />
                </button>
                <div className="w-6 border-t my-1" style={{ borderColor: "hsl(var(--sidebar-hover))" }} />
                {groupedSessions.flatMap(g => g.items).slice(0, 10).map(s => (
                  <button key={s.id} onClick={() => navigate(`/chat?id=${s.id}`)}
                    className="w-10 h-10 rounded-lg flex items-center justify-center hover:bg-[hsl(var(--sidebar-hover))]"
                    style={{ color: s.id === currentChatId ? "hsl(var(--accent))" : "hsl(var(--sidebar-text-muted))" }}
                    title={s.title}>
                    <MessageCircle size={18} />
                  </button>
                ))}
                <div className="flex-1" />
                <button onClick={() => { setNavMode(true); navigate("/dashboard"); }}
                  className="w-10 h-10 rounded-lg flex items-center justify-center hover:bg-[hsl(var(--sidebar-hover))]"
                  style={{ color: "hsl(var(--sidebar-text-muted))" }} title="传统模式">
                  <RotateCw size={16} />
                </button>
              </div>
            )
          ) : navMode ? (
            /* ────── Nav 模式：传统导航 ────── */
            <>
              <nav className="flex-1 py-3 overflow-y-auto sidebar-scroll">
                {NAV_GROUPS.map((group, gi) => (
                  <div key={gi} className={gi > 0 ? "mt-3" : ""}>
                    {group.label && (
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
                            className={`flex items-center gap-3 w-full select-none transition-all duration-150 rounded-lg
                              px-3 h-9 ${active ? "font-medium" : "hover:bg-[hsl(var(--sidebar-hover))]"}`}
                            style={active
                              ? { backgroundColor: sidebarActive, color: "hsl(var(--accent-foreground))" }
                              : { color: "hsl(var(--sidebar-text-muted))" }
                            }
                          >
                            <Icon size={18} className="shrink-0" />
                            <span className="text-[13px] truncate">{item.label}</span>
                          </button>
                        );
                      })}
                    </div>
                  </div>
                ))}
              </nav>
              {/* 底部：切换到 AI模式 */}
              <div className="px-2 pb-2" style={{ borderColor: "hsl(var(--sidebar-hover))", borderTopWidth: 1 }}>
                <button onClick={() => { setNavMode(false); navigate("/chat"); }}
                  className="flex items-center gap-3 w-full px-3 h-9 mt-2 rounded-lg text-[13px] transition-colors hover:bg-[hsl(var(--sidebar-hover))]"
                  style={{ color: "hsl(var(--sidebar-text-muted))" }}>
                  <Bot size={18} />
                  <span className="truncate">AI模式</span>
                </button>
              </div>
            </>
          ) : (
            /* ────── Chat 模式：聊天历史 ────── */
            <>
              {/* 新对话按钮 */}
              <div className="px-2 pt-3 pb-2">
                <button onClick={newChat}
                  className="flex items-center gap-3 w-full px-3 h-9 rounded-lg text-[13px] transition-colors
                    hover:bg-[hsl(var(--sidebar-hover))]"
                  style={{ color: "hsl(var(--sidebar-text-muted))", border: "1px solid hsl(var(--sidebar-hover))" }}>
                  <Plus size={16} />
                  <span>新对话</span>
                </button>
              </div>

              {/* 聊天历史 */}
              <nav className="flex-1 overflow-y-auto sidebar-scroll px-2 pb-2 space-y-1">
                {groupedSessions.length === 0 && (
                  <p className="text-[12px] px-3 pt-4 text-center" style={{ color: "hsl(var(--sidebar-text-muted))" }}>
                    暂无对话历史
                  </p>
                )}
                {groupedSessions.map((group, gi) => (
                  <div key={gi}>
                    <div className="px-3 pt-2 pb-1 text-[11px] font-semibold" style={{ color: "hsl(var(--sidebar-text-muted))" }}>
                      {group.label}
                    </div>
                    {group.items.map(s => {
                      const isActive = s.id === currentChatId;
                      return (
                        <div key={s.id} className="group relative">
                          <button
                            onClick={() => navigate(`/chat?id=${s.id}`)}
                            className={`flex items-center gap-2 w-full px-3 py-2 rounded-lg text-left text-[13px] transition-colors
                              ${isActive ? "font-medium" : "hover:bg-[hsl(var(--sidebar-hover))]"}`}
                            style={isActive
                              ? { backgroundColor: sidebarActive, color: "hsl(var(--sidebar-text))" }
                              : { color: "hsl(var(--sidebar-text-muted))" }
                            }
                          >
                            <MessageCircle size={14} className="shrink-0" />
                            <span className="truncate">{s.title}</span>
                          </button>
                          <button
                            onClick={() => { deleteSession(s.id); refreshSessions(); if (isActive) navigate("/chat"); }}
                            className="absolute right-1 top-1/2 -translate-y-1/2 w-6 h-6 rounded flex items-center justify-center
                              opacity-0 group-hover:opacity-100 transition-opacity hover:bg-[hsl(var(--sidebar-hover) / 0.8)]"
                            style={{ color: "hsl(var(--danger))" }}
                            title="删除对话"
                          >
                            <Trash2 size={12} />
                          </button>
                        </div>
                      );
                    })}
                  </div>
                ))}
              </nav>

              {/* 底部：切换到传统模式 */}
              <div className="px-2 pb-2" style={{ borderColor: "hsl(var(--sidebar-hover))", borderTopWidth: 1 }}>
                <button onClick={() => { setNavMode(true); navigate("/dashboard"); }}
                  className="flex items-center gap-3 w-full px-3 h-9 mt-2 rounded-lg text-[13px] transition-colors hover:bg-[hsl(var(--sidebar-hover))]"
                  style={{ color: "hsl(var(--sidebar-text-muted))" }}>
                  <RotateCw size={16} />
                  <span className="truncate">传统模式</span>
                </button>
              </div>
            </>
          )}
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
          <span className="px-2 py-[1px] rounded text-[10px] font-medium animate-in cursor-pointer"
            style={{
              backgroundColor: hint.level === "error" ? "hsl(var(--danger) / 0.18)" : hint.level === "warn" ? "hsl(45 93% 50% / 0.18)" : hint.level === "success" ? "hsl(var(--success) / 0.18)" : "hsl(var(--accent) / 0.18)",
              color: hint.level === "error" ? "hsl(var(--danger))" : hint.level === "warn" ? "hsl(45 93% 65%)" : hint.level === "success" ? "hsl(var(--success))" : "hsl(var(--accent))",
            }}
            title="点击关闭" onClick={() => setHint(null)}>
            {hint.text}
          </span>
        )}
        <span className="flex-1" />
        {updateVersion && (
          <button onClick={() => navigate("/about")}
            className="flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-medium animate-in cursor-pointer hover:opacity-80 transition-opacity"
            style={{ backgroundColor: "hsl(var(--accent) / 0.15)", color: "hsl(var(--accent))" }}
            title="点击查看详情">
            🆕 v{updateVersion}
          </button>
        )}
      </footer>
    </div>
  );
}
