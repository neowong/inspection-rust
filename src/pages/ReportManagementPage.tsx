import { useState, useEffect, useCallback, useMemo, useRef } from "react";
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

// 模块级引用，跨页面切换保持处理状态
const processingBatchesRef: { current: Record<number, "ai" | "manual"> } = { current: {} };

export default function ReportManagementPage() {
  const [batches, setBatches] = useState<any[]>([]);
  const [selectedBatch, setSelectedBatch] = useState<any>(null);
  const [devices, setDevices] = useState<Device[]>([]);
  const selectedIdRef = useRef<number | null>(null);

  // Selected record detail
  const [expandedRecordId, setExpandedRecordId] = useState<number | null>(null);
  const [fullRecord, setFullRecord] = useState<InspectionRecord | null>(null);
  const [recordLoading, setRecordLoading] = useState(false);

  const [batchGenerating, setBatchGenerating] = useState<"" | "ai" | "manual" | "combined">("");
  const [processingBatches, setProcessingBatches] = useState<Record<number, "ai" | "manual">>(processingBatchesRef.current);
  // 批次操作完成后显示简短反馈
  const [batchDone, setBatchDone] = useState<{type: "ai" | "manual"; batchId: number} | null>(null);
  // Log analysis
  const [logAnalyzing, setLogAnalyzing] = useState(false);
  const [logResult, setLogResult] = useState<Record<string, unknown> | null>(null);

  // Download / delete
  const [downloading, setDownloading] = useState<number | null>(null);
  const [deleting, setDeleting] = useState<number | null>(null);
  const [deletingReports, setDeletingReports] = useState(false);

  // AI 配置状态
  const [hasActiveAiConfig, setHasActiveAiConfig] = useState<boolean>(true);

  useEffect(() => {
    loadBatches();
    invoke<Device[]>("list_devices", {}).then(setDevices).catch(console.error);
    // 检查是否有激活的 AI 配置
    invoke<any[]>("list_ai_configs")
      .then((configs) => {
        const hasActive = configs.some((c) => c.is_active);
        setHasActiveAiConfig(hasActive);
      })
      .catch(console.error);
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
    selectedIdRef.current = batch.id;
    setSelectedBatch(batch);
    setExpandedRecordId(null);
    setFullRecord(null);
    setLogResult(null);
    // 恢复之前的处理状态（跨页面导航保持）
    if (processingBatchesRef.current[batch.id]) {
      setProcessingBatches({...processingBatchesRef.current});
    }
    try {
      const full: any = await invoke("get_batch", { batchId: batch.id });
      if (selectedIdRef.current !== batch.id) return;
      setSelectedBatch(full);
      // 自动检测：如果还有记录在分析中，恢复处理状态
      if (full.records?.some((r: any) => r.ai_status === 'processing')) {
        const sp = (v: Record<number, "ai" | "manual">) => { processingBatchesRef.current = v; setProcessingBatches(v); };
        sp({...processingBatchesRef.current, [batch.id]: "ai"});
      } else {
        // 记录都已完成，清理处理状态
        delete processingBatchesRef.current[batch.id];
        setProcessingBatches({...processingBatchesRef.current});
      }
    } catch (e) { console.error(e); }
  };

  // Auto-refresh（仅在选中批次处于运行中/等待中时轮询）
  useEffect(() => {
    const id = selectedBatch?.id;
    if (!id) return;
    const status = selectedBatch?.status;
    if (status !== 'running' && status !== 'pending') return;
    const timer = setInterval(() => {
      loadBatches();
      invoke<any>("get_batch", { batchId: id }).then((full) => {
        // 仅当仍选中该批次时才更新，避免覆盖用户已切换到的新批次
        if (selectedIdRef.current === id) setSelectedBatch(full);
      }).catch(() => {});
    }, 3000);
    return () => clearInterval(timer);
  }, [selectedBatch?.id, selectedBatch?.status, loadBatches]);

  // 页面获得焦点时刷新 AI 配置状态（用户可能在设置页面添加了配置）
  useEffect(() => {
    const handleFocus = () => {
      invoke<any[]>("list_ai_configs")
        .then((configs) => {
          const hasActive = configs.some((c) => c.is_active);
          setHasActiveAiConfig(hasActive);
        })
        .catch(console.error);
    };
    window.addEventListener("focus", handleFocus);
    return () => window.removeEventListener("focus", handleFocus);
  }, []);

  // ----- Record detail -----
  // 序号守卫：快速切换记录时，仅最后一次请求的响应会更新 state，避免旧响应覆盖新数据
  const recordReqSeq = useRef(0);
  const loadRecordDetail = useCallback((recordId: number) => {
    setExpandedRecordId(recordId);
    setRecordLoading(true);
    const seq = ++recordReqSeq.current;
    invoke<InspectionRecord>("get_record", { recordId })
      .then((r) => { if (seq === recordReqSeq.current) setFullRecord(r); })
      .catch((e) => { if (seq === recordReqSeq.current) console.error(String(e)); })
      .finally(() => { if (seq === recordReqSeq.current) setRecordLoading(false); });
  }, []);

  const refreshAfterMutation = useCallback((recordId?: number) => {
    if (recordId) {
      const seq = ++recordReqSeq.current;
      invoke<InspectionRecord>("get_record", { recordId })
        .then((r) => { if (seq === recordReqSeq.current) setFullRecord(r); })
        .catch(console.error);
    }
    if (selectedBatch?.id) {
      const id = selectedBatch.id;
      invoke<any>("get_batch", { batchId: id })
        .then((full) => { if (selectedIdRef.current === id) setSelectedBatch(full); })
        .catch(() => {});
    }
    loadBatches();
  }, [selectedBatch?.id, loadBatches]);


  // 单设备：仅 AI 分析
  const refreshBatch = async (batchId: number) => {
    const full = await invoke<any>("get_batch", { batchId });
    if (selectedIdRef.current === batchId) setSelectedBatch(full);
  };

  // 生成单个报告（AI/人工共用），然后刷新显示下载按钮
  // records 参数显式传入，避免闭包引用 stale selectedBatch
  const generateAllReports = async (batchId: number, records: any[]) => {
    for (const r of records) {
      await invoke("generate_docx_report", { recordId: r.id });
    }
    await refreshBatch(batchId);
    await refreshAfterMutation(expandedRecordId ?? undefined);
  };

  // 显示批次操作成功提示，仅当用户仍在该任务时显示
  const flashBatchDone = (type: "ai" | "manual", batchId: number) => {
    if (selectedBatch?.id !== batchId) return;
    setBatchDone({ type, batchId });
    setTimeout(() => setBatchDone(null), 2000);
  };

  // 批次：AI 评判 — 先分析再生成单个报告
  const handleBatchAiJudge = async () => {
    if (!selectedBatch) return;
    // 检查是否有激活的 AI 配置
    if (!hasActiveAiConfig) {
      alert("请先在「系统设置」中添加并激活 AI 模型配置，再执行 AI 评判。");
      return;
    }
    const batchId = selectedBatch.id;
    const records = selectedBatch.records || [];
    const startId = selectedIdRef.current; // 记录开始时的批次 ID
    setBatchGenerating("ai");
    const sp = (v: Record<number, "ai" | "manual">) => { processingBatchesRef.current = v; setProcessingBatches(v); };
    sp({...processingBatchesRef.current, [batchId]: "ai"});
    try {
      await invoke("analyze_batch", { batchId, force: hasAnalyzedRecords });
      await refreshAfterMutation(expandedRecordId ?? undefined);
      if (selectedIdRef.current !== startId) return; // 用户已切换批次，放弃输出
      await generateAllReports(batchId, records);
      flashBatchDone("ai", batchId);
    } catch (e) {
      console.error("AI评判失败:", String(e));
      alert(`AI评判失败: ${String(e)}`);
    }
    finally { setBatchGenerating(""); const n = {...processingBatchesRef.current}; delete n[batchId]; sp(n); }
  };

  // 批次：人工评判 — 直接生成单个报告（跳过 AI）
  const handleBatchManual = async () => {
    if (!selectedBatch) return;
    const batchId = selectedBatch.id;
    const records = selectedBatch.records || [];
    const startId = selectedIdRef.current;
    setBatchGenerating("manual");
    const sp = (v: Record<number, "ai" | "manual">) => { processingBatchesRef.current = v; setProcessingBatches(v); };
    sp({...processingBatchesRef.current, [batchId]: "manual"});
    try {
      if (selectedIdRef.current !== startId) return;
      await generateAllReports(batchId, records);
      flashBatchDone("manual", batchId);
    } catch (e) {
      console.error("人工评判失败:", String(e));
      alert(`人工评判失败: ${String(e)}`);
    }
    finally { setBatchGenerating(""); const n = {...processingBatchesRef.current}; delete n[batchId]; sp(n); }
  };

  // 批次：下载综合报告（合并已有单报告 + 保存对话框）
  const handleDownloadCombined = async () => {
    if (!selectedBatch) return;
    setBatchGenerating("combined");
    try {
      const path = await invoke<string>("generate_batch_docx_combined", { batchId: selectedBatch.id });
      await refreshBatch(selectedBatch.id);
      const safeName = (selectedBatch.name || `batch_${selectedBatch.id}`).replace(/[/\\:*?"<>|]/g, "_");
      await invoke("save_generated_file", { sourcePath: path, suggestedName: `${safeName}-综合报告.docx`, extension: "docx" });
    } catch (e) { console.error(String(e)); }
    finally { setBatchGenerating(""); }
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
      .catch((e) => console.error(String(e)))
      .finally(() => setDownloading(prev => prev === recordId ? null : prev));
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
      console.error(String(e));
    } finally {
      setDeleting(prev => prev === recordId ? null : prev);
    }
  };

  // ----- Memoized -----
  const parsedOutputs = useMemo(() => parseCommandOutputs(fullRecord?.command_outputs), [fullRecord?.command_outputs]);
  const aiResult = useMemo(() => parseAiResult(fullRecord?.ai_result), [fullRecord?.ai_result]);

  const hasAnalyzedRecords = useMemo(
    () => (selectedBatch?.records || []).some((r: any) => r.ai_status === "completed"),
    [selectedBatch?.records]
  );
  const hasAnyReport = useMemo(
    () => (selectedBatch?.records || []).some((r: any) => r.report_path),
    [selectedBatch?.records]
  );

  const recordColumns = useMemo(() => [
    { key: "device", header: "设备", width: "200px", maxWidth: "300px", render: (r: any) => {
      const d = deviceMap.get(r.device_id);
      return d ? <span>{d.name} <span className="text-[hsl(var(--text-tertiary))]">{d.ip}</span></span> : `#${r.device_id}`;
    }},
    { key: "status", header: "巡检状态", width: "90px", noTruncate: true, render: (r: any) => <StatusBadge status={batchStatusColor(r.status)} /> },
    { key: "ai_status", header: "AI", width: "80px", render: (r: any) =>
      r.ai_status === "completed" ? <span className="text-[hsl(var(--success))] text-xs font-medium">已完成</span>
        : r.ai_status === "processing" ? <span className="text-[hsl(var(--warning))] text-xs">分析中</span>
        : (r.ai_status === "none" || r.ai_status === "pending") ? "-" : r.ai_status
    },
    { key: "report", header: "报告", width: "70px", render: (r: any) =>
      r.report_path ? <span className="text-[hsl(var(--success))] text-xs">已生成</span> : "-" },
    { key: "actions", header: "操作", width: "120px", noTruncate: true,
      render: (r: any) => (
        <div className="flex gap-1">
          {r.report_path ? (
            <Button variant="ghost" size="sm" loading={downloading === r.id}
              onClick={(e: any) => { e.stopPropagation(); handleDownload(r.id); }}>下载</Button>
          ) : (
            <Button variant="ghost" size="sm" disabled onClick={(e: any) => e.stopPropagation()}>下载</Button>
          )}
        </div>
      ),
    },
  ], [deviceMap, downloading, handleDownload, loadRecordDetail]);

  return (
    <div>
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm">
        <h1 className="text-lg font-bold">报告管理</h1>
        <p className="text-xs text-[hsl(var(--text-tertiary))] mt-0.5">AI 分析、DOCX 报告生成与下载</p>
      </div>


      <div className="flex gap-4" style={{ height: "calc(100vh - 160px)" }}>
        {/* Left: Batch list */}
        <div className="w-[300px] shrink-0 flex flex-col border border-[hsl(var(--border))] rounded-lg bg-[hsl(var(--bg-card))] overflow-hidden">
          <div className="p-3 border-b border-[hsl(var(--border))]">
            <p className="text-xs text-[hsl(var(--text-tertiary))]">{batches.length} 个任务</p>
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
              <p className="text-sm">← 选择左侧任务</p>
            </div>
          ) : (
            <>
              {/* Toolbar */}
              {/* Batch toolbar */}
              <div className="flex items-center gap-2 flex-wrap mb-3">
                <h2 className="text-base font-semibold mr-2">{selectedBatch.name || `任务 #${selectedBatch.id}`}</h2>
                <div className="relative group">
                  <Button size="sm" variant="ghost"
                    loading={processingBatches[selectedBatch?.id ?? -1] === "ai"}
                    disabled={processingBatches[selectedBatch?.id ?? -1] !== undefined || !hasActiveAiConfig}
                    onClick={handleBatchAiJudge}>
                    {batchDone?.type === "ai" && batchDone?.batchId === selectedBatch?.id ? "✓ 已重新评判" : (hasAnalyzedRecords ? "重新AI评判" : "AI评判")}
                  </Button>
                  {!hasActiveAiConfig && (
                    <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-3 py-2 bg-[hsl(var(--text-primary))] text-[hsl(var(--bg-card))] text-xs rounded-lg shadow-lg opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-50">
                      请先在「系统设置」中添加并激活 AI 模型配置
                      <div className="absolute top-full left-1/2 -translate-x-1/2 -mt-1 w-2 h-2 bg-[hsl(var(--text-primary))] rotate-45"></div>
                    </div>
                  )}
                </div>
                <Button size="sm" variant="ghost"
                  loading={processingBatches[selectedBatch?.id ?? -1] === "manual"}
                  disabled={processingBatches[selectedBatch?.id ?? -1] !== undefined}
                  onClick={handleBatchManual}>
                  {batchDone?.type === "manual" && batchDone?.batchId === selectedBatch?.id ? "✓ 已重新生成" : (hasAnyReport ? "重新生成" : "人工评判")}
                </Button>
                <Button size="sm" variant="ghost"
                  loading={batchGenerating === "combined"}
                  disabled={!!batchGenerating || !selectedBatch?.records?.some((r: any) => r.report_path)}
                  onClick={handleDownloadCombined}>
                  下载综合报告
                </Button>
                <Button size="sm" variant="ghost"
                  onClick={() => invoke("open_reports_dir").catch(console.error)}>
                  报告目录
                </Button>
                {selectedBatch?.records?.some((r: any) => r.report_path) && (
                  <Button size="sm" variant="danger" loading={deletingReports}
                    onClick={async () => {
                      if (!confirm("确定删除该批次所有报告文件吗？巡检记录会保留。")) return;
                      setDeletingReports(true);
                      try { await invoke("delete_batch_reports", { batchId: selectedBatch.id }); loadBatches(); invoke<any>("get_batch", { batchId: selectedBatch.id }).then(setSelectedBatch).catch(() => {}); } catch (e: any) { console.error(String(e)); } finally { setDeletingReports(false); }
                    }}>
                    删除报告
                  </Button>
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
                      {fullRecord.report_path ? (
                        <>
                          <Button variant="ghost" size="sm" loading={downloading === fullRecord.id}
                            onClick={() => handleDownload(fullRecord.id)}>下载</Button>
                          <Button variant="ghost" size="sm" loading={deleting === fullRecord.id}
                            onClick={() => handleDelete(fullRecord.id)}>删除</Button>
                        </>
                      ) : (
                        <Button variant="ghost" size="sm" disabled>下载</Button>
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
                          <details key={o.command || i} className="text-xs">
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
