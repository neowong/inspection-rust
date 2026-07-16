import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import Button from "./ui/Button";
import Modal from "./Modal";

interface ScheduledTask {
  id: number;
  name: string;
  task_type: string;
  cron_expr: string;
  enabled: number;
  config_json: string;
  last_run_at: string | null;
  next_run_at: string | null;
  run_count: number;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

interface Device {
  id: number;
  name: string;
  ip: string;
  vendor: string;
}

const CRON_PRESETS = [
  { label: "每天 00:00", value: "0 0 * * *" },
  { label: "每天 08:00", value: "0 8 * * *" },
  { label: "每周一 00:00", value: "0 0 * * 1" },
  { label: "每月1日 00:00", value: "0 0 1 * *" },
  { label: "每小时", value: "0 * * * *" },
];

export default function ScheduledTaskSection() {
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [selectedTask, setSelectedTask] = useState<ScheduledTask | null>(null);
  const selectedTaskIdRef = useRef<number | null>(null);
  const [devices, setDevices] = useState<Device[]>([]);
  const [loading, setLoading] = useState(false);
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [editingTask, setEditingTask] = useState<ScheduledTask | null>(null);
  const [deleteIds, setDeleteIds] = useState<Set<number>>(new Set());
  const [confirmBatchDelete, setConfirmBatchDelete] = useState(false);

  // 表单状态
  const [name, setName] = useState("");
  const [cronExpr, setCronExpr] = useState("0 0 * * *");
  const [enabled, setEnabled] = useState(true);
  const [selectedDeviceIds, setSelectedDeviceIds] = useState<number[]>([]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [runningTaskId, setRunningTaskId] = useState<number | null>(null);
  const [runSuccess, setRunSuccess] = useState<{taskId: number; message: string} | null>(null);

  const loadTasks = useCallback(async () => {
    setLoading(true);
    try {
      const result = await invoke<ScheduledTask[]>("list_scheduled_tasks", {});
      setTasks(result);
      const currentId = selectedTaskIdRef.current;
      if (currentId !== null) {
        const updated = result.find(t => t.id === currentId);
        if (updated) setSelectedTask(updated);
      }
    } catch (e) {
      console.error("加载定时任务列表失败:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  const loadDevices = useCallback(async () => {
    try {
      const result = await invoke<Device[]>("list_devices", {});
      setDevices(result);
    } catch (e) {
      console.error("加载设备列表失败:", e);
    }
  }, []);

  useEffect(() => {
    loadTasks();
    loadDevices();
  }, [loadTasks, loadDevices]);

  const resetForm = () => {
    setName("");
    setCronExpr("0 0 * * *");
    setEnabled(true);
    setSelectedDeviceIds([]);
    setError(null);
    setEditingTask(null);
  };

  const openCreateModal = () => {
    resetForm();
    setShowCreateModal(true);
  };

  const openEditModal = (task: ScheduledTask) => {
    setName(task.name);
    setCronExpr(task.cron_expr);
    setEnabled(task.enabled === 1);
    setEditingTask(task);
    try {
      const config = JSON.parse(task.config_json);
      setSelectedDeviceIds(config.device_ids || []);
    } catch {}
    setShowCreateModal(true);
  };

  const handleSave = async () => {
    if (!name.trim()) {
      setError("请输入任务名称");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const configJson = JSON.stringify({ device_ids: selectedDeviceIds });
      if (editingTask) {
        await invoke("update_scheduled_task", {
          taskId: editingTask.id,
          update: { name, cron_expr: cronExpr, enabled, config_json: configJson },
        });
      } else {
        await invoke("create_scheduled_task", {
          task: { name, task_type: "inspection", cron_expr: cronExpr, enabled, config_json: configJson },
        });
      }
      setShowCreateModal(false);
      resetForm();
      await loadTasks();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleToggle = async (taskId: number, currentEnabled: boolean) => {
    try {
      await invoke("toggle_scheduled_task", { taskId, enabled: !currentEnabled });
      await loadTasks();
    } catch (e) {
      console.error("切换任务状态失败:", e);
    }
  };

  const handleBatchDelete = async () => {
    if (deleteIds.size === 0) return;
    if (!confirm(`确定要删除选中的 ${deleteIds.size} 个定时任务吗？`)) return;
    try {
      for (const id of deleteIds) {
        await invoke("delete_scheduled_task", { taskId: id });
      }
      if (selectedTask && deleteIds.has(selectedTask.id)) {
        selectedTaskIdRef.current = null;
        setSelectedTask(null);
      }
      setDeleteIds(new Set());
      setConfirmBatchDelete(false);
      await loadTasks();
    } catch (e) {
      console.error("批量删除失败:", e);
    }
  };

  const handleRunNow = async (taskId: number) => {
    setRunningTaskId(taskId);
    setRunSuccess(null);
    try {
      await invoke("run_scheduled_task", { taskId });
      setRunSuccess({ taskId, message: "巡检任务已触发，请在「手动巡检」标签页查看进度" });
      await loadTasks();
      setTimeout(() => setRunSuccess(null), 3000);
    } catch (e) {
      setError("触发任务失败: " + String(e));
      setTimeout(() => setError(null), 5000);
    } finally {
      setRunningTaskId(null);
    }
  };

  const toggleDevice = (deviceId: number) => {
    setSelectedDeviceIds((prev) =>
      prev.includes(deviceId) ? prev.filter((id) => id !== deviceId) : [...prev, deviceId]
    );
  };

  const getCronLabel = (cron: string) => CRON_PRESETS.find((p) => p.value === cron)?.label || cron;

  const getDeviceCount = (configJson: string) => {
    try {
      const config = JSON.parse(configJson);
      const ids: number[] = config.device_ids || [];
      return ids.length === 0 ? "全部设备" : `${ids.length} 台设备`;
    } catch {
      return "未知";
    }
  };

  const selectedTaskDevices = selectedTask ? (() => {
    try {
      const config = JSON.parse(selectedTask.config_json);
      const ids: number[] = config.device_ids || [];
      if (ids.length === 0) return [];
      return ids.map(id => devices.find(d => d.id === id)).filter(Boolean) as Device[];
    } catch {
      return [];
    }
  })() : [];

  return (
    <div className="flex gap-4" style={{ height: "calc(100vh - 180px)" }}>
      {/* ── Left: Task list panel ── */}
      <div className="w-[300px] shrink-0 flex flex-col border border-[hsl(var(--border))] rounded-lg bg-[hsl(var(--bg-card))] overflow-hidden">
        <div className="p-3 border-b border-[hsl(var(--border))]">
          <div className="flex items-center justify-between mb-2">
            <h2 className="text-base font-bold text-[hsl(var(--text-primary))]">定时巡检</h2>
            <Button onClick={openCreateModal} size="sm">+</Button>
          </div>
          {/* 固定高度的批量操作区域 */}
          <div className="h-6 flex items-center">
            {tasks.length > 0 ? (
              <div className="flex items-center gap-2">
                <label className="flex items-center gap-1 text-xs text-[hsl(var(--text-secondary))] cursor-pointer select-none">
                  <input type="checkbox" className="w-3.5 h-3.5 accent-[hsl(var(--accent))]"
                    checked={deleteIds.size === tasks.length && tasks.length > 0}
                    onChange={() => {
                      if (deleteIds.size === tasks.length) setDeleteIds(new Set());
                      else setDeleteIds(new Set(tasks.map(t => t.id)));
                    }} />
                  全选
                </label>
                <Button
                  size="sm"
                  variant="danger"
                  className={`transition-opacity ${deleteIds.size > 0 ? "opacity-100" : "opacity-0 pointer-events-none"}`}
                  onClick={() => setConfirmBatchDelete(true)}
                >
                  删除选中 ({deleteIds.size})
                </Button>
              </div>
            ) : (
              <p className="text-[11px] text-[hsl(var(--text-tertiary))]">{tasks.length} 个任务</p>
            )}
          </div>
        </div>

        {/* 成功/错误提示 */}
        {runSuccess && (
          <div className="px-3 py-2 bg-[hsl(var(--success)_/_0.1)] text-[hsl(var(--success))] text-xs">
            {runSuccess.message}
          </div>
        )}
        {error && (
          <div className="px-3 py-2 bg-[hsl(var(--danger)_/_0.1)] text-[hsl(var(--danger))] text-xs">
            {error}
          </div>
        )}

        <div className="flex-1 overflow-y-auto">
          {loading && tasks.length === 0 && (
            <p className="text-xs text-[hsl(var(--text-tertiary))] text-center py-8">加载中...</p>
          )}
          {!loading && tasks.length === 0 && (
            <p className="text-xs text-[hsl(var(--text-tertiary))] text-center py-8">暂无定时任务</p>
          )}
          {tasks.map((task) => {
            const selected = selectedTask?.id === task.id;
            const isChecked = deleteIds.has(task.id);
            const toggleCheck = () => {
              setDeleteIds(prev => {
                const next = new Set(prev);
                if (next.has(task.id)) next.delete(task.id); else next.add(task.id);
                return next;
              });
            };
            return (
              <div
                key={task.id}
                onClick={() => {
                  selectedTaskIdRef.current = task.id;
                  setSelectedTask(task);
                }}
                className={`px-3 py-2.5 cursor-pointer select-none border-l-2 transition-colors ${
                  selected
                    ? "border-l-[hsl(var(--accent))] bg-[hsl(var(--accent)_/_0.08)]"
                    : "border-l-transparent hover:bg-[hsl(var(--bg-hover))]"
                }`}
              >
                <div className="flex items-center justify-between mb-1">
                  <div className="flex items-center gap-1.5 min-w-0">
                    <input
                      type="checkbox"
                      checked={isChecked}
                      onChange={toggleCheck}
                      className="w-3.5 h-3.5 shrink-0 accent-[hsl(var(--accent))]"
                      onClick={(e) => e.stopPropagation()}
                    />
                    <span className="text-sm font-medium text-[hsl(var(--text-primary))] truncate">{task.name}</span>
                  </div>
                  <span className={`text-[10px] px-1.5 py-0.5 rounded-full ${
                    task.enabled
                      ? "bg-[hsl(var(--success)_/_0.1)] text-[hsl(var(--success))]"
                      : "bg-[hsl(var(--bg-secondary))] text-[hsl(var(--text-tertiary))]"
                  }`}>
                    {task.enabled ? "已启用" : "已禁用"}
                  </span>
                </div>
                <div className="flex items-center gap-3 text-[11px] text-[hsl(var(--text-tertiary))]">
                  <span>{getDeviceCount(task.config_json)}</span>
                  <span>{getCronLabel(task.cron_expr)}</span>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* ── Right: Detail panel ── */}
      <div className="flex-1 overflow-y-auto space-y-4">
        {!selectedTask ? (
          <div className="flex items-center justify-center h-full text-[hsl(var(--text-tertiary))]">
            <p className="text-sm">← 选择左侧任务查看详情</p>
          </div>
        ) : (
          <>
            {/* 任务信息卡片 */}
            <div className="border border-[hsl(var(--border))] rounded-lg bg-[hsl(var(--bg-card))] p-4">
              <div className="flex items-center justify-between mb-4">
                <div className="flex items-center gap-3">
                  <h3 className="text-lg font-bold text-[hsl(var(--text-primary))]">{selectedTask.name}</h3>
                  <span className={`text-xs px-2 py-0.5 rounded-full ${
                    selectedTask.enabled
                      ? "bg-[hsl(var(--success)_/_0.1)] text-[hsl(var(--success))]"
                      : "bg-[hsl(var(--bg-secondary))] text-[hsl(var(--text-tertiary))]"
                  }`}>
                    {selectedTask.enabled ? "已启用" : "已禁用"}
                  </span>
                </div>
                <div className="flex gap-2">
                  <Button
                    size="sm"
                    loading={runningTaskId === selectedTask.id}
                    disabled={runningTaskId !== null}
                    onClick={() => handleRunNow(selectedTask.id)}
                  >
                    {runningTaskId === selectedTask.id ? "执行中..." : "立即执行"}
                  </Button>
                  <Button size="sm" variant="secondary" onClick={() => openEditModal(selectedTask)}>编辑</Button>
                  <Button size="sm" variant="secondary" onClick={() => handleToggle(selectedTask.id, selectedTask.enabled === 1)}>
                    {selectedTask.enabled ? "禁用" : "启用"}
                  </Button>
                </div>
              </div>

              <div className="grid grid-cols-2 gap-4 text-sm">
                <div>
                  <span className="text-[hsl(var(--text-tertiary))]">执行计划：</span>
                  <span className="font-medium">{getCronLabel(selectedTask.cron_expr)}</span>
                  <span className="text-[hsl(var(--text-tertiary))] ml-1">({selectedTask.cron_expr})</span>
                </div>
                <div>
                  <span className="text-[hsl(var(--text-tertiary))]">已执行：</span>
                  <span className="font-medium">{selectedTask.run_count} 次</span>
                </div>
                <div>
                  <span className="text-[hsl(var(--text-tertiary))]">下次执行：</span>
                  <span className="font-medium">{selectedTask.next_run_at || "未安排"}</span>
                </div>
                {selectedTask.last_run_at && (
                  <div>
                    <span className="text-[hsl(var(--text-tertiary))]">上次执行：</span>
                    <span>{new Date(selectedTask.last_run_at).toLocaleString()}</span>
                  </div>
                )}
                {selectedTask.last_error && (
                  <div className="col-span-2">
                    <span className="text-[hsl(var(--danger))]">上次错误：{selectedTask.last_error}</span>
                  </div>
                )}
              </div>
            </div>

            {/* 巡检设备列表 */}
            <div className="border border-[hsl(var(--border))] rounded-lg bg-[hsl(var(--bg-card))] p-4">
              <h4 className="text-sm font-bold text-[hsl(var(--text-primary))] mb-3">
                巡检设备 {selectedTaskDevices.length > 0 ? `(${selectedTaskDevices.length} 台)` : ""}
              </h4>
              {selectedTaskDevices.length === 0 ? (
                <p className="text-sm text-[hsl(var(--text-tertiary))]">全部设备</p>
              ) : (
                <div className="space-y-2">
                  {selectedTaskDevices.map((device) => (
                    <div
                      key={device.id}
                      className="flex items-center justify-between px-3 py-2 bg-[hsl(var(--bg-hover))] rounded-lg"
                    >
                      <div>
                        <span className="text-sm font-medium">{device.name}</span>
                        <span className="text-xs text-[hsl(var(--text-tertiary))] ml-2">{device.ip}</span>
                      </div>
                      <span className="text-xs text-[hsl(var(--text-tertiary))]">{device.vendor}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </>
        )}
      </div>

      {/* ── Create/Edit Modal ── */}
      <Modal
        open={showCreateModal}
        title={editingTask ? "编辑定时巡检" : "新建定时巡检"}
        width="max-w-lg"
        onClose={() => { setShowCreateModal(false); resetForm(); }}
        footer={
          <>
            <Button variant="ghost" onClick={() => { setShowCreateModal(false); resetForm(); }}>取消</Button>
            <Button onClick={handleSave} loading={saving} disabled={saving}>{saving ? "保存中..." : "保存"}</Button>
          </>
        }
      >
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium mb-1">任务名称</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="例如：每日核心设备巡检"
              className="w-full px-3 py-2 border rounded-lg bg-[hsl(var(--bg-input))] border-[hsl(var(--border))]"
            />
          </div>

          <div>
            <label className="block text-sm font-medium mb-1">执行时间</label>
            <div className="flex gap-2 mb-2">
              {CRON_PRESETS.map((preset) => (
                <button
                  key={preset.value}
                  onClick={() => setCronExpr(preset.value)}
                  className={`px-3 py-1 rounded text-xs ${
                    cronExpr === preset.value
                      ? "bg-[hsl(var(--accent))] text-white"
                      : "bg-[hsl(var(--bg-secondary))]"
                  }`}
                >
                  {preset.label}
                </button>
              ))}
            </div>
            <input
              type="text"
              value={cronExpr}
              onChange={(e) => setCronExpr(e.target.value)}
              placeholder="Cron 表达式，如 0 0 * * *"
              className="w-full px-3 py-2 border rounded-lg bg-[hsl(var(--bg-input))] border-[hsl(var(--border))] font-mono text-sm"
            />
          </div>

          <div>
            <label className="block text-sm font-medium mb-1">巡检设备</label>
            <div className="border rounded-lg p-3 max-h-[300px] overflow-y-auto bg-[hsl(var(--bg-secondary))]">
              <div className="flex items-center gap-2 mb-2 sticky top-0 bg-[hsl(var(--bg-secondary))] pb-2">
                <button onClick={() => setSelectedDeviceIds([])} className="text-xs text-[hsl(var(--accent))] hover:underline">清空</button>
                <button onClick={() => setSelectedDeviceIds(devices.map((d) => d.id))} className="text-xs text-[hsl(var(--accent))] hover:underline">全选</button>
                <span className="text-xs text-[hsl(var(--text-tertiary))] ml-auto">
                  已选 {selectedDeviceIds.length} / {devices.length} 台
                </span>
              </div>
              {devices.map((device) => (
                <label key={device.id} className="flex items-center gap-2 py-1.5 cursor-pointer hover:bg-[hsl(var(--bg-hover))] rounded px-2">
                  <input type="checkbox" checked={selectedDeviceIds.includes(device.id)} onChange={() => toggleDevice(device.id)} className="rounded" />
                  <span className="text-sm flex-1">{device.name}</span>
                  <span className="text-xs text-[hsl(var(--text-tertiary))]">{device.ip}</span>
                  <span className="text-xs text-[hsl(var(--text-tertiary))]">{device.vendor}</span>
                </label>
              ))}
            </div>
          </div>

          <div>
            <label className="flex items-center gap-2 cursor-pointer">
              <input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} className="rounded" />
              <span className="text-sm font-medium">启用任务</span>
            </label>
          </div>

          {error && (
            <div className="p-3 bg-[hsl(var(--danger)_/_0.1)] text-[hsl(var(--danger))] rounded-lg text-sm">{error}</div>
          )}
        </div>
      </Modal>

      {/* ── Batch Delete Confirmation ── */}
      <Modal
        open={confirmBatchDelete}
        title={`批量删除 (${deleteIds.size} 个任务)`}
        width="max-w-sm"
        onClose={() => setConfirmBatchDelete(false)}
        footer={
          <>
            <Button variant="ghost" onClick={() => setConfirmBatchDelete(false)}>取消</Button>
            <Button variant="danger" onClick={handleBatchDelete}>确认删除</Button>
          </>
        }
      >
        <p className="text-sm text-[hsl(var(--text-secondary))]">
          确定要删除选中的 {deleteIds.size} 个定时任务吗？此操作不可恢复。
        </p>
      </Modal>
    </div>
  );
}
