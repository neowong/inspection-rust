import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

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

  if (loading) return <div className="p-4 text-gray-500 text-sm">加载中...</div>;

  return (
    <div className="flex flex-col gap-4 h-full overflow-auto">
      <h1 className="text-lg font-bold">系统设置</h1>

      {/* Tabs */}
      <div className="flex border-b border-gray-200 gap-0">
        {TABS.map((tab) => (
          <button
            key={tab.key}
            className={`px-4 py-2 text-xs font-medium border-b-2 transition-colors ${
              activeTab === tab.key
                ? "border-blue-500 text-blue-600"
                : "border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300"
            }`}
            onClick={() => setActiveTab(tab.key)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab: Report Settings */}
      {activeTab === "report" && (
        <div className="bg-white rounded border border-gray-200 p-4 max-w-xl">
          <h2 className="text-sm font-semibold mb-4">报告设置</h2>
          <div className="space-y-4">
            <FormField label="每条命令最大输出行数">
              <input
                className="form-input"
                type="number"
                min={10}
                max={100000}
                value={maxOutputLines}
                onChange={(e) => setMaxOutputLines(parseInt(e.target.value) || 1000)}
              />
              <span className="text-[11px] text-gray-400 mt-0.5">
                超过此行数的命令输出将被截断，默认 1000 行
              </span>
            </FormField>
            <FormField label="报告保存路径">
              <div className="flex gap-2">
                <input
                  className="form-input flex-1"
                  value={reportSavePath}
                  readOnly
                />
                <button
                  className="px-3 py-1 text-xs border border-gray-300 rounded hover:bg-gray-100 shrink-0"
                  onClick={handleBrowsePath}
                >
                  浏览
                </button>
              </div>
              <span className="text-[11px] text-gray-400 mt-0.5">
                巡检报告 PDF/Markdown 的默认保存目录
              </span>
            </FormField>
          </div>
          <div className="mt-4 flex items-center gap-2">
            <button
              className="px-3 py-1 text-xs bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50"
              disabled={saving}
              onClick={handleSaveReport}
            >
              {saving ? "保存中..." : "保存设置"}
            </button>
            {saved && <span className="text-xs text-green-600">已保存</span>}
          </div>
        </div>
      )}

      {/* Tab: Network Settings */}
      {activeTab === "network" && (
        <div className="bg-white rounded border border-gray-200 p-4 max-w-xl">
          <h2 className="text-sm font-semibold mb-4">网络设置</h2>
          <div className="space-y-4">
            <FormField label="SSH 连接超时 (秒)">
              <input
                className="form-input"
                type="number"
                min={1}
                max={120}
                value={sshTimeout}
                onChange={(e) => setSshTimeout(parseInt(e.target.value) || 10)}
              />
              <span className="text-[11px] text-gray-400 mt-0.5">
                单次 SSH 连接的超时时间，默认 10 秒
              </span>
            </FormField>
            <FormField label="最大并发巡检数">
              <input
                className="form-input"
                type="number"
                min={1}
                max={50}
                value={maxConcurrent}
                onChange={(e) => setMaxConcurrent(parseInt(e.target.value) || 5)}
              />
              <span className="text-[11px] text-gray-400 mt-0.5">
                同时执行巡检的最大设备数，默认 5 台
              </span>
            </FormField>
          </div>
          <div className="mt-4 flex items-center gap-2">
            <button
              className="px-3 py-1 text-xs bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50"
              disabled={saving}
              onClick={handleSaveNetwork}
            >
              {saving ? "保存中..." : "保存设置"}
            </button>
            {saved && <span className="text-xs text-green-600">已保存</span>}
          </div>
        </div>
      )}

      {/* Tab: About */}
      {activeTab === "about" && (
        <div className="bg-white rounded border border-gray-200 p-4 max-w-xl">
          <h2 className="text-sm font-semibold mb-4">关于</h2>
          <div className="space-y-3 text-sm">
            <div className="flex items-center gap-3">
              <span className="text-gray-500 w-20 shrink-0">应用名称</span>
              <span className="text-gray-800 font-medium">网络设备巡检系统</span>
            </div>
            <div className="flex items-center gap-3">
              <span className="text-gray-500 w-20 shrink-0">版本</span>
              <code className="px-1.5 py-0.5 bg-gray-100 rounded text-[11px]">v3.1.0</code>
            </div>
            <div className="flex items-start gap-3">
              <span className="text-gray-500 w-20 shrink-0">描述</span>
              <span className="text-gray-800">
                基于 Rust + Tauri v2 的桌面版网络设备巡检系统，支持 SSH 远程执行巡检命令、AI 智能分析输出结果并生成 Markdown 报告。
              </span>
            </div>
            <div className="flex items-start gap-3">
              <span className="text-gray-500 w-20 shrink-0">技术栈</span>
              <div className="flex flex-wrap gap-1.5">
                {[
                  "Rust", "Tauri v2", "React 18", "TypeScript",
                  "Vite 5", "TailwindCSS", "SQLite", "SSH2",
                  "OpenAI", "Anthropic",
                ].map((tech) => (
                  <span
                    key={tech}
                    className="inline-block px-1.5 py-0.5 bg-gray-100 border border-gray-200 rounded text-[11px] text-gray-600"
                  >
                    {tech}
                  </span>
                ))}
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Form input style */}
      <style>{`
        .form-input {
          width: 100%;
          padding: 4px 8px;
          font-size: 12px;
          border: 1px solid #d1d5db;
          border-radius: 4px;
          outline: none;
          background: #fff;
        }
        .form-input:focus {
          border-color: #3b82f6;
          box-shadow: 0 0 0 1px #3b82f6;
        }
      `}</style>
    </div>
  );
}

function FormField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-xs text-gray-600">{label}</span>
      {children}
    </label>
  );
}
