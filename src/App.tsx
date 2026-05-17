import { Routes, Route } from "react-router-dom";
import AppLayout from "./components/layout/AppLayout";
import DashboardPage from "./features/dashboard/DashboardPage";
import DevicesPage from "./features/devices/DevicesPage";
import TemplatesPage from "./features/templates/TemplatesPage";
import CommandsPage from "./features/commands/CommandsPage";
import BatchesPage from "./features/batches/BatchesPage";
import BatchDetailPage from "./features/batches/BatchDetailPage";
import InspectionPage from "./features/inspection/InspectionPage";
import ScheduledTasksPage from "./features/scheduled/ScheduledTasksPage";
import AiConfigPage from "./features/settings/AiConfigPage";
import ReportTemplatesPage from "./features/report-templates/ReportTemplatesPage";
import OfflinePage from "./features/offline/OfflinePage";
import ChatPage from "./features/chat/ChatPage";
import SettingsPage from "./features/settings/SettingsPage";

export default function App() {
  return (
    <Routes>
      <Route element={<AppLayout />}>
        <Route path="/" element={<DashboardPage />} />
        <Route path="/devices" element={<DevicesPage />} />
        <Route path="/templates" element={<TemplatesPage />} />
        <Route path="/commands" element={<CommandsPage />} />
        <Route path="/batches" element={<BatchesPage />} />
        <Route path="/batches/:id" element={<BatchDetailPage />} />
        <Route path="/inspection" element={<InspectionPage />} />
        <Route path="/scheduled" element={<ScheduledTasksPage />} />
        <Route path="/ai-config" element={<AiConfigPage />} />
        <Route path="/report-templates" element={<ReportTemplatesPage />} />
        <Route path="/offline" element={<OfflinePage />} />
        <Route path="/chat" element={<ChatPage />} />
        <Route path="/settings" element={<SettingsPage />} />
      </Route>
    </Routes>
  );
}
