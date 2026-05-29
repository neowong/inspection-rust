import { useState, useEffect, useCallback, useRef, Fragment } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNavigate } from "react-router-dom";
import Button from "../components/ui/Button";
import Input, { Select } from "../components/ui/Input";
import StatusBadge from "../components/StatusBadge";
import type { Device } from "../types";

// --------------- types ---------------

interface BatchRecord {
  id: number;
  device_id: number;
  status: string;
  ai_status: string;
  report_path: string | null;
  error_message: string | null;
  command_outputs?: string;
  ai_result?: string | null;
  command_judgments?: string | null;
  summary_judgment?: string | null;
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
  records: BatchRecord[];
}

// --------------- helpers ---------------

function parseDeviceIds(raw: string | number[]): number[] {
  if (Array.isArray(raw)) return raw;
  try {
    return JSON.parse(raw);
  } catch {
    return [];
  }
}

function formatTs(ts: string | null): string {
  if (!ts) return "-";
  return ts.replace("T", " ").substring(0, 19);
}

const MODE_OPTIONS = [
  { value: "ssh", label: "SSH 模式" },
  { value: "offline", label: "离线模式" },
  { value: "mixed", label: "混合模式" },
];

// --------------- component ---------------

export default function InspectionPage() {
  const navigate = useNavigate();

  // Data
  const [devices, setDevices] = useState<Device[]>([]);
  const [batches, setBatches] = useState<BatchData[]>([]);
  const [loading, setLoading] = useState(true);

  // Device selection
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());

  // Active batch (viewing/executing)
  const [activeBatch, setActiveBatch] = useState<BatchData | null>(null);
  const [expandedDeviceId, setExpandedDeviceId] = useState<number | null>(null);

  // Create batch form
  const [batchName, setBatchName] = useState("");
  const [mode, setMode] = useState("ssh");
  const [creating, setCreating] = useState(false);

  // Polling
  const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);

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

  // --------------- polling ---------------

  const stopPolling = useCallback(() => {
    if (pollingRef.current) {
      clearInterval(pollingRef.current);
      pollingRef.current = null;
    }
  }, []);

  const startPolling = useCallback(
    (batchId: number) => {
      stopPolling();
      pollingRef.current = setInterval(async () => {
        try {
          const b = await invoke<BatchData>("get_batch", { batchId });
          setActiveBatch(b);
          if (b.status !== "running") {
            stopPolling();
            loadData();
          }
        } catch (e) {
          console.error(e);
        }
      }, 2000);
    },
    [stopPolling, loadData],
  );

  useEffect(() => () => stopPolling(), [stopPolling]);

  // --------------- batch actions ---------------

  const viewBatch = useCallback(
    async (batchId: number) => {
      try {
        const b = await invoke<BatchData>("get_batch", { batchId });
        setActiveBatch(b);
        if (b.status === "running") {
          startPolling(batchId);
        } else {
          stopPolling();
        }
      } catch (e) {
        console.error(e);
      }
    },
    [startPolling, stopPolling],
  );

  const handleCreateBatch = async () => {
    if (selectedIds.size === 0) return;
    setCreating(true);
    try {
      const result = await invoke<any>("create_batch", {
        data: {
          name: batchName.trim() || null,
          mode,
          device_ids: Array.from(selectedIds),
          auto_start: false,
        },
      });
      if (result?.success) {
        await loadData();
        setBatchName("");
        if (result.data?.id) {
          viewBatch(result.data.id);
        }
      }
    } catch (e) {
      console.error(e);
    } finally {
      setCreating(false);
    }
  };

  const handleStartBatch = async () => {
    if (!activeBatch) return;
    try {
      await invoke("run_batch", { batchId: activeBatch.id });
      startPolling(activeBatch.id);
    } catch (e) {
      console.error(e);
    }
  };

  const handlePauseBatch = async () => {
    if (!activeBatch) return;
    try {
      await invoke("pause_batch", { batchId: activeBatch.id });
      stopPolling();
      viewBatch(activeBatch.id);
    } catch (e) {
      console.error(e);
    }
  };

  const handleStopBatch = async () => {
    if (!activeBatch) return;
    try {
      await invoke("stop_batch", { batchId: activeBatch.id });
      stopPolling();
      viewBatch(activeBatch.id);
    } catch (e) {
      console.error(e);
    }
  };

  const handleRestartBatch = async () => {
    if (!activeBatch) return;
    try {
      await invoke("restart_batch", { batchId: activeBatch.id });
      viewBatch(activeBatch.id);
    } catch (e) {
      console.error(e);
    }
  };

  const handleRetryDevice = async (deviceId: number) => {
    if (!activeBatch) return;
    try {
      await invoke("retry_device", { batchId: activeBatch.id, deviceId });
      viewBatch(activeBatch.id);
    } catch (e) {
      console.error(e);
    }
  };

  // --------------- selection ---------------

  const toggleDevice = (id: number) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleSelectAll = () => {
    const allIds = new Set(devices.map((d) => d.id));
    if (allIds.size > 0 && [...allIds].every((id) => selectedIds.has(id))) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(allIds);
    }
  };

  // --------------- derived ---------------

  const deviceMap = new Map(devices.map((d) => [d.id, d]));

  const batchDeviceIds = activeBatch ? parseDeviceIds(activeBatch.device_ids) : [];
  const totalDevices = batchDeviceIds.length;
  const completedRecords = activeBatch
    ? activeBatch.records.filter((r) => r.status === "completed" || r.status === "failed").length
    : 0;

  const progressPercent = totalDevices > 0 ? Math.round((completedRecords / totalDevices) * 100) : 0;

  const isRunning = activeBatch?.status === "running";
  const isPending = activeBatch?.status === "pending";
  const isPaused = activeBatch?.status === "paused";
  const isStopped = activeBatch?.status === "stopped";
  const isCompleted =
    activeBatch?.status === "completed" || activeBatch?.status === "partially_completed";

  const canStart = isPending || isPaused;
  const canPauseStop = isRunning;
  const canRestart = isCompleted || isStopped;
  const hasReport =
    (isCompleted || isStopped) && activeBatch?.records.some((r) => r.report_path);

  // --------------- render helpers ---------------

  const renderCommandOutputs = (raw: string | undefined) => {
    if (!raw) return null;
    // command_outputs is stored as a JSON string on the backend; it may be a
    // JSON-encoded object whose values are per-command output text.
    try {
      const obj = JSON.parse(raw);
      if (typeof obj === "object" && obj !== null) {
        return Object.entries(obj).map(([cmd, out]) => (
          <div key={cmd} className="mb-2">
            <div className="text-[hsl(var(--text-secondary))] mb-1">$ {cmd}</div>
            <pre className="bg-[hsl(var(--bg-app))] text-emerald-400 p-2 rounded overflow-auto max-h-48 text-[11px] whitespace-pre-wrap">
              {String(out)}
            </pre>
          </div>
        ));
      }
    } catch {
      // not valid JSON, show as plain text
    }
    return (
      <pre className="bg-[hsl(var(--bg-app))] text-emerald-400 p-2 rounded overflow-auto max-h-48 text-[11px] whitespace-pre-wrap">
        {raw}
      </pre>
    );
  };

  const renderJsonBlock = (label: string, raw: string | null | undefined) => {
    if (!raw) return null;
    let text = raw;
    try {
      text = JSON.stringify(JSON.parse(raw), null, 2);
    } catch {
      // keep as-is
    }
    return (
      <div>
        <span className="font-medium text-[hsl(var(--text-secondary))]">{label}:</span>
        <pre className="mt-1 bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] p-2 rounded max-h-40 overflow-auto whitespace-pre-wrap text-[11px]">
          {text}
        </pre>
      </div>
    );
  };

  // --------------- loading ---------------

  if (loading) return <div className="p-4 text-[hsl(var(--text-secondary))] text-sm">加载中...</div>;

  // --------------- render ---------------

  return (
    <div className="flex gap-3 h-full">
      {/* ===== Left: Device selector ===== */}
      <div className="w-56 shrink-0 bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg flex flex-col">
        <div className="p-2 border-b border-[hsl(var(--border-light))]">
          <h3 className="text-xs font-semibold text-[hsl(var(--text-secondary))] uppercase tracking-wide">
            选择设备
          </h3>
        </div>
        <div className="flex-1 overflow-auto p-1">
          {devices.length === 0 ? (
            <p className="text-xs text-[hsl(var(--text-tertiary))] p-2">暂无设备</p>
          ) : (
            devices.map((d) => (
              <label
                key={d.id}
                className="flex items-center gap-2 px-2 py-1 hover:bg-[hsl(var(--bg-hover))] rounded cursor-pointer text-xs"
              >
                <input
                  type="checkbox"
                  className="w-3.5 h-3.5 shrink-0"
                  checked={selectedIds.has(d.id)}
                  onChange={() => toggleDevice(d.id)}
                />
                <span className="truncate">{d.name}</span>
                <span className="text-[hsl(var(--text-tertiary))] ml-auto text-[10px] hidden">{d.ip}</span>
              </label>
            ))
          )}
        </div>
        <div className="p-2 border-t border-[hsl(var(--border-light))] flex items-center gap-1">
          <input
            type="checkbox"
            className="w-3.5 h-3.5"
            checked={devices.length > 0 && devices.every((d) => selectedIds.has(d.id))}
            onChange={toggleSelectAll}
          />
          <span className="text-xs text-[hsl(var(--text-secondary))]">
            全选 ({selectedIds.size}/{devices.length})
          </span>
        </div>
      </div>

      {/* ===== Right: Main content ===== */}
      <div className="flex-1 min-w-0 flex flex-col gap-3">
        {/* Create batch bar */}
        <div className="bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg p-3">
          <h2 className="text-sm font-semibold mb-2">创建巡检批次</h2>
          <div className="flex items-center gap-2">
            <Input size="sm" className="w-48" placeholder="批次名称（可选）" value={batchName} onChange={(e) => setBatchName(e.target.value)} />
            <Select size="sm" value={mode} onChange={(e) => setMode(e.target.value)}>
              {MODE_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </Select>
            <Button size="sm" disabled={selectedIds.size === 0 || creating} loading={creating} onClick={handleCreateBatch}>
              创建批次
            </Button>
          </div>
          {selectedIds.size > 0 && (
            <p className="text-xs text-[hsl(var(--text-secondary))] mt-2">
              已选择 {selectedIds.size} 个设备
            </p>
          )}
        </div>

        {/* Active batch view */}
        {activeBatch ? (
          <>
            {/* Header + controls + progress */}
            <div className="bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg p-3">
              <div className="flex items-center justify-between mb-3">
                <div className="flex items-center gap-2">
                  <h2 className="text-sm font-semibold">
                    {activeBatch.name || `批次 #${activeBatch.id}`}
                  </h2>
                  <StatusBadge status={activeBatch.status as any} />
                </div>
                <div className="flex items-center gap-1">
                  {canStart && (
                    <Button size="sm" className="bg-emerald-500 hover:bg-emerald-600 text-white" onClick={handleStartBatch}>
                      开始巡检
                    </Button>
                  )}
                  {canPauseStop && (
                    <>
                      <Button size="sm" className="bg-amber-500 hover:bg-amber-600 text-white" onClick={handlePauseBatch}>
                        暂停
                      </Button>
                      <Button variant="danger" size="sm" onClick={handleStopBatch}>
                        停止
                      </Button>
                    </>
                  )}
                  {canRestart && (
                    <Button variant="secondary" size="sm" onClick={handleRestartBatch}>
                      重新开始
                    </Button>
                  )}
                  {hasReport && (
                    <Button variant="secondary" size="sm" onClick={() => navigate("/reports")}>
                      查看报告
                    </Button>
                  )}
                  <Button variant="ghost" size="sm" onClick={() => { setActiveBatch(null); stopPolling(); }}>
                    返回列表
                  </Button>
                </div>
              </div>

              {/* Progress bar */}
              <div className="mb-2">
                <div className="flex items-center justify-between text-xs text-[hsl(var(--text-secondary))] mb-1">
                  <span>巡检进度</span>
                  <span>
                    {completedRecords} / {totalDevices} 设备
                  </span>
                </div>
                <div className="w-full h-2.5 bg-[hsl(var(--bg-hover))] rounded-full overflow-hidden">
                  <div
                    className={`h-full rounded-full transition-all duration-500 ${
                      isCompleted ? "bg-emerald-500" : isRunning ? "bg-[hsl(var(--accent))] animate-pulse" : "bg-[hsl(var(--text-tertiary))]"
                    }`}
                    style={{ width: `${progressPercent}%` }}
                  />
                </div>
              </div>

              {/* Meta */}
              <div className="flex gap-4 text-xs text-[hsl(var(--text-secondary))]">
                <span>模式: {activeBatch.mode}</span>
                <span>
                  触发: {activeBatch.triggered_by === "manual" ? "手动" : "定时"}
                </span>
                {activeBatch.started_at && <span>开始: {formatTs(activeBatch.started_at)}</span>}
                {activeBatch.completed_at && (
                  <span>完成: {formatTs(activeBatch.completed_at)}</span>
                )}
              </div>
            </div>

            {/* Device execution status table */}
            <div className="bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg flex-1 flex flex-col min-h-0">
              <div className="p-2 border-b border-[hsl(var(--border-light))]">
                <h3 className="text-xs font-semibold text-[hsl(var(--text-secondary))] uppercase tracking-wide">
                  设备执行状态
                </h3>
              </div>
              <div className="overflow-auto flex-1">
                <table className="w-full text-xs">
                  <thead className="bg-[hsl(var(--bg-hover))] sticky top-0 z-10">
                    <tr>
                      <th className="text-left px-3 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))] w-8">
                        #
                      </th>
                      <th className="text-left px-3 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))]">
                        设备
                      </th>
                      <th className="text-left px-3 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))]">
                        IP
                      </th>
                      <th className="text-left px-3 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))]">
                        状态
                      </th>
                      <th className="text-left px-3 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))]">
                        AI 分析
                      </th>
                      <th className="text-right px-3 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))] w-16">
                        操作
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {activeBatch.records.length === 0 ? (
                      <tr>
                        <td colSpan={6} className="text-center py-8 text-[hsl(var(--text-tertiary))]">
                          {isPending ? "等待开始巡检..." : "暂无执行记录"}
                        </td>
                      </tr>
                    ) : (
                      activeBatch.records.map((rec, idx) => {
                        const dev = deviceMap.get(rec.device_id);
                        const isExpanded = expandedDeviceId === rec.id;
                        return (
                          <Fragment key={rec.id}>
                            <tr
                              className="border-b border-[hsl(var(--border-light))] hover:bg-[hsl(var(--bg-hover))] cursor-pointer"
                              onClick={() =>
                                setExpandedDeviceId(isExpanded ? null : rec.id)
                              }
                            >
                              <td className="px-3 py-1.5 text-[hsl(var(--text-tertiary))]">{idx + 1}</td>
                              <td className="px-3 py-1.5 font-medium">
                                {dev?.name || `设备 #${rec.device_id}`}
                              </td>
                              <td className="px-3 py-1.5 text-[hsl(var(--text-secondary))]">
                                {dev?.ip || "-"}
                              </td>
                              <td className="px-3 py-1.5">
                                <StatusBadge status={rec.status as any} />
                              </td>
                              <td className="px-3 py-1.5">
                                <AiStatusLabel status={rec.ai_status} />
                              </td>
                              <td className="px-3 py-1.5 text-right">
                                {rec.status === "failed" && (
                                  <Button variant="secondary" size="sm" onClick={(e) => { e.stopPropagation(); handleRetryDevice(rec.device_id); }}>
                                    重试
                                  </Button>
                                )}
                              </td>
                            </tr>
                            {isExpanded && (
                              <tr>
                                <td colSpan={6} className="bg-[hsl(var(--bg-app))] px-4 py-2 border-b border-[hsl(var(--border))]">
                                  <DeviceDetail rec={rec} renderCmd={renderCommandOutputs} renderJson={renderJsonBlock} />
                                </td>
                              </tr>
                            )}
                          </Fragment>
                        );
                      })
                    )}
                  </tbody>
                </table>
              </div>
            </div>
          </>
        ) : (
          /* ===== Recent batches list (no active batch) ===== */
          <div className="bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg flex-1 flex flex-col min-h-0">
            <div className="p-2 border-b border-[hsl(var(--border-light))]">
              <h3 className="text-xs font-semibold text-[hsl(var(--text-secondary))] uppercase tracking-wide">
                最近批次
              </h3>
            </div>
            <div className="overflow-auto flex-1">
              <table className="w-full text-xs">
                <thead className="bg-[hsl(var(--bg-hover))] sticky top-0 z-10">
                  <tr>
                    <th className="text-left px-3 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))]">
                      ID
                    </th>
                    <th className="text-left px-3 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))]">
                      名称
                    </th>
                    <th className="text-left px-3 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))]">
                      状态
                    </th>
                    <th className="text-left px-3 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))]">
                      设备数
                    </th>
                    <th className="text-left px-3 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))]">
                      模式
                    </th>
                    <th className="text-left px-3 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))]">
                      创建时间
                    </th>
                    <th className="text-right px-3 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))] w-16">
                      操作
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {batches.length === 0 ? (
                    <tr>
                      <td colSpan={7} className="text-center py-8 text-[hsl(var(--text-tertiary))]">
                        暂无批次，请选择设备后创建
                      </td>
                    </tr>
                  ) : (
                    batches.map((b) => (
                      <tr
                        key={b.id}
                        className="border-b border-[hsl(var(--border-light))] hover:bg-[hsl(var(--bg-hover))]"
                      >
                        <td className="px-3 py-1.5 font-mono text-[hsl(var(--text-secondary))]">#{b.id}</td>
                        <td className="px-3 py-1.5 font-medium">
                          {b.name || "-"}
                        </td>
                        <td className="px-3 py-1.5">
                          <StatusBadge status={b.status as any} />
                        </td>
                        <td className="px-3 py-1.5">{parseDeviceIds(b.device_ids).length}</td>
                        <td className="px-3 py-1.5">{b.mode}</td>
                        <td className="px-3 py-1.5 text-[hsl(var(--text-secondary))]">
                          {formatTs(b.created_at)}
                        </td>
                        <td className="px-3 py-1.5 text-right">
                          <Button size="sm" onClick={() => viewBatch(b.id)}>
                            查看
                          </Button>
                        </td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </div>

    </div>
  );
}

// --------------- sub-components ---------------

function AiStatusLabel({ status }: { status: string }) {
  const map: Record<string, { cls: string; text: string }> = {
    completed: { cls: "text-emerald-400", text: "已完成" },
    running: { cls: "text-blue-400", text: "分析中..." },
    failed: { cls: "text-red-400", text: "失败" },
    pending: { cls: "text-[hsl(var(--text-tertiary))]", text: "等待中" },
  };
  const m = map[status] || { cls: "text-[hsl(var(--text-tertiary))]", text: status || "-" };
  return <span className={`text-[11px] ${m.cls}`}>{m.text}</span>;
}

function DeviceDetail({
  rec,
  renderCmd,
  renderJson,
}: {
  rec: BatchRecord;
  renderCmd: (raw: string | undefined) => React.ReactNode;
  renderJson: (label: string, raw: string | null | undefined) => React.ReactNode;
}) {
  const hasData = rec.error_message || rec.command_outputs || rec.ai_result || rec.command_judgments || rec.summary_judgment;

  if (!hasData) {
    return <span className="text-xs text-[hsl(var(--text-tertiary))]">暂无详细数据</span>;
  }

  return (
    <div className="text-xs space-y-2">
      {rec.error_message && (
        <div className="bg-[hsl(var(--danger)/0.1)] border border-[hsl(var(--danger)/0.3)] rounded p-2">
          <span className="font-medium text-[hsl(var(--danger))]">错误信息:</span>
          <pre className="mt-1 text-red-400 whitespace-pre-wrap text-[11px]">
            {rec.error_message}
          </pre>
        </div>
      )}
      {rec.command_outputs && (
        <div>
          <span className="font-medium text-[hsl(var(--text-secondary))]">命令输出:</span>
          <div className="mt-1">{renderCmd(rec.command_outputs)}</div>
        </div>
      )}
      {renderJson("AI 分析结果", rec.ai_result)}
      {renderJson("命令评判", rec.command_judgments)}
      {renderJson("综合评判", rec.summary_judgment)}
    </div>
  );
}