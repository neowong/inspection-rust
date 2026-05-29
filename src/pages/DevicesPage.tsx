import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import Toolbar from "../components/Toolbar";
import SearchInput from "../components/SearchInput";
import DataTable from "../components/DataTable";
import StatusBadge from "../components/StatusBadge";
import Modal from "../components/Modal";
import ContextMenu, { ContextMenuItem } from "../components/ContextMenu";
import type { Device } from "../types";

const VENDORS = ["H3C", "Huawei", "Cisco", "Ruijie"];

interface DeviceForm {
  name: string;
  ip: string;
  vendor: string;
  model: string;
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
  const [selected, setSelected] = useState<Set<number>>(new Set());

  const [ctxVisible, setCtxVisible] = useState(false);
  const [ctxPos, setCtxPos] = useState({ x: 0, y: 0 });
  const [ctxDevice, setCtxDevice] = useState<Device | null>(null);

  const [editOpen, setEditOpen] = useState(false);
  const [editingDevice, setEditingDevice] = useState<Device | null>(null);
  const [form, setForm] = useState<DeviceForm>(EMPTY_FORM);
  const [saving, setSaving] = useState(false);

  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<{ single?: Device; batch?: number[] }>({});

  const loadDevices = useCallback(async () => {
    try {
      setDevices(await invoke<Device[]>("list_devices"));
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadDevices(); }, [loadDevices]);

  const filtered = useMemo(() => {
    if (!search.trim()) return devices;
    const kw = search.trim().toLowerCase();
    return devices.filter((d) => d.name.toLowerCase().includes(kw) || d.ip.toLowerCase().includes(kw));
  }, [devices, search]);

  const toggleSelect = (id: number) => {
    setSelected((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });
  };

  const toggleSelectAll = () => {
    const ids = new Set(filtered.map((d) => d.id));
    setSelected(ids.size > 0 && [...ids].every((id) => selected.has(id)) ? new Set() : ids);
  };

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

  const handleInspect = async (id: number) => {
    try { await invoke("check_device_status", { id }); await loadDevices(); } catch (e) { console.error(e); }
  };

  const handleRefreshAll = async () => {
    try { await invoke("check_all_devices_status"); await loadDevices(); } catch (e) { console.error(e); }
  };

  const setFormFromDevice = (device: Device, copy?: boolean) => {
    setForm({
      name: copy ? device.name + " (副本)" : device.name,
      ip: device.ip,
      vendor: device.vendor,
      model: device.model || "",
      ssh_username: device.ssh_username || "",
      ssh_password: "",
      ssh_port: device.ssh_port,
      template_id: device.template_id?.toString() || "",
    });
  };

  const openEdit = (device: Device) => {
    setEditingDevice(device);
    setFormFromDevice(device);
    setEditOpen(true);
  };

  const openAdd = () => {
    setEditingDevice(null);
    setForm(EMPTY_FORM);
    setEditOpen(true);
  };

  const handleCopy = (device: Device) => {
    setEditingDevice(null);
    setFormFromDevice(device, true);
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
      if (deleteTarget.batch?.length) {
        await invoke("batch_delete_devices", { ids: deleteTarget.batch });
      } else if (deleteTarget.single) {
        await invoke("delete_device", { id: deleteTarget.single.id });
      }
      setDeleteOpen(false);
      setDeleteTarget({});
      setSelected(new Set());
      await loadDevices();
    } catch (e) { console.error(e); }
  };

  const columns = [
    {
      key: "checkbox", header: "", width: "32px",
      render: (d: Device) => (
        <input type="checkbox" className="w-3.5 h-3.5" checked={selected.has(d.id)}
          onChange={() => toggleSelect(d.id)} onClick={(e) => e.stopPropagation()} />
      ),
    },
    { key: "name", header: "设备名称", width: "160px", render: (d: Device) => <span className="font-medium">{d.name}</span> },
    { key: "ip", header: "IP地址", width: "130px", render: (d: Device) => <code className="text-[11px]">{d.ip}</code> },
    { key: "vendor", header: "厂商", width: "70px", render: (d: Device) => d.vendor },
    { key: "model", header: "型号", width: "100px", render: (d: Device) => d.model || "-" },
    { key: "status", header: "状态", width: "60px", render: (d: Device) => <StatusBadge status={d.status} /> },
    { key: "last", header: "最后检测", width: "140px", render: (d: Device) => <span className="text-gray-500">{formatTime(d.last_checked_at)}</span> },
    {
      key: "actions", header: "操作", width: "120px",
      render: (d: Device) => (
        <div className="flex gap-1">
          <button className="px-2 py-0.5 text-[11px] bg-blue-500 text-white rounded hover:bg-blue-600" onClick={() => handleInspect(d.id)}>巡检</button>
          <button className="px-2 py-0.5 text-[11px] border border-gray-300 rounded hover:bg-gray-100" onClick={() => openEdit(d)}>编辑</button>
        </div>
      ),
    },
  ];

