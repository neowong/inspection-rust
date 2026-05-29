import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { InspectionBatch, Device, InspectionRecordSummary } from "../types";
import Toolbar from "../components/Toolbar";
import DataTable from "../components/DataTable";
import Modal from "../components/Modal";
import Button from "../components/ui/Button";
import Input from "../components/ui/Input";
import StatusBadge from "../components/StatusBadge";
import { batchStatusColor } from "../lib/status";

interface BatchForm {
  name: string;
  device_ids: number[];
  auto_start: boolean;
}

const EMPTY_BATCH_FORM: BatchForm = { name: "", device_ids: [], auto_start: false };

export default function InspectionPage() {
  const [batches, setBatches] = useState<InspectionBatch[]>([]);
  const [selectedBatch, setSelectedBatch] = useState<InspectionBatch | null>(null);
  const [devices, setDevices] = useState<Device[]>([]);
  const [modalOpen, setModalOpen] = useState(false);
  const [batchForm, setBatchForm] = useState<BatchForm>(EMPTY_BATCH_FORM);
  const [confirmDelete, setConfirmDelete] = useState<number | null>(null);
  const [retrying, setRetrying] = useState<number | null>(null);

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

  const refreshSelectedBatch = useCallback(() => {
    if (!selectedBatch) return;
    invoke<InspectionBatch>("get_batch", { batchId: selectedBatch.id })
      .then((b) => {
        setSelectedBatch(b);
        setBatches((prev) => prev.map((bp) => bp.id === b.id ? b : bp));
      })
      .catch(console.error);
  }, [selectedBatch]);

  // Auto-refresh selected batch every 3 seconds
  useEffect(() => {
    if (!selectedBatch) return;
    const id = setInterval(refreshSelectedBatch, 3000);
    return () => clearInterval(id);
  }, [refreshSelectedBatch, selectedBatch]);

  const handleCreateBatch = () => {
    const data: Record<string, unknown> = {
      name: batchForm.name,
      device_ids: JSON.stringify(batchForm.device_ids),
    };
    invoke<InspectionBatch>("create_batch", { data, autoStart: batchForm.auto_start })
      .then(() => {
        setModalOpen(false);
        setBatchForm(EMPTY_BATCH_FORM);
        loadBatches();
      })
      .catch(console.error);
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
    invoke<void>(cmd, { batchId })
      .then(() => { loadBatches(); if (selectedBatch?.id === batchId) refreshSelectedBatch(); })
      .catch(console.error);
  };

  const handleRetry = (recordId: number) => {
    setRetrying(recordId);
    invoke<void>("retry_device", { recordId })
      .then(() => {
        setRetrying(null);
        refreshSelectedBatch();
      })
      .catch(() => setRetrying(null));
  };

  const handleDelete = (batchId: number) => {
    invoke<void>("delete_batch", { batchId })
      .then(() => {
        setConfirmDelete(null);
        if (selectedBatch?.id === batchId) setSelectedBatch(null);
        loadBatches();
      })
      .catch(console.error);
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">执行巡检</h1>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1">创建和管理巡检批次</p>
      </div>

      <Toolbar>
        <Button onClick={() => { setBatchForm(EMPTY_BATCH_FORM); setModalOpen(true); }} size="sm">创建批次</Button>
      </Toolbar>

      {/* Batch list */}
      <DataTable<InspectionBatch>
        columns={[
          { key: "id", header: "ID", width: "60px", render: (r) => `#${r.id}` },
          { key: "name", header: "名称", render: (r) => r.name || "-" },
          { key: "status", header: "状态", render: (r) => <StatusBadge status={batchStatusColor(r.status)} /> },
          { key: "device_count", header: "设备数", width: "80px", render: (r) => String(r.device_ids?.length || 0) },
          { key: "started_at", header: "开始时间", render: (r) => r.started_at ? new Date(r.started_at).toLocaleString("zh-CN") : "-" },
          { key: "completed_at", header: "完成时间", render: (r) => r.completed_at ? new Date(r.completed_at).toLocaleString("zh-CN") : "-" },
          {
            key: "actions", header: "操作", width: "280px", render: (r) => (
              <div className="flex gap-1" onClick={(e) => e.stopPropagation()}>
                {(r.status === "pending" || r.status === "waiting") && (
                  <Button size="sm" variant="ghost" onClick={() => handleAction(r.id, "run")}>执行</Button>
                )}
                {r.status === "running" && (
                  <>
                    <Button size="sm" variant="ghost" onClick={() => handleAction(r.id, "pause")}>暂停</Button>
                    <Button size="sm" variant="ghost" onClick={() => handleAction(r.id, "stop")}>停止</Button>
                  </>
                )}
                {r.status === "stopped" || r.status === "paused" || r.status === "failed" ? (
                  <Button size="sm" variant="ghost" onClick={() => handleAction(r.id, "restart")}>重启</Button>
                ) : null}
                <Button size="sm" variant="ghost" onClick={() => setConfirmDelete(r.id)}>删除</Button>
              </div>
            ),
          },
        ]}
        data={batches}
        rowKey={(r) => r.id}
        onRowClick={(r) => setSelectedBatch(r)}
        selectedKey={selectedBatch?.id}
        emptyText="暂无巡检批次"
      />

      {/* Batch detail */}
      {selectedBatch && (
        <div>
          <h2 className="text-lg font-semibold text-[hsl(var(--text-primary))] mb-3">
            批次详情: {selectedBatch.name || `#${selectedBatch.id}`}
          </h2>
          <DataTable<InspectionRecordSummary>
            columns={[
              { key: "device_id", header: "设备 ID", width: "80px", render: (r) => `#${r.device_id}` },
              { key: "status", header: "状态", render: (r) => <StatusBadge status={batchStatusColor(r.status)} /> },
              { key: "ai_status", header: "AI 状态", render: (r) => {
                if (!r.ai_status || r.ai_status === "none") return <span className="text-[hsl(var(--text-tertiary))]">-</span>;
                return <StatusBadge status={batchStatusColor(r.ai_status)} />;
              }},
              { key: "error_message", header: "错误信息", render: (r) =>
                r.error_message
                  ? <span className="text-[hsl(var(--danger))] text-xs">{r.error_message}</span>
                  : "-",
              },
              {
                key: "actions", header: "操作", width: "120px", render: (r) => (
                  <div className="flex gap-1" onClick={(e) => e.stopPropagation()}>
                    {r.status === "failed" && (
                      <Button size="sm" variant="ghost" loading={retrying === r.id} onClick={() => handleRetry(r.id)}>
                        重试
                      </Button>
                    )}
                  </div>
                ),
              },
            ]}
            data={selectedBatch.records || []}
            rowKey={(r) => r.id}
            emptyText="暂无记录"
          />
        </div>
      )}

      {/* Create batch modal */}
      <Modal
        open={modalOpen}
        title="创建巡检批次"
        width="max-w-lg"
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
            <Input value={batchForm.name} onChange={(e) => setBatchForm({ ...batchForm, name: e.target.value })} placeholder="例如: 核心交换机周检" />
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">选择设备</label>
            <div className="max-h-48 overflow-y-auto border border-[hsl(var(--border))] rounded-md p-2 space-y-1">
              {devices.length === 0 && <p className="text-xs text-[hsl(var(--text-tertiary))]">暂无设备</p>}
              {devices.map((d) => {
                const checked = batchForm.device_ids.includes(d.id);
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
