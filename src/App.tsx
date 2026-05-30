import { Routes, Route, Navigate } from "react-router-dom";
import AppShell from "./layouts/AppShell";
import { useGlobalShortcuts } from "./hooks/useKeyboardShortcut";
import DashboardPage from "./pages/DashboardPage";
import DevicesPage from "./pages/DevicesPage";
import TemplatesPage from "./pages/TemplatesPage";
import InspectionPage from "./pages/InspectionPage";
import ReportsPage from "./pages/ReportsPage";
import SettingsPage from "./pages/SettingsPage";
import LogAnalysisPage from "./pages/LogAnalysisPage";

export default function App() {
  useGlobalShortcuts();
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route path="/" element={<Navigate to="/dashboard" replace />} />
        <Route path="/dashboard" element={<DashboardPage />} />
        <Route path="/devices" element={<DevicesPage />} />
        <Route path="/templates" element={<TemplatesPage />} />
        <Route path="/inspection" element={<InspectionPage />} />
        <Route path="/reports" element={<ReportsPage />} />
        <Route path="/logs" element={<LogAnalysisPage />} />
        <Route path="/settings" element={<SettingsPage />} />
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