  const deleteTitle = deleteTarget.batch ? "批量删除设备" : "删除设备";
  const deleteBody = deleteTarget.batch
    ? `确定要删除选中的 ${deleteTarget.batch.length} 个设备吗？此操作不可撤销。`
    : `确定要删除设备「${deleteTarget.single?.name}」吗？此操作不可撤销。`;

  if (loading) return <div className="flex items-center justify-center h-64 text-gray-400 text-sm">加载中...</div>;

  return (
    <div className="flex flex-col h-full">
      <div className="page-header">
        <div>
          <h1 className="page-title">设备管理</h1>
          <p className="page-desc">管理网络设备的连接信息和巡检配置</p>
        </div>
        <div className="flex items-center gap-2">
          <button className="btn btn-primary" onClick={openAdd}>+ 添加设备</button>
          <button className="btn btn-outline" onClick={handleRefreshAll}>刷新状态</button>
        </div>
      </div>

      <div className="card flex-1 flex flex-col min-h-0">
        <div className="flex items-center justify-between px-4 py-2.5 border-b border-gray-100">
          <div className="flex items-center gap-3">
            <SearchInput value={search} onChange={setSearch} placeholder="搜索设备名称或IP..." />
            {selected.size > 0 && (
              <span className="text-xs text-gray-500">{selected.size} 项已选</span>
            )}
          </div>
          {selected.size > 0 && (
            <button className="btn btn-danger btn-sm" onClick={() => { setDeleteTarget({ batch: [...selected] }); setDeleteOpen(true); }}>
              删除选中
            </button>
          )}
        </div>
        <div className="flex-1 overflow-auto">
          <DataTable columns={columns} data={filtered} rowKey={(d) => d.id}
            onRowDoubleClick={(d) => openEdit(d)} onContextMenu={onContextMenu}
            emptyText="暂无设备" />
        </div>
        {filtered.length > 0 && (
          <div className="flex items-center gap-2 px-4 py-1.5 text-xs text-gray-400 border-t border-gray-100">
            <input type="checkbox" className="w-3.5 h-3.5"
              checked={filtered.length > 0 && filtered.every((d) => selected.has(d.id))}
              onChange={toggleSelectAll} />
            <span>{selected.size} / {filtered.length} 项</span>
          </div>
        )}
      </div>

      <ContextMenu items={ctxItems} visible={ctxVisible} x={ctxPos.x} y={ctxPos.y} onClose={() => setCtxVisible(false)} />

      <Modal open={editOpen} title={editingDevice ? "编辑设备" : "添加设备"} width="max-w-xl"
        onClose={() => setEditOpen(false)}
        footer={
          <>
            <button className="btn btn-outline btn-sm" onClick={() => setEditOpen(false)}>取消</button>
            <button className="btn btn-primary btn-sm" disabled={saving || !form.name.trim() || !form.ip.trim()} onClick={handleSave}>
              {saving ? "保存中..." : "保存"}
            </button>
          </>
        }
      >
        <div className="grid grid-cols-2 gap-3">
          <FormField label="设备名称 *">
            <input className="input" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="例如: core-switch-01" />
          </FormField>
          <FormField label="IP地址 *">
            <input className="input" value={form.ip} onChange={(e) => setForm({ ...form, ip: e.target.value })} placeholder="例如: 192.168.1.1" />
          </FormField>
          <FormField label="厂商">
            <select className="select" value={form.vendor} onChange={(e) => setForm({ ...form, vendor: e.target.value })}>
              {VENDORS.map((v) => (<option key={v} value={v}>{v}</option>))}
            </select>
          </FormField>
          <FormField label="型号">
            <input className="input" value={form.model} onChange={(e) => setForm({ ...form, model: e.target.value })} placeholder="例如: S5130-52S-EI" />
          </FormField>
          <FormField label="SSH用户名">
            <input className="input" value={form.ssh_username} onChange={(e) => setForm({ ...form, ssh_username: e.target.value })} placeholder="例如: admin" />
          </FormField>
          <FormField label="SSH密码">
            <input className="input" type="password" value={form.ssh_password} onChange={(e) => setForm({ ...form, ssh_password: e.target.value })} placeholder="留空则不修改" />
          </FormField>
          <FormField label="SSH端口">
            <input className="input" type="number" value={form.ssh_port} onChange={(e) => setForm({ ...form, ssh_port: parseInt(e.target.value) || 22 })} />
          </FormField>
          <FormField label="模板ID">
            <input className="input" type="number" value={form.template_id} onChange={(e) => setForm({ ...form, template_id: e.target.value })} placeholder="可选" />
          </FormField>
        </div>
      </Modal>

      <Modal open={deleteOpen} title={deleteTitle}
        onClose={() => { setDeleteOpen(false); setDeleteTarget({}); }}
        footer={
          <>
            <button className="btn btn-outline btn-sm" onClick={() => { setDeleteOpen(false); setDeleteTarget({}); }}>取消</button>
            <button className="btn btn-danger btn-sm" onClick={handleDelete}>确认删除</button>
          </>
        }
      >
        <p className="text-sm text-gray-700">{deleteBody}</p>
      </Modal>
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
