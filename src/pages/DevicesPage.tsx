import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Device, InspectionTemplate } from "../types";
import { useShakeValidation } from "../hooks/useShakeValidation";
import { friendlyError, showStatusHint } from "../lib/utils";
import Toolbar from "../components/Toolbar";
import SearchInput from "../components/SearchInput";
import DataTable from "../components/DataTable";
import Modal from "../components/Modal";
import ContextMenu, { type ContextMenuItem } from "../components/ContextMenu";
import Button from "../components/ui/Button";
import Input, { Select } from "../components/ui/Input";
import StatusBadge from "../components/StatusBadge";
import AuthBadge from "../components/AuthBadge";

const NETWORK_VENDORS = ["H3C", "华为", "思科", "锐捷", "飞塔", "其它"];
const SERVER_VENDORS = ["Linux", "Ubuntu", "CentOS", "Rocky", "Debian", "其它"];
const DB_VENDORS = ["MySQL", "PostgreSQL", "Oracle", "SQL Server", "Redis", "MongoDB", "其它"];

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
  deployment: string;
  db_version: string;
  instance_name: string;
  db_username: string;
  db_password: string;
}

const EMPTY_FORM: DeviceForm = {
  name: "", ip: "", device_type: "router", vendor: "H3C", model: "",
  ssh_username: "", ssh_password: "", ssh_port: 22, template_id: null,
  serial_number: "", manufacturing_date: "", sysname: "", cpu_cores: "", memory_gb: "",
  deployment: "", db_version: "", instance_name: "", db_username: "", db_password: "",
};

