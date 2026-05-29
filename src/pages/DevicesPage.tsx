import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import Toolbar from "../components/Toolbar";
import SearchInput from "../components/SearchInput";
import DataTable from "../components/DataTable";
import StatusBadge from "../components/StatusBadge";
import Modal from "../components/Modal";
import ContextMenu, { ContextMenuItem } from "../components/ContextMenu";
import type { Device } from "../types";

const VENDORS = ["H3C", "Huawei", "Cisco", "Ruijie", "Ubuntu", "CentOS", "Debian", "RedHat", "MySQL", "PostgreSQL", "Oracle"];
const GROUPS = [
  { value: "network", label: "网络设备" },
  { value: "system", label: "系统设备" },
];
const MODES = [
  { value: "ssh", label: "SSH" },
  { value: "offline", label: "离线" },
  { value: "web", label: "Web" },
];

interface DeviceForm {
  name: string;
  ip: string;
  vendor: string;
  model: string;
  group: string;
  inspection_mode: string;
  ssh_username: string;
  ssh_password: string;
  ssh_port: number;
  template_id: string;
}

const EMPTY_FORM: DeviceForm = {
  name: "",
  ip: "",
  vendor: "H3C",
  model: "",
  group: "network",
  inspection_mode: "ssh",
  ssh_username: "",
  ssh_password: "",
  ssh_port: 22,
  template_id: "",
};

function formatTime(ts: string | null) {
  if (!ts) return "-";
  return ts.replace("T", " ").substring(0, 19);
}

