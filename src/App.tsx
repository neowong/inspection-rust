import { Routes, Route, Navigate } from "react-router-dom";
import AppShell from "./layouts/AppShell";
import DashboardPage from "./pages/DashboardPage";
import DevicesPage from "./pages/DevicesPage";
import TemplatesPage from "./pages/TemplatesPage";
import InspectionPage from "./pages/InspectionPage";
import ReportManagementPage from "./pages/ReportManagementPage";
import SettingsPage from "./pages/SettingsPage";
import LogAnalysisPage from "./pages/LogAnalysisPage";
import ToolsPage from "./pages/ToolsPage";
import AboutPage from "./pages/AboutPage";
import ChatPage from "./pages/ChatPage";

export default function App() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route path="/" element={<Navigate to="/dashboard" replace />} />
        <Route path="/dashboard" element={<DashboardPage />} />
        <Route path="/devices" element={<DevicesPage />} />
        <Route path="/templates" element={<TemplatesPage />} />
        <Route path="/inspection" element={<InspectionPage />} />
        <Route path="/reports" element={<ReportManagementPage />} />
        <Route path="/tools" element={<ToolsPage />} />
        <Route path="/logs" element={<LogAnalysisPage />} />
        <Route path="/settings" element={<SettingsPage />} />
        <Route path="/about" element={<AboutPage />} />
        <Route path="/chat" element={<ChatPage />} />
        <Route path="*" element={
          <div className="flex flex-col items-center justify-center h-64 text-[hsl(var(--text-tertiary))]">
            <p className="text-lg font-medium">404 — 页面不存在</p>
            <a href="/" className="mt-2 text-sm text-[hsl(var(--accent))] hover:underline">返回首页</a>
          </div>
        } />
      </Route>
    </Routes>
  );
}
