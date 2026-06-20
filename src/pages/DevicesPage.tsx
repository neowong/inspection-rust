import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Search } from "lucide-react";
import type { Device, InspectionTemplate } from "../types";
import { useShakeValidation } from "../hooks/useShakeValidation";
import { friendlyError } from "../lib/utils";
import Toolbar from "../components/Toolbar";
import SearchInput from "../components/SearchInput";
import DataTable from "../components/DataTable";
import Modal from "../components/Modal";
import ContextMenu, { type ContextMenuItem } from "../components/ContextMenu";
import Button from "../components/ui/Button";
import Input, { Select } from "../components/ui/Input";
import StatusBadge from "../components/StatusBadge";
import { VENDORS } from "../lib/constants";

const NETWORK_VENDORS = ["H3C", "华为", "思科", "锐捷", "飞塔", "其它"];
const SERVER_VENDORS = ["Linux", "Ubuntu", "CentOS", "Rocky", "Debian", "其它"];

interface DeviceForm {
  name: string;
  ip: string;
  device_type: string;
  vendor: string;
  model: string;
  ssh_username: string;
  ssh_password: string;
  ssh_port: number;
  template_id: number | null;
  serial_number: string;
  manufacturing_date: string;
  sysname: string;
  cpu_cores: string;
  memory_gb: string;
}

