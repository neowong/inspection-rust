import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { InspectionBatch, InspectionRecord, ReportTemplate } from "../types";
import DataTable from "../components/DataTable";
import Button from "../components/ui/Button";
import Card from "../components/ui/Card";
import StatusBadge from "../components/StatusBadge";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { batchStatusColor } from "../lib/status";

export default function ReportsPage() {
  const [batches, setBatches] = useState<InspectionBatch[]>([]);
  const [selectedBatch, setSelectedBatch] = useState<InspectionBatch | null>(null);
  const [selectedRecord, setSelectedRecord] = useState<InspectionRecord | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [logAnalyzing, setLogAnalyzing] = useState(false);
  const [logResult, setLogResult] = useState<Record<string, unknown> | null>(null);
  const [htmlExporting, setHtmlExporting] = useState(false);
  const [reportTemplates, setReportTemplates] = useState<ReportTemplate[]>([]);
  const [selectedTemplateId, setSelectedTemplateId] = useState<number | null>(null);

  const loadBatches = useCallback(() => {
    invoke<InspectionBatch[]>("list_batches").then(setBatches).catch(console.error);
  }, []);

  useEffect(() => { loadBatches(); }, [loadBatches]);
  useEffect(() => {
    invoke<ReportTemplate[]>("list_report_templates").then(setReportTemplates).catch(() => {});
  }, []);

  const loadRecord = useCallback((recordId: number) => {
    invoke<InspectionRecord>("get_record", { recordId })
      .then(setSelectedRecord)
      .catch(console.error);
  }, []);

  const selectBatch = useCallback((batch: InspectionBatch) => {
    setSelectedBatch(batch);
    setSelectedRecord(null);
  }, []);

  const handleAnalyze = (recordId: number) => {
    setAnalyzing(true);
    invoke("analyze_record", { recordId })
      .then(() => {
        loadRecord(recordId); // reload full record with AI data
        setAnalyzing(false);
        loadBatches();
      })
      .catch(() => setAnalyzing(false));
  };

  const handleGenerateReport = (recordId: number) => {
    setGenerating(true);
    invoke<string>("generate_report", { recordId, templateId: selectedTemplateId })
      .then(() => {
        setGenerating(false);
        loadRecord(recordId);
        loadBatches();
      })
      .catch(() => setGenerating(false));
  };

  const handleDownloadReport = (recordId: number) => {
    setDownloading(true);
    invoke<void>("download_report", { recordId })
      .then(() => setDownloading(false))
      .catch(() => setDownloading(false));
  };

  const handleHtmlExport = (batchId: number) => {
    setHtmlExporting(true);
    invoke<string>("generate_html_report", { batchId, templateId: selectedTemplateId })
      .then((filePath) => {
        invoke("open_in_browser", { filePath }).catch(console.error);
        setHtmlExporting(false);
      })
      .catch((e) => {
        console.error(e);
        setHtmlExporting(false);
      });
  };

  const handleLogAnalyze = (recordId: number) => {
    setLogAnalyzing(true);
    setLogResult(null);
    invoke<Record<string, unknown>>("analyze_record_logs", { recordId })
      .then((r) => setLogResult(r))
      .catch((e) => setLogResult({ error: typeof e === "string" ? e : JSON.stringify(e) }))
      .finally(() => setLogAnalyzing(false));
  };

  // Parse command outputs (HashMap<string, string> → array of {command, content})
  const parsedOutputs = useMemo(() => {
    if (!selectedRecord?.command_outputs) return [];
    try {
      const parsed = JSON.parse(selectedRecord.command_outputs);
      // Object form: { "display version": "output...", ... }
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        return Object.entries(parsed).map(([command, content]) => ({
          command,
          content: typeof content === "string" ? content : JSON.stringify(content),
        }));
      }
      // Array form fallback
      if (Array.isArray(parsed)) return parsed;
      return [{ command: "output", content: selectedRecord.command_outputs }];
    } catch {
      return [{ command: "output", content: selectedRecord.command_outputs }];
    }
  }, [selectedRecord?.command_outputs]);

  // Parse AI result
  const aiResult = useMemo(() => {
    if (!selectedRecord?.ai_result) return null;
    try {
      return JSON.parse(selectedRecord.ai_result);
    } catch {
      return null;
    }
  }, [selectedRecord?.ai_result]);

  return (
    <div className="space-y-6">
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm relative">
        <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">巡检报告</h1>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1">查看巡检结果、AI 分析和生成报告</p>
      </div>

      {/* Batch list */}
      <Card>
        <h2 className="text-base font-semibold text-[hsl(var(--text-primary))] mb-3">巡检批次</h2>
        <DataTable<InspectionBatch>
          columns={[
            { key: "id", header: "ID", width: "60px", render: (r) => `#${r.id}` },
            { key: "name", header: "名称", render: (r) => r.name || "-" },
            { key: "status", header: "状态", render: (r) => <StatusBadge status={batchStatusColor(r.status)} /> },
            { key: "device_count", header: "设备数", width: "80px", render: (r) => String(r.device_ids?.length || 0) },
            { key: "started_at", header: "开始时间", render: (r) => r.started_at ? new Date(r.started_at).toLocaleString("zh-CN") : "-" },
            { key: "completed_at", header: "完成时间", render: (r) => r.completed_at ? new Date(r.completed_at).toLocaleString("zh-CN") : "-" },
          ]}
          data={batches}
          rowKey={(r) => r.id}
          onRowClick={(r) => selectBatch(r)}
          selectedKey={selectedBatch?.id}
          emptyText="暂无批次"
        />
      </Card>

      {/* Record list */}
      {selectedBatch && (
        <Card>
          <h2 className="text-base font-semibold text-[hsl(var(--text-primary))] mb-3">
            记录: {selectedBatch.name || `#${selectedBatch.id}`}
          </h2>
          <DataTable<InspectionBatch["records"][0]>
            columns={[
              { key: "device_id", header: "设备 ID", width: "80px", render: (r) => `#${r.device_id}` },
              { key: "status", header: "状态", render: (r) => <StatusBadge status={batchStatusColor(r.status)} /> },
              { key: "ai_status", header: "AI 状态", render: (r) => {
                if (!r.ai_status || r.ai_status === "none") return <span className="text-[hsl(var(--text-tertiary))]">-</span>;
                return <StatusBadge status={batchStatusColor(r.ai_status)} />;
              }},
              { key: "report_path", header: "报告", render: (r) =>
                r.report_path ? <span className="text-[hsl(var(--success))] text-xs">已生成</span> : "-",
              },
              { key: "actions", header: "操作", width: "100px", render: (r) => (
                <div className="flex gap-1" onClick={(e) => e.stopPropagation()}>
                  <Button size="sm" variant="ghost" onClick={() => { loadRecord(r.id); handleLogAnalyze(r.id); }}>分析日志</Button>
                </div>
              )},
            ]}
            data={selectedBatch.records || []}
            rowKey={(r) => r.id}
            onRowClick={(r) => loadRecord(r.id)}
            selectedKey={selectedRecord?.id}
            emptyText="暂无记录"
          />
        </Card>
      )}

      {/* Record detail */}
      {selectedRecord && (
        <Card>
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-base font-semibold text-[hsl(var(--text-primary))]">
              设备 #{selectedRecord.device_id} 详情
            </h2>
            <div className="flex gap-2 items-center flex-wrap">
              <Button size="sm" onClick={() => handleLogAnalyze(selectedRecord.id)} loading={logAnalyzing}>
                分析日志
              </Button>
              <Button size="sm" onClick={() => handleAnalyze(selectedRecord.id)} loading={analyzing}>
                AI 分析
              </Button>
              <Button size="sm" variant="secondary" onClick={() => handleGenerateReport(selectedRecord.id)} loading={generating}>
                生成报告
              </Button>
              <Button size="sm" variant="primary" onClick={() => selectedBatch && handleHtmlExport(selectedBatch.id)} loading={htmlExporting}>
                导出 HTML 报告
              </Button>
              {reportTemplates.length > 0 && (
                <select
                  value={selectedTemplateId ?? ""}
                  onChange={(e) => setSelectedTemplateId(e.target.value ? Number(e.target.value) : null)}
                  className="h-7 px-2 text-xs rounded-md border border-[hsl(var(--border))] bg-[hsl(var(--bg-card))] text-[hsl(var(--text-secondary))] focus:outline-none focus:border-[hsl(var(--accent))]"
                >
                  <option value="">模板: 跟随默认</option>
                  {reportTemplates.map((rt) => (
                    <option key={rt.id} value={rt.id}>{rt.name}{rt.is_default ? " (默认)" : ""}</option>
                  ))}
                </select>
              )}
              {selectedRecord.report_path && (
                <Button size="sm" variant="secondary" onClick={() => handleDownloadReport(selectedRecord.id)} loading={downloading}>
                  下载报告
                </Button>
              )}
            </div>
          </div>

          {/* Status info */}
          <div className="flex gap-4 mb-4 text-sm">
            <span>状态: <StatusBadge status={batchStatusColor(selectedRecord.status)} /></span>
            <span>AI 状态: {selectedRecord.ai_status ? <StatusBadge status={batchStatusColor(selectedRecord.ai_status)} /> : <span className="text-[hsl(var(--text-tertiary))]">-</span>}</span>
            {selectedRecord.error_message && (
              <span className="text-[hsl(var(--danger))]">错误: {selectedRecord.error_message}</span>
            )}
          </div>

          {/* Command outputs */}
          {parsedOutputs.length > 0 && (
            <div className="mb-4">
              <h3 className="text-sm font-medium text-[hsl(var(--text-primary))] mb-2">命令输出</h3>
              <div className="space-y-2">
                {parsedOutputs.map((item: { command?: string; content?: string }, i: number) => (
                  <details key={i} className="border border-[hsl(var(--border))] rounded-md overflow-hidden">
                    <summary className="px-3 py-1.5 bg-[hsl(var(--bg-hover))] cursor-pointer text-xs font-mono text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))]">
                      {item.command || `输出 #${i + 1}`}
                    </summary>
                    <pre className="p-3 text-xs font-mono whitespace-pre-wrap overflow-auto max-h-60 text-[hsl(var(--text-primary))] bg-[hsl(var(--bg-app))]">
                      {item.content || "(空)"}
                    </pre>
                  </details>
                ))}
              </div>
            </div>
          )}

          {/* Log Analysis Result */}
          {logResult && !logResult.error && logResult.entries && (
            <div className="mb-4">
              <h3 className="text-sm font-medium text-[hsl(var(--text-primary))] mb-2">日志分析</h3>
              <div className="grid grid-cols-4 gap-3 mb-3">
                <StatBadge label="总计" value={String(logResult.total ?? 0)} color="info" />
                <StatBadge label="ERROR" value={String(logResult.errors ?? 0)} color="danger" />
                <StatBadge label="WARNING" value={String(logResult.warnings ?? 0)} color="warning" />
                <StatBadge label="INFO/DEBUG" value={String(Number(logResult.info ?? 0) + Number(logResult.debug ?? 0))} color="text-secondary" />
              </div>
              <p className="text-xs text-[hsl(var(--text-secondary))] mb-2">{logResult.summary as string}</p>
              <div className="border border-[hsl(var(--border))] rounded-md overflow-hidden max-h-80 overflow-y-auto">
                <table className="w-full text-xs">
                  <thead className="bg-[hsl(var(--bg-hover))] sticky top-0">
                    <tr>
                      <th className="px-2 py-1.5 text-left font-medium text-[hsl(var(--text-secondary))] w-[140px]">时间</th>
                      <th className="px-2 py-1.5 text-left font-medium text-[hsl(var(--text-secondary))] w-[60px]">级别</th>
                      <th className="px-2 py-1.5 text-left font-medium text-[hsl(var(--text-secondary))] w-[80px]">模块</th>
                      <th className="px-2 py-1.5 text-left font-medium text-[hsl(var(--text-secondary))]">消息</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-[hsl(var(--border-light))]">
                    {(logResult.entries as Array<{timestamp: string; severity: string; module: string; mnemonic: string; message: string}>).map((e, i) => (
                      <tr key={i} className="hover:bg-[hsl(var(--bg-hover))]">
                        <td className="px-2 py-1 font-mono text-[hsl(var(--text-tertiary))]">{e.timestamp}</td>
                        <td className="px-2 py-1">
                          <span className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${
                            e.severity === "ERROR" || e.severity === "CRIT" || e.severity === "EMERG" ? "bg-[hsl(var(--danger)_/_0.1)] text-[hsl(var(--danger))]" :
                            e.severity === "WARNING" || e.severity === "NOTICE" ? "bg-[hsl(var(--warning)_/_0.1)] text-[hsl(var(--warning))]" :
                            "bg-[hsl(var(--bg-hover))] text-[hsl(var(--text-secondary))]"
                          }`}>{e.severity}</span>
                        </td>
                        <td className="px-2 py-1 text-[hsl(var(--text-secondary))]">{e.module}/{e.mnemonic}</td>
                        <td className="px-2 py-1 text-[hsl(var(--text-primary))]">{e.message}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
          {logResult?.error && (
            <div className="mb-4 p-3 bg-[hsl(var(--danger)_/_0.1)] border border-[hsl(var(--danger)_/_0.3)] rounded text-sm text-[hsl(var(--danger))]">
              {String(logResult.error)}
            </div>
          )}

          {/* AI Analysis Result */}
          {aiResult && (
            <div className="mb-4">
              <h3 className="text-sm font-medium text-[hsl(var(--text-primary))] mb-2">AI 分析结果</h3>
              <div className="border border-[hsl(var(--border))] rounded-md p-4 bg-[hsl(var(--bg-app))]">
                <div className="prose prose-sm max-w-none text-[hsl(var(--text-primary))] [&_h1]:text-lg [&_h2]:text-base [&_h3]:text-sm [&_h1]:font-semibold [&_h2]:font-semibold [&_h3]:font-medium [&_h1]:mt-4 [&_h2]:mt-3 [&_h3]:mt-2 [&_p]:my-1 [&_ul]:my-1 [&_ol]:my-1 [&_li]:my-0.5 [&_code]:text-xs [&_code]:bg-[hsl(var(--bg-hover))] [&_code]:px-1 [&_code]:rounded [&_pre]:bg-[hsl(var(--bg-card))] [&_pre]:p-3 [&_pre]:rounded-md [&_pre]:overflow-auto [&_pre]:max-h-60 [&_pre]:text-xs [&_table]:w-full [&_table]:text-xs [&_th]:text-left [&_th]:px-2 [&_th]:py-1 [&_th]:bg-[hsl(var(--bg-hover))] [&_td]:px-2 [&_td]:py-1 [&_td]:border-b [&_td]:border-[hsl(var(--border-light))]]">
                  <ReactMarkdown remarkPlugins={[remarkGfm]}>
                    {(() => {
                      const result = aiResult;
                      if (!result) return "";
                      if (typeof result === "string") return result;
                      const parts: string[] = [];
                      if (result.summary) {
                        const overall = result.overall ? ` [${result.overall}]` : "";
                        parts.push(`## 总结${overall}\n\n${result.summary}`);
                      }
                      if (result.items && Array.isArray(result.items)) {
                        parts.push(`## 逐项分析\n\n| 命令 | 状态 | 发现 | 建议 |\n|------|------|------|------|\n${
                          result.items.map((item: { command?: string; title?: string; status?: string; finding?: string; suggestion?: string }) =>
                            `| ${item.title || item.command || "-"} | ${item.status || "-"} | ${item.finding || "-"} | ${item.suggestion || "-"} |`
                          ).join("\n")
                        }`);
                      }
                      return parts.join("\n\n") || JSON.stringify(result, null, 2);
                    })()}
                  </ReactMarkdown>
                </div>
              </div>
            </div>
          )}

          {/* Report preview */}
          {selectedRecord.report_path && (
            <div>
              <h3 className="text-sm font-medium text-[hsl(var(--text-primary))] mb-2">报告路径</h3>
              <p className="text-xs text-[hsl(var(--text-secondary))] font-mono bg-[hsl(var(--bg-hover))] px-2 py-1 rounded">
                {selectedRecord.report_path}
              </p>
            </div>
          )}
        </Card>
      )}
    </div>
  );
}

function StatBadge({ label, value, color }: { label: string; value: string; color: string }) {
  const c = color.startsWith("text") ? `hsl(var(--${color}))` : `hsl(var(--${color}))`;
  const bg = color.startsWith("text") ? "bg-[hsl(var(--bg-hover))]" : `bg-[hsl(var(--${color})_/_0.1)]`;
  return (
    <div className={`rounded-lg ${bg} px-3 py-2 text-center`}>
      <div className="text-lg font-bold" style={{ color: c }}>{value}</div>
      <div className="text-[10px] uppercase tracking-wide" style={{ color: c, opacity: 0.7 }}>{label}</div>
    </div>
  );
}
