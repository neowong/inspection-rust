import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { useSearchParams } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import type { InspectionBatch, Device, InspectionRecord } from "../types";
import { useShakeValidation } from "../hooks/useShakeValidation";
import { parseCommandOutputs } from "../lib/utils";
import DataTable from "../components/DataTable";
import Modal from "../components/Modal";
import Button from "../components/ui/Button";
import Card from "../components/ui/Card";
import Input from "../components/ui/Input";
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
  const [creating, setCreating] = useState(false);

  // Record detail
  const [expandedRecordId, setExpandedRecordId] = useState<number | null>(null);
  const [fullRecord, setFullRecord] = useState<InspectionRecord | null>(null);
  const [recordLoading, setRecordLoading] = useState(false);

  const { shakeFields, triggerShake } = useShakeValidation();

  const [searchParams] = useSearchParams();
  const statusFilter = searchParams.get("status") || "";

  const selectedIdRef = useRef<number | null>(null);

  const loadBatches = useCallback(() => {
    invoke<any[]>("list_batches", { status: statusFilter || undefined })
      .then((all) => {
        setBatches(all);
        // Sync selected batch from fresh data via ref (avoids stale closure)
        const sid = selectedIdRef.current;
        if (sid !== null) {
          const updated = all.find((x: any) => x.id === sid);
          if (updated) setSelectedBatch(updated);
        }
      }).catch(console.error);
  }, [statusFilter]);

  const loadDevices = useCallback(() => {
    invoke<Device[]>("list_devices", {})
      .then(setDevices).catch(console.error);
  }, []);

  useEffect(() => {
    loadBatches();
    loadDevices();
  }, [loadBatches, loadDevices]);

  // Auto-refresh while any batch is running（用 hasRunning 作为依赖，确保每次状态切换都能正确启停 interval）
  const hasRunning = batches.some((b: any) => b.status === "running");
  useEffect(() => {
    if (!hasRunning) return;
    const id = setInterval(() => {
      loadBatches();
    }, 3000);
    return () => clearInterval(id);
  }, [hasRunning, loadBatches]);

  // ----- Batch actions -----
  const handleAction = (batchId: number, action: string) => {
    setActionLoading(batchId);
    invoke(`${action}_batch`, { batchId })
      .then(() => {
        setActionLoading(null);
        loadBatches();
      })
      .catch((e) => { setActionLoading(null); console.error(typeof e === "string" ? e : JSON.stringify(e)); });
  };

  const handleRestartAndRun = (batchId: number) => {
    setActionLoading(batchId);
    invoke("restart_and_run_batch", { batchId })
      .then(() => {
        setActionLoading(null);
        loadBatches();
      })
      .catch((e) => { setActionLoading(null); console.error(typeof e === "string" ? e : JSON.stringify(e)); });
  };

  const handleRetry = (recordId: number) => {
    setRetrying(recordId);
    invoke("retry_device", { recordId })
      .then(() => { setRetrying(null); loadBatches(); })
      .catch((e) => { setRetrying(null); console.error(typeof e === "string" ? e : JSON.stringify(e)); });
  };

  // ----- Record detail -----
  useEffect(() => {
    if (!expandedRecordId) { setFullRecord(null); return; }
    let cancelled = false;
    setRecordLoading(true);
    invoke<InspectionRecord>("get_record", { recordId: expandedRecordId })
      .then((r) => { if (!cancelled) setFullRecord(r); })
      .catch((e) => { if (!cancelled) console.error(typeof e === "string" ? e : JSON.stringify(e)); })
      .finally(() => { if (!cancelled) setRecordLoading(false); });
    return () => { cancelled = true; };
  }, [expandedRecordId]);

  const parsedOutputs = useMemo(
    () => parseCommandOutputs(fullRecord?.command_outputs),
    [fullRecord?.command_outputs],
  );

  const deviceMap = useMemo(() => {
    const m = new Map<number, Device>();
    for (const d of devices) m.set(d.id, d);
    return m;
  }, [devices]);

  // ----- Create batch -----
  const handleCreate = async () => {
    if (!batchForm.name.trim()) {
      triggerShake("name");
      return;
    }
    if (batchForm.device_ids.length === 0) {
      triggerShake("devices");
      return;
    }
    setCreating(true);
    try {
      await invoke("create_batch", {
        data: {
          name: batchForm.name,
          device_ids: JSON.stringify(batchForm.device_ids),
        },
        autoStart: batchForm.auto_start,
      });
      setModalOpen(false);
      loadBatches();
    } catch (e: any) {
      console.error(typeof e === "string" ? e : JSON.stringify(e));
    } finally {
      setCreating(false);
    }
  };

  return (
    <div className="flex gap-4" style={{ height: "calc(100vh - 120px)" }}>
      {/* ── Left: Batch list panel ── */}
      <div className="w-[300px] shrink-0 flex flex-col border border-[hsl(var(--border))] rounded-lg bg-[hsl(var(--bg-card))] overflow-hidden">
        <div className="p-3 border-b border-[hsl(var(--border))] space-y-2">
          <div className="flex items-center justify-between">
            <h1 className="text-base font-bold text-[hsl(var(--text-primary))]">巡检任务</h1>
            <Button onClick={() => { setBatchForm(getDefaultBatchForm()); setModalOpen(true); }} size="sm">+</Button>
          </div>
          <p className="text-[11px] text-[hsl(var(--text-tertiary))]">{batches.length} 个任务</p>
        </div>
        <div className="flex-1 overflow-y-auto">
          {batches.length === 0 && (
            <p className="text-xs text-[hsl(var(--text-tertiary))] text-center py-8">暂无巡检任务</p>
          )}
          {batches.map((b) => {
            const selected = selectedBatch?.id === b.id;
            return (
              <div
                key={b.id}
                onMouseDown={(e) => {
                  e.preventDefault();
                  selectedIdRef.current = b.id;
                  setSelectedBatch(b);
                  setExpandedRecordId(null);
                  setFullRecord(null);
                }}
                className={`px-3 py-2.5 cursor-pointer select-none border-l-2 ${
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
                    <Button size="sm" variant="ghost" loading={actionLoading === b.id} onClick={() => handleRestartAndRun(b.id)}>重新巡检</Button>
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

        {!selectedBatch ? (
          <div className="flex items-center justify-center h-full text-[hsl(var(--text-tertiary))]">
            <p className="text-sm">← 选择左侧任务查看详情</p>
          </div>
        ) : (
          <>
            <div className="sticky top-0 z-10 bg-[hsl(var(--bg-content))] pb-2">
              <h2 className="text-lg font-semibold text-[hsl(var(--text-primary))]">
                {selectedBatch.name || `任务 #${selectedBatch.id}`}
              </h2>
            </div>


            {/* Device list when batch hasn't run yet */}
            {(!selectedBatch.records || selectedBatch.records.length === 0) && selectedBatch.device_ids && (() => {
              const unrunDevices = devices.filter((d: Device) => (selectedBatch.device_ids || []).includes(d.id));
              if (unrunDevices.length === 0) return null;
              return (
                <Card>
                  <h3 className="text-sm font-semibold mb-3">待巡检设备 ({unrunDevices.length})</h3>
                  <div className="grid grid-cols-3 gap-2">
                    {unrunDevices.map((d) => (
                      <div key={d.id} className="px-3 py-2 rounded-md bg-[hsl(var(--bg-hover))] text-sm">
                        <div className="font-medium">{d.name}{!d.template_id && <span className="ml-1 text-[hsl(var(--warning))]" title="未配置巡检模板">!</span>}</div>
                        <div className="text-xs text-[hsl(var(--text-tertiary))]">{d.ip} · {d.vendor}</div>
                      </div>
                    ))}
                  </div>
                </Card>
              );
            })()}

            {/* Records table */}
            {selectedBatch.records && selectedBatch.records.length > 0 && (
              <Card>
                <h3 className="text-sm font-semibold mb-3">巡检记录 ({selectedBatch.records.length})</h3>
                <DataTable
                  columns={[
                    { key: "device", header:"设备", render: (r: any) => { const d = deviceMap.get(r.device_id); return d ? <span>{d.name} <span className="text-[hsl(var(--text-tertiary))]">{d.ip}</span></span> : `#${r.device_id}`; }},
                    { key: "status", header:"巡检状态", width: "w-24", noTruncate: true, render: (r: any) => <StatusBadge status={batchStatusColor(r.status)} /> },
                    { key: "progress", header:"详情", wrap: true, render: (r: any) => (r.status === "failed" && r.error_message) ? <span className="text-xs text-[hsl(var(--danger))]">{r.error_message}</span> : r.status === "running" ? <span className="text-xs text-[hsl(var(--warning))]">执行中...</span> : r.status === "completed" ? <span className="text-xs text-[hsl(var(--text-secondary))]">{r.completed_at?.slice(0, 19) || "已完成"}</span> : <span className="text-xs text-[hsl(var(--text-secondary))]">{r.status}</span> },
                    {
                      key: "actions", header:"操作", width: "w-24", noTruncate: true,
                      render: (r: any) => (
                        <div className="flex gap-1">
                          <Button variant="ghost" size="sm" onClick={(e: any) => { e.stopPropagation(); setExpandedRecordId(r.id); }}>详情</Button>
                          <Button variant="ghost" size="sm" loading={retrying === r.id}
                            onClick={(e: any) => { e.stopPropagation(); handleRetry(r.id); setExpandedRecordId(r.id); }}>
                            重新巡检
                          </Button>
                        </div>
                      ),
                    },
                  ]}
                  data={selectedBatch.records}
                  rowKey={(r: any) => String(r.id)}
                  selectedKey={expandedRecordId ?? undefined}
                  onRowClick={(r: any) => setExpandedRecordId(r.id)}
                />
              </Card>
            )}

            {/* Expanded record detail */}
            {recordLoading && <Card><p className="text-sm text-[hsl(var(--text-tertiary))]">加载中...</p></Card>}

            {fullRecord && expandedRecordId && (
              <Card>
                <div className="flex items-center justify-between mb-3">
                  <h3 className="text-sm font-semibold">
                    记录详情 — {(() => { const d = deviceMap.get(fullRecord.device_id); return d ? `${d.name} (${d.ip})` : `#${fullRecord.device_id}`; })()}
                  </h3>
                  <div className="flex gap-1.5">
                    <Button variant="ghost" size="sm" loading={retrying === fullRecord.id} onClick={() => handleRetry(fullRecord.id)}>重新巡检</Button>
                    <Button variant="ghost" size="sm" onClick={() => setExpandedRecordId(null)}>关闭</Button>
                  </div>
                </div>

                <div className="grid grid-cols-4 gap-3 mb-4 text-xs">
                  <div><span className="text-[hsl(var(--text-tertiary))]">状态:</span> <StatusBadge status={batchStatusColor(fullRecord.status)} /></div>
                  <div><span className="text-[hsl(var(--text-tertiary))]">开始:</span> {fullRecord.started_at?.slice(0, 19) || "-"}</div>
                  <div><span className="text-[hsl(var(--text-tertiary))]">完成:</span> {fullRecord.completed_at?.slice(0, 19) || "-"}</div>
                  {fullRecord.status === "failed" && fullRecord.error_message && <div className="col-span-4 text-[hsl(var(--danger))]">{fullRecord.error_message}</div>}
                </div>

                {parsedOutputs.length > 0 && (
                  <details open>
                    <summary className="cursor-pointer text-xs font-medium text-[hsl(var(--text-secondary))] mb-2">命令输出 ({parsedOutputs.length})</summary>
                    <div className="space-y-2 max-h-[400px] overflow-auto">
                      {parsedOutputs.map((o: any, i: number) => (
                        <details key={i} className="text-xs">
                          <summary className="cursor-pointer font-mono text-[hsl(var(--accent))] py-0.5">{o.command}</summary>
                          <pre className="mt-1 p-2 rounded bg-[hsl(var(--bg-hover))] text-[hsl(var(--text-secondary))] whitespace-pre-wrap max-h-[200px] overflow-auto">{o.content || "(空)"}</pre>
                        </details>
                      ))}
                    </div>
                  </details>
                )}

                {fullRecord.report_path && (
                  <p className="text-xs text-[hsl(var(--text-secondary))] mt-3">报告已生成: <code className="text-[hsl(var(--accent))] bg-[hsl(var(--bg-hover))] px-1 rounded">{fullRecord.report_path}</code></p>
                )}

                {/* AI analysis result summary */}
                {fullRecord.ai_status === "completed" && fullRecord.summary_judgment && (
                  <div className="mt-3 p-2 rounded bg-[hsl(var(--bg-hover))] text-xs">
                    <span className="font-medium text-[hsl(var(--text-secondary))]">评判结论: </span>
                    <span className="text-[hsl(var(--text-primary))]">{fullRecord.summary_judgment}</span>
                  </div>
                )}
                {fullRecord.ai_status === "failed" && fullRecord.error_message && (
                  <div className="mt-3 p-2 rounded bg-[hsl(var(--danger)_/_0.1)] text-xs text-[hsl(var(--danger))]">
                    <span className="font-medium">AI 分析失败: </span>
                    {fullRecord.error_message}
                  </div>
                )}
                {fullRecord.ai_status === "processing" && (
                  <div className="mt-3 p-2 rounded bg-[hsl(var(--bg-hover))] text-xs text-[hsl(var(--warning))]">
                    AI 分析中...
                  </div>
                )}
              </Card>
            )}
          </>
        )}
      </div>

      {/* Create batch modal */}
      <Modal open={modalOpen} title="创建巡检任务" width="w-[560px]" onClose={() => setModalOpen(false)}>
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium mb-1">任务名称</label>
            <Input className={shakeFields.has("name") ? "shake border-[hsl(var(--danger))]" : ""} value={batchForm.name} onChange={(e) => setBatchForm({ ...batchForm, name: e.target.value })} placeholder="例如: 巡检_2026-06-03_1430" />
          </div>
          <div>
            <label className="block text-sm font-medium mb-1">
              选择设备 ({batchForm.device_ids.length} 台)
              {shakeFields.has("devices") && <span className="ml-2 text-xs text-[hsl(var(--danger))] shake">请选择设备</span>}
            </label>
            <div className="max-h-[300px] overflow-y-auto space-y-1 border border-[hsl(var(--border))] rounded-lg p-2">
              {devices.map((d) => {
                const checked = batchForm.device_ids.includes(d.id);
                const noTemplate = !d.template_id;
                const offline = d.status === "offline";
                // 账号异常：在线但 auth_status 标记为认证失败/无凭据等（排除正常和未验证）
                const authBad =
                  d.status === "online" &&
                  !!d.auth_status &&
                  d.auth_status !== "ok" &&
                  d.auth_status !== "unknown";
                const disabled = offline || authBad; // 离线或账号异常不可选中
                const dimClass = noTemplate || offline || authBad ? "opacity-60" : "";
                const titleText = offline
                  ? "设备离线，无法巡检"
                  : authBad
                    ? `账号异常（${d.auth_message || d.auth_status}），无法巡检`
                    : noTemplate
                      ? "未配置巡检模板"
                      : "";
                return (
                  <label
                    key={d.id}
                    className={`flex items-center gap-2 px-2 py-1.5 rounded ${
                      disabled ? "cursor-not-allowed" : "cursor-pointer hover:bg-[hsl(var(--bg-hover))]"
                    } ${checked ? "bg-[hsl(var(--accent)_/_0.08)]" : ""} ${dimClass}`}
                    title={titleText}
                  >
                    <input
                      type="checkbox"
                      checked={checked}
                      disabled={disabled}
                      onChange={() => {
                        if (disabled) return;
                        setBatchForm({
                          ...batchForm,
                          device_ids: checked
                            ? batchForm.device_ids.filter((id) => id !== d.id)
                            : [...batchForm.device_ids, d.id],
                        });
                      }}
                      className="rounded"
                    />
                    <span className="text-sm flex-1">{d.name}</span>
                    <span className="text-xs text-[hsl(var(--text-tertiary))]">{d.ip} · {d.vendor}</span>
                    {offline && (
                      <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-[hsl(var(--danger)_/_0.12)] text-[hsl(var(--danger))] border border-[hsl(var(--danger)_/_0.25)]">
                        离线
                      </span>
                    )}
                    {authBad && (
                      <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-[hsl(var(--danger)_/_0.12)] text-[hsl(var(--danger))] border border-[hsl(var(--danger)_/_0.25)]">
                        账号异常
                      </span>
                    )}
                    {!offline && !authBad && d.status === "unknown" && (
                      <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-[hsl(var(--text-tertiary)_/_0.12)] text-[hsl(var(--text-secondary))] border border-[hsl(var(--text-tertiary)_/_0.25)]">
                        未知
                      </span>
                    )}
                    {noTemplate && (
                      <span className="text-xs text-[hsl(var(--warning))]" title="未配置巡检模板">!</span>
                    )}
                  </label>
                );
              })}
            </div>
          </div>
          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" checked={batchForm.auto_start} onChange={(e) => setBatchForm({ ...batchForm, auto_start: e.target.checked })} />
            创建后自动执行
          </label>
        </div>
        <div className="flex justify-end gap-2 mt-4">
          <Button variant="ghost" onClick={() => setModalOpen(false)}>取消</Button>
          <Button onClick={handleCreate} loading={creating}>创建</Button>
        </div>
      </Modal>

      {/* Delete confirmation modal */}
      <Modal open={confirmDelete !== null} title="删除巡检任务" onClose={() => setConfirmDelete(null)}>
        <p className="text-sm text-[hsl(var(--text-secondary))]">此操作不可恢复，所有相关巡检记录也将被删除。</p>
        <div className="flex justify-end gap-2 mt-4">
          <Button variant="ghost" onClick={() => setConfirmDelete(null)}>取消</Button>
          <Button variant="danger" onClick={async () => {
            if (confirmDelete === null) return;
            try { await invoke("delete_batch", { batchId: confirmDelete }); setConfirmDelete(null); loadBatches(); selectedIdRef.current = null; setSelectedBatch(null); } catch (e: any) { console.error(typeof e === "string" ? e : JSON.stringify(e)); setConfirmDelete(null); }
          }}>确认删除</Button>
        </div>
      </Modal>
    </div>
  );
}
