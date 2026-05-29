import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { InspectionBatch, InspectionRecord } from "../types";
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

  const loadBatches = useCallback(() => {
    invoke<InspectionBatch[]>("list_batches").then(setBatches).catch(console.error);
  }, []);

  useEffect(() => { loadBatches(); }, [loadBatches]);

  const selectBatch = useCallback((batch: InspectionBatch) => {
    setSelectedBatch(batch);
    setSelectedRecord(null);
  }, []);

  const selectRecord = useCallback((record: InspectionRecord) => {
    setSelectedRecord(record);
  }, []);

  const handleAnalyze = (recordId: number) => {
    setAnalyzing(true);
    invoke<InspectionRecord>("analyze_record", { recordId })
      .then((rec) => {
        setSelectedRecord(rec);
        setAnalyzing(false);
      })
      .catch(() => setAnalyzing(false));
  };

  const handleGenerateReport = (recordId: number) => {
    setGenerating(true);
    invoke<string>("generate_report", { recordId })
      .then(() => {
        // Reload record after report generation
        setGenerating(false);
      })
      .catch(() => setGenerating(false));
  };

  const handleDownloadReport = (recordId: number) => {
    setDownloading(true);
    invoke<void>("download_report", { recordId })
      .then(() => setDownloading(false))
      .catch(() => setDownloading(false));
  };

  // Parse command outputs if it's a JSON string with per-command output
  const parsedOutputs = useMemo(() => {
    if (!selectedRecord?.command_outputs) return [];
    try {
      const parsed = JSON.parse(selectedRecord.command_outputs);
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
      <div>
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
            ]}
            data={selectedBatch.records || []}
            rowKey={(r) => r.id}
            onRowClick={(r) => {
              // Convert summary to full record
              const rec: InspectionRecord = {
                id: r.id,
                batch_id: r.batch_id,
                device_id: r.device_id,
                status: r.status,
                command_outputs: "",
                ai_status: r.ai_status,
                ai_result: null,
                ai_analysis: null,
                ai_suggestions: null,
                command_judgments: null,
                summary_judgment: null,
                report_path: r.report_path,
                error_message: r.error_message,
                started_at: null,
                completed_at: null,
                created_at: "",
              };
              selectRecord(rec);
            }}
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
            <div className="flex gap-2">
              <Button size="sm" onClick={() => handleAnalyze(selectedRecord.id)} loading={analyzing}>
                AI 分析
              </Button>
              <Button size="sm" variant="secondary" onClick={() => handleGenerateReport(selectedRecord.id)} loading={generating}>
                生成报告
              </Button>
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
                      // Try to format structured result
                      const parts: string[] = [];
                      if (result.summary) parts.push(`## 总结\n\n${result.summary}`);
                      if (result.details) parts.push(`## 详细分析\n\n${result.details}`);
                      if (result.suggestions) {
                        const suggestions = Array.isArray(result.suggestions) ? result.suggestions : [result.suggestions];
                        parts.push(`## 建议\n\n${suggestions.map((s: string) => `- ${s}`).join("\n")}`);
                      }
                      if (result.items && Array.isArray(result.items)) {
                        parts.push(`## 逐项分析\n\n| 项目 | 状态 | 发现 | 建议 |\n|------|------|------|------|\n${
                          result.items.map((item: { name?: string; status?: string; finding?: string; suggestion?: string }) =>
                            `| ${item.name || "-"} | ${item.status || "-"} | ${item.finding || "-"} | ${item.suggestion || "-"} |`
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
