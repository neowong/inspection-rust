import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "@/types";

export default function SettingsPage() {
  const [settings, setSettings] = useState<Settings>({ report_max_output_lines: 100 });
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    invoke<Settings>("get_settings").then(setSettings).catch(console.error);
    invoke("get_report_info_fields").then(r => console.log("Report fields:", r));
  }, []);

  const save = async () => {
    await invoke("update_settings", { reportMaxOutputLines: settings.report_max_output_lines });
    setSaved(true); setTimeout(() => setSaved(false), 2000);
  };

  return (
    <div className="space-y-6 max-w-lg">
      <h2 className="text-2xl font-bold">系统设置</h2>

      <div className="border rounded-lg p-4 bg-card space-y-3">
        <h3 className="font-semibold">报告输出设置</h3>
        <label className="block">
          <span className="text-sm text-muted-foreground">报告命令输出最大行数 (1-10000)</span>
          <input className="w-full border rounded px-2 py-1.5 mt-0.5 text-sm" type="number" min={1} max={10000}
            value={settings.report_max_output_lines} onChange={e => setSettings(s => ({ ...s, report_max_output_lines: parseInt(e.target.value) || 100 }))} />
        </label>
        <button onClick={save} className="bg-primary text-primary-foreground px-4 py-1.5 rounded-md text-sm">{saved ? "已保存" : "保存设置"}</button>
      </div>

      <div className="border rounded-lg p-4 bg-card space-y-3">
        <h3 className="font-semibold">关于</h3>
        <div className="text-sm text-muted-foreground space-y-1">
          <p>网络设备巡检系统 v3.0.0</p>
          <p>基于 Rust + Tauri 构建的桌面版</p>
          <p>支持 SSH 在线巡检、离线巡检、Web 截图巡检三种模式</p>
          <p>数据存储在本地 SQLite 数据库</p>
        </div>
      </div>
    </div>
  );
}
