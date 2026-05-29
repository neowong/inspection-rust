import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import StatusBadge from "../components/StatusBadge";
import Button from "../components/ui/Button";
import type { Device, InspectionRecord } from "../types";

// --------------- local types ---------------

interface BatchRecordSummary {
  id: number;
  batch_id: number;
  device_id: number;
  status: string;
  ai_status: string;
  report_path: string | null;
  error_message: string | null;
}

interface BatchData {
  id: number;
  name: string | null;
  mode: string;
  status: string;
  triggered_by: string;
  device_ids: string | number[];
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
  records: BatchRecordSummary[];
}

interface FullRecord {
  id: number;
  device_id: number;
  status: string;
  error_message: string | null;
  ai_status: string;
  report_path: string | null;
  command_outputs: string;
  ai_result: string | null;
  command_judgments: string | null;
  summary_judgment: string | null;
  created_at: string;
  completed_at: string | null;
}

// --------------- helpers ---------------

function formatTs(ts: string | null): string {
  if (!ts) return "-";
  return ts.replace("T", " ").substring(0, 19);
}

function parseDeviceIds(raw: string | number[]): number[] {
  if (Array.isArray(raw)) return raw;
  try {
    return JSON.parse(raw);
  } catch {
    return [];
  }
}

function downloadBlob(content: string, filename: string, mime: string) {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

// --------------- markdown builder ---------------

function buildMarkdown(
  record: FullRecord,
  device: Device | undefined,
): string {
  const deviceName = device?.name ?? `设备 #${record.device_id}`;
  const deviceIp = device?.ip ?? "-";
  const vendor = device?.vendor ?? "-";
  const model = device?.model ?? "-";

  let commandOutputs: Record<string, string> = {};
  try {
    commandOutputs = JSON.parse(record.command_outputs || "{}");
  } catch {
    /* ignore parse errors */
  }

  let commandJudgments: Record<string, string> = {};
  try {
    commandJudgments = JSON.parse(record.command_judgments || "{}");
  } catch {
    /* ignore parse errors */
  }

  const summary = record.summary_judgment || "";
  const ts = formatTs(record.completed_at) || new Date().toISOString().replace("T", " ").substring(0, 19);

  let md = "";
  md += `# ${deviceName} 巡检报告\n\n`;
  md += `> 生成时间: ${ts}\n\n`;

  // Basic info
  md += "## 基本信息\n\n";
  md += "| 项目 | 内容 |\n|------|------|\n";
  md += `| 设备名称 | ${deviceName} |\n`;
  md += `| IP 地址 | ${deviceIp} |\n`;
  md += `| 厂商 | ${vendor} |\n`;
  md += `| 型号 | ${model} |\n`;

  // Try to extract extra fields from command outputs
  const extractFromOutputs = (key: string) => {
    if (commandOutputs[key]) {
      return commandOutputs[key].trim().split("\n")[0].substring(0, 60);
    }
    return null;
  };

  const hostname = extractFromOutputs("hostname") || extractFromOutputs("show hostname");
  if (hostname) md += `| 主机名 | ${hostname} |\n`;

  const osRelease = extractFromOutputs("uname -a") || extractFromOutputs("show version");
  if (osRelease) md += `| OS / 版本 | ${osRelease} |\n`;

  md += "\n";

  // Inspection records
  md += "## 巡检记录\n\n";
  const entries = Object.entries(commandOutputs);
  if (entries.length === 0) {
    md += "（无命令输出）\n\n";
  } else {
    md += "| 序号 | 巡检项目 | 评判结论 |\n|------|---------|----------|\n";
    entries.forEach(([cmd, output], i) => {
      const judgmentRaw = commandJudgments[cmd] || "";
      const judgment = judgmentRaw.split("\x00")[0] || "";
      const outputShort = output
        .split("\n")
        .slice(0, 3)
        .join("  \n")
        .replace(/\|/g, "\\|");
      let statusIcon = "ℹ️"; // info
      if (judgment.includes("[OK]")) statusIcon = "✅";
      else if (judgment.includes("[WARNING]")) statusIcon = "⚠️";
      else if (judgment.includes("[CRITICAL]")) statusIcon = "🔴";
      md += `| ${i + 1} | **${cmd.replace(/\|/g, "\\|")}**<br/>${outputShort.substring(0, 200)} | ${statusIcon} ${judgment} |\n`;
    });
    md += "\n";
  }

  // AI summary
  if (summary) {
    md += "## AI 分析总结\n\n";
    md += `${summary}\n`;
  }

  // AI detailed result
  if (record.ai_result) {
    md += "\n## AI 详细分析\n\n";
    md += `${record.ai_result}\n`;
  }

  return md;
}

// --------------- component ---------------

export default function ReportsPage() {
  // data
  const [batches, setBatches] = useState<BatchData[]>([]);
  const [devices, setDevices] = useState<Device[]>([]);
  const [loading, setLoading] = useState(true);

  // list view state
  const [expandedBatchId, setExpandedBatchId] = useState<number | null>(null);
  const [selectedRecordIds, setSelectedRecordIds] = useState<Set<number>>(
    new Set(),
  );

  // preview state
  const [preview, setPreview] = useState<{
    batchId: number;
    recordId: number;
  } | null>(null);
  const [previewRecord, setPreviewRecord] = useState<FullRecord | null>(null);
  const [previewMd, setPreviewMd] = useState("");
  const [previewLoading, setPreviewLoading] = useState(false);

  // context menu
  const [ctxMenu, setCtxMenu] = useState<{
    x: number;
    y: number;
    recordId: number;
    batchId: number;
  } | null>(null);

  // polling for batch status updates
  const loadTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // --------------- data loading ---------------

  const loadData = useCallback(async () => {
    try {
      const [deviceList, batchList] = await Promise.all([
        invoke<Device[]>("list_devices"),
        invoke<any[]>("list_batches"),
      ]);
      setDevices(deviceList);
      setBatches(batchList);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();
  }, [loadData]);

  // periodic refresh for list view (every 10s)
  useEffect(() => {
    if (preview) return;
    loadTimer.current = setInterval(() => {
      invoke<any[]>("list_batches")
        .then(setBatches)
        .catch(console.error);
    }, 10000);
    return () => {
      if (loadTimer.current) clearInterval(loadTimer.current);
    };
  }, [preview]);

  // close context menu on click outside
  useEffect(() => {
    const handler = () => setCtxMenu(null);
    window.addEventListener("click", handler);
    return () => window.removeEventListener("click", handler);
  }, []);

  // --------------- preview ---------------

  const openPreview = useCallback(
    async (batchId: number, recordId: number) => {
      setPreviewLoading(true);
      setPreview({ batchId, recordId });
      try {
        const batch = await invoke<any>("get_batch", { batchId });
        const records: FullRecord[] = batch.records || [];
        const rec = records.find((r) => r.id === recordId);
        if (rec) {
          setPreviewRecord(rec);
          const dev = devices.find((d) => d.id === rec.device_id);
          setPreviewMd(buildMarkdown(rec, dev));
        } else {
          setPreviewRecord(null);
          setPreviewMd("");
        }
      } catch (e) {
        console.error(e);
        setPreviewRecord(null);
        setPreviewMd("");
      } finally {
        setPreviewLoading(false);
      }
    },
    [devices],
  );

  const closePreview = useCallback(() => {
    setPreview(null);
    setPreviewRecord(null);
    setPreviewMd("");
    loadData();
  }, [loadData]);

  // --------------- actions ---------------

  const handlePrint = () => {
    window.print();
  };

  const handleExportMarkdown = () => {
    if (!previewMd) return;
    const filename = `inspection_report_${preview?.recordId ?? "unknown"}.md`;
    downloadBlob(previewMd, filename, "text/markdown");
  };

  const handleDeleteRecord = async (recordId: number) => {
    try {
      await invoke("delete_record", { recordId });
      setCtxMenu(null);
      await loadData();
    } catch (e) {
      console.error(e);
    }
  };

  const handleBatchDelete = async () => {
    if (selectedRecordIds.size === 0) return;
    try {
      await invoke("batch_delete_records", {
        ids: Array.from(selectedRecordIds),
      });
      setSelectedRecordIds(new Set());
      await loadData();
    } catch (e) {
      console.error(e);
    }
  };

  const handleGenerateReport = async (recordId: number) => {
    try {
      await invoke("generate_report", { recordId });
      setCtxMenu(null);
      await loadData();
    } catch (e) {
      console.error(e);
    }
  };

  const handleBatchGenerateReports = async (batchId: number) => {
    try {
      await invoke("generate_batch_reports", { batchId });
      await loadData();
    } catch (e) {
      console.error(e);
    }
  };

  // --------------- selection ---------------

  const toggleRecord = (recordId: number) => {
    setSelectedRecordIds((prev) => {
      const next = new Set(prev);
      if (next.has(recordId)) next.delete(recordId);
      else next.add(recordId);
      return next;
    });
  };

  const toggleBatchSelectAll = (batchId: number) => {
    const batch = batches.find((b) => b.id === batchId);
    if (!batch) return;
    const allIds = batch.records.map((r) => r.id);
    const allSelected = allIds.every((id) => selectedRecordIds.has(id));
    setSelectedRecordIds((prev) => {
      const next = new Set(prev);
      if (allSelected) {
        allIds.forEach((id) => next.delete(id));
      } else {
        allIds.forEach((id) => next.add(id));
      }
      return next;
    });
  };

  // --------------- device map ---------------

  const deviceMap = new Map(devices.map((d) => [d.id, d]));

  // --------------- loading ---------------

  if (loading) {
    return <div className="p-4 text-[hsl(var(--text-secondary))] text-sm">加载中...</div>;
  }

  // --------------- preview view ---------------

  if (preview) {
    return (
      <div className="flex flex-col h-full">
        {/* Toolbar */}
        <div className="no-print flex items-center gap-2 mb-3 p-2 bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg">
          <Button variant="secondary" size="sm" onClick={closePreview}>
            ← 返回列表
          </Button>
          <span className="flex-1" />
          <Button size="sm" onClick={handlePrint}>
            导出 PDF
          </Button>
          <Button variant="secondary" size="sm" onClick={handlePrint}>
            打印
          </Button>
          <Button variant="secondary" size="sm" onClick={handleExportMarkdown}>
            导出 Markdown
          </Button>
          {previewRecord && (
            <Button variant="secondary" size="sm" onClick={() => { handleGenerateReport(previewRecord.id); }}>
              生成报告
            </Button>
          )}
        </div>

        {/* Content */}
        <div className="flex-1 overflow-auto bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg p-6">
          {previewLoading ? (
            <div className="text-sm text-[hsl(var(--text-secondary))]">加载报告内容...</div>
          ) : previewMd ? (
            <div className="prose prose-sm max-w-none prose-headings:text-[hsl(var(--text-primary))] prose-table:border-collapse prose-th:border prose-th:border-[hsl(var(--border))] prose-th:bg-[hsl(var(--bg-hover))] prose-th:px-3 prose-th:py-1.5 prose-td:border prose-td:border-[hsl(var(--border))] prose-td:px-3 prose-td:py-1.5 prose-p:text-[hsl(var(--text-primary))] prose-li:text-[hsl(var(--text-primary))] prose-code:text-[hsl(var(--text-primary))]">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
                {previewMd}
              </ReactMarkdown>
            </div>
          ) : (
            <div className="text-sm text-[hsl(var(--text-tertiary))]">
              无法加载报告内容，记录数据为空或已被删除。
            </div>
          )}
        </div>
      </div>
    );
  }

  // --------------- list view ---------------

  return (
    <div className="flex flex-col h-full gap-3">
      {/* Header with bulk actions */}
      <div className="no-print flex items-center justify-between bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg px-3 py-2">
        <h2 className="text-sm font-semibold">巡检报告</h2>
        <div className="flex items-center gap-2">
          {selectedRecordIds.size > 0 && (
            <Button variant="danger" size="sm" onClick={handleBatchDelete}>
              删除选中 ({selectedRecordIds.size})
            </Button>
          )}
        </div>
      </div>

      {/* Batch list */}
      <div className="flex-1 overflow-auto">
        {batches.length === 0 ? (
          <div className="text-center py-12 text-[hsl(var(--text-tertiary))] text-sm">
            暂无巡检批次，请先在「执行巡检」页面创建并执行巡检。
          </div>
        ) : (
          <div className="space-y-2">
            {batches.map((batch) => {
              const isExpanded = expandedBatchId === batch.id;
              const batchDeviceIds = parseDeviceIds(batch.device_ids);
              const batchRecords = batch.records || [];
              const allSelected =
                batchRecords.length > 0 &&
                batchRecords.every((r) => selectedRecordIds.has(r.id));

              return (
                <div
                  key={batch.id}
                  className="bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg"
                >
                  {/* Batch header */}
                  <div
                    className="flex items-center gap-3 px-3 py-2 cursor-pointer hover:bg-[hsl(var(--bg-hover))]"
                    onClick={() =>
                      setExpandedBatchId(isExpanded ? null : batch.id)
                    }
                  >
                    <span className="text-xs text-[hsl(var(--text-tertiary))]">
                      {isExpanded ? "▼" : "▶"}
                    </span>
                    <span className="text-sm font-medium">
                      {batch.name || `批次 #${batch.id}`}
                    </span>
                    <StatusBadge status={batch.status as any} />
                    <span className="text-xs text-[hsl(var(--text-secondary))]">
                      {batchRecords.length} 条记录
                    </span>
                    <span className="text-xs text-[hsl(var(--text-tertiary))]">
                      {formatTs(batch.created_at)}
                    </span>
                    <span className="flex-1" />
                    {/* Batch header actions */}
                    <Button variant="secondary" size="sm" onClick={(e) => { e.stopPropagation(); handleBatchGenerateReports(batch.id); }}>
                      批量生成报告
                    </Button>
                    <label
                      className="flex items-center gap-1 text-xs text-[hsl(var(--text-secondary))] cursor-pointer"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <input
                        type="checkbox"
                        className="w-3.5 h-3.5 rounded accent-[hsl(var(--accent))]"
                        checked={allSelected}
                        onChange={() => toggleBatchSelectAll(batch.id)}
                      />
                      全选
                    </label>
                  </div>

                  {/* Batch records table */}
                  {isExpanded && (
                    <div className="border-t border-[hsl(var(--border))]">
                      {batchRecords.length === 0 ? (
                        <div className="p-4 text-center text-xs text-[hsl(var(--text-tertiary))]">
                          该批次暂无巡检记录
                        </div>
                      ) : (
                        <table className="w-full text-xs">
                          <thead className="bg-[hsl(var(--bg-hover))]">
                            <tr>
                              <th className="w-8 px-2 py-1.5 text-left text-[hsl(var(--text-secondary))] font-medium">
                                #
                              </th>
                              <th className="px-3 py-1.5 text-left text-[hsl(var(--text-secondary))] font-medium">
                                设备
                              </th>
                              <th className="px-3 py-1.5 text-left text-[hsl(var(--text-secondary))] font-medium">
                                IP
                              </th>
                              <th className="px-3 py-1.5 text-left text-[hsl(var(--text-secondary))] font-medium">
                                巡检状态
                              </th>
                              <th className="px-3 py-1.5 text-left text-[hsl(var(--text-secondary))] font-medium">
                                AI 评判
                              </th>
                              <th className="px-3 py-1.5 text-left text-[hsl(var(--text-secondary))] font-medium">
                                报告
                              </th>
                              <th className="px-3 py-1.5 text-left text-[hsl(var(--text-secondary))] font-medium">
                                时间
                              </th>
                              <th className="w-16 px-2 py-1.5 text-right text-[hsl(var(--text-secondary))] font-medium">
                                选择
                              </th>
                            </tr>
                          </thead>
                          <tbody>
                            {batchRecords.map((rec, idx) => {
                              const dev = deviceMap.get(rec.device_id);
                              return (
                                <tr
                                  key={rec.id}
                                  className="border-t border-[hsl(var(--border-light))] hover:bg-[hsl(var(--bg-hover))] cursor-pointer"
                                  onClick={() =>
                                    openPreview(batch.id, rec.id)
                                  }
                                  onContextMenu={(e) => {
                                    e.preventDefault();
                                    setCtxMenu({
                                      x: e.clientX,
                                      y: e.clientY,
                                      recordId: rec.id,
                                      batchId: batch.id,
                                    });
                                  }}
                                >
                                  <td className="px-2 py-1.5 text-[hsl(var(--text-tertiary))]">
                                    {idx + 1}
                                  </td>
                                  <td className="px-3 py-1.5 font-medium">
                                    {dev?.name ?? `设备 #${rec.device_id}`}
                                  </td>
                                  <td className="px-3 py-1.5 text-[hsl(var(--text-secondary))]">
                                    {dev?.ip ?? "-"}
                                  </td>
                                  <td className="px-3 py-1.5">
                                    <StatusBadge
                                      status={rec.status as any}
                                    />
                                  </td>
                                  <td className="px-3 py-1.5">
                                    <AiJudgmentBadge
                                      aiStatus={rec.ai_status}
                                      summaryJudgment={null}
                                    />
                                  </td>
                                  <td className="px-3 py-1.5">
                                    {rec.report_path ? (
                                      <span className="text-emerald-400">
                                        已生成
                                      </span>
                                    ) : (
                                      <span className="text-[hsl(var(--text-tertiary))]">
                                        -
                                      </span>
                                    )}
                                  </td>
                                  <td className="px-3 py-1.5 text-[hsl(var(--text-secondary))]">
                                    {formatTs(
                                      (rec as any).created_at ??
                                        (rec as any).completed_at,
                                    )}
                                  </td>
                                  <td
                                    className="px-2 py-1.5 text-right"
                                    onClick={(e) => e.stopPropagation()}
                                  >
                                    <input
                                      type="checkbox"
                                      className="w-3.5 h-3.5 rounded accent-[hsl(var(--accent))]"
                                      checked={selectedRecordIds.has(
                                        rec.id,
                                      )}
                                      onChange={() =>
                                        toggleRecord(rec.id)
                                      }
                                    />
                                  </td>
                                </tr>
                              );
                            })}
                          </tbody>
                        </table>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Context menu */}
      {ctxMenu && (
        <div
          className="fixed z-50 bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg shadow-2xl py-1 w-44"
          style={{ left: ctxMenu.x, top: ctxMenu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            className="w-full text-left px-3 py-1.5 text-xs hover:bg-[hsl(var(--bg-hover))]"
            onClick={() => {
              const batchId = ctxMenu.batchId;
              const recordId = ctxMenu.recordId;
              setCtxMenu(null);
              openPreview(batchId, recordId);
            }}
          >
            查看报告
          </button>
          <button
            className="w-full text-left px-3 py-1.5 text-xs hover:bg-[hsl(var(--bg-hover))]"
            onClick={() => {
              const rid = ctxMenu.recordId;
              setCtxMenu(null);
              handleGenerateReport(rid);
            }}
          >
            生成报告
          </button>
          <hr className="my-1 border-[hsl(var(--border-light))]" />
          <button
            className="w-full text-left px-3 py-1.5 text-xs text-[hsl(var(--danger))] hover:bg-[hsl(var(--danger)/0.1)]"
            onClick={() => {
              const rid = ctxMenu.recordId;
              setCtxMenu(null);
              handleDeleteRecord(rid);
            }}
          >
            删除记录
          </button>
        </div>
      )}
    </div>
  );
}

// --------------- sub-components ---------------

function AiJudgmentBadge({
  aiStatus,
  summaryJudgment,
}: {
  aiStatus: string;
  summaryJudgment: string | null;
}) {
  if (aiStatus === "completed") {
    if (summaryJudgment) {
      const isOk = summaryJudgment.includes("[OK]");
      const isWarn = summaryJudgment.includes("[WARNING]");
      const isCrit = summaryJudgment.includes("[CRITICAL]");
      if (isCrit)
        return (
          <span className="inline-block px-1.5 py-0.5 rounded border text-[11px] font-medium bg-red-500/15 text-red-400 border-red-500/30">
            严重
          </span>
        );
      if (isWarn)
        return (
          <span className="inline-block px-1.5 py-0.5 rounded border text-[11px] font-medium bg-amber-500/15 text-amber-400 border-amber-500/30">
            警告
          </span>
        );
      if (isOk)
        return (
          <span className="inline-block px-1.5 py-0.5 rounded border text-[11px] font-medium bg-emerald-500/15 text-emerald-400 border-emerald-500/30">
            正常
          </span>
        );
    }
    return (
      <span className="inline-block px-1.5 py-0.5 rounded border text-[11px] font-medium bg-emerald-500/15 text-emerald-400 border-emerald-500/30">
        已完成
      </span>
    );
  }
  if (aiStatus === "running" || aiStatus === "processing")
    return (
      <span className="inline-block px-1.5 py-0.5 rounded border text-[11px] font-medium bg-blue-500/15 text-blue-400 border-blue-500/30">
        分析中
      </span>
    );
  if (aiStatus === "failed")
    return (
      <span className="inline-block px-1.5 py-0.5 rounded border text-[11px] font-medium bg-red-500/15 text-red-400 border-red-500/30">
        失败
      </span>
    );
  return (
    <span className="inline-block px-1.5 py-0.5 rounded border text-[11px] font-medium bg-gray-500/15 text-[hsl(var(--text-tertiary))] border-gray-500/30">
      等待中
    </span>
  );
}