export default function DevicesPage() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [templates, setTemplates] = useState<InspectionTemplate[]>([]);
  const [searchText, setSearchText] = useState("");
  const [typeFilter, setTypeFilter] = useState("");
  const [statusFilter, setStatusFilter] = useState("");
  const [selectedDevice, setSelectedDevice] = useState<Device | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<Device | null>(null);
  const [form, setForm] = useState<DeviceForm>(EMPTY_FORM);
  const [deleteConfirm, setDeleteConfirm] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);
  const [passwordSet, setPasswordSet] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  /** 正在检测的设备 ID 集合，用于按钮 loading/disable 状态 */
  const [checkingIds, setCheckingIds] = useState<Set<number>>(new Set());
  const [checkingAll, setCheckingAll] = useState(false);
  const { shakeFields, triggerShake } = useShakeValidation();

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

  /** 将检测错误浓缩为简短提示 */
  const detectErrorHint = (deviceName: string, e: unknown): string => {
    const raw = typeof e === "string" ? e : JSON.stringify(e);
    if (/auth|password|认证|密码|登录失败/i.test(raw)) return `${deviceName}: SSH 认证失败，请检查用户名/密码`;
    if (/timeout|超时|timed out/i.test(raw)) return `${deviceName}: SSH 连接超时`;
    if (/refused|拒绝/i.test(raw)) return `${deviceName}: SSH 连接被拒绝`;
    if (/未保存 SSH 密码/.test(raw)) return `${deviceName}: 未保存 SSH 密码，请先编辑设备`;
    if (/SSH 端口非法/.test(raw)) return `${deviceName}: SSH 端口配置错误`;
    if (/不存在/.test(raw)) return `${deviceName}: 设备不存在`;
    // 其他错误：截断到 60 字符
    const trimmed = raw.length > 60 ? raw.slice(0, 60) + "…" : raw;
    return `${deviceName}: ${trimmed}`;
  };

  const loadDevices = useCallback(() => {
    // Tauri v2 默认参数命名为 camelCase，对应 Rust 的 snake_case
    invoke<Device[]>("list_devices", {
      deviceType: typeFilter || undefined,
      status: statusFilter || undefined,
    }).then(setDevices).catch(console.error);
  }, [typeFilter, statusFilter]);

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
    setPasswordSet(false);
    setForm(EMPTY_FORM);
    setSaveError(null);
    setModalOpen(true);
  };

  const openEdit = (d: Device) => {
    setEditing(d);
    setPasswordSet(true);
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
      deployment: (d as any).deployment || "",
      db_version: (d as any).db_version || "",
      instance_name: (d as any).instance_name || "",
      db_username: (d as any).db_username || "",
      db_password: "",
    });
    setModalOpen(true);
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
    if (form.deployment) data.deployment = form.deployment;
    if (form.db_version) data.db_version = form.db_version;
    if (form.instance_name) data.instance_name = form.instance_name;
    if (form.db_username) data.db_username = form.db_username;
    if (form.db_password) data.db_password_encrypted = form.db_password;

    setSaving(true);
    setSaveError(null);

    const promise = editing
      ? invoke<Device>("update_device", { deviceId: editing.id, data })
      : invoke<Device>("create_device", { data });

    promise
      .then((saved) => {
        setSaving(false);
        setModalOpen(false);
        loadDevices();
        // 后台检测：不阻塞保存流程，saving 已立即置 false
        if (!saved?.id) return;
        const devId = saved.id;
        const devName = form.name;
        const hasCred = !!form.ssh_username.trim();

        invoke<{ new_status: string }>("check_device_status", { deviceId: devId })
          .then((res) => {
            loadDevices();
            if (res?.new_status !== "online") {
              showStatusHint(`${devName}: 设备离线（端口不通）`, "warn");
              return;
            }
            if (!hasCred) {
              showStatusHint(`${devName}: 在线，但未填 SSH 用户名，跳过静态信息检测`, "warn");
              return;
            }
            showStatusHint(`正在后台检测 ${devName} 的静态信息...`, "info", 30000);
            return invoke<string>("detect_device_model_by_id", { deviceId: devId })
              .then((json) => {
                console.log("[detect] 检测结果:", json);
                loadDevices();
                showStatusHint(`${devName}: 静态信息检测完成`, "success");
              })
              .catch((e) => {
                console.error("[detect] 检测失败:", e);
                showStatusHint(detectErrorHint(devName, e), "error");
              });
          })
          .catch((e) => {
            console.error("[check_device_status] 失败:", e);
            showStatusHint(`${devName}: 在线状态检测失败`, "error");
          });
      })
      .catch((e) => {
        setSaving(false);
        const msg = friendlyError(e);
        setSaveError(msg);
        if (msg.includes("IP")) triggerShake("ip");
        else triggerShake("name");
      });
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
    if (checkingIds.has(d.id)) return; // 防重复点击
    setCheckingIds((prev) => new Set(prev).add(d.id));
    showStatusHint(`正在检测 ${d.name}...`, "info", 30000);
    // 先做状态检测，在线后再做静态信息检测（离线时跳过避免 SSH 超时）
    invoke<{ new_status: string }>("check_device_status", { deviceId: d.id })
      .then((res) => {
        if (res?.new_status !== "online") {
          showStatusHint(`${d.name}: 设备离线，已跳过静态信息检测`, "warn");
          return;
        }
        return invoke<string>("detect_device_model_by_id", { deviceId: d.id })
          .then(() => {
            showStatusHint(`${d.name}: 状态与静态信息检测完成`, "success");
          })
          .catch((e) => {
            console.error("[check] 静态信息检测失败:", e);
            showStatusHint(detectErrorHint(d.name, e), "error");
          });
      })
      .catch((e) => {
        console.error("[check] 状态检测失败:", e);
        showStatusHint(`${d.name}: 状态检测失败`, "error");
      })
      .finally(() => {
        setCheckingIds((prev) => {
          const n = new Set(prev);
          n.delete(d.id);
          return n;
        });
        loadDevices();
      });
  };

  const handleCheckAll = () => {
    if (checkingAll) return; // 防重复点击
    setCheckingAll(true);
    showStatusHint("正在检测全部设备状态...", "info", 60000);
    let failCount = 0;
    let okCount = 0;
    // 全部设备：并发跑状态检测；完成后对在线设备串行触发静态信息检测
    invoke<{ total: number; online: number; offline: number }>("check_all_devices_status")
      .catch((e) => { console.error("[check_all] 状态检测失败:", e); })
      .then(() => loadDevices())
      .then(() =>
        // 重新读最新状态后再筛选在线设备
        invoke<Device[]>("list_devices", { deviceType: typeFilter || undefined, status: "online" })
      )
      .then((onlineDevices) => {
        if (!onlineDevices || onlineDevices.length === 0) return;
        // 已有静态信息的设备跳过 SSH 检测（保存设备时已自动采集过）
        const needsDetect = onlineDevices.filter(
          (d) => !d.model && !d.sysname,
        );
        const skipped = onlineDevices.length - needsDetect.length;
        if (skipped > 0) {
          showStatusHint(`在线 ${onlineDevices.length} 台，其中 ${skipped} 台已有静态信息，跳过二次检测`, "info");
        }
        if (needsDetect.length === 0) return;
        showStatusHint(`正在检测 ${needsDetect.length} 台设备的静态信息...`, "info", 120000);
        // 并发执行（最多 3 台同时），避免大量 SSH 同时冲击 sshd
        const CONCURRENCY = 3;
        const targets = needsDetect;
        const runOne = (dev: Device): Promise<void> =>
          invoke<string>("detect_device_model_by_id", { deviceId: dev.id })
            .then(() => { okCount++; })
            .catch((e) => {
              failCount++;
              console.error(`[check_all] ${dev.name} 静态信息检测失败:`, e);
            });
        // 分批执行：每次取出 CONCURRENCY 台并发，完成后再取下一批
        const runBatch = async (start: number): Promise<void> => {
          const batch = targets.slice(start, start + CONCURRENCY);
          if (batch.length === 0) return;
          await Promise.all(batch.map(runOne));
          return runBatch(start + CONCURRENCY);
        };
        return runBatch(0);
      })
      .then(() => loadDevices())
      .then(() => {
        if (okCount === 0 && failCount === 0) {
          showStatusHint("设备状态检测完成", "success");
        } else if (failCount === 0) {
          showStatusHint(`检测完成：${okCount} 台静态信息已更新`, "success");
        } else {
          showStatusHint(`检测完成：${okCount} 台成功，${failCount} 台失败（详见控制台）`, "warn");
        }
      })
      .catch((e) => {
        console.error(e);
        showStatusHint("检测过程出错", "error");
      })
      .finally(() => setCheckingAll(false));
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
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1">管理网络设备与服务器信息</p>
      </div>

      <Toolbar>
        <Button onClick={openAdd} size="sm">添加设备</Button>
        <Select
          size="sm"
          className="w-28"
          value={typeFilter}
          onChange={(e) => setTypeFilter(e.target.value)}
        >
          <option value="">全部类型</option>
          <option value="switch,router">网络设备</option>
          <option value="firewall,loadbalancer">安全设备</option>
          <option value="server">服务器</option>
          <option value="database">数据库</option>
        </Select>
        <Select
          size="sm"
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
        <Button size="sm" variant="secondary" loading={checkingAll} onClick={handleCheckAll}>
          {checkingAll ? "检测中..." : "检测全部"}
        </Button>
        {selectedIds.size > 0 && (
          <Button variant="danger" size="sm" onClick={handleBatchDelete}>
            批量删除 ({selectedIds.size})
          </Button>
        )}
      </Toolbar>

      <DataTable<Device>
        columns={[
          {
            key: "checkbox", header: "", width: "36px", noTruncate: true, render: (r) => (
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
          {
            key: "status",
            header: "状态",
            noTruncate: true,
            render: (r) => {
              // 离线时只显示离线徽章——账号无法验证不算"账号错误"，避免误导
              // 在线时：账号正常/未验证 → 仅在线徽章；账号异常 → 在线 + 异常徽章
              const showAuth =
                r.status === "online" &&
                r.auth_status &&
                r.auth_status !== "ok" &&
                r.auth_status !== "unknown";
              return (
                <div className="flex items-center gap-1.5 whitespace-nowrap">
                  <StatusBadge status={r.status} />
                  {showAuth && <AuthBadge status={r.auth_status} message={r.auth_message} />}
                </div>
              );
            },
          },
          {
            key: "template_id", header: "关联模板", render: (r) => {
              const t = templates.find((t) => t.id === r.template_id);
              return t ? t.name : "-";
            },
          },
          {
            key: "last_checked_at", header: "最后检测", width: "105px", render: (r) =>
              r.last_checked_at
                ? (() => { const d = new Date(r.last_checked_at); return `${String(d.getMonth()+1).padStart(2,"0")}-${String(d.getDate()).padStart(2,"0")} ${String(d.getHours()).padStart(2,"0")}:${String(d.getMinutes()).padStart(2,"0")}`; })()
                : "-",
          },
          {
            key: "actions",
            header: "操作",
            width: "210px",
            noTruncate: true,
            render: (r) => (
              <div className="flex gap-1" onClick={(e) => e.stopPropagation()}>
                <Button
                  size="sm"
                  variant="ghost"
                  loading={checkingIds.has(r.id)}
                  onClick={() => handleCheckDevice(r)}
                >
                  {checkingIds.has(r.id) ? "检测中" : "检测"}
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
                    } else if (dt === "database" && !DB_VENDORS.includes(form.vendor)) {
                      updated.vendor = "MySQL";
                    } else if (dt !== "server" && dt !== "database" && !NETWORK_VENDORS.includes(form.vendor)) {
                      updated.vendor = "H3C";
                    }
                    setForm({ ...form, ...updated });
                  }}>
                    <option value="switch">交换机</option>
                    <option value="router">路由器</option>
                    <option value="firewall">防火墙</option>
                    <option value="loadbalancer">负载均衡</option>
                    <option value="server">服务器</option>
                    <option value="database">数据库</option>
                  </Select>
                </div>
                <div>
                  <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">厂商</label>
                  <Select value={form.vendor} onChange={(e) => setForm({ ...form, vendor: e.target.value })}>
                    {(form.device_type === "server" ? SERVER_VENDORS : form.device_type === "database" ? DB_VENDORS : NETWORK_VENDORS).map((v) => <option key={v} value={v}>{v}</option>)}
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
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">{form.device_type === "server" || form.device_type === "database" ? "发行版本号" : "型号"}</label>
                  <Input
                    value={form.model}
                    onChange={(e) => setForm({ ...form, model: e.target.value })}
                    placeholder="自动检测"
                    className={shakeFields.has("model") ? "animate-shake" : ""}
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">主机名</label>
                  <Input value={form.sysname} onChange={(e) => setForm({ ...form, sysname: e.target.value })} placeholder="自动检测" />
                </div>
              </div>
              <div className="grid grid-cols-[5fr_5fr_2fr] gap-3">
                <div>
                  <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">SSH 用户名 <span className="text-[hsl(var(--danger))]">*</span></label>
                  <Input value={form.ssh_username} className={shakeFields.has("ssh_username") ? "animate-shake" : ""} onChange={(e) => setForm({ ...form, ssh_username: e.target.value })} placeholder="admin" />
                </div>
                <div>
                  <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">SSH 密码 <span className="text-[hsl(var(--danger))]">*</span></label>
                  <Input
                    type="password"
                    value={passwordSet ? "••••••••" : form.ssh_password}
                    className={shakeFields.has("ssh_password") ? "animate-shake" : ""}
                    onFocus={() => { if (passwordSet) { setPasswordSet(false); setForm({ ...form, ssh_password: "" }); } }}
                    onChange={(e) => setForm({ ...form, ssh_password: e.target.value })}
                    placeholder={editing ? "点击修改密码" : "输入密码"}
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">端口</label>
                  <Input type="number" value={form.ssh_port} onChange={(e) => setForm({ ...form, ssh_port: Number(e.target.value) || 22 })} />
                </div>
              </div>
              {/* ── 服务器 — CPU/内存 ── */}
              {form.device_type === "server" && (
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">CPU 核心数</label>
                    <Input value={form.cpu_cores} onChange={(e) => setForm({ ...form, cpu_cores: e.target.value })} placeholder="自动检测" />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">内存</label>
                    <Input value={form.memory_gb} onChange={(e) => setForm({ ...form, memory_gb: e.target.value })} placeholder="自动检测" />
                  </div>
                </div>
              )}
              {/* ── 数据库：操作系统信息 ── */}
              {form.device_type === "database" && (
                <>
                  <div className="col-span-2 mt-2 pt-2 border-t border-[hsl(var(--border))]">
                    <p className="text-[11px] font-medium text-[hsl(var(--text-secondary))] mb-2">操作系统信息</p>
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">CPU 核心数</label>
                    <Input value={form.cpu_cores} onChange={(e) => setForm({ ...form, cpu_cores: e.target.value })} placeholder="自动检测" />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">内存</label>
                    <Input value={form.memory_gb} onChange={(e) => setForm({ ...form, memory_gb: e.target.value })} placeholder="自动检测" />
                  </div>
                </>
              )}
              {/* ── 数据库专属信息 ── */}
              {form.device_type === "database" && (
                <div className="col-span-2 mt-2 pt-2 border-t border-[hsl(var(--border))]">
                  <p className="text-[11px] font-medium text-[hsl(var(--text-secondary))] mb-2">数据库信息</p>
                  <div className="space-y-2">
                    <div className="grid grid-cols-2 gap-2">
                      <div>
                        <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">数据库版本</label>
                        <Input value={form.db_version} onChange={(e) => setForm({ ...form, db_version: e.target.value })} placeholder="如 MySQL 8.0" />
                      </div>
                      <div>
                        <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">实例名</label>
                        <Input value={form.instance_name} onChange={(e) => setForm({ ...form, instance_name: e.target.value })} placeholder="如 prod-db-01" />
                      </div>
                    </div>
                    <div className="grid grid-cols-2 gap-2">
                      <div>
                        <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">数据库用户名</label>
                        <Input value={form.db_username} onChange={(e) => setForm({ ...form, db_username: e.target.value })} placeholder="如 root" />
                      </div>
                      <div>
                        <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">数据库密码</label>
                        <Input type="password" value={form.db_password} onChange={(e) => setForm({ ...form, db_password: e.target.value })} placeholder="留空不修改" />
                      </div>
                    </div>
                    <div>
                      <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">部署方式</label>
                      <Select value={form.deployment} onChange={(e) => setForm({ ...form, deployment: e.target.value })}>
                        <option value="">未知</option>
                        <option value="direct">物理机 / 包安装</option>
                        <option value="docker">Docker 容器</option>
                        <option value="podman">Podman 容器</option>
                        <option value="k8s">Kubernetes</option>
                      </Select>
                    </div>
                  </div>
                </div>
              )}
              {/* ── 网络/安全设备信息 ── */}
              {form.device_type !== "server" && form.device_type !== "database" && (
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
