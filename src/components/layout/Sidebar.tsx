import { NavLink } from "react-router-dom";
import {
  LayoutDashboard, Server, FileText, Terminal, ClipboardList,
  Search, Clock, Cpu, FileCheck, Download, MessageSquare, Settings,
} from "lucide-react";
import { cn } from "@/lib/utils";

const navItems = [
  { to: "/", icon: LayoutDashboard, label: "仪表盘" },
  { to: "/devices", icon: Server, label: "设备管理" },
  { to: "/templates", icon: FileText, label: "巡检模板" },
  { to: "/commands", icon: Terminal, label: "命令库" },
  { to: "/batches", icon: ClipboardList, label: "巡检批次" },
  { to: "/inspection", icon: Search, label: "巡检记录" },
  { to: "/scheduled", icon: Clock, label: "定时任务" },
  { to: "/ai-config", icon: Cpu, label: "AI配置" },
  { to: "/report-templates", icon: FileCheck, label: "报告模板" },
  { to: "/offline", icon: Download, label: "离线巡检" },
  { to: "/chat", icon: MessageSquare, label: "AI对话" },
  { to: "/settings", icon: Settings, label: "系统设置" },
];

export default function Sidebar() {
  return (
    <aside className="w-56 bg-card border-r flex flex-col shrink-0">
      <div className="p-4 border-b">
        <h1 className="text-sm font-bold text-primary">网络设备巡检系统</h1>
        <p className="text-xs text-muted-foreground">v3.0 Tauri版</p>
      </div>
      <nav className="flex-1 p-2 space-y-0.5 overflow-auto">
        {navItems.map(({ to, icon: Icon, label }) => (
          <NavLink
            key={to}
            to={to}
            end={to === "/"}
            className={({ isActive }) =>
              cn(
                "flex items-center gap-2.5 px-3 py-2 rounded-md text-sm transition-colors",
                isActive
                  ? "bg-primary text-primary-foreground font-medium"
                  : "text-muted-foreground hover:bg-muted hover:text-foreground"
              )
            }
          >
            <Icon className="h-4 w-4 shrink-0" />
            <span>{label}</span>
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}
