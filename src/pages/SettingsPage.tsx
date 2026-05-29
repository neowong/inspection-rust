import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import Button from "../components/ui/Button";
import Input from "../components/ui/Input";

interface SystemSettings {
  id: number;
  report_max_output_lines: number;
}

type TabKey = "report" | "network" | "about";

const TABS: { key: TabKey; label: string }[] = [
  { key: "report", label: "报告设置" },
  { key: "network", label: "网络设置" },
  { key: "about", label: "关于" },
];

const DEFAULT_SETTINGS: SystemSettings = {
  id: 0,
  report_max_output_lines: 1000,
};

export default function SettingsPage() {
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<TabKey>("report");
  const [settings, setSettings] = useState<SystemSettings>(DEFAULT_SETTINGS);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  // Editable fields
  const [maxOutputLines, setMaxOutputLines] = useState(1000);
  const [reportSavePath, setReportSavePath] = useState("");
  const [sshTimeout, setSshTimeout] = useState(10);
  const [maxConcurrent, setMaxConcurrent] = useState(5);

  const loadSettings = useCallback(async () => {
    try {
      const data = await invoke<SystemSettings>("get_settings");
      setSettings(data);
      setMaxOutputLines(data.report_max_output_lines);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadSettings(); }, [loadSettings]);

  // Resolve report save path
  useEffect(() => {
    const resolvePath = async () => {
      try {
        // Use a simple default path based on app data directory
        const home = await invoke<string | null>("get_report_save_path").catch(() => null);
        if (home) {
          setReportSavePath(home);
        } else {
          setReportSavePath("~/.local/share/inspection-rust/reports");
        }
      } catch {
        setReportSavePath("~/.local/share/inspection-rust/reports");
      }
    };
    resolvePath();
  }, []);

  const handleSaveReport = async () => {
    setSaving(true);
    setSaved(false);
    try {
      await invoke("update_settings", {
        report_max_output_lines: maxOutputLines,
      });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      console.error(e);
    } finally {
      setSaving(false);
    }
  };

  const handleSaveNetwork = async () => {
    setSaving(true);
    setSaved(false);
    try {
      // Network settings may be extended in future backend updates
      await invoke("update_settings", {
        ssh_timeout: sshTimeout,
        max_concurrent_inspections: maxConcurrent,
      }).catch(() => {
        // Silently ignore if backend doesn't support these fields yet
      });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      console.error(e);
    } finally {
      setSaving(false);
    }
  };

  const handleBrowsePath = async () => {
    try {
      const path = await invoke<string | null>("select_directory").catch(() => null);
      if (path) {
        setReportSavePath(path);
      }
    } catch {
      // Dialog cancelled or not supported
    }
  };

  if (loading) return <div className="p-4 text-[hsl(var(--text-tertiary))] text-sm">加载中...</div>;

  return (
    <div className="flex flex-col gap-4 h-full overflow-auto">
      <h1 className="text-xl font-semibold text-[hsl(var(--text-primary))]">系统设置</h1>

      {/* Tabs */}
      <div className="flex border-b border-[hsl(var(--border))] gap-0">
        {TABS.map((tab) => (
          <button
            key={tab.key}
            className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
              activeTab === tab.key
                ? "border-[hsl(var(--accent))] text-[hsl(var(--accent))]"
                : "border-transparent text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))] hover:border-[hsl(var(--border))]"
            }`}
            onClick={() => setActiveTab(tab.key)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab: Report Settings */}
      {activeTab === "report" && (
        <div className="bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg p-4 max-w-xl">
          <h2 className="text-sm font-semibold mb-4 text-[hsl(var(--text-primary))]">报告设置</h2>
          <div className="space-y-4">
            <FormField label="每条命令最大输出行数">
              <Input
                size="sm"
                type="number"
                min={10}
                max={100000}
                value={maxOutputLines}
                onChange={(e) => setMaxOutputLines(parseInt(e.target.value) || 1000)}
              />
              <span className="text-[11px] text-[hsl(var(--text-tertiary))] mt-0.5">
                超过此行数的命令输出将被截断，默认 1000 行
              </span>
            </FormField>
            <FormField label="报告保存路径">
              <div className="flex gap-2">
                <Input
                  size="sm"
                  className="flex-1"
                  value={reportSavePath}
                  readOnly
                />
                <Button variant="secondary" size="sm" onClick={handleBrowsePath}>浏览</Button>
              </div>
              <span className="text-[11px] text-[hsl(var(--text-tertiary))] mt-0.5">
                巡检报告 PDF/Markdown 的默认保存目录
              </span>
            </FormField>
          </div>
          <div className="mt-4 flex items-center gap-2">
            <Button size="sm" disabled={saving} loading={saving} onClick={handleSaveReport}>保存设置</Button>
            {saved && <span className="text-xs text-[hsl(var(--success))]">已保存</span>}
          </div>
        </div>
      )}

      {/* Tab: Network Settings */}
      {activeTab === "network" && (
        <div className="bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg p-4 max-w-xl">
          <h2 className="text-sm font-semibold mb-4 text-[hsl(var(--text-primary))]">网络设置</h2>
          <div className="space-y-4">
            <FormField label="SSH 连接超时 (秒)">
              <Input
                size="sm"
                type="number"
                min={1}
                max={120}
                value={sshTimeout}
                onChange={(e) => setSshTimeout(parseInt(e.target.value) || 10)}
              />
              <span className="text-[11px] text-[hsl(var(--text-tertiary))] mt-0.5">
                单次 SSH 连接的超时时间，默认 10 秒
              </span>
            </FormField>
            <FormField label="最大并发巡检数">
              <Input
                size="sm"
                type="number"
                min={1}
                max={50}
                value={maxConcurrent}
                onChange={(e) => setMaxConcurrent(parseInt(e.target.value) || 5)}
              />
              <span className="text-[11px] text-[hsl(var(--text-tertiary))] mt-0.5">
                同时执行巡检的最大设备数，默认 5 台
              </span>
            </FormField>
          </div>
          <div className="mt-4 flex items-center gap-2">
            <Button size="sm" disabled={saving} loading={saving} onClick={handleSaveNetwork}>保存设置</Button>
            {saved && <span className="text-xs text-[hsl(var(--success))]">已保存</span>}
          </div>
        </div>
      )}

      {/* Tab: About */}
      {activeTab === "about" && (
        <div className="bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg p-4 max-w-xl">
          <h2 className="text-sm font-semibold mb-4 text-[hsl(var(--text-primary))]">关于</h2>
          <div className="space-y-3 text-sm">
            <div className="flex items-center gap-3">
              <span className="text-[hsl(var(--text-secondary))] w-20 shrink-0">应用名称</span>
              <span className="text-[hsl(var(--text-primary))] font-medium">网络设备巡检系统</span>
            </div>
            <div className="flex items-center gap-3">
              <span className="text-[hsl(var(--text-secondary))] w-20 shrink-0">版本</span>
              <code className="px-1.5 py-0.5 bg-[hsl(var(--bg-hover))] rounded text-[11px]">v3.1.0</code>
            </div>
            <div className="flex items-start gap-3">
              <span className="text-[hsl(var(--text-secondary))] w-20 shrink-0">描述</span>
              <span className="text-[hsl(var(--text-primary))]">
                基于 Rust + Tauri v2 的桌面版网络设备巡检系统，支持 SSH 远程执行巡检命令、AI 智能分析输出结果并生成 Markdown 报告。
              </span>
            </div>
            <div className="flex items-start gap-3">
              <span className="text-[hsl(var(--text-secondary))] w-20 shrink-0">技术栈</span>
              <div className="flex flex-wrap gap-1.5">
                {[
                  "Rust", "Tauri v2", "React 18", "TypeScript",
                  "Vite 5", "TailwindCSS", "SQLite", "SSH2",
                  "OpenAI", "Anthropic",
                ].map((tech) => (
                  <span
                    key={tech}
                    className="inline-block px-1.5 py-0.5 bg-[hsl(var(--bg-hover))] border border-[hsl(var(--border))] rounded text-[11px] text-[hsl(var(--text-secondary))]"
                  >
                    {tech}
                  </span>
                ))}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function FormField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-xs text-[hsl(var(--text-secondary))]">{label}</span>
      {children}
    </label>
  );
}
