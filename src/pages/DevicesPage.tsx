import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Device, InspectionTemplate } from "../types";
import Toolbar from "../components/Toolbar";
import SearchInput from "../components/SearchInput";
import DataTable from "../components/DataTable";
import Modal from "../components/Modal";
import ContextMenu, { type ContextMenuItem } from "../components/ContextMenu";
import Button from "../components/ui/Button";
import Input, { Select } from "../components/ui/Input";
import StatusBadge from "../components/StatusBadge";
import { VENDORS } from "../lib/constants";

interface DeviceForm {
  name: string;
  ip: string;
  vendor: string;
  model: string;
  ssh_username: string;
  ssh_password: string;
  ssh_port: number;
  template_id: number | null;
}

const EMPTY_FORM: DeviceForm = {
  name: "", ip: "", vendor: "H3C", model: "",
  ssh_username: "", ssh_password: "", ssh_port: 22, template_id: null,
};

export default function DevicesPage() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [templates, setTemplates] = useState<InspectionTemplate[]>([]);
  const [searchText, setSearchText] = useState("");
  const [vendorFilter, setVendorFilter] = useState("");
  const [statusFilter, setStatusFilter] = useState("");
  const [selectedDevice, setSelectedDevice] = useState<Device | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<Device | null>(null);
  const [form, setForm] = useState<DeviceForm>(EMPTY_FORM);
  const [deleteConfirm, setDeleteConfirm] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const loadDevices = useCallback(() => {
    invoke<Device[]>("list_devices", {
      vendor: vendorFilter || undefined,
      status: statusFilter || undefined,
    }).then(setDevices).catch(console.error);
  }, [vendorFilter, statusFilter]);

  const loadTemplates = useCallback(() => {
    invoke<InspectionTemplate[]>("list_templates", { vendor: undefined })
      .then(setTemplates).catch(console.error);
  }, []);

  useEffect(() => { loadDevices(); }, [loadDevices]);
  useEffect(() => { loadTemplates(); }, [loadTemplates]);

  const filteredDevices = useMemo(() => devices.filter((d) =>
    !searchText || d.name.toLowerCase().includes(searchText.toLowerCase()) || d.ip.includes(searchText)
  ), [devices, searchText]);

  const openAdd = () => {
    setEditing(null);
    setForm(EMPTY_FORM);
    setModalOpen(true);
  };

  const openEdit = (d: Device) => {
    setEditing(d);
    setForm({
      name: d.name,
      ip: d.ip,
      vendor: d.vendor,
      model: d.model || "",
      ssh_username: d.ssh_username || "",
      ssh_password: "",
      ssh_port: d.ssh_port,
      template_id: d.template_id,
    });
    setModalOpen(true);
  };

  const handleSave = () => {
    if (!form.name.trim()) {
      setSaveError("请输入设备名称");
      return;
    }
    if (!form.ip.trim()) {
      setSaveError("请输入 IP 地址");
      return;
    }

    const data: Record<string, unknown> = {
      name: form.name,
      ip: form.ip,
      device_type: form.model ? "switch" : "router",
      vendor: form.vendor,
      ssh_port: form.ssh_port,
    };
    if (form.model) data.model = form.model;
    if (form.ssh_username) data.ssh_username = form.ssh_username;
    if (form.ssh_password) data.ssh_password_encrypted = form.ssh_password;
    if (form.template_id !== null) data.template_id = form.template_id;

    setSaving(true);
    setSaveError(null);

    const promise = editing
      ? invoke<Device>("update_device", { deviceId: editing.id, data })
      : invoke<Device>("create_device", { data });

    promise
      .then(() => {
        setModalOpen(false);
        loadDevices();
      })
      .catch((e) => {
        setSaveError(typeof e === "string" ? e : JSON.stringify(e));
      })
      .finally(() => setSaving(false));
  };

  const handleDelete = (id: number) => {
    invoke<void>("delete_device", { deviceId: id })
      .then(() => {
        setDeleteConfirm(null);
        setSelectedIds((prev) => {
          const n = new Set(prev);
          n.delete(id);
          return n;
        });
        loadDevices();
      })
      .catch((e) => {
        console.error("删除设备失败:", e);
        alert(`删除设备失败: ${typeof e === "string" ? e : JSON.stringify(e)}`);
      });
  };

  const handleBatchDelete = () => {
    if (selectedIds.size === 0) return;
    const ids = Array.from(selectedIds);
    invoke<void>("batch_delete_devices", { ids })
      .then(() => { setSelectedIds(new Set()); loadDevices(); })
      .catch(console.error);
  };

  const handleCheckDevice = (d: Device) => {
    invoke<{ status: string }>("check_device_status", { deviceId: d.id })
      .then(() => {
        loadDevices();
      })
      .catch(console.error);
  };

  const handleCheckAll = () => {
    invoke<{ total: number; online: number; offline: number }>("check_all_devices_status")
      .then(() => loadDevices())
      .catch(console.error);
  };

  const toggleSelect = (id: number) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };

  const handleContextMenu = (e: React.MouseEvent, d: Device) => {
    e.preventDefault();
    setSelectedDevice(d);
    setContextMenu({ x: e.clientX, y: e.clientY });
  };

  const ctxItems: ContextMenuItem[] = selectedDevice
    ? [
        { label: "编辑", action: () => openEdit(selectedDevice) },
        { label: "", separator: true },
        { label: "检测状态", action: () => handleCheckDevice(selectedDevice) },
        { label: "", separator: true },
        { label: "删除", danger: true, action: () => setDeleteConfirm(selectedDevice.id) },
      ]
    : [];

  return (
    <div className="space-y-6">
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm relative">
        <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">设备管理</h1>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1">管理网络设备信息</p>
      </div>

      <Toolbar>
        <Button onClick={openAdd} size="sm">添加设备</Button>
        <Select
          className="w-28"
          value={vendorFilter}
          onChange={(e) => setVendorFilter(e.target.value)}
        >
          <option value="">全部厂商</option>
          {VENDORS.map((v) => <option key={v} value={v}>{v}</option>)}
        </Select>
        <Select
          className="w-28"
          value={statusFilter}
          onChange={(e) => setStatusFilter(e.target.value)}
        >
          <option value="">全部状态</option>
          <option value="online">在线</option>
          <option value="offline">离线</option>
          <option value="unknown">未知</option>
        </Select>
        <SearchInput value={searchText} onChange={setSearchText} placeholder="搜索设备名称/IP..." />
        <Button size="sm" variant="secondary" onClick={handleCheckAll}>检测全部</Button>
        {selectedIds.size > 0 && (
          <Button variant="danger" size="sm" onClick={handleBatchDelete}>
            批量删除 ({selectedIds.size})
          </Button>
        )}
      </Toolbar>

      <DataTable<Device>
        columns={[
          {
            key: "checkbox", header: "", width: "36px", render: (r) => (
              <input
                type="checkbox"
                checked={selectedIds.has(r.id)}
                onChange={() => toggleSelect(r.id)}
                onClick={(e) => e.stopPropagation()}
                className="accent-[hsl(var(--accent))]"
              />
            ),
          },
          { key: "name", header: "名称", render: (r) => r.name },
          { key: "ip", header: "IP", render: (r) => r.ip },
          { key: "vendor", header: "厂商", render: (r) => r.vendor },
          { key: "model", header: "型号", render: (r) => r.model || "-" },
          { key: "status", header: "状态", render: (r) => <StatusBadge status={r.status} /> },
          {
            key: "template_id", header: "关联模板", render: (r) => {
              const t = templates.find((t) => t.id === r.template_id);
              return t ? t.name : "-";
            },
          },
          {
            key: "last_checked_at", header: "最后检测时间", render: (r) =>
              r.last_checked_at ? new Date(r.last_checked_at).toLocaleString("zh-CN") : "-",
          },
          {
            key: "actions",
            header: "操作",
            width: "200px",
            render: (r) => (
              <div className="flex gap-1" onClick={(e) => e.stopPropagation()}>
                <Button size="sm" variant="ghost" onClick={() => handleCheckDevice(r)}>
                  检测
                </Button>
                <Button size="sm" variant="ghost" onClick={() => openEdit(r)}>
                  编辑
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => setDeleteConfirm(r.id)}
                >
                  删除
                </Button>
              </div>
            ),
          },
        ]}
        data={filteredDevices}
        rowKey={(r) => r.id}
        onRowClick={(r) => setSelectedDevice(r)}
        onRowDoubleClick={(r) => openEdit(r)}
        onContextMenu={handleContextMenu}
      />

      <ContextMenu
        items={ctxItems}
        visible={contextMenu !== null}
        x={contextMenu?.x ?? 0}
        y={contextMenu?.y ?? 0}
        onClose={() => setContextMenu(null)}
      />

      <Modal
        open={modalOpen}
        title={editing ? "编辑设备" : "添加设备"}
        width="max-w-md"
        onClose={() => setModalOpen(false)}
        footer={
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => setModalOpen(false)}>取消</Button>
            <Button onClick={handleSave} loading={saving}>{editing ? "保存" : "添加"}</Button>
          </div>
        }
      >
        <div className="space-y-3">
          {saveError && (
            <div className="p-2 bg-[hsl(var(--danger)_/_0.1)] border border-[hsl(var(--danger)_/_0.3)] rounded text-sm text-[hsl(var(--danger))]">
              {saveError}
            </div>
          )}
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">名称</label>
              <Input value={form.name} onChange={(e) => { setForm({ ...form, name: e.target.value }); setSaveError(null); }} placeholder="设备名称" />
            </div>
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">IP</label>
              <Input value={form.ip} onChange={(e) => { setForm({ ...form, ip: e.target.value }); setSaveError(null); }} placeholder="192.168.1.1" />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">厂商</label>
              <Select value={form.vendor} onChange={(e) => setForm({ ...form, vendor: e.target.value })}>
                {VENDORS.map((v) => <option key={v} value={v}>{v}</option>)}
              </Select>
            </div>
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">型号</label>
              <Input value={form.model} onChange={(e) => setForm({ ...form, model: e.target.value })} placeholder="S5560X" />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">SSH 用户名</label>
              <Input value={form.ssh_username} onChange={(e) => setForm({ ...form, ssh_username: e.target.value })} placeholder="admin" />
            </div>
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">SSH 密码</label>
              <Input type="password" value={form.ssh_password} onChange={(e) => setForm({ ...form, ssh_password: e.target.value })} placeholder={editing ? "留空则不修改" : "输入密码"} />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">SSH 端口</label>
              <Input type="number" value={form.ssh_port} onChange={(e) => setForm({ ...form, ssh_port: Number(e.target.value) })} />
            </div>
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">关联模板</label>
              <Select value={form.template_id ?? ""} onChange={(e) => setForm({ ...form, template_id: e.target.value ? Number(e.target.value) : null })}>
                <option value="">无模板</option>
                {templates.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
              </Select>
            </div>
          </div>
        </div>
      </Modal>

      <Modal
        open={deleteConfirm !== null}
        title="确认删除"
        width="max-w-sm"
        onClose={() => setDeleteConfirm(null)}
        footer={
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => setDeleteConfirm(null)}>取消</Button>
            <Button variant="danger" onClick={() => handleDelete(deleteConfirm!)}>删除</Button>
          </div>
        }
      >
        <p>确定要删除此设备吗？此操作不可恢复。</p>
      </Modal>
    </div>
  );
}
