import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import Button from "./ui/Button";

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

// 卡片式网格布局
export default function ScheduledTaskSectionV2() {
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [devices, setDevices] = useState<Device[]>([]);
  const [loading, setLoading] = useState(false);
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [editingTask, setEditingTask] = useState<ScheduledTask | null>(null);
  const [expandedTaskId, setExpandedTaskId] = useState<number | null>(null);

  // 表单状态
  const [name, setName] = useState("");
  const [cronExpr, setCronExpr] = useState("0 0 * * *");
  const [enabled, setEnabled] = useState(true);
  const [selectedDeviceIds, setSelectedDeviceIds] = useState<number[]>([]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [runningTaskId, setRunningTaskId] = useState<number | null>(null);
  const [toast, setToast] = useState<{type: "success" | "error"; message: string} | null>(null);

  const loadTasks = useCallback(async () => {
    setLoading(true);
    try {
      const result = await invoke<ScheduledTask[]>("list_scheduled_tasks", {});
      setTasks(result);
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

  const showToast = (type: "success" | "error", message: string) => {
    setToast({ type, message });
    setTimeout(() => setToast(null), 3000);
  };

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

  const openEditModal = (task: ScheduledTask, e?: React.MouseEvent) => {
    e?.stopPropagation();
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
        showToast("success", "任务已更新");
      } else {
        await invoke("create_scheduled_task", {
          task: { name, task_type: "inspection", cron_expr: cronExpr, enabled, config_json: configJson },
        });
        showToast("success", "任务已创建");
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

  const handleToggle = async (taskId: number, currentEnabled: boolean, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await invoke("toggle_scheduled_task", { taskId, enabled: !currentEnabled });
      showToast("success", currentEnabled ? "任务已禁用" : "任务已启用");
      await loadTasks();
    } catch (e) {
      showToast("error", "操作失败");
    }
  };

  const handleDelete = async (taskId: number, e: React.MouseEvent) => {
    e.stopPropagation();
    if (!confirm("确定要删除此定时任务吗？")) return;
    try {
      await invoke("delete_scheduled_task", { taskId });
      showToast("success", "任务已删除");
      await loadTasks();
    } catch (e) {
      showToast("error", "删除失败");
    }
  };

  const handleRunNow = async (taskId: number, e: React.MouseEvent) => {
    e.stopPropagation();
    setRunningTaskId(taskId);
    try {
      await invoke("run_scheduled_task", { taskId });
      showToast("success", "巡检任务已触发，请在「手动巡检」标签页查看进度");
      await loadTasks();
    } catch (e) {
      showToast("error", "触发失败: " + String(e));
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

  const getDeviceNames = (configJson: string) => {
    try {
      const config = JSON.parse(configJson);
      const ids: number[] = config.device_ids || [];
      if (ids.length === 0) return "全部设备";
      const names = ids.map(id => devices.find(d => d.id === id)?.name || `#${id}`);
      return names.length > 2 ? `${names.slice(0, 2).join(", ")} +${names.length - 2}` : names.join(", ");
    } catch {
      return "未知";
    }
  };

  const getTaskDevices = (configJson: string): Device[] => {
    try {
      const config = JSON.parse(configJson);
      const ids: number[] = config.device_ids || [];
      return ids.map(id => devices.find(d => d.id === id)).filter(Boolean) as Device[];
    } catch {
      return [];
    }
  };

  // 统计卡片数据
  const stats = {
    total: tasks.length,
    enabled: tasks.filter(t => t.enabled).length,
    disabled: tasks.filter(t => !t.enabled).length,
    totalRuns: tasks.reduce((sum, t) => sum + t.run_count, 0),
  };

  return (
    <div className="space-y-6">
      {/* 顶部统计卡片 */}
      <div className="grid grid-cols-4 gap-4">
        <div className="bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-xl p-4">
          <div className="text-2xl font-bold text-[hsl(var(--accent))]">{stats.total}</div>
          <div className="text-sm text-[hsl(var(--text-tertiary))]">总任务数</div>
        </div>
        <div className="bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-xl p-4">
          <div className="text-2xl font-bold text-[hsl(var(--success))]">{stats.enabled}</div>
          <div className="text-sm text-[hsl(var(--text-tertiary))]">已启用</div>
        </div>
        <div className="bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-xl p-4">
          <div className="text-2xl font-bold text-[hsl(var(--text-tertiary))]">{stats.disabled}</div>
          <div className="text-sm text-[hsl(var(--text-tertiary))]">已禁用</div>
        </div>
        <div className="bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-xl p-4">
          <div className="text-2xl font-bold text-[hsl(var(--info))]">{stats.totalRuns}</div>
          <div className="text-sm text-[hsl(var(--text-tertiary))]">累计执行</div>
        </div>
      </div>

      {/* 操作栏 */}
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold">任务列表</h2>
        <Button onClick={openCreateModal}>+ 新建定时巡检</Button>
      </div>

      {/* Toast 提示 */}
      {toast && (
        <div
          className={`fixed top-4 right-4 z-50 px-4 py-3 rounded-lg shadow-lg text-sm flex items-center gap-2 animate-in slide-in-from-top-2 ${
            toast.type === "success"
              ? "bg-[hsl(var(--success))] text-white"
              : "bg-[hsl(var(--danger))] text-white"
          }`}
        >
          {toast.type === "success" ? (
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
            </svg>
          ) : (
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          )}
          {toast.message}
        </div>
      )}

      {/* 任务卡片网格 */}
      {loading && tasks.length === 0 ? (
        <div className="text-center py-12 text-[hsl(var(--text-tertiary))]">加载中...</div>
      ) : tasks.length === 0 ? (
        <div className="text-center py-12 bg-[hsl(var(--bg-card))] border border-dashed border-[hsl(var(--border))] rounded-xl">
          <div className="text-4xl mb-2">⏰</div>
          <div className="text-[hsl(var(--text-tertiary))]">暂无定时任务</div>
          <Button onClick={openCreateModal} className="mt-4">创建第一个定时巡检</Button>
        </div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-4">
          {tasks.map((task) => {
            const isExpanded = expandedTaskId === task.id;
            const taskDevices = getTaskDevices(task.config_json);

            return (
              <div
                key={task.id}
                onClick={() => setExpandedTaskId(isExpanded ? null : task.id)}
                className={`bg-[hsl(var(--bg-card))] border rounded-xl overflow-hidden cursor-pointer transition-all hover:shadow-md ${
                  task.enabled
                    ? "border-[hsl(var(--border))]"
                    : "border-[hsl(var(--border))] opacity-60"
                } ${isExpanded ? "ring-2 ring-[hsl(var(--accent))]" : ""}`}
              >
                {/* 卡片头部 */}
                <div className="p-4 pb-3">
                  <div className="flex items-start justify-between mb-3">
                    <div className="flex-1 min-w-0">
                      <h3 className="font-semibold text-[hsl(var(--text-primary))] truncate">{task.name}</h3>
                      <div className="flex items-center gap-2 mt-1">
                        <span
                          className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${
                            task.enabled
                              ? "bg-[hsl(var(--success)_/_0.1)] text-[hsl(var(--success))]"
                              : "bg-[hsl(var(--bg-secondary))] text-[hsl(var(--text-tertiary))]"
                          }`}
                        >
                          <span className={`w-1.5 h-1.5 rounded-full ${task.enabled ? "bg-[hsl(var(--success))]" : "bg-[hsl(var(--text-tertiary))]"}`} />
                          {task.enabled ? "运行中" : "已暂停"}
                        </span>
                      </div>
                    </div>

                    {/* 启用/禁用开关 */}
                    <button
                      onClick={(e) => handleToggle(task.id, task.enabled === 1, e)}
                      className={`w-11 h-6 rounded-full transition-colors flex-shrink-0 ${
                        task.enabled ? "bg-[hsl(var(--accent))]" : "bg-[hsl(var(--bg-secondary))]"
                      }`}
                    >
                      <div
                        className={`w-5 h-5 rounded-full bg-white shadow transition-transform ${
                          task.enabled ? "translate-x-5" : "translate-x-0.5"
                        }`}
                      />
                    </button>
                  </div>

                  {/* 执行计划 */}
                  <div className="flex items-center gap-2 text-sm text-[hsl(var(--text-secondary))] mb-2">
                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    {getCronLabel(task.cron_expr)}
                  </div>

                  {/* 设备信息 */}
                  <div className="flex items-center gap-2 text-sm text-[hsl(var(--text-secondary))]">
                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01" />
                    </svg>
                    {getDeviceNames(task.config_json)}
                  </div>
                </div>

                {/* 统计信息 */}
                <div className="px-4 py-3 bg-[hsl(var(--bg-secondary))] border-t border-[hsl(var(--border))]">
                  <div className="flex items-center justify-between text-xs">
                    <div className="flex items-center gap-4">
                      <span className="text-[hsl(var(--text-tertiary))]">
                        已执行 <span className="font-medium text-[hsl(var(--text-primary))]">{task.run_count}</span> 次
                      </span>
                      {task.last_run_at && (
                        <span className="text-[hsl(var(--text-tertiary))]">
                          上次 {new Date(task.last_run_at).toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" })}
                        </span>
                      )}
                    </div>
                    {task.last_error && (
                      <span className="text-[hsl(var(--danger))]">有错误</span>
                    )}
                  </div>
                </div>

                {/* 展开的详情 */}
                {isExpanded && (
                  <div className="border-t border-[hsl(var(--border))]">
                    {/* 设备列表 */}
                    {taskDevices.length > 0 && (
                      <div className="px-4 py-3 border-b border-[hsl(var(--border))]">
                        <div className="text-xs font-medium text-[hsl(var(--text-tertiary))] mb-2">巡检设备</div>
                        <div className="flex flex-wrap gap-1.5">
                          {taskDevices.map((device) => (
                            <span
                              key={device.id}
                              className="inline-flex items-center gap-1 px-2 py-1 bg-[hsl(var(--bg-secondary))] rounded-md text-xs"
                            >
                              <span className="w-1.5 h-1.5 rounded-full bg-[hsl(var(--accent))]" />
                              {device.name}
                            </span>
                          ))}
                        </div>
                      </div>
                    )}

                    {/* 操作按钮 */}
                    <div className="px-4 py-3 flex items-center gap-2">
                      <Button
                        size="sm"
                        loading={runningTaskId === task.id}
                        disabled={runningTaskId !== null}
                        onClick={(e) => handleRunNow(task.id, e!)}
                        className="flex-1"
                      >
                        {runningTaskId === task.id ? "执行中..." : "立即执行"}
                      </Button>
                      <Button size="sm" variant="secondary" onClick={(e) => openEditModal(task, e)} className="flex-1">
                        编辑
                      </Button>
                      <Button size="sm" variant="danger" onClick={(e) => handleDelete(task.id, e)}>
                        删除
                      </Button>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* 创建/编辑模态框 */}
      {showCreateModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="bg-[hsl(var(--bg-content))] rounded-xl shadow-xl w-full max-w-lg max-h-[90vh] overflow-y-auto">
            <div className="p-6">
              <h2 className="text-xl font-bold mb-4">
                {editingTask ? "编辑定时巡检" : "新建定时巡检"}
              </h2>

              <div className="space-y-4">
                {/* 任务名称 */}
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

                {/* 执行时间 */}
                <div>
                  <label className="block text-sm font-medium mb-1">执行时间</label>
                  <div className="flex flex-wrap gap-2 mb-2">
                    {CRON_PRESETS.map((preset) => (
                      <button
                        key={preset.value}
                        onClick={() => setCronExpr(preset.value)}
                        className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
                          cronExpr === preset.value
                            ? "bg-[hsl(var(--accent))] text-white"
                            : "bg-[hsl(var(--bg-secondary))] hover:bg-[hsl(var(--bg-hover))]"
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

                {/* 巡检设备 */}
                <div>
                  <label className="block text-sm font-medium mb-1">巡检设备</label>
                  <div className="border rounded-lg p-3 max-h-48 overflow-y-auto bg-[hsl(var(--bg-secondary))]">
                    <div className="flex items-center gap-2 mb-2 pb-2 border-b border-[hsl(var(--border))]">
                      <button
                        onClick={() => setSelectedDeviceIds([])}
                        className="text-xs text-[hsl(var(--accent))] hover:underline"
                      >
                        清空
                      </button>
                      <button
                        onClick={() => setSelectedDeviceIds(devices.map((d) => d.id))}
                        className="text-xs text-[hsl(var(--accent))] hover:underline"
                      >
                        全选
                      </button>
                      <span className="text-xs text-[hsl(var(--text-tertiary))] ml-auto">
                        {selectedDeviceIds.length}/{devices.length}
                      </span>
                    </div>
                    {devices.map((device) => (
                      <label
                        key={device.id}
                        className="flex items-center gap-2 py-1.5 cursor-pointer hover:bg-[hsl(var(--bg-hover))] rounded px-2"
                      >
                        <input
                          type="checkbox"
                          checked={selectedDeviceIds.includes(device.id)}
                          onChange={() => toggleDevice(device.id)}
                          className="rounded"
                        />
                        <span className="text-sm flex-1">{device.name}</span>
                        <span className="text-xs text-[hsl(var(--text-tertiary))]">{device.ip}</span>
                        <span className="text-xs text-[hsl(var(--text-tertiary))]">{device.vendor}</span>
                      </label>
                    ))}
                  </div>
                </div>

                {/* 启用状态 */}
                <div>
                  <label className="flex items-center gap-3 cursor-pointer">
                    <div
                      className={`w-11 h-6 rounded-full transition-colors ${
                        enabled ? "bg-[hsl(var(--accent))]" : "bg-[hsl(var(--bg-secondary))]"
                      }`}
                      onClick={() => setEnabled(!enabled)}
                    >
                      <div
                        className={`w-5 h-5 rounded-full bg-white shadow transition-transform mt-0.5 ${
                          enabled ? "translate-x-5 ml-0.5" : "translate-x-0.5"
                        }`}
                      />
                    </div>
                    <span className="text-sm font-medium">创建后立即启用</span>
                  </label>
                </div>
              </div>

              {/* 错误提示 */}
              {error && (
                <div className="mt-4 p-3 bg-[hsl(var(--danger)_/_0.1)] text-[hsl(var(--danger))] rounded-lg text-sm">
                  {error}
                </div>
              )}

              {/* 操作按钮 */}
              <div className="flex justify-end gap-2 mt-6">
                <Button variant="ghost" onClick={() => { setShowCreateModal(false); resetForm(); }}>
                  取消
                </Button>
                <Button onClick={handleSave} loading={saving} disabled={saving}>
                  {saving ? "保存中..." : "保存"}
                </Button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
