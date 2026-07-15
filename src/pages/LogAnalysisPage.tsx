import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import {
  Upload, Search, FileText, Download, AlertTriangle, AlertCircle,
  Shield, Server, Radio, ChevronRight, Lightbulb,
} from "lucide-react";
import Button from "../components/ui/Button";
import { Select } from "../components/ui/Input";
import type { AiModelConfig } from "../types";

/* ── 日志类型定义 ── */
const LOG_TYPES = [
  { key: "network",  label: "网络设备日志", icon: Radio },
  { key: "security", label: "安全日志",     icon: Shield },
  { key: "linux",    label: "Linux 系统日志", icon: Server },
] as const;

type LogType = (typeof LOG_TYPES)[number]["key"];

/* 空字符串传给后端 = 让 AI 自动识别厂商和设备类型 */
const AUTO = "";

/* ── 严重程度样式 ── */
const SEV_STYLES: Record<string, string> = {
  high:   "bg-[hsl(var(--danger)/0.12)] text-[hsl(var(--danger))] border border-[hsl(var(--danger)/0.25)]",
  medium: "bg-[hsl(var(--warning)/0.12)] text-[hsl(var(--warning))] border border-[hsl(var(--warning)/0.25)]",
  low:    "bg-[hsl(var(--info)/0.12)] text-[hsl(var(--info))] border border-[hsl(var(--info)/0.25)]",
};

const OVERALL_STYLES: Record<string, string> = {
  ok:       "bg-[hsl(var(--success)/0.12)] text-[hsl(var(--success))] border border-[hsl(var(--success)/0.25)]",
  info:     "bg-[hsl(var(--info)/0.12)] text-[hsl(var(--info))] border border-[hsl(var(--info)/0.25)]",
  warning:  "bg-[hsl(var(--warning)/0.12)] text-[hsl(var(--warning))] border border-[hsl(var(--warning)/0.25)]",
  critical: "bg-[hsl(var(--danger)/0.12)] text-[hsl(var(--danger))] border border-[hsl(var(--danger)/0.25)]",
};

