import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { InspectionBatch, Device, InspectionRecordSummary, InspectionRecord, ReportTemplate } from "../types";
import { useShakeValidation } from "../hooks/useShakeValidation";
import { parseCommandOutputs, parseAiResult } from "../lib/utils";
import DataTable from "../components/DataTable";
import Modal from "../components/Modal";
import Button from "../components/ui/Button";
import Card from "../components/ui/Card";
import Input from "../components/ui/Input";
import StatBadge from "../components/StatBadge";
import StatusBadge from "../components/StatusBadge";
import { batchStatusColor } from "../lib/status";

interface BatchForm {
  name: string;
  device_ids: number[];
  auto_start: boolean;
}

function getDefaultBatchForm(): BatchForm {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const dateStr = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}_${pad(d.getHours())}${pad(d.getMinutes())}`;
  return { name: `巡检_${dateStr}`, device_ids: [], auto_start: false };
}

export default function InspectionPage() {
  const [batches, setBatches] = useState<InspectionBatch[]>([]);
  const [selectedBatch, setSelectedBatch] = useState<InspectionBatch | null>(null);
  const [devices, setDevices] = useState<Device[]>([]);
  const [modalOpen, setModalOpen] = useState(false);
  const [batchForm, setBatchForm] = useState<BatchForm>(getDefaultBatchForm());
  const [confirmDelete, setConfirmDelete] = useState<number | null>(null);
  const [retrying, setRetrying] = useState<number | null>(null);
  const [actionLoading, setActionLoading] = useState<number | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // Report/analysis state
  const [expandedRecordId, setExpandedRecordId] = useState<number | null>(null);
  const [fullRecord, setFullRecord] = useState<InspectionRecord | null>(null);
  const [recordLoading, setRecordLoading] = useState(false);
  const [analyzing, setAnalyzing] = useState<number | null>(null);
  const [batchAnalyzing, setBatchAnalyzing] = useState<number | null>(null);
  const [generating, setGenerating] = useState<number | null>(null);
  const [batchGenerating, setBatchGenerating] = useState(false);
  const [htmlExporting, setHtmlExporting] = useState(false);
  const [logAnalyzing, setLogAnalyzing] = useState(false);
  const [logResult, setLogResult] = useState<Record<string, unknown> | null>(null);
  const [reportTemplates, setReportTemplates] = useState<ReportTemplate[]>([]);
  const [selectedTemplateId, setSelectedTemplateId] = useState<number | null>(null);

  const { shakeFields, triggerShake } = useShakeValidation();

  const loadBatches = useCallback(() => {
    invoke<InspectionBatch[]>("list_batches", { status: undefined })
      .then(setBatches).catch(console.error);
  }, []);

  const loadDevices = useCallback(() => {
    invoke<Device[]>("list_devices", { vendor: undefined, status: undefined })
      .then(setDevices).catch(console.error);
  }, []);

  useEffect(() => { loadBatches(); }, [loadBatches]);
  useEffect(() => { loadDevices(); }, [loadDevices]);
  useEffect(() => {
    invoke<ReportTemplate[]>("list_report_templates").then(setReportTemplates).catch(() => {});
  }, []);

  const refreshSelectedBatch = useCallback(() => {
    if (!selectedBatch) return;
    invoke<InspectionBatch>("get_batch", { batchId: selectedBatch.id })
      .then((b) => {
        setSelectedBatch(b);
        setBatches((prev) => prev.map((bp) => bp.id === b.id ? b : bp));
      })
      .catch(console.error);
  }, [selectedBatch]);

  // Auto-refresh every 3 seconds (both list and detail)
  useEffect(() => {
    const id = setInterval(() => {
      loadBatches();
      if (selectedBatch) refreshSelectedBatch();
    }, 3000);
    return () => clearInterval(id);
  }, [loadBatches, refreshSelectedBatch, selectedBatch]);

  const handleCreateBatch = () => {
    if (!batchForm.name.trim()) { triggerShake("batch_name"); return; }
    if (batchForm.device_ids.length === 0) { triggerShake("batch_devices"); return; }

    const data: Record<string, unknown> = {
      name: batchForm.name,
      device_ids: JSON.stringify(batchForm.device_ids),
    };
    setErrorMsg(null);
    invoke<InspectionBatch>("create_batch", { data, autoStart: batchForm.auto_start })
      .then(() => {
        setModalOpen(false);
        setBatchForm(getDefaultBatchForm());
        loadBatches();
      })
      .catch((e) => {
        setErrorMsg(`创建批次失败: ${typeof e === "string" ? e : JSON.stringify(e)}`);
      });
  };

  const handleAction = (batchId: number, action: string) => {
    const cmdMap: Record<string, string> = {
      run: "run_batch",
      pause: "pause_batch",
      stop: "stop_batch",
      restart: "restart_batch",
    };
    const cmd = cmdMap[action];
    if (!cmd) return;
    setErrorMsg(null);
    setActionLoading(batchId);
    invoke<void>(cmd, { batchId })
      .then(() => {
        setActionLoading(null);
        loadBatches();
        if (selectedBatch?.id === batchId) refreshSelectedBatch();
      })
      .catch((e) => {
        setActionLoading(null);
        const msg = typeof e === "string" ? e : JSON.stringify(e);
        setErrorMsg(`${action === "run" ? "执行" : action === "pause" ? "暂停" : action === "stop" ? "停止" : "重启"}批次失败: ${msg}`);
        loadBatches();
        if (selectedBatch?.id === batchId) refreshSelectedBatch();
      });
  };

  const handleExport = async (batchId: number) => {
    try {
      const path = await save({
        filters: [{ name: "CSV 文件", extensions: ["csv"] }],
        defaultPath: `batch_${batchId}.csv`,
      });
      if (!path) return;
      await invoke<string>("export_batch_csv", { batchId, savePath: path });
    } catch (e) {
      setErrorMsg(`导出失败: ${typeof e === "string" ? e : JSON.stringify(e)}`);
    }
  };

  const handleRetry = (recordId: number) => {
    setRetrying(recordId);
    setErrorMsg(null);
    invoke<void>("retry_device", { recordId })
      .then(() => {
        setRetrying(null);
        refreshSelectedBatch();
        loadBatches();
      })
      .catch((e) => {
        setRetrying(null);
        setErrorMsg(`重试失败: ${typeof e === "string" ? e : JSON.stringify(e)}`);
        refreshSelectedBatch();
        loadBatches();
      });
  };

  const handleDelete = (batchId: number) => {
    setErrorMsg(null);
    invoke<void>("delete_batch", { batchId })
      .then(() => {
        setConfirmDelete(null);
        if (selectedBatch?.id === batchId) setSelectedBatch(null);
        loadBatches();
      })
      .catch((e) => {
        setErrorMsg(`删除失败: ${typeof e === "string" ? e : JSON.stringify(e)}`);
      });
  };

  // --- Report & Analysis handlers ---

  const loadFullRecord = useCallback((recordId: number) => {
    if (expandedRecordId === recordId) {
      setExpandedRecordId(null);
      setFullRecord(null);
      setLogResult(null);
      return;
    }
    setRecordLoading(true);
    setLogResult(null);
    invoke<InspectionRecord>("get_record", { recordId })
      .then((r) => {
        setFullRecord(r);
        setExpandedRecordId(recordId);
        setRecordLoading(false);
      })
      .catch((e) => {
        setRecordLoading(false);
        setErrorMsg(`加载记录失败: ${typeof e === "string" ? e : JSON.stringify(e)}`);
      });
  }, [expandedRecordId]);

  const handleAnalyzeRecord = (recordId: number) => {
    setAnalyzing(recordId);
    setErrorMsg(null);
    invoke("analyze_record", { recordId })
      .then(() => {
        setAnalyzing(null);
        invoke<InspectionRecord>("get_record", { recordId }).then(setFullRecord).catch(console.error);
        refreshSelectedBatch();
        loadBatches();
      })
      .catch((e) => {
        setAnalyzing(null);
        setErrorMsg(`AI 分析失败: ${typeof e === "string" ? e : JSON.stringify(e)}`);
      });
  };

  const handleAnalyzeBatch = (batchId: number) => {
    setBatchAnalyzing(batchId);
    setErrorMsg(null);
    invoke("analyze_batch", { batchId })
      .then(() => {
        setBatchAnalyzing(null);
        if (expandedRecordId) {
          invoke<InspectionRecord>("get_record", { recordId: expandedRecordId }).then(setFullRecord).catch(console.error);
        }
        refreshSelectedBatch();
        loadBatches();
      })
      .catch((e) => {
        setBatchAnalyzing(null);
        setErrorMsg(`批量 AI 分析失败: ${typeof e === "string" ? e : JSON.stringify(e)}`);
      });
  };

  const handleGenerateReport = (recordId: number) => {
    setGenerating(recordId);
    setErrorMsg(null);
    invoke<string>("generate_report", { recordId, templateId: selectedTemplateId })
      .then(() => {
        setGenerating(null);
        invoke<InspectionRecord>("get_record", { recordId }).then(setFullRecord).catch(console.error);
        refreshSelectedBatch();
        loadBatches();
      })
      .catch((e) => {
        setGenerating(null);
        setErrorMsg(`生成报告失败: ${typeof e === "string" ? e : JSON.stringify(e)}`);
      });
  };

  const handleGenerateBatchReports = () => {
    if (!selectedBatch) return;
    setBatchGenerating(true);
    setErrorMsg(null);
    invoke<string[]>("generate_batch_reports", { batchId: selectedBatch.id, templateId: selectedTemplateId })
      .then(() => {
        setBatchGenerating(false);
        refreshSelectedBatch();
        loadBatches();
      })
      .catch((e) => {
        setBatchGenerating(false);
        setErrorMsg(`批量生成报告失败: ${typeof e === "string" ? e : JSON.stringify(e)}`);
      });
  };

  const handleHtmlExport = () => {
    if (!selectedBatch) return;
    setHtmlExporting(true);
    setErrorMsg(null);
    invoke<string>("generate_html_report", { batchId: selectedBatch.id, templateId: selectedTemplateId })
      .then((filePath) => {
        invoke("open_in_browser", { filePath }).catch(console.error);
        setHtmlExporting(false);
      })
      .catch((e) => {
        setHtmlExporting(false);
        setErrorMsg(`导出 HTML 失败: ${typeof e === "string" ? e : JSON.stringify(e)}`);
      });
  };

  const handleGenerateDocx = (recordId: number) => {
    setGenerating(recordId);
    setErrorMsg(null);
    invoke<string>("generate_docx_report", { recordId, templateId: selectedTemplateId })
      .then(() => {
        setGenerating(null);
        invoke<InspectionRecord>("get_record", { recordId }).then(setFullRecord).catch(console.error);
        refreshSelectedBatch();
      })
      .catch((e) => {
        setGenerating(null);
        setErrorMsg(`生成 DOCX 报告失败: ${typeof e === "string" ? e : JSON.stringify(e)}`);
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

  const handleDownloadReport = (recordId: number) => {
    invoke<void>("download_report", { recordId }).catch((e) => {
      setErrorMsg(`下载报告失败: ${typeof e === "string" ? e : JSON.stringify(e)}`);
    });
  };

  // Memoized parsers for expanded record detail
  const parsedOutputs = useMemo(
    () => parseCommandOutputs(fullRecord?.command_outputs),
    [fullRecord?.command_outputs],
  );

  const aiResult = useMemo(
    () => parseAiResult(fullRecord?.ai_result),
    [fullRecord?.ai_result],
  );

  const deviceMap = useMemo(() => {
    const m = new Map<number, Device>();
    for (const d of devices) m.set(d.id, d);
    return m;
  }, [devices]);

  const batchCompleted = selectedBatch?.status === "completed" || selectedBatch?.status === "partially_completed";

  return (
    <div className="flex gap-4" style={{ height: "calc(100vh - 120px)" }}>
      {/* ── Left: Batch list panel ── */}
      <div className="w-[300px] shrink-0 flex flex-col border border-[hsl(var(--border))] rounded-lg bg-[hsl(var(--bg-card))] overflow-hidden">
        <div className="p-3 border-b border-[hsl(var(--border))] space-y-2">
          <div className="flex items-center justify-between">
            <h1 className="text-base font-bold text-[hsl(var(--text-primary))]">巡检批次</h1>
            <Button onClick={() => { setBatchForm(getDefaultBatchForm()); setModalOpen(true); }} size="sm">+</Button>
          </div>
          <p className="text-[11px] text-[hsl(var(--text-tertiary))]">{batches.length} 个批次</p>
        </div>
        <div className="flex-1 overflow-y-auto">
          {batches.length === 0 && (
            <p className="text-xs text-[hsl(var(--text-tertiary))] text-center py-8">暂无巡检批次</p>
          )}
          {batches.map((b) => {
            const selected = selectedBatch?.id === b.id;
            return (
              <div
                key={b.id}
                onClick={() => {
                  setSelectedBatch(b);
                  setExpandedRecordId(null);
                  setFullRecord(null);
                  setLogResult(null);
                  // 立即加载完整批次详情（含 records）
                  invoke<InspectionBatch>("get_batch", { batchId: b.id })
                    .then(setSelectedBatch)
                    .catch(console.error);
                }}
                className={`px-3 py-2.5 cursor-pointer border-l-2 transition-colors ${
                  selected
                    ? "border-l-[hsl(var(--accent))] bg-[hsl(var(--accent)_/_0.08)]"
                    : "border-l-transparent hover:bg-[hsl(var(--bg-hover))]"
                }`}
              >
                <div className="flex items-center justify-between mb-1">
                  <span className="text-sm font-medium text-[hsl(var(--text-primary))] truncate">{b.name || `#${b.id}`}</span>
                  <StatusBadge status={batchStatusColor(b.status)} />
                </div>
                <div className="flex items-center gap-3 text-[11px] text-[hsl(var(--text-tertiary))]">
                  <span>{b.device_ids?.length || 0} 台设备</span>
                  {b.started_at && <span>{new Date(b.started_at).toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" })}</span>}
                </div>
                {/* Quick action buttons */}
                <div className="flex gap-1 mt-1.5" onClick={(e) => e.stopPropagation()}>
                  {(b.status === "pending" || b.status === "waiting") && (
                    <Button size="sm" variant="ghost" loading={actionLoading === b.id} onClick={() => handleAction(b.id, "run")}>执行</Button>
                  )}
                  {b.status === "running" && (
                    <>
                      <Button size="sm" variant="ghost" onClick={() => handleAction(b.id, "pause")}>暂停</Button>
                      <Button size="sm" variant="ghost" onClick={() => handleAction(b.id, "stop")}>停止</Button>
                    </>
                  )}
                  {(b.status === "stopped" || b.status === "paused" || b.status === "failed") && (
                    <Button size="sm" variant="ghost" loading={actionLoading === b.id} onClick={() => handleAction(b.id, "restart")}>重启</Button>
                  )}
                  {(b.status === "completed" || b.status === "partially_completed") && (
                    <Button size="sm" variant="ghost" loading={batchAnalyzing === b.id} onClick={() => handleAnalyzeBatch(b.id)}>AI 分析</Button>
                  )}
                  <Button size="sm" variant="ghost" onClick={() => setConfirmDelete(b.id)}>删除</Button>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* ── Right: Detail panel ── */}
      <div className="flex-1 overflow-y-auto space-y-4">
        {/* Error banner */}
        {errorMsg && (
          <div className="p-3 bg-[hsl(var(--danger)_/_0.1)] border border-[hsl(var(--danger)_/_0.3)] rounded-md text-sm text-[hsl(var(--danger))] flex items-start gap-2">
            <span className="flex-1">{errorMsg}</span>
            <button onClick={() => setErrorMsg(null)} className="text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-primary))] shrink-0">×</button>
          </div>
        )}

        {!selectedBatch ? (
          <div className="flex items-center justify-center h-full text-[hsl(var(--text-tertiary))]">
            <p className="text-sm">← 选择左侧批次查看详情</p>
          </div>
        ) : (
          <>
            <div className="sticky top-0 z-10 bg-[hsl(var(--bg-content))] pb-2">
              <h2 className="text-lg font-semibold text-[hsl(var(--text-primary))]">
                {selectedBatch.name || `批次 #${selectedBatch.id}`}
              </h2>
            </div>

            {/* Batch action toolbar */}
            {batchCompleted && (
              <div className="flex items-center gap-2 flex-wrap">
                <Button size="sm" variant="ghost" loading={batchAnalyzing === selectedBatch.id} onClick={() => handleAnalyzeBatch(selectedBatch.id)}>AI 分析全部</Button>
                <Button size="sm" variant="ghost" loading={batchGenerating} onClick={handleGenerateBatchReports}>生成全部报告</Button>
                <Button size="sm" loading={htmlExporting} onClick={handleHtmlExport}>导出 HTML 报告</Button>
                <Button size="sm" variant="ghost" onClick={() => handleExport(selectedBatch.id)}>导出CSV</Button>
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
              </div>
            )}

            {/* Device list when batch hasn't run yet (no records) */}
            {(!selectedBatch.records || selectedBatch.records.length === 0) && selectedBatch.device_ids && (() => {
              try {
                const raw = selectedBatch.device_ids;
                const ids: number[] = typeof raw === "string" ? JSON.parse(raw) : (raw as unknown as number[]);
                if (!Array.isArray(ids) || ids.length === 0) return null;
                return (
                  <div className="border border-[hsl(var(--border))] rounded-lg p-3">
                    <h3 className="text-sm font-medium text-[hsl(var(--text-primary))] mb-2">
                      待执行设备 ({ids.length})
                    </h3>
                    <div className="grid grid-cols-2 gap-x-6 gap-y-1">
                      {ids.map((id) => {
                        const d = deviceMap.get(id);
                        return (
                          <div key={id} className="flex items-center gap-2 text-xs py-0.5">
                            <span className="w-1.5 h-1.5 rounded-full bg-[hsl(var(--text-tertiary))] shrink-0" />
                            <span className="font-medium text-[hsl(var(--text-primary))]">{d ? d.name : `设备 #${id}`}</span>
                            {d && <span className="text-[hsl(var(--text-tertiary))]">({d.ip})</span>}
                          </div>
                        );
                      })}
                    </div>
                  </div>
                );
              } catch { return null; }
            })()}

            {/* Records table (shown after batch execution) */}
            {selectedBatch.records && selectedBatch.records.length > 0 && (
            <DataTable<InspectionRecordSummary>
              className="table-fixed"
              columns={[
                { key: "device_id", header: "设备", width: "160px", render: (r) => {
                  const d = deviceMap.get(r.device_id);
                  return d ? <span className="text-xs"><span className="font-medium">{d.name}</span> <span className="text-[hsl(var(--text-tertiary))]">({d.ip})</span></span> : `#${r.device_id}`;
                }},
                { key: "status", header: "状态", width: "90px", render: (r) => <StatusBadge status={batchStatusColor(r.status)} /> },
                { key: "ai_status", header: "AI 状态", width: "90px", render: (r) => {
                  if (!r.ai_status || r.ai_status === "none") return <span className="text-[hsl(var(--text-tertiary))]">-</span>;
                  return <StatusBadge status={batchStatusColor(r.ai_status)} />;
                }},
                { key: "report_path", header: "报告", width: "70px", render: (r) =>
                  r.report_path ? <span className="text-[hsl(var(--success))] text-xs">已生成</span> : <span className="text-[hsl(var(--text-tertiary))] text-xs">-</span>,
                },
                { key: "progress", header: "巡检进度", width: "180px", render: (r) => {
                  if (r.ai_status === "processing") return <span className="text-xs text-[hsl(var(--info))]">AI 分析中...</span>;
                  if (r.status === "running" && r.error_message) return <span className="text-xs text-[hsl(var(--info))] truncate block" title={r.error_message}>{r.error_message}</span>;
                  if (r.status === "completed") return <span className="text-xs text-[hsl(var(--success))]" title={r.error_message || ""}>{r.error_message || "已完成"}</span>;
                  if (r.status === "failed") return <span className="text-xs text-[hsl(var(--danger))] truncate block" title={r.error_message || ""}>{r.error_message || "执行失败"}</span>;
                  if (r.status === "stopped") return <span className="text-xs text-[hsl(var(--warning))]">已停止</span>;
                  return <span className="text-xs text-[hsl(var(--text-tertiary))]">等待中</span>;
                }},
                {
                  key: "actions", header: "操作", width: "280px", render: (r) => (
                    <div className="flex gap-1" onClick={(e) => e.stopPropagation()}>
                      {r.status === "failed" && (
                        <Button size="sm" variant="ghost" loading={retrying === r.id} onClick={() => handleRetry(r.id)}>重试</Button>
                      )}
                      {r.status === "completed" && (
                        <>
                          {analyzing === r.id ? (
                            <span className="text-xs text-[hsl(var(--info))]">AI 分析中...</span>
                          ) : r.ai_status === "processing" ? (
                            <Button size="sm" variant="ghost" onClick={() => loadFullRecord(r.id)}>查看进度</Button>
                          ) : (
                            <Button size="sm" variant="ghost" onClick={() => handleAnalyzeRecord(r.id)}>AI 分析</Button>
                          )}
                          <Button size="sm" variant="ghost" loading={generating === r.id} onClick={() => handleGenerateReport(r.id)}>生成报告</Button>
                          <Button size="sm" variant="ghost" loading={generating === r.id} onClick={() => handleGenerateDocx(r.id)}>DOCX</Button>
                        </>
                      )}
                      {r.report_path && (
                        <Button size="sm" variant="ghost" onClick={() => handleDownloadReport(r.id)}>下载</Button>
                      )}
                    </div>
                  ),
                },
              ]}
              data={selectedBatch.records || []}
              rowKey={(r) => r.id}
              onRowClick={(r) => loadFullRecord(r.id)}
              selectedKey={expandedRecordId ?? undefined}
              emptyText="暂无记录"
            />
            )}

            {/* Expanded record detail panel */}
            {expandedRecordId && fullRecord && (
              <Card>
                <div className="flex items-center justify-between mb-4">
                  <h3 className="text-base font-semibold text-[hsl(var(--text-primary))]">
                    {(() => { const d = deviceMap.get(fullRecord.device_id); return d ? `${d.name} (${d.ip})` : `设备 #${fullRecord.device_id}`; })()} 详情
                  </h3>
                  <div className="flex gap-2 items-center flex-wrap">
                    <Button size="sm" variant="ghost" loading={logAnalyzing} onClick={() => handleLogAnalyze(fullRecord.id)}>分析日志</Button>
                    <Button size="sm" variant="ghost" loading={analyzing === fullRecord.id} onClick={() => handleAnalyzeRecord(fullRecord.id)}>AI 分析</Button>
                    <Button size="sm" variant="secondary" loading={generating === fullRecord.id} onClick={() => handleGenerateReport(fullRecord.id)}>生成报告</Button>
                    <Button size="sm" variant="secondary" loading={generating === fullRecord.id} onClick={() => handleGenerateDocx(fullRecord.id)}>生成 DOCX</Button>
                    {fullRecord.report_path && (
                      <Button size="sm" variant="ghost" onClick={() => handleDownloadReport(fullRecord.id)}>下载报告</Button>
                    )}
                    <button
                      onClick={() => { setExpandedRecordId(null); setFullRecord(null); setLogResult(null); }}
                      className="text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-primary))] text-lg leading-none px-1"
                    >×</button>
                  </div>
                </div>

                {/* Status bar */}
                <div className="flex gap-4 mb-4 text-sm flex-wrap items-center">
                  <span>状态: <StatusBadge status={batchStatusColor(fullRecord.status)} /></span>
                  <span>AI 状态: {fullRecord.ai_status ? <StatusBadge status={batchStatusColor(fullRecord.ai_status)} /> : <span className="text-[hsl(var(--text-tertiary))]">-</span>}</span>
                  {fullRecord.error_message && fullRecord.status === "running" && (
                    <span className="text-xs text-[hsl(var(--info))] font-mono">进度: {fullRecord.error_message}</span>
                  )}
                  {fullRecord.error_message && fullRecord.status === "failed" && (
                    <span className="text-xs text-[hsl(var(--danger))]">失败原因: {fullRecord.error_message}</span>
                  )}
                  {fullRecord.error_message && fullRecord.status === "completed" && (
                    <span className="text-xs text-[hsl(var(--text-secondary))]">{fullRecord.error_message}</span>
                  )}
                  {fullRecord.started_at && (
                    <span className="text-xs text-[hsl(var(--text-tertiary))]">开始: {new Date(fullRecord.started_at).toLocaleTimeString("zh-CN")}</span>
                  )}
                  {fullRecord.completed_at && (
                    <span className="text-xs text-[hsl(var(--text-tertiary))]">完成: {new Date(fullRecord.completed_at).toLocaleTimeString("zh-CN")}</span>
                  )}
                </div>

                {/* Command outputs */}
                {parsedOutputs.length > 0 && (
                  <div className="mb-4">
                    <h4 className="text-sm font-medium text-[hsl(var(--text-primary))] mb-2">命令输出</h4>
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

                {/* Log analysis */}
                {logResult && !logResult.error && logResult.entries && (
                  <div className="mb-4">
                    <h4 className="text-sm font-medium text-[hsl(var(--text-primary))] mb-2">日志分析</h4>
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

                {/* AI analysis result */}
                {aiResult && (
                  <div className="mb-4">
                    <h4 className="text-sm font-medium text-[hsl(var(--text-primary))] mb-2">AI 分析结果</h4>
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

                {/* Report path */}
                {fullRecord.report_path && (
                  <div>
                    <h4 className="text-sm font-medium text-[hsl(var(--text-primary))] mb-2">报告路径</h4>
                    <p className="text-xs text-[hsl(var(--text-secondary))] font-mono bg-[hsl(var(--bg-hover))] px-2 py-1 rounded">
                      {fullRecord.report_path}
                    </p>
                  </div>
                )}

                {/* AI analysis progress */}
                {fullRecord.ai_status === "processing" && (
                  <div className="mb-4 p-3 bg-[hsl(var(--info)_/_0.08)] border border-[hsl(var(--info)_/_0.2)] rounded-md flex items-center gap-2">
                    <span className="w-4 h-4 border-2 border-[hsl(var(--info))] border-t-transparent rounded-full animate-spin" />
                    <span className="text-sm text-[hsl(var(--info))]">AI 正在分析 {(() => { const d = deviceMap.get(fullRecord.device_id); return d ? d.name : ""; })()} 的巡检数据...</span>
                  </div>
                )}

                {recordLoading && (
                  <div className="text-center py-4 text-sm text-[hsl(var(--text-tertiary))]">加载中...</div>
                )}
              </Card>
            )}
          </>
        )}
      </div>

      {/* Create batch modal */}
      <Modal
        open={modalOpen}
        title="创建巡检批次"
        width="max-w-xl"
        onClose={() => setModalOpen(false)}
        footer={
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => setModalOpen(false)}>取消</Button>
            <Button onClick={handleCreateBatch}>创建</Button>
          </div>
        }
      >
        <div className="space-y-3">
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">批次名称</label>
            <Input value={batchForm.name} className={shakeFields.has("batch_name") ? "animate-shake" : ""} onChange={(e) => setBatchForm({ ...batchForm, name: e.target.value })} placeholder="例如: 核心交换机周检" />
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">选择设备</label>
            <div className={`max-h-48 overflow-y-auto border rounded-md p-2 space-y-1 ${shakeFields.has("batch_devices") ? "animate-shake" : "border-[hsl(var(--border))]"}`}>
              {devices.length === 0 && <p className="text-xs text-[hsl(var(--text-tertiary))]">暂无设备</p>}
              {devices.map((d) => {
                const checked = batchForm.device_ids.includes(d.id);
                const noTemplate = !d.template_id;
                return (
                  <label key={d.id} className="flex items-center gap-2 cursor-pointer hover:bg-[hsl(var(--bg-hover))] rounded px-1 py-0.5">
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() => {
                        setBatchForm({
                          ...batchForm,
                          device_ids: checked
                            ? batchForm.device_ids.filter((id) => id !== d.id)
                            : [...batchForm.device_ids, d.id],
                        });
                      }}
                      className="accent-[hsl(var(--accent))]"
                    />
                    <span className="text-xs">{d.name} ({d.ip})</span>
                    {noTemplate && (
                      <span className="text-[10px] text-[hsl(var(--warning))]" title="未关联巡检模板，将无法执行巡检">⚠ 未配置模板</span>
                    )}
                  </label>
                );
              })}
            </div>
          </div>
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={batchForm.auto_start}
              onChange={(e) => setBatchForm({ ...batchForm, auto_start: e.target.checked })}
              className="accent-[hsl(var(--accent))]"
            />
            <span className="text-xs text-[hsl(var(--text-secondary))]">创建后自动开始执行</span>
          </label>
        </div>
      </Modal>

      {/* Delete confirm */}
      <Modal
        open={confirmDelete !== null}
        title="确认删除"
        width="max-w-sm"
        onClose={() => setConfirmDelete(null)}
        footer={
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => setConfirmDelete(null)}>取消</Button>
            <Button variant="danger" onClick={() => handleDelete(confirmDelete!)}>删除</Button>
          </div>
        }
      >
        <p>确定要删除此批次吗？此操作不可恢复。</p>
      </Modal>
    </div>
  );
}