const EMPTY_FORM: DeviceForm = {
  name: "", ip: "", device_type: "router", vendor: "H3C", model: "",
  ssh_username: "", ssh_password: "", ssh_port: 22, template_id: null,
  serial_number: "", manufacturing_date: "", sysname: "", cpu_cores: "", memory_gb: "",
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
  const [detecting, setDetecting] = useState(false);
  const [detectError, setDetectError] = useState<string | null>(null);
  const { shakeFields, triggerShake } = useShakeValidation();

  const canDetect = !!(form.ip.trim() && form.ssh_username.trim() && form.ssh_password.trim() && (form.vendor === "H3C" || form.vendor === "华三"));

  const isValidIp = (ip: string) => {
    const p = ip.trim();
    if (!p) return false;
    const parts = p.split(".");
    if (parts.length !== 4) return false;
    return parts.every((part) => {
      const n = Number(part);
      return part !== "" && !isNaN(n) && n >= 0 && n <= 255 && part === String(n);
    });
  };

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
    setSaveError(null);
    setDetectError(null);
    setModalOpen(true);
  };

  const openEdit = (d: Device) => {
    setEditing(d);
    setDetectError(null);
    setForm({
      name: d.name,
      ip: d.ip,
      device_type: d.device_type || "router",
      vendor: d.vendor,
      model: d.model || "",
      ssh_username: d.ssh_username || "",
      ssh_password: "",
      ssh_port: d.ssh_port,
      template_id: d.template_id,
      serial_number: d.serial_number || "",
      manufacturing_date: d.manufacturing_date || "",
      sysname: d.sysname || "",
      cpu_cores: d.cpu_cores != null ? String(d.cpu_cores) : "",
      memory_gb: d.memory_gb != null ? String(d.memory_gb) : "",
    });
    setModalOpen(true);
  };

  const handleDetect = () => {
    setDetecting(true);
    setDetectError(null);
    invoke<string>("detect_device_model", {
      ip: form.ip.trim(),
      sshPort: form.ssh_port,
      sshUsername: form.ssh_username.trim(),
      sshPassword: form.ssh_password,
      vendor: form.vendor,
    })
      .then((json) => {
        try {
          const info = JSON.parse(json);
          setForm({
            ...form,
            model: info.model || form.model,
            serial_number: info.serial_number || form.serial_number,
            manufacturing_date: info.manufacturing_date || form.manufacturing_date,
            sysname: info.sysname || form.sysname,
          });
        } catch {
          // 兼容旧格式（纯字符串）
          setForm({ ...form, model: json });
        }
      })
      .catch((e) => setDetectError(typeof e === "string" ? e : JSON.stringify(e)))
      .finally(() => setDetecting(false));
  };

  const handleSave = () => {
    if (!form.name.trim()) { triggerShake("name"); return; }
    if (!isValidIp(form.ip)) { triggerShake("ip"); return; }
    if (!form.ssh_username.trim()) { triggerShake("ssh_username"); return; }
    if (!editing && !form.ssh_password.trim()) { triggerShake("ssh_password"); return; }
    if (form.template_id === null) { triggerShake("template_id"); return; }

    const data: Record<string, unknown> = {
      name: form.name,
      ip: form.ip,
      device_type: form.device_type,
      vendor: form.vendor,
      ssh_port: form.ssh_port,
    };
    if (form.model) data.model = form.model;
    if (form.ssh_username) data.ssh_username = form.ssh_username;
    if (form.ssh_password) data.ssh_password_encrypted = form.ssh_password;
    if (form.template_id !== null) data.template_id = form.template_id;
    if (form.serial_number) data.serial_number = form.serial_number;
    if (form.manufacturing_date) data.manufacturing_date = form.manufacturing_date;
    if (form.sysname) data.sysname = form.sysname;
    if (form.cpu_cores) data.cpu_cores = Number(form.cpu_cores);
    if (form.memory_gb) data.memory_gb = Number(form.memory_gb);

    setSaving(true);
    setSaveError(null);

    const promise = editing
      ? invoke<Device>("update_device", { deviceId: editing.id, data })
      : invoke<Device>("create_device", { data });

    promise
      .then((saved) => {
        setModalOpen(false);
        loadDevices();
        if (!saved?.id) return;
        const devId = saved.id;

        // 静默检测在线状态
        invoke("check_device_status", { deviceId: devId }).then(() => loadDevices()).catch(() => {});

        // 静默补全型号/SN/出厂日期/sysname（H3C 且有 SSH 凭据时）
        const needModel = !form.model.trim();
        const needSn = !form.serial_number.trim();
        const needDate = !form.manufacturing_date.trim();
        const needSysname = !form.sysname.trim();
        const canDetect = form.ssh_username.trim() && form.ssh_password.trim()
          && (form.vendor === "H3C" || form.vendor === "华三");
        if ((needModel || needSn || needDate || needSysname) && canDetect) {
          invoke<string>("detect_device_model", {
            ip: form.ip.trim(), sshPort: form.ssh_port,
            sshUsername: form.ssh_username.trim(), sshPassword: form.ssh_password,
            vendor: form.vendor,
          }).then((json) => {
            try {
              const info = JSON.parse(json);
              const patch: Record<string, unknown> = {};
              if (needModel && info.model) patch.model = info.model;
              if (needSn && info.serial_number) patch.serial_number = info.serial_number;
              if (needDate && info.manufacturing_date) patch.manufacturing_date = info.manufacturing_date;
              if (needSysname && info.sysname) patch.sysname = info.sysname;
              if (Object.keys(patch).length > 0) {
                invoke("update_device", { deviceId: devId, data: patch }).then(() => loadDevices());
              }
            } catch { /* 忽略 */ }
          }).catch(() => {});
        }
      })
      .catch((e) => {
        const msg = friendlyError(e);
        setSaveError(msg);
        if (msg.includes("IP")) triggerShake("ip");
        else triggerShake("name");
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
    if (!confirm(`确定删除选中的 ${selectedIds.size} 台设备？此操作不可撤销。`)) return;
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
        width="max-w-lg"
        onClose={() => setModalOpen(false)}
        footer={
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => setModalOpen(false)}>取消</Button>
            <Button onClick={handleSave} loading={saving}>{editing ? "保存" : "添加"}</Button>
          </div>
        }
      >
        <div className="space-y-3">
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">设备类型</label>
                  <Select value={form.device_type} onChange={(e) => {
                    const dt = e.target.value;
                    const updated: Partial<DeviceForm> = { device_type: dt };
                    if (dt === "server" && !SERVER_VENDORS.includes(form.vendor)) {
                      updated.vendor = "Linux";
                    } else if (dt !== "server" && !NETWORK_VENDORS.includes(form.vendor)) {
                      updated.vendor = "H3C";
                    }
                    setForm({ ...form, ...updated });
                  }}>
                    <option value="switch">交换机</option>
                    <option value="router">路由器</option>
                    <option value="firewall">防火墙</option>
                    <option value="loadbalancer">负载均衡</option>
                    <option value="server">服务器</option>
                  </Select>
                </div>
                <div>
                  <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">厂商</label>
                  <Select value={form.vendor} onChange={(e) => setForm({ ...form, vendor: e.target.value })}>
                    {(form.device_type === "server" ? SERVER_VENDORS : NETWORK_VENDORS).map((v) => <option key={v} value={v}>{v}</option>)}
                  </Select>
                </div>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">名称</label>
                  <Input value={form.name} className={shakeFields.has("name") ? "animate-shake" : ""} onChange={(e) => { setForm({ ...form, name: e.target.value }); setSaveError(null); }} placeholder="设备名称" />
                  {saveError && shakeFields.has("name") && <p className="mt-1 text-xs text-[hsl(var(--danger))]">{saveError}</p>}
                </div>
                <div>
                  <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">IP <span className="text-[hsl(var(--danger))]">*</span></label>
                  <Input value={form.ip} className={shakeFields.has("ip") ? "animate-shake" : ""} onChange={(e) => { setForm({ ...form, ip: e.target.value }); setSaveError(null); }} placeholder="192.168.1.1" />
                  {saveError && shakeFields.has("ip") && <p className="mt-1 text-xs text-[hsl(var(--danger))]">{saveError}</p>}
                </div>
              </div>
              <div>
                <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">{form.device_type === "server" ? "发行版本号" : "型号"}</label>
                <div className="flex gap-1">
                  <Input
                    value={form.model}
                    onChange={(e) => { setForm({ ...form, model: e.target.value }); setDetectError(null); }}
                    placeholder="自动检测"
                    className={shakeFields.has("model") ? "animate-shake" : ""}
                  />
                  {canDetect && (
                    <button
                      type="button"
                      onClick={handleDetect}
                      disabled={detecting}
                      title="自动检测型号"
                      className="flex-shrink-0 w-8 h-8 flex items-center justify-center rounded border border-[hsl(var(--border))] text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--accent))] hover:border-[hsl(var(--accent))] disabled:opacity-50 transition-colors"
                    >
                      <Search className={`w-4 h-4 ${detecting ? "animate-spin" : ""}`} />
                    </button>
                  )}
                </div>
                {detectError && <p className="mt-1 text-xs text-[hsl(var(--danger))]">{detectError}</p>}
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">SSH 用户名 <span className="text-[hsl(var(--danger))]">*</span></label>
                  <Input value={form.ssh_username} className={shakeFields.has("ssh_username") ? "animate-shake" : ""} onChange={(e) => setForm({ ...form, ssh_username: e.target.value })} placeholder="admin" />
                </div>
                <div>
                  <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">SSH 密码 <span className="text-[hsl(var(--danger))]">*</span></label>
                  <Input type="password" value={form.ssh_password} className={shakeFields.has("ssh_password") ? "animate-shake" : ""} onChange={(e) => setForm({ ...form, ssh_password: e.target.value })} placeholder={editing ? "留空则不修改" : "输入密码"} />
                </div>
              </div>
              <div>
                <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">SSH 端口</label>
                <Input type="number" value={form.ssh_port} onChange={(e) => setForm({ ...form, ssh_port: Number(e.target.value) || 22 })} />
              </div>
              {form.device_type === "server" && (
                <>
                  <div>
                    <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">主机名</label>
                    <Input value={form.sysname} onChange={(e) => setForm({ ...form, sysname: e.target.value })} placeholder="自动检测" />
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">CPU 核心数</label>
                      <Input value={form.cpu_cores} placeholder="自动检测" readOnly />
                    </div>
                    <div>
                      <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">内存</label>
                      <Input value={form.memory_gb} placeholder="自动检测" readOnly />
                    </div>
                  </div>
                </>
              )}
              {form.device_type !== "server" && (
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">SN</label>
                    <Input value={form.serial_number} onChange={(e) => setForm({ ...form, serial_number: e.target.value })} placeholder="自动检测" />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">出厂日期</label>
                    <Input value={form.manufacturing_date} onChange={(e) => setForm({ ...form, manufacturing_date: e.target.value })} placeholder="自动检测" />
                  </div>
                </div>
              )}
              <div>
                <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">关联模板 <span className="text-[hsl(var(--danger))]">*</span></label>
                <Select
                  value={form.template_id ?? ""}
                  className={shakeFields.has("template_id") ? "animate-shake" : ""}
                  onChange={(e) => { setForm({ ...form, template_id: e.target.value ? Number(e.target.value) : null }); }}
                >
                  <option value="">请选择模板</option>
                  {templates.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
                </Select>
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