export default function DevicesPage() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");
  const [groupFilter, setGroupFilter] = useState<string>("all");
  const [selected, setSelected] = useState<Set<number>>(new Set());

  // Context menu
  const [ctxVisible, setCtxVisible] = useState(false);
  const [ctxPos, setCtxPos] = useState({ x: 0, y: 0 });
  const [ctxDevice, setCtxDevice] = useState<Device | null>(null);

  // Modals
  const [editOpen, setEditOpen] = useState(false);
  const [editingDevice, setEditingDevice] = useState<Device | null>(null);
  const [form, setForm] = useState<DeviceForm>(EMPTY_FORM);
  const [saving, setSaving] = useState(false);

  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<{ single?: Device; batch?: number[] }>({});

  const loadDevices = useCallback(async () => {
    try {
      const list = await invoke<Device[]>("list_devices");
      setDevices(list);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadDevices(); }, [loadDevices]);

  // Filtering
  const filtered = useMemo(() => {
    let list = devices;
    if (groupFilter !== "all") {
      list = list.filter((d) => d.group === groupFilter);
    }
    if (search.trim()) {
      const kw = search.trim().toLowerCase();
      list = list.filter((d) => d.name.toLowerCase().includes(kw) || d.ip.toLowerCase().includes(kw));
    }
    return list;
  }, [devices, groupFilter, search]);

  // Group counts
  const groupCounts = useMemo(() => {
    const network = devices.filter((d) => d.group === "network").length;
    const system = devices.filter((d) => d.group === "system").length;
    return { network, system, all: devices.length };
  }, [devices]);

  // Selection handlers
  const toggleSelect = (id: number) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleSelectAll = () => {
    const allIds = new Set(filtered.map((d) => d.id));
    if (allIds.size > 0 && [...allIds].every((id) => selected.has(id))) {
      setSelected(new Set());
    } else {
      setSelected(allIds);
    }
  };

  // Context menu
  const onContextMenu = useCallback((e: React.MouseEvent, row: Device) => {
    e.preventDefault();
    setCtxPos({ x: e.clientX, y: e.clientY });
    setCtxDevice(row);
    setCtxVisible(true);
  }, []);

  const ctxItems: ContextMenuItem[] = useMemo(() => {
    const d = ctxDevice;
    if (!d) return [];
    return [
      { label: "编辑设备", action: () => openEdit(d) },
      { label: "手动巡检", action: () => handleInspect(d.id) },
      { label: "-", separator: true },
      { label: "复制设备", action: () => handleCopy(d) },
      { label: "-", separator: true },
      { label: "删除设备", danger: true, action: () => { setDeleteTarget({ single: d }); setDeleteOpen(true); } },
    ] as ContextMenuItem[];
  }, [ctxDevice]);

  // Actions
  const handleInspect = async (id: number) => {
    try {
      await invoke("check_device_status", { id });
      await loadDevices();
    } catch (e) {
      console.error(e);
    }
  };

  const handleRefreshAll = async () => {
    try {
      await invoke("check_all_devices_status");
      await loadDevices();
    } catch (e) {
      console.error(e);
    }
  };

  const openEdit = (device: Device) => {
    setEditingDevice(device);
    setForm({
      name: device.name,
      ip: device.ip,
      vendor: device.vendor,
      model: device.model || "",
      group: device.group,
      inspection_mode: device.inspection_mode,
      ssh_username: device.ssh_username || "",
      ssh_password: "",
      ssh_port: device.ssh_port,
      template_id: device.template_id?.toString() || "",
    });
    setEditOpen(true);
  };

  const openAdd = () => {
    setEditingDevice(null);
    setForm(EMPTY_FORM);
    setEditOpen(true);
  };

  const handleCopy = (device: Device) => {
    setEditingDevice(null);
    setForm({
      name: device.name + " (副本)",
      ip: device.ip,
      vendor: device.vendor,
      model: device.model || "",
      group: device.group,
      inspection_mode: device.inspection_mode,
      ssh_username: device.ssh_username || "",
      ssh_password: "",
      ssh_port: device.ssh_port,
      template_id: device.template_id?.toString() || "",
    });
    setEditOpen(true);
  };

  const handleSave = async () => {
    if (!form.name.trim() || !form.ip.trim()) return;
    setSaving(true);
    try {
      const payload = {
        name: form.name.trim(),
        ip: form.ip.trim(),
        vendor: form.vendor,
        model: form.model.trim() || undefined,
        group: form.group,
        inspection_mode: form.inspection_mode,
        ssh_username: form.ssh_username.trim() || undefined,
        ssh_password: form.ssh_password || undefined,
        ssh_port: form.ssh_port,
        template_id: form.template_id ? parseInt(form.template_id) : undefined,
      };
      if (editingDevice) {
        await invoke("update_device", { id: editingDevice.id, ...payload });
      } else {
        await invoke("create_device", payload);
      }
      setEditOpen(false);
      await loadDevices();
    } catch (e) {
      console.error(e);
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    try {
      if (deleteTarget.batch && deleteTarget.batch.length > 0) {
        await invoke("batch_delete_devices", { ids: deleteTarget.batch });
      } else if (deleteTarget.single) {
        await invoke("delete_device", { id: deleteTarget.single.id });
      }
      setDeleteOpen(false);
      setDeleteTarget({});
      setSelected(new Set());
      await loadDevices();
    } catch (e) {
      console.error(e);
    }
  };

  const handleBatchDelete = () => {
    if (selected.size === 0) return;
    setDeleteTarget({ batch: [...selected] });
    setDeleteOpen(true);
  };

  const handleBatchInspect = async () => {
    for (const id of selected) {
      try { await invoke("check_device_status", { id }); } catch (e) { console.error(e); }
    }
    await loadDevices();
    setSelected(new Set());
  };

  // Table columns
  const columns = [
    {
      key: "checkbox",
      header: "",
      width: "32px",
      render: (d: Device) => (
        <input
          type="checkbox"
          className="w-3.5 h-3.5"
          checked={selected.has(d.id)}
          onChange={() => toggleSelect(d.id)}
          onClick={(e) => e.stopPropagation()}
        />
      ),
    },
    { key: "name", header: "设备名称", width: "140px", render: (d: Device) => <span className="font-medium">{d.name}</span> },
    { key: "ip", header: "IP地址", width: "130px", render: (d: Device) => <code className="text-[11px]">{d.ip}</code> },
    { key: "vendor", header: "厂商", width: "80px", render: (d: Device) => d.vendor },
    { key: "model", header: "型号", width: "100px", render: (d: Device) => d.model || "-" },
    {
      key: "mode",
      header: "巡检方式",
      width: "70px",
      render: (d: Device) => MODES.find((m) => m.value === d.inspection_mode)?.label || d.inspection_mode,
    },
    { key: "status", header: "状态", width: "60px", render: (d: Device) => <StatusBadge status={d.status} /> },
    { key: "last", header: "最后检测", width: "140px", render: (d: Device) => <span className="text-gray-500">{formatTime(d.last_checked_at)}</span> },
    {
      key: "actions",
      header: "操作",
      width: "120px",
      render: (d: Device) => (
        <div className="flex gap-1">
          <button className="px-2 py-0.5 text-[11px] bg-blue-500 text-white rounded hover:bg-blue-600" onClick={() => handleInspect(d.id)}>
            巡检
          </button>
          <button className="px-2 py-0.5 text-[11px] border border-gray-300 rounded hover:bg-gray-100" onClick={() => openEdit(d)}>
            编辑
          </button>
        </div>
      ),
    },
  ];

  const deleteTitle = deleteTarget.batch ? "批量删除设备" : "删除设备";
  const deleteBody = deleteTarget.batch
    ? `确定要删除选中的 ${deleteTarget.batch.length} 个设备吗？此操作不可撤销。`
    : `确定要删除设备「${deleteTarget.single?.name}」吗？此操作不可撤销。`;

  if (loading) return <div className="p-4 text-gray-500 text-sm">加载中...</div>;

  return (
    <div className="flex gap-3 h-full">
      {/* Left: Group filter */}
      <div className="w-36 shrink-0 bg-white rounded border border-gray-200 p-2">
        <h3 className="text-xs font-semibold text-gray-500 mb-2 uppercase tracking-wide">设备分组</h3>
        <div className="space-y-0.5">
          {[{ key: "all", label: "全部", count: groupCounts.all }, ...GROUPS.map((g) => ({ key: g.value, label: g.label, count: groupCounts[g.value as keyof typeof groupCounts] }))].map(
            (item) => (
              <button
                key={item.key}
                className={`w-full text-left px-2 py-1 rounded text-xs flex justify-between items-center ${
                  groupFilter === item.key ? "bg-blue-100 text-blue-700 font-medium" : "hover:bg-gray-100 text-gray-700"
                }`}
                onClick={() => setGroupFilter(item.key)}
              >
                <span>{item.label}</span>
                <span className="text-[10px] bg-gray-200 px-1.5 py-0.5 rounded-full">{item.count}</span>
              </button>
            )
          )}
        </div>
      </div>

      {/* Right: Main content */}
      <div className="flex-1 min-w-0 flex flex-col gap-2">
        {/* Toolbar */}
        <div className="flex items-center justify-between">
          <Toolbar>
            <button className="px-3 py-1 text-xs bg-blue-500 text-white rounded hover:bg-blue-600" onClick={openAdd}>
              + 添加设备
            </button>
            <button className="px-3 py-1 text-xs border border-gray-300 rounded hover:bg-gray-100" onClick={handleRefreshAll}>
              刷新状态
            </button>
            {selected.size > 0 && (
              <>
                <button className="px-3 py-1 text-xs border border-green-300 text-green-700 rounded hover:bg-green-50" onClick={handleBatchInspect}>
                  批量巡检 ({selected.size})
                </button>
                <button className="px-3 py-1 text-xs border border-red-300 text-red-600 rounded hover:bg-red-50" onClick={handleBatchDelete}>
                  批量删除 ({selected.size})
                </button>
              </>
            )}
          </Toolbar>
          <SearchInput value={search} onChange={setSearch} placeholder="搜索设备名称或IP..." />
        </div>

        {/* DataTable */}
        <DataTable
          columns={columns}
          data={filtered}
          rowKey={(d) => d.id}
          onRowDoubleClick={(d) => openEdit(d)}
          onContextMenu={onContextMenu}
          emptyText="暂无设备，请点击「添加设备」按钮创建"
        />

        {/* Select-all bar */}
        {filtered.length > 0 && (
          <div className="flex items-center gap-2 text-xs text-gray-500">
            <input type="checkbox" className="w-3.5 h-3.5" checked={filtered.length > 0 && filtered.every((d) => selected.has(d.id))} onChange={toggleSelectAll} />
            <span>
              已选 {selected.size} / {filtered.length} 项
            </span>
          </div>
        )}
      </div>

      {/* Context Menu */}
      <ContextMenu items={ctxItems} visible={ctxVisible} x={ctxPos.x} y={ctxPos.y} onClose={() => setCtxVisible(false)} />

      {/* Add/Edit Modal */}
      <Modal
        open={editOpen}
        title={editingDevice ? "编辑设备" : "添加设备"}
        width="max-w-xl"
        onClose={() => setEditOpen(false)}
        footer={
          <>
            <button className="px-3 py-1 text-xs border border-gray-300 rounded hover:bg-gray-100" onClick={() => setEditOpen(false)}>
              取消
            </button>
            <button className="px-3 py-1 text-xs bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50" disabled={saving || !form.name.trim() || !form.ip.trim()} onClick={handleSave}>
              {saving ? "保存中..." : "保存"}
            </button>
          </>
        }
      >
        <div className="grid grid-cols-2 gap-3">
          <FormField label="设备名称 *">
            <input className="form-input" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="例如: core-switch-01" />
          </FormField>
          <FormField label="IP地址 *">
            <input className="form-input" value={form.ip} onChange={(e) => setForm({ ...form, ip: e.target.value })} placeholder="例如: 192.168.1.1" />
          </FormField>
          <FormField label="厂商">
            <select className="form-input" value={form.vendor} onChange={(e) => setForm({ ...form, vendor: e.target.value })}>
              {VENDORS.map((v) => (
                <option key={v} value={v}>{v}</option>
              ))}
            </select>
          </FormField>
          <FormField label="型号">
            <input className="form-input" value={form.model} onChange={(e) => setForm({ ...form, model: e.target.value })} placeholder="例如: S5130-52S-EI" />
          </FormField>
          <FormField label="分组">
            <select className="form-input" value={form.group} onChange={(e) => setForm({ ...form, group: e.target.value })}>
              {GROUPS.map((g) => (
                <option key={g.value} value={g.value}>{g.label}</option>
              ))}
            </select>
          </FormField>
          <FormField label="巡检方式">
            <select className="form-input" value={form.inspection_mode} onChange={(e) => setForm({ ...form, inspection_mode: e.target.value })}>
              {MODES.map((m) => (
                <option key={m.value} value={m.value}>{m.label}</option>
              ))}
            </select>
          </FormField>
          <FormField label="SSH用户名">
            <input className="form-input" value={form.ssh_username} onChange={(e) => setForm({ ...form, ssh_username: e.target.value })} placeholder="例如: admin" />
          </FormField>
          <FormField label="SSH密码">
            <input className="form-input" type="password" value={form.ssh_password} onChange={(e) => setForm({ ...form, ssh_password: e.target.value })} placeholder="留空则不修改" />
          </FormField>
          <FormField label="SSH端口">
            <input className="form-input" type="number" value={form.ssh_port} onChange={(e) => setForm({ ...form, ssh_port: parseInt(e.target.value) || 22 })} />
          </FormField>
          <FormField label="模板ID">
            <input className="form-input" type="number" value={form.template_id} onChange={(e) => setForm({ ...form, template_id: e.target.value })} placeholder="可选" />
          </FormField>
        </div>
      </Modal>

      {/* Delete Confirmation Modal */}
      <Modal
        open={deleteOpen}
        title={deleteTitle}
        onClose={() => { setDeleteOpen(false); setDeleteTarget({}); }}
        footer={
          <>
            <button className="px-3 py-1 text-xs border border-gray-300 rounded hover:bg-gray-100" onClick={() => { setDeleteOpen(false); setDeleteTarget({}); }}>
              取消
            </button>
            <button className="px-3 py-1 text-xs bg-red-500 text-white rounded hover:bg-red-600" onClick={handleDelete}>
              确认删除
            </button>
          </>
        }
      >
        <p className="text-sm text-gray-700">{deleteBody}</p>
      </Modal>

      {/* Form input style */}
      <style>{`
        .form-input {
          width: 100%;
          padding: 4px 8px;
          font-size: 12px;
          border: 1px solid #d1d5db;
          border-radius: 4px;
          outline: none;
          background: #fff;
        }
        .form-input:focus {
          border-color: #3b82f6;
          box-shadow: 0 0 0 1px #3b82f6;
        }
      `}</style>
    </div>
  );
}

function FormField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-xs text-gray-600">{label}</span>
      {children}
    </label>
  );
}
