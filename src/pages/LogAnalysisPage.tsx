import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Upload, Search, FileText } from "lucide-react";
import Button from "../components/ui/Button";
import { Select } from "../components/ui/Input";
import StatBadge from "../components/StatBadge";

interface LogEntry {
  timestamp: string;
  hostname: string;
  severity: string;
  module: string;
  mnemonic: string;
  message: string;
}

interface LogAnalysis {
  total: number;
  errors: number;
  warnings: number;
  info: number;
  debug: number;
  entries: LogEntry[];
  summary: string;
}

export default function LogAnalysisPage() {
  const [vendor, setVendor] = useState("H3C");
  const [logText, setLogText] = useState("");
  const [analyzing, setAnalyzing] = useState(false);
  const [result, setResult] = useState<LogAnalysis | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [fileName, setFileName] = useState("");

  const handleFile = (file: File) => {
    setFileName(file.name);
    const reader = new FileReader();
    reader.onload = (e) => {
      setLogText(e.target?.result as string || "");
    };
    reader.readAsText(file);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    const file = e.dataTransfer.files[0];
    if (file) handleFile(file);
  };

  const handleAnalyze = async () => {
    if (!logText.trim()) { setError("请先上传或粘贴日志内容"); return; }
    setAnalyzing(true);
    setError(null);
    try {
      const r = await invoke<LogAnalysis>("parse_log_text", { text: logText, vendor });
      setResult(r);
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
    } finally {
      setAnalyzing(false);
    }
  };

  const severityColor = (s: string) => {
    if (s === "ERROR" || s === "CRIT" || s === "EMERG") return "text-[hsl(var(--danger))] bg-[hsl(var(--danger)_/_0.1)]";
    if (s === "WARNING" || s === "NOTICE") return "text-[hsl(var(--warning))] bg-[hsl(var(--warning)_/_0.1)]";
    return "text-[hsl(var(--text-secondary))] bg-[hsl(var(--bg-hover))]";
  };

  return (
    <div className="space-y-6">
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm relative">
        <div className="flex items-center gap-3">
          <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">日志分析</h1>
          <span className="px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-[hsl(var(--warning)_/_0.15)] text-[hsl(var(--warning))] border border-[hsl(var(--warning)_/_0.3)]">
            功能有待完善
          </span>
        </div>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1">上传设备日志文件，自动解析并统计</p>
      </div>

      <div className="grid grid-cols-2 gap-6">
        {/* Left: Upload */}
        <div>
          <h2 className="text-sm font-semibold text-[hsl(var(--text-primary))] mb-3">导入日志</h2>

          {/* Drop zone */}
          <div
            onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
            onDragLeave={() => setDragOver(false)}
            onDrop={handleDrop}
            className={`border-2 border-dashed rounded-lg p-8 text-center transition-colors cursor-pointer ${
              dragOver ? "border-[hsl(var(--accent))] bg-[hsl(var(--accent-subtle))]" : "border-[hsl(var(--border))] hover:border-[hsl(var(--text-tertiary))]"
            }`}
            onClick={() => document.getElementById("log-file-input")?.click()}
          >
            <input
              id="log-file-input"
              type="file"
              accept=".txt,.log,.csv"
              className="hidden"
              onChange={(e) => { const f = e.target.files?.[0]; if (f) handleFile(f); }}
            />
            <Upload size={28} className="mx-auto mb-2 text-[hsl(var(--text-tertiary))]" />
            <p className="text-sm text-[hsl(var(--text-secondary))]">拖拽日志文件到此处</p>
            <p className="text-xs text-[hsl(var(--text-tertiary))] mt-1">或点击选择文件（.txt / .log / .csv）</p>
            {fileName && <p className="text-xs text-[hsl(var(--accent))] mt-2"><FileText size={12} className="inline mr-1" />{fileName}</p>}
          </div>

          {/* Vendor select */}
          <div className="flex items-center gap-3 mt-3">
            <span className="text-xs text-[hsl(var(--text-secondary))]">厂商格式:</span>
            <Select className="w-32" value={vendor} onChange={(e) => setVendor(e.target.value)}>
              <option value="H3C">H3C / 华为</option>
              <option value="Cisco">思科 / 锐捷</option>
            </Select>
          </div>

          {/* Paste area */}
          <div className="mt-3">
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">或直接粘贴日志内容</label>
            <textarea
              value={logText}
              onChange={(e) => { setLogText(e.target.value); setFileName(""); }}
              placeholder="%May 30 11:03:59:450 2026 aHope SSHS/6/SSHS_VERSION_MISMATCH: SSH client..."
              className="w-full h-40 rounded-md border border-[hsl(var(--border))] bg-[hsl(var(--bg-card))] px-3 py-2 text-xs font-mono text-[hsl(var(--text-primary))] placeholder:text-[hsl(var(--text-tertiary))] resize-none focus:outline-none focus:border-[hsl(var(--accent))]"
            />
          </div>

          <div className="flex gap-2 mt-3">
            <Button onClick={handleAnalyze} loading={analyzing}>
              <Search size={14} className="mr-1" />分析日志
            </Button>
            <Button variant="secondary" onClick={() => { setLogText(""); setResult(null); setError(null); setFileName(""); }}>
              清空
            </Button>
          </div>

          {error && (
            <div className="mt-3 p-3 bg-[hsl(var(--danger)_/_0.1)] border border-[hsl(var(--danger)_/_0.3)] rounded text-sm text-[hsl(var(--danger))]">
              {error}
            </div>
          )}
        </div>

        {/* Right: Results */}
        <div>
          <h2 className="text-sm font-semibold text-[hsl(var(--text-primary))] mb-3">分析结果</h2>
          {!result && !analyzing && (
            <div className="text-center py-12 text-sm text-[hsl(var(--text-tertiary))]">
              导入日志后点击"分析日志"查看结果
            </div>
          )}
          {analyzing && (
            <div className="text-center py-12 text-sm text-[hsl(var(--text-secondary))]">解析中...</div>
          )}
          {result && result.total > 0 && (
            <>
              <div className="grid grid-cols-4 gap-2 mb-3">
                <StatBadge label="总计" value={result.total} color="info" />
                <StatBadge label="错误" value={result.errors} color="danger" />
                <StatBadge label="警告" value={result.warnings} color="warning" />
                <StatBadge label="信息" value={result.info + result.debug} color="text-secondary" />
              </div>
              <p className="text-xs text-[hsl(var(--text-secondary))] mb-2">{result.summary}</p>
              <div className="border border-[hsl(var(--border))] rounded-md overflow-hidden max-h-[500px] overflow-y-auto">
                <table className="w-full text-xs">
                  <thead className="bg-[hsl(var(--bg-hover))] sticky top-0">
                    <tr>
                      <th className="px-2 py-1.5 text-left font-medium text-[hsl(var(--text-secondary))] w-[130px]">时间</th>
                      <th className="px-2 py-1.5 text-left font-medium text-[hsl(var(--text-secondary))] w-[60px]">级别</th>
                      <th className="px-2 py-1.5 text-left font-medium text-[hsl(var(--text-secondary))] w-[80px]">模块</th>
                      <th className="px-2 py-1.5 text-left font-medium text-[hsl(var(--text-secondary))]">消息</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-[hsl(var(--border-light))]">
                    {result.entries.map((e, i) => (
                      <tr key={i} className="hover:bg-[hsl(var(--bg-hover))]">
                        <td className="px-2 py-1 font-mono text-[hsl(var(--text-tertiary))]">{e.timestamp}</td>
                        <td className="px-2 py-1">
                          <span className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${severityColor(e.severity)}`}>
                            {e.severity}
                          </span>
                        </td>
                        <td className="px-2 py-1 text-[hsl(var(--text-secondary))]">{e.module}/{e.mnemonic}</td>
                        <td className="px-2 py-1 text-[hsl(var(--text-primary))] truncate max-w-[260px]" title={e.message}>{e.message}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          )}
          {result && result.total === 0 && (
            <div className="text-center py-12 text-sm text-[hsl(var(--text-secondary))]">
              {result.summary}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