/* ── 组件 ── */
export default function LogAnalysisPage() {
  const [logType, setLogType] = useState<LogType>("network");
  const [aiConfigs, setAiConfigs] = useState<AiModelConfig[]>([]);
  const [aiConfigId, setAiConfigId] = useState<number>(0);
  const [logText, setLogText] = useState("");
  const [analyzing, setAnalyzing] = useState(false);
  const [result, setResult] = useState<{
    summary: string;
    overall: string;
    entries: { time?: string; level?: string; source?: string; content: string; analysis?: string; severity?: string }[];
    advice: string;
    raw: string;
    identified_vendor?: string;
    identified_device_type?: string;
    total_lines?: number;
    kept_lines?: number;
  } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [fileName, setFileName] = useState("");
  const [expandedEntries, setExpandedEntries] = useState<Set<number>>(new Set());
  const fileInputRef = useRef<HTMLInputElement>(null);
  const resultRef = useRef<HTMLDivElement>(null);

  // 加载 AI 配置
  useEffect(() => {
    invoke<AiModelConfig[]>("list_ai_configs").then(list => {
      setAiConfigs(list);
      const active = list.find(c => c.is_active) || list[0];
      if (active) setAiConfigId(active.id);
    }).catch(console.error);
  }, []);

  // 日志类型切换时 AI 自动识别厂商和类型，无需手动选择

  const handleFile = useCallback((file: File) => {
    setFileName(file.name);
    const reader = new FileReader();
    reader.onload = (e) => setLogText(e.target?.result as string || "");
    reader.readAsText(file);
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    const file = e.dataTransfer.files[0];
    if (file) handleFile(file);
  }, [handleFile]);

  const handleAnalyze = async () => {
    if (!logText.trim()) { setError("请先上传或粘贴日志内容"); return; }
    if (!aiConfigId) { setError("请先添加并激活 AI 模型配置"); return; }
    setAnalyzing(true);
    setError(null);
    setResult(null);
    try {
      const r = await invoke<{
        success: boolean;
        summary: string;
        overall: string;
        entries: { time?: string; level?: string; source?: string; content: string; analysis?: string; severity?: string }[];
        advice: string;
        raw: string;
        identified_vendor?: string;
        identified_device_type?: string;
      }>("analyze_logs_ai", {
        text: logText,
        logType,
        vendor: AUTO,
        deviceType: AUTO,
        aiConfigId,
      });
      setResult(r);
      // 分析完成后滚动到结果区
      setTimeout(() => resultRef.current?.scrollIntoView({ behavior: "smooth", block: "start" }), 100);
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
    } finally {
      setAnalyzing(false);
    }
  };

  const handleExport = async () => {
    if (!result) return;
    try {
      const savePath = await save({
        defaultPath: `日志分析报告_${new Date().toISOString().slice(0, 10)}.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }, { name: "文本文件", extensions: ["txt"] }],
      });
      if (!savePath) return;

      // 构建导出内容
      const lines: string[] = [];
      lines.push("# 日志分析报告");
      lines.push("");
      lines.push(`- **日志类型**: ${LOG_TYPES.find(t => t.key === logType)?.label}`);
      lines.push(`- **厂商/系统**: ${result.identified_vendor || "已自动识别"}`);
      lines.push(`- **设备类型**: ${result.identified_device_type || "已自动识别"}`);
      lines.push(`- **分析时间**: ${new Date().toLocaleString()}`);
      lines.push("");
      lines.push("## 总体评价");
      lines.push("");
      lines.push(`**${result.summary}**`);
      lines.push("");
      lines.push("## 详细分析");
      lines.push("");
      if (result.entries.length > 0) {
        result.entries.forEach((e, i) => {
          lines.push(`### ${i + 1}. ${e.content}`);
          if (e.time) lines.push(`- **时间**: ${e.time}`);
          if (e.level) lines.push(`- **级别**: ${e.level}`);
          if (e.source) lines.push(`- **来源**: ${e.source}`);
          if (e.analysis) lines.push(`- **分析**: ${e.analysis}`);
          if (e.severity) lines.push(`- **危害**: ${e.severity}`);
          lines.push("");
        });
      } else {
        lines.push(result.raw);
        lines.push("");
      }
      if (result.advice) {
        lines.push("## 运维建议");
        lines.push("");
        lines.push(result.advice);
        lines.push("");
      }
      lines.push("---");
      lines.push(`*由 @Hope 巡检助手 AI 生成*`);

      await invoke("export_log_analysis", {
        savePath,
        content: lines.join("\n"),
      });
    } catch (e) {
      setError(typeof e === "string" ? e : "导出失败");
    }
  };

  const toggleEntry = (i: number) => {
    setExpandedEntries(prev => {
      const next = new Set(prev);
      if (next.has(i)) next.delete(i);
      else next.add(i);
      return next;
    });
  };

  const LogTypeIcon = LOG_TYPES.find(t => t.key === logType)?.icon ?? Radio;

  return (
    <div className="space-y-5">
      {/* Sticky Header */}
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm relative">
        <div className="flex items-center gap-3">
          <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">日志分析</h1>
          <span className="px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-[hsl(var(--accent)/0.12)] text-[hsl(var(--accent))] border border-[hsl(var(--accent)/0.25)]">
            AI 驱动
          </span>
        </div>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1">上传日志文件，AI 自动分析异常和安全隐患</p>
      </div>

      <div className="grid grid-cols-5 gap-5">
        {/* ── 左侧：配置区 ── */}
        <div className="col-span-2 space-y-4">

          {/* 日志类型 */}
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-2">日志类型</label>
            <div className="flex gap-2">
              {LOG_TYPES.map(t => {
                const Icon = t.icon;
                const active = logType === t.key;
                return (
                  <button
                    key={t.key}
                    onClick={() => setLogType(t.key)}
                    className={`flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium transition-all border ${
                      active
                        ? "bg-[hsl(var(--accent)/0.1)] text-[hsl(var(--accent))] border-[hsl(var(--accent)/0.3)]"
                        : "bg-[hsl(var(--bg-card))] text-[hsl(var(--text-secondary))] border-[hsl(var(--border))] hover:border-[hsl(var(--text-tertiary))]"
                    }`}
                  >
                    <Icon size={14} />
                    {t.label}
                  </button>
                );
              })}
            </div>
          </div>

          {/* AI 模型选择 */}
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">AI 模型</label>
            <Select value={String(aiConfigId)} onChange={e => setAiConfigId(Number(e.target.value))} className="w-full">
              {aiConfigs.length === 0 && <option value="0">未配置 AI</option>}
              {aiConfigs.map(c => (
                <option key={c.id} value={c.id}>{c.name}</option>
              ))}
            </Select>
          </div>

          {/* 上传区域 */}
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1.5">导入日志</label>
            <div
              onDragOver={e => { e.preventDefault(); setDragOver(true); }}
              onDragLeave={() => setDragOver(false)}
              onDrop={handleDrop}
              onClick={() => fileInputRef.current?.click()}
              className={`border-2 border-dashed rounded-lg p-6 text-center transition-colors cursor-pointer ${
                dragOver
                  ? "border-[hsl(var(--accent))] bg-[hsl(var(--accent-subtle))]"
                  : "border-[hsl(var(--border))] hover:border-[hsl(var(--text-tertiary))]"
              }`}
            >
              <input
                ref={fileInputRef}
                type="file"
                accept=".txt,.log,.csv,.syslog"
                className="hidden"
                onChange={e => { const f = e.target.files?.[0]; if (f) handleFile(f); }}
              />
              <Upload size={24} className="mx-auto mb-1.5 text-[hsl(var(--text-tertiary))]" />
              <p className="text-sm text-[hsl(var(--text-secondary))]">拖拽日志文件到此处</p>
              <p className="text-xs text-[hsl(var(--text-tertiary))] mt-0.5">或点击选择文件（.txt / .log / .csv）</p>
              {fileName && (
                <p className="text-xs text-[hsl(var(--accent))] mt-1.5">
                  <FileText size={12} className="inline mr-1" />{fileName}
                </p>
              )}
            </div>
          </div>

          {/* 粘贴区 */}
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">或直接粘贴日志内容</label>
            <textarea
              value={logText}
              onChange={e => { setLogText(e.target.value); setFileName(""); }}
              placeholder={
                logType === "network"
                  ? '%May 30 11:03:59:450 2026 aHope SSHS/6/SSHS_VERSION_MISMATCH: SSH client...'
                  : logType === "security"
                  ? 'May 30 11:03:59 fw01 kernel: IN=eth0 OUT= MAC=... SRC=10.0.0.1 DST=...'
                  : 'May 30 11:03:59 server1 sshd[1234]: Failed password for root from 10.0.0.1 port 22 ssh2'
              }
              className="w-full h-36 rounded-md border border-[hsl(var(--border))] bg-[hsl(var(--bg-card))] px-3 py-2 text-xs font-mono text-[hsl(var(--text-primary))] placeholder:text-[hsl(var(--text-tertiary))] resize-none focus:outline-none focus:ring-2 focus:ring-[hsl(var(--accent)/0.2)] focus:border-[hsl(var(--accent))] transition-all"
            />
          </div>

          {/* 按钮 */}
          <div className="flex gap-2">
            <Button onClick={handleAnalyze} loading={analyzing} className="flex-1">
              <Search size={14} className="mr-1" />AI 分析日志
            </Button>
            <Button variant="secondary" onClick={() => { setLogText(""); setResult(null); setError(null); setFileName(""); }}>
              清空
            </Button>
          </div>

          {error && (
            <div className="p-3 rounded-lg border text-xs leading-relaxed"
              style={{ backgroundColor: "hsl(var(--danger)/0.08)", borderColor: "hsl(var(--danger)/0.25)", color: "hsl(var(--danger))" }}>
              <AlertCircle size={14} className="inline mr-1 align-text-top" />{error}
            </div>
          )}
        </div>

        {/* ── 右侧：结果区 ── */}
        <div className="col-span-3" ref={resultRef}>
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-sm font-semibold text-[hsl(var(--text-primary))]">分析结果</h2>
            {result && (
              <Button size="sm" variant="secondary" onClick={handleExport}>
                <Download size={14} className="mr-1" />导出报告
              </Button>
            )}
          </div>

          {!result && !analyzing && (
            <div className="flex flex-col items-center justify-center py-16 text-sm text-[hsl(var(--text-tertiary))] border border-dashed border-[hsl(var(--border-light))] rounded-xl">
              <LogTypeIcon size={36} className="mb-3 opacity-30" />
              <p>导入日志后点击"AI 分析日志"查看结果</p>
            </div>
          )}

          {analyzing && (
            <div className="flex flex-col items-center justify-center py-16 border border-dashed border-[hsl(var(--border-light))] rounded-xl relative overflow-hidden">
              {/* 扫描线 */}
              <div className="absolute inset-x-0 h-12 pointer-events-none animate-scan"
                style={{ background: "linear-gradient(180deg, transparent, hsl(var(--accent)/0.08), transparent)" }} />

              {/* 旋转雷达图标 */}
              <div className="relative mb-5">
                <div className="w-16 h-16 rounded-full border-2 border-[hsl(var(--accent)/0.15)] flex items-center justify-center animate-spin"
                  style={{ animationDuration: "3s" }}>
                  <div className="w-12 h-12 rounded-full border-2 border-[hsl(var(--accent)/0.25)] flex items-center justify-center animate-spin"
                    style={{ animationDuration: "2s", animationDirection: "reverse" }}>
                    <Search size={18} className="text-[hsl(var(--accent))]" />
                  </div>
                </div>
                {/* 扫描扇形 */}
                <div className="absolute top-0 left-1/2 -translate-x-1/2 w-16 h-16 overflow-hidden rounded-full animate-spin"
                  style={{ animationDuration: "1.2s" }}>
                  <div className="w-8 h-8 absolute top-0 left-1/2 origin-bottom-left"
                    style={{ background: "linear-gradient(to bottom right, hsl(var(--accent)/0.2), transparent)" }} />
                </div>
              </div>

              <p className="text-sm font-medium text-[hsl(var(--accent))]">AI 正在分析日志</p>
              <div className="flex items-center gap-1 mt-2">
                {[0,1,2].map(i => (
                  <div key={i} className="w-6 h-1 rounded-full bg-[hsl(var(--accent)/0.3)]"
                    style={{ animation: `pulse-bar 1.5s ease-in-out ${i * 0.3}s infinite` }} />
                ))}
              </div>
              <p className="text-[11px] text-[hsl(var(--text-tertiary))] mt-3">
                正在识别异常事件<span className="animate-cursor">▌</span>
              </p>
            </div>
          )}

          {result && (
            <div className="space-y-4 animate-in">
              {/* 总体评价卡 */}
              <div className={`rounded-xl border p-4 ${OVERALL_STYLES[result.overall] || OVERALL_STYLES.info}`}>
                <div className="flex items-center gap-2 mb-1.5">
                  <span className="text-xs font-semibold uppercase tracking-wider">总体评价</span>
                  <span className="text-[11px] px-2 py-0.5 rounded-full font-medium border"
                    style={{
                      backgroundColor: "hsl(var(--bg-card)/0.5)",
                      borderColor: "currentColor",
                    }}>
                    {result.overall === "ok" ? "正常" : result.overall === "warning" ? "警告" : result.overall === "critical" ? "严重" : "注意"}
                  </span>
                </div>
                <p className="text-sm leading-relaxed">{result.summary}</p>
                {result.total_lines !== undefined && result.total_lines > 0 && (
                  <div className="flex items-center gap-2 mt-2 text-[11px] text-[hsl(var(--text-tertiary))]">
                    <span>原始 {result.total_lines} 行</span>
                    {result.kept_lines !== undefined && result.kept_lines < result.total_lines && (
                      <>
                        <span>·</span>
                        <span>AI 分析 {result.kept_lines} 行（按优先级保留关键日志）</span>
                      </>
                    )}
                  </div>
                )}
              </div>

              {/* 条目列表 */}
              {result.entries.length > 0 && (
                <div>
                  <h3 className="text-xs font-semibold text-[hsl(var(--text-tertiary))] uppercase tracking-wider mb-2">
                    详细分析 ({result.entries.length} 条)
                  </h3>
                  <div className="space-y-1.5">
                    {result.entries.map((entry, i) => (
                      <div
                        key={i}
                        className="border border-[hsl(var(--border-light))] rounded-lg overflow-hidden transition-all"
                      >
                        <button
                          onClick={() => toggleEntry(i)}
                          className="w-full flex items-start gap-2.5 px-3 py-2.5 text-left hover:bg-[hsl(var(--bg-hover))] transition-colors"
                        >
                          {entry.severity === "high" ? (
                            <AlertCircle size={14} className="shrink-0 mt-0.5 text-[hsl(var(--danger))]" />
                          ) : entry.severity === "medium" ? (
                            <AlertTriangle size={14} className="shrink-0 mt-0.5 text-[hsl(var(--warning))]" />
                          ) : (
                            <ChevronRight size={14} className={`shrink-0 mt-0.5 text-[hsl(var(--text-tertiary))] transition-transform ${expandedEntries.has(i) ? "rotate-90" : ""}`} />
                          )}
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2 mb-0.5">
                              {entry.level && (
                                <span className="text-[10px] font-medium px-1.5 py-0.5 rounded"
                                  style={{
                                    backgroundColor: entry.level === "ERROR" || entry.level === "CRIT" || entry.level === "EMERG"
                                      ? "hsl(var(--danger)/0.1)" : entry.level === "WARNING" || entry.level === "ALERT"
                                      ? "hsl(var(--warning)/0.1)" : "hsl(var(--info)/0.1)",
                                    color: entry.level === "ERROR" || entry.level === "CRIT" || entry.level === "EMERG"
                                      ? "hsl(var(--danger))" : entry.level === "WARNING" || entry.level === "ALERT"
                                      ? "hsl(var(--warning))" : "hsl(var(--info))",
                                  }}>
                                  {entry.level}
                                </span>
                              )}
                              {entry.time && (
                                <span className="text-[10px] font-mono text-[hsl(var(--text-tertiary))]">{entry.time}</span>
                              )}
                              {entry.source && (
                                <span className="text-[10px] text-[hsl(var(--text-tertiary))]">· {entry.source}</span>
                              )}
                            </div>
                            <div className="text-xs font-mono text-[hsl(var(--text-primary))] break-all line-clamp-2">{entry.content}</div>
                          </div>
                          {entry.severity && (
                            <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium shrink-0 ${SEV_STYLES[entry.severity] || ""}`}>
                              {entry.severity === "high" ? "高危" : entry.severity === "medium" ? "中危" : "低危"}
                            </span>
                          )}
                        </button>

                        {/* 展开详情 */}
                        {expandedEntries.has(i) && entry.analysis && (
                          <div className="px-3 pb-2.5 pt-0 text-xs leading-relaxed border-t border-[hsl(var(--border-light))]"
                            style={{ backgroundColor: "hsl(var(--bg-hover)/0.5)", color: "hsl(var(--text-secondary))" }}>
                            <span className="font-medium text-[hsl(var(--text-primary))]">分析：</span>{entry.analysis}
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* 非结构化结果 */}
              {result.entries.length === 0 && result.raw && (
                <div className="border border-[hsl(var(--border))] rounded-xl p-4">
                  <pre className="text-xs font-mono text-[hsl(var(--text-primary))] whitespace-pre-wrap leading-relaxed max-h-96 overflow-y-auto">{result.raw}</pre>
                </div>
              )}

              {/* 运维建议 */}
              {result.advice && (
                <div className="rounded-xl border p-4" style={{
                  backgroundColor: "hsl(var(--accent)/0.05)",
                  borderColor: "hsl(var(--accent)/0.2)",
                }}>
                  <div className="flex items-center gap-1.5 mb-2">
                    <Lightbulb size={14} className="text-[hsl(var(--accent))]" />
                    <span className="text-xs font-semibold text-[hsl(var(--accent))]">运维建议</span>
                  </div>
                  <p className="text-sm text-[hsl(var(--text-primary))] leading-relaxed">{result.advice}</p>
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
