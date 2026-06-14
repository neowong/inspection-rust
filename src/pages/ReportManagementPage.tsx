import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { InspectionRecord, Device } from "../types";
import { parseCommandOutputs, parseAiResult } from "../lib/utils";
import DataTable from "../components/DataTable";
import Button from "../components/ui/Button";
import Card from "../components/ui/Card";
import StatBadge from "../components/StatBadge";
import StatusBadge from "../components/StatusBadge";
import { batchStatusColor } from "../lib/status";

const STATUS_META: Record<string, { label: string; color: string }> = {
  ok:       { label: "正常", color: "var(--success)" },
  info:     { label: "提示", color: "var(--info)" },
  warning:  { label: "注意", color: "var(--warning)" },
  critical: { label: "严重", color: "var(--danger)" },
};

export default function ReportManagementPage() {
  const [batches, setBatches] = useState<any[]>([]);
  const [selectedBatch, setSelectedBatch] = useState<any>(null);
  const [devices, setDevices] = useState<Device[]>([]);

  // Selected record detail
  const [expandedRecordId, setExpandedRecordId] = useState<number | null>(null);
  const [fullRecord, setFullRecord] = useState<InspectionRecord | null>(null);
  const [recordLoading, setRecordLoading] = useState(false);

  // AI analysis
  const [analyzing, setAnalyzing] = useState<number | null>(null);
  const [batchAnalyzing, setBatchAnalyzing] = useState<number | null>(null);
  // Report generation
  const [generating, setGenerating] = useState<number | null>(null);
  const [batchGenerating, setBatchGenerating] = useState<"" | "zip" | "combined">("");
  // Log analysis
  const [logAnalyzing, setLogAnalyzing] = useState(false);
  const [logResult, setLogResult] = useState<Record<string, unknown> | null>(null);
  // Download / delete
  const [downloading, setDownloading] = useState<number | null>(null);
  const [deleting, setDeleting] = useState<number | null>(null);
  // Error
  const [errorMsg, setErrorMsg] = useState("");
  const [info, setInfo] = useState("");

  useEffect(() => {
    loadBatches();
    invoke<Device[]>("list_devices", {}).then(setDevices).catch(console.error);
  }, []);

  const deviceMap = useMemo(() => {
    const m = new Map<number, Device>();
    for (const d of devices) m.set(d.id, d);
    return m;
  }, [devices]);

  const loadBatches = useCallback(() => {
    invoke<any[]>("list_batches", { status: undefined })
      .then(setBatches)
      .catch(console.error);
  }, []);

  const selectBatch = async (batch: any) => {
    setSelectedBatch(batch);
    setExpandedRecordId(null);
    setFullRecord(null);
    setLogResult(null);
    try {
      const full: any = await invoke("get_batch", { batchId: batch.id });
      setSelectedBatch(full);
    } catch (e) { console.error(e); }
  };

  // Auto-refresh
  useEffect(() => {
    const timer = setInterval(() => {
      loadBatches();
      if (selectedBatch?.id) {
        invoke<any>("get_batch", { batchId: selectedBatch.id }).then(setSelectedBatch).catch(() => {});
      }
    }, 3000);
    return () => clearInterval(timer);
  }, [selectedBatch?.id, loadBatches]);

  // ----- Record detail -----
  const loadRecordDetail = useCallback((recordId: number) => {
    setExpandedRecordId(recordId);
    setRecordLoading(true);
    invoke<InspectionRecord>("get_record", { recordId })
      .then(setFullRecord)
      .catch((e) => setErrorMsg(String(e)))
      .finally(() => setRecordLoading(false));
  }, []);

  const refreshAfterMutation = useCallback((recordId?: number) => {
    if (recordId) {
      invoke<InspectionRecord>("get_record", { recordId }).then(setFullRecord).catch(console.error);
    }
    if (selectedBatch?.id) {
      invoke<any>("get_batch", { batchId: selectedBatch.id }).then(setSelectedBatch).catch(() => {});
    }
    loadBatches();
  }, [selectedBatch?.id, loadBatches]);

  // ----- AI Analysis -----
  const handleAnalyzeRecord = (recordId: number) => {
    setAnalyzing(recordId); setErrorMsg("");
    invoke("analyze_record", { recordId })
      .then(() => { setAnalyzing(null); refreshAfterMutation(recordId); })
      .catch((e) => { setAnalyzing(null); setErrorMsg(String(e)); });
  };

  const handleAnalyzeBatch = (batchId: number) => {
    setBatchAnalyzing(batchId); setErrorMsg("");
    invoke("analyze_batch", { batchId })
      .then(() => { setBatchAnalyzing(null); refreshAfterMutation(expandedRecordId ?? undefined); })
      .catch((e) => { setBatchAnalyzing(null); setErrorMsg(String(e)); });
  };

  // ----- DOCX Generation -----
  const handleGenerateDocx = (recordId: number) => {
    setGenerating(recordId); setErrorMsg(""); setInfo("");
    invoke<string>("generate_docx_report", { recordId })
      .then(() => {
        setGenerating(null);
        setInfo("报告已生成");
        setTimeout(() => setInfo(""), 2500);
        refreshAfterMutation(recordId);
      })
      .catch((e) => { setGenerating(null); setErrorMsg(String(e)); });
  };

  const handleBatchZip = async () => {
    if (!selectedBatch) return;
    setBatchGenerating("zip"); setErrorMsg(""); setInfo("");
    try {
      const path = await invoke<string>("generate_batch_docx_zip", { batchId: selectedBatch.id });
      const safeName = (selectedBatch.name || `batch_${selectedBatch.id}`).replace(/[/\\:*?"<>|]/g, "_");
      await invoke("save_generated_file", {
        sourcePath: path,
        suggestedName: `${safeName}-巡检报告.zip`,
        extension: "zip",
      });
      setInfo("已生成 ZIP 报告，请在保存对话框选择目标位置");
      setTimeout(() => setInfo(""), 3000);
    } catch (e) {
      setErrorMsg(String(e));
    } finally {
      setBatchGenerating("");
    }
  };

  const handleBatchCombined = async () => {
    if (!selectedBatch) return;
    setBatchGenerating("combined"); setErrorMsg(""); setInfo("");
    try {
      const path = await invoke<string>("generate_batch_docx_combined", { batchId: selectedBatch.id });
      const safeName = (selectedBatch.name || `batch_${selectedBatch.id}`).replace(/[/\\:*?"<>|]/g, "_");
      await invoke("save_generated_file", {
        sourcePath: path,
        suggestedName: `${safeName}-合并报告.docx`,
        extension: "docx",
      });
      setInfo("已生成合并 DOCX，请在保存对话框选择目标位置");
      setTimeout(() => setInfo(""), 3000);
    } catch (e) {
      setErrorMsg(String(e));
    } finally {
      setBatchGenerating("");
    }
  };

  // ----- Log analysis -----
  const handleLogAnalyze = (recordId: number) => {
    setLogAnalyzing(true);
    setLogResult(null);
    invoke<Record<string, unknown>>("analyze_record_logs", { recordId })
      .then(setLogResult)
      .catch((e) => setLogResult({ error: String(e) }))
      .finally(() => setLogAnalyzing(false));
  };

  // ----- Download / Delete -----
  const handleDownload = (recordId: number) => {
    setDownloading(recordId);
    invoke("download_report", { recordId })
      .catch((e) => setErrorMsg(String(e)))
      .finally(() => setDownloading(null));
  };

  const handleDelete = async (recordId: number) => {
    if (!confirm("确定删除此报告？文件将被清除。")) return;
    setDeleting(recordId);
    try {
      await invoke("delete_record_report", { recordId });
      if (expandedRecordId === recordId) {
        setExpandedRecordId(null);
        setFullRecord(null);
      }
      refreshAfterMutation();
    } catch (e: any) {
      setErrorMsg(String(e));
    } finally {
      setDeleting(null);
    }
  };

  // ----- Memoized -----
  const parsedOutputs = useMemo(() => parseCommandOutputs(fullRecord?.command_outputs), [fullRecord?.command_outputs]);
  const aiResult = useMemo(() => parseAiResult(fullRecord?.ai_result), [fullRecord?.ai_result]);

  const batchCompleted = selectedBatch?.status === "completed" || selectedBatch?.status === "partially_completed";

  const recordColumns = [
    { key: "device", header: "设备", render: (r: any) => {
      const d = deviceMap.get(r.device_id);
      return d ? <span>{d.name} <span className="text-[hsl(var(--text-tertiary))]">{d.ip}</span></span> : `#${r.device_id}`;
    }},
    { key: "status", header: "状态", width: "w-24", render: (r: any) => <StatusBadge status={batchStatusColor(r.status)} /> },
    { key: "ai_status", header: "AI", width: "w-20", render: (r: any) =>
      r.ai_status === "completed" ? <span className="text-[hsl(var(--success))] text-xs font-medium">已完成</span>
        : r.ai_status === "processing" ? <span className="text-[hsl(var(--warning))] text-xs">分析中</span>
        : r.ai_status === "none" ? "-" : r.ai_status
    },
    { key: "report", header: "报告", width: "w-16", render: (r: any) =>
      r.report_path ? <span className="text-[hsl(var(--success))] text-xs">已生成</span> : "-" },
    { key: "actions", header: "操作", width: "w-72",
      render: (r: any) => (
        <div className="flex gap-1 flex-wrap">
          <Button variant="ghost" size="sm" onClick={(e: any) => { e.stopPropagation(); loadRecordDetail(r.id); }}>详情</Button>
          <Button variant="ghost" size="sm" loading={analyzing === r.id} disabled={r.ai_status === "processing"}
            onClick={(e: any) => { e.stopPropagation(); handleAnalyzeRecord(r.id); }}>AI 分析</Button>
          <Button variant="ghost" size="sm" loading={generating === r.id}
            onClick={(e: any) => { e.stopPropagation(); handleGenerateDocx(r.id); }}>生成报告</Button>
          {r.report_path && (
            <>
              <Button variant="ghost" size="sm" loading={downloading === r.id}
                onClick={(e: any) => { e.stopPropagation(); handleDownload(r.id); }}>下载</Button>
              <Button variant="ghost" size="sm" loading={deleting === r.id}
                onClick={(e: any) => { e.stopPropagation(); handleDelete(r.id); }}>删除</Button>
            </>
          )}
        </div>
      ),
    },
  ];

  return (
    <div>
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm">
        <h1 className="text-lg font-bold">报告管理</h1>
        <p className="text-xs text-[hsl(var(--text-tertiary))] mt-0.5">AI 分析、DOCX 报告生成与下载</p>
      </div>

      {errorMsg && (
        <div className="mb-3 px-3 py-2 rounded-lg text-sm bg-[hsl(var(--danger)_/_0.1)] text-[hsl(var(--danger))] flex items-center justify-between">
          <span>{errorMsg}</span>
          <button onClick={() => setErrorMsg("")} className="ml-2 text-xs hover:underline">关闭</button>
        </div>
      )}
      {info && (
        <div className="mb-3 px-3 py-2 rounded-lg text-sm bg-[hsl(var(--success)_/_0.1)] text-[hsl(var(--success))]">
          {info}
        </div>
      )}

      <div className="flex gap-4" style={{ height: "calc(100vh - 160px)" }}>
        {/* Left: Batch list */}
        <div className="w-[300px] shrink-0 flex flex-col border border-[hsl(var(--border))] rounded-lg bg-[hsl(var(--bg-card))] overflow-hidden">
          <div className="p-3 border-b border-[hsl(var(--border))]">
            <p className="text-xs text-[hsl(var(--text-tertiary))]">{batches.length} 个批次</p>
          </div>
          <div className="flex-1 overflow-y-auto">
            {batches.map((b) => {
              const sel = selectedBatch?.id === b.id;
              return (
                <div
                  key={b.id}
                  onClick={() => selectBatch(b)}
                  className={`px-3 py-2.5 cursor-pointer border-l-2 transition-colors ${
                    sel ? "border-l-[hsl(var(--accent))] bg-[hsl(var(--accent)_/_0.08)]" : "border-l-transparent hover:bg-[hsl(var(--bg-hover))]"
                  }`}
                >
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-sm font-medium truncate">{b.name || `#${b.id}`}</span>
                    <StatusBadge status={batchStatusColor(b.status)} />
                  </div>
                  <div className="text-[11px] text-[hsl(var(--text-tertiary))]">
                    {b.device_ids?.length || 0} 台设备
                    {b.started_at && <span className="ml-2">{new Date(b.started_at).toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" })}</span>}
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {/* Right: Detail */}
        <div className="flex-1 overflow-y-auto space-y-4">
          {!selectedBatch ? (
            <div className="flex items-center justify-center h-full text-[hsl(var(--text-tertiary))]">
              <p className="text-sm">← 选择左侧批次</p>
            </div>
          ) : (
            <>
              {/* Toolbar */}
              <div className="flex items-center gap-2 flex-wrap">
                <h2 className="text-base font-semibold mr-2">{selectedBatch.name || `批次 #${selectedBatch.id}`}</h2>
                {batchCompleted && (
                  <>
                    <Button size="sm" variant="ghost" loading={batchAnalyzing === selectedBatch.id}
                      onClick={() => handleAnalyzeBatch(selectedBatch.id)}>AI 分析全部</Button>
                    <Button size="sm" variant="ghost" loading={batchGenerating === "zip"}
                      onClick={handleBatchZip}>下载 ZIP</Button>
                    <Button size="sm" variant="ghost" loading={batchGenerating === "combined"}
                      onClick={handleBatchCombined}>下载合并 DOCX</Button>
                  </>
                )}
              </div>

              {/* Records table */}
              <Card>
                <h3 className="text-sm font-semibold mb-2">巡检记录 ({(selectedBatch.records || []).length})</h3>
                <DataTable
                  columns={recordColumns}
                  data={selectedBatch.records || []}
                  rowKey={(r: any) => String(r.id)}
                  selectedKey={expandedRecordId ?? undefined}
                  onRowClick={(r: any) => loadRecordDetail(r.id)}
                />
              </Card>

              {/* Record detail */}
              {recordLoading && <Card><p className="text-sm text-[hsl(var(--text-tertiary))]">加载中...</p></Card>}

              {fullRecord && expandedRecordId && (
                <Card>
                  <div className="flex items-center justify-between mb-3">
                    <h3 className="text-sm font-semibold">
                      记录详情 — {(() => { const d = deviceMap.get(fullRecord.device_id); return d ? `${d.name} (${d.ip})` : `#${fullRecord.device_id}`; })()}
                    </h3>
                    <div className="flex gap-1.5 flex-wrap">
                      <Button variant="ghost" size="sm" loading={logAnalyzing} onClick={() => handleLogAnalyze(fullRecord.id)}>分析日志</Button>
                      <Button variant="ghost" size="sm" loading={analyzing === fullRecord.id} disabled={fullRecord.ai_status === "processing"}
                        onClick={() => handleAnalyzeRecord(fullRecord.id)}>AI 分析</Button>
                      <Button variant="ghost" size="sm" loading={generating === fullRecord.id}
                        onClick={() => handleGenerateDocx(fullRecord.id)}>生成报告</Button>
                      {fullRecord.report_path && (
                        <>
                          <Button variant="ghost" size="sm" loading={downloading === fullRecord.id}
                            onClick={() => handleDownload(fullRecord.id)}>下载</Button>
                          <Button variant="ghost" size="sm" loading={deleting === fullRecord.id}
                            onClick={() => handleDelete(fullRecord.id)}>删除</Button>
                        </>
                      )}
                    </div>
                  </div>

                  {/* Status info */}
                  <div className="grid grid-cols-4 gap-3 mb-4 text-xs">
                    <div><span className="text-[hsl(var(--text-tertiary))]">状态:</span> <StatusBadge status={batchStatusColor(fullRecord.status)} /></div>
                    <div><span className="text-[hsl(var(--text-tertiary))]">AI 状态:</span> {fullRecord.ai_status}</div>
                    <div><span className="text-[hsl(var(--text-tertiary))]">开始:</span> {fullRecord.started_at?.slice(0, 19) || "-"}</div>
                    <div><span className="text-[hsl(var(--text-tertiary))]">完成:</span> {fullRecord.completed_at?.slice(0, 19) || "-"}</div>
                    {fullRecord.status === "failed" && fullRecord.error_message && <div className="col-span-4 text-[hsl(var(--danger))]">{fullRecord.error_message}</div>}
                  </div>

                  {/* Command outputs */}
                  {parsedOutputs.length > 0 && (
                    <details className="mb-3" open>
                      <summary className="cursor-pointer text-xs font-medium text-[hsl(var(--text-secondary))] mb-2">命令输出 ({parsedOutputs.length})</summary>
                      <div className="space-y-2 max-h-[300px] overflow-auto">
                        {parsedOutputs.map((o: any, i: number) => (
                          <details key={i} className="text-xs">
                            <summary className="cursor-pointer font-mono text-[hsl(var(--accent))] py-0.5">{o.command}</summary>
                            <pre className="mt-1 p-2 rounded bg-[hsl(var(--bg-hover))] text-[hsl(var(--text-secondary))] whitespace-pre-wrap max-h-[200px] overflow-auto">{o.content || "(空)"}</pre>
                          </details>
                        ))}
                      </div>
                    </details>
                  )}

                  {/* Log analysis */}
                  {logResult && !logResult.error && logResult.entries && (
                    <div className="mb-3">
                      <div className="grid grid-cols-4 gap-2 mb-2">
                        <StatBadge label="总计" value={String(logResult.total ?? 0)} color="info" />
                        <StatBadge label="ERROR" value={String(logResult.errors ?? 0)} color="danger" />
                        <StatBadge label="WARNING" value={String(logResult.warnings ?? 0)} color="warning" />
                        <StatBadge label="INFO/DEBUG" value={String(Number(logResult.info ?? 0) + Number(logResult.debug ?? 0))} color="text-secondary" />
                      </div>
                      <p className="text-xs text-[hsl(var(--text-secondary))] mb-2">{logResult.summary as string}</p>
                      <div className="max-h-[200px] overflow-auto">
                        <table className="w-full text-xs">
                          <thead><tr className="text-left text-[hsl(var(--text-tertiary))]">
                            <th className="p-1">时间</th><th className="p-1">级别</th><th className="p-1">模块</th><th className="p-1">消息</th>
                          </tr></thead>
                          <tbody>
                            {(logResult.entries as Array<any>).map((e, i) => (
                              <tr key={i} className={e.severity === "ERROR" ? "text-[hsl(var(--danger))]" : e.severity === "WARNING" ? "text-[hsl(var(--warning))]" : ""}>
                                <td className="p-1 font-mono">{e.timestamp}</td>
                                <td className="p-1">{e.severity}</td>
                                <td className="p-1">{e.module}</td>
                                <td className="p-1 max-w-[300px] truncate">{e.message}</td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    </div>
                  )}
                  {logResult?.error && <p className="text-xs text-[hsl(var(--danger))] mb-3">{String(logResult.error)}</p>}

                  {/* AI result —— 纯结构化展示，无 markdown 渲染 */}
                  {aiResult && (
                    <div className="mb-3">
                      <h4 className="text-xs font-semibold text-[hsl(var(--text-secondary))] mb-2">AI 分析结果</h4>
                      {aiResult.summary && (
                        <p className="text-xs text-[hsl(var(--text-primary))] mb-2 whitespace-pre-wrap">{aiResult.summary}</p>
                      )}
                      {Array.isArray(aiResult.items) && aiResult.items.length > 0 && (
                        <div className="overflow-auto">
                          <table className="w-full text-xs border-collapse">
                            <thead>
                              <tr className="bg-[hsl(var(--bg-hover))] text-[hsl(var(--text-secondary))]">
                                <th className="p-1.5 text-left border border-[hsl(var(--border))] w-[28%]">命令</th>
                                <th className="p-1.5 text-left border border-[hsl(var(--border))] w-[12%]">状态</th>
                                <th className="p-1.5 text-left border border-[hsl(var(--border))] w-[30%]">发现</th>
                                <th className="p-1.5 text-left border border-[hsl(var(--border))]">建议</th>
                              </tr>
                            </thead>
                            <tbody>
                              {aiResult.items.map((it: any, i: number) => {
                                const m = STATUS_META[it.status as string];
                                return (
                                  <tr key={i}>
                                    <td className="p-1.5 border border-[hsl(var(--border))] font-mono text-[hsl(var(--accent))]">{it.command || "-"}</td>
                                    <td className="p-1.5 border border-[hsl(var(--border))]" style={m ? { color: `hsl(${m.color})` } : undefined}>
                                      {m ? m.label : (it.status || "-")}
                                    </td>
                                    <td className="p-1.5 border border-[hsl(var(--border))]">{it.finding || "-"}</td>
                                    <td className="p-1.5 border border-[hsl(var(--border))]">{it.suggestion || "-"}</td>
                                  </tr>
                                );
                              })}
                            </tbody>
                          </table>
                        </div>
                      )}
                    </div>
                  )}

                  {fullRecord.ai_status === "processing" && (
                    <div className="flex items-center gap-2 text-xs text-[hsl(var(--warning))]">
                      <div className="w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin" />
                      AI 正在分析...
                    </div>
                  )}

                  {fullRecord.report_path && (
                    <p className="text-xs text-[hsl(var(--text-secondary))]">报告文件: <code className="text-[hsl(var(--accent))] bg-[hsl(var(--bg-hover))] px-1 rounded">{fullRecord.report_path}</code></p>
                  )}
                </Card>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
