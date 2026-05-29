import { Routes, Route, Navigate } from "react-router-dom";
import AppShell from "./layouts/AppShell";
import { useGlobalShortcuts } from "./hooks/useKeyboardShortcut";
import DevicesPage from "./pages/DevicesPage";
import TemplatesPage from "./pages/TemplatesPage";
import InspectionPage from "./pages/InspectionPage";
import ReportsPage from "./pages/ReportsPage";
import AiConfigPage from "./pages/AiConfigPage";
import SettingsPage from "./pages/SettingsPage";

export default function App() {
  useGlobalShortcuts();
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route path="/" element={<Navigate to="/templates" replace />} />
        <Route path="/devices" element={<DevicesPage />} />
        <Route path="/templates" element={<TemplatesPage />} />
        <Route path="/inspection" element={<InspectionPage />} />
        <Route path="/reports" element={<ReportsPage />} />
        <Route path="/ai-config" element={<AiConfigPage />} />
        <Route path="/settings" element={<SettingsPage />} />
      </Route>
    </Routes>
  );
}
