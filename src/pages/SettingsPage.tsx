import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import Card from "../components/ui/Card";
import Input from "../components/ui/Input";
import Button from "../components/ui/Button";

interface SystemSettings {
  report_max_output_lines: number;
}

export default function SettingsPage() {
  const [reportMaxOutputLines, setReportMaxOutputLines] = useState(100);
  const [loading, setLoading] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    invoke<SystemSettings>("get_settings")
      .then((s) => setReportMaxOutputLines(s.report_max_output_lines))
      .catch(console.error);
  }, []);

  const handleSave = () => {
    setLoading(true);
    setSaved(false);
    invoke<void>("update_settings", { reportMaxOutputLines })
      .then(() => {
        setSaved(true);
        setTimeout(() => setSaved(false), 2000);
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">系统设置</h1>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1">配置系统运行参数</p>
      </div>

      <Card className="max-w-lg">
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-[hsl(var(--text-primary))] mb-1.5">
              报告最大输出行数
            </label>
            <Input
              type="number"
              value={reportMaxOutputLines}
              onChange={(e) => setReportMaxOutputLines(Number(e.target.value))}
              min={10}
              max={10000}
            />
            <p className="text-xs text-[hsl(var(--text-tertiary))] mt-1">
              生成报告时每台设备命令输出的最大行数，超出部分将被截断
            </p>
          </div>
          <div className="flex items-center gap-3">
            <Button onClick={handleSave} loading={loading}>
              保存设置
            </Button>
            {saved && (
              <span className="text-sm text-[hsl(var(--success))]">设置已保存</span>
            )}
          </div>
        </div>
      </Card>
    </div>
  );
}
