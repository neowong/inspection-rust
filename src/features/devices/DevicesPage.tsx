import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus, Trash2, Edit, Wifi, WifiOff, HelpCircle } from "lucide-react";
import type { Device } from "@/types";

const GROUPS = ["network", "system"] as const;
const MODES = ["ssh", "offline", "web"] as const;
const TYPES = ["交换机", "路由器", "防火墙", "服务器", "无线控制器"] as const;
const VENDORS = ["H3C", "华为", "思科", "深信服", "锐捷", "Linux", "CentOS", "Ubuntu", "openEuler", "其它"] as const;
const DB_TYPES = ["mysql", "postgresql", "oracle"] as const;

const STATUS_ICONS: Record<string, typeof Wifi> = { online: Wifi, offline: WifiOff, unknown: HelpCircle };

export default function DevicesPage() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState<Record<string, any>>({ group: "network", inspection_mode: "ssh", ssh_port: 22, type: "交换机", vendor: "H3C" });
  const [editingId, setEditingId] = useState<number | null>(null);
  const [filterGroup, setFilterGroup] = useState<string>("");
  const [filterStatus, setFilterStatus] = useState<string>("");

  const load = () => {
    invoke<Device[]>("list_devices", { group: filterGroup || null, status: filterStatus || null, vendor: null })
      .then(setDevices).catch(console.error);
  };
  useEffect(load, [filterGroup, filterStatus]);

  const reset = () => { setForm({ group: "network", inspection_mode: "ssh", ssh_port: 22, type: "交换机", vendor: "H3C" }); setEditingId(null); setShowForm(false); };
  const submit = async () => {
    const cmd = editingId ? "update_device" : "create_device";
    await invoke(cmd, editingId ? { deviceId: editingId, data: form } : { data: form });
    reset(); load();
  };
  const edit = (d: Device) => {
    setForm({ ...d, ssh_password: "", db_password: "" });
    setEditingId(d.id); setShowForm(true);
  };
  const del = async (id: number) => { await invoke("delete_device", { deviceId: id }); load(); };
  const statusCheck = async (id: number) => { await invoke("check_device_status", { deviceId: id }); load(); };

  const input = (label: string, key: string, opts?: Record<string, any>) => (
    <label className="block text-sm"><span className="text-muted-foreground">{label}</span>
      <input className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form[key] ?? ""} onChange={e => setForm(f => ({ ...f, [key]: e.target.value }))} {...opts} />
    </label>
  );

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold">设备管理</h2>
        <button onClick={() => setShowForm(true)} className="flex items-center gap-1 bg-primary text-primary-foreground px-3 py-1.5 rounded-md text-sm"><Plus className="h-4 w-4"/>添加设备</button>
      </div>

      <div className="flex gap-2">
        <select className="border rounded px-2 py-1 text-sm" value={filterGroup} onChange={e => setFilterGroup(e.target.value)}>
          <option value="">全部分组</option><option value="network">network</option><option value="system">system</option>
        </select>
        <select className="border rounded px-2 py-1 text-sm" value={filterStatus} onChange={e => setFilterStatus(e.target.value)}>
          <option value="">全部状态</option><option value="online">在线</option><option value="offline">离线</option><option value="unknown">未知</option>
        </select>
        <button onClick={() => { setFilterGroup(""); setFilterStatus(""); }} className="text-sm text-muted-foreground hover:text-foreground">清除</button>
        <span className="text-sm text-muted-foreground ml-auto">{devices.length} 台设备</span>
      </div>

      <div className="border rounded-lg overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-muted"><tr>
            {["名称","IP","分组","类型","厂商","巡检模式","状态","操作"].map(h => <th key={h} className="text-left px-3 py-2">{h}</th>)}
          </tr></thead>
          <tbody>
            {devices.map(d => {
              const StatusIcon = STATUS_ICONS[d.status] ?? HelpCircle;
              return (
                <tr key={d.id} className="border-t hover:bg-muted/50">
                  <td className="px-3 py-2 font-medium">{d.name}</td><td className="px-3 py-2 text-muted-foreground">{d.ip}</td>
                  <td className="px-3 py-2"><span className="text-xs border rounded px-1.5 py-0.5">{d.group}</span></td>
                  <td className="px-3 py-2">{d.device_type}</td><td className="px-3 py-2">{d.vendor}</td>
                  <td className="px-3 py-2"><span className="text-xs bg-blue-50 text-blue-700 rounded px-1.5 py-0.5">{d.inspection_mode}</span></td>
                  <td className="px-3 py-2"><StatusIcon className={`h-4 w-4 inline mr-1 ${d.status === 'online' ? 'text-green-600' : d.status === 'offline' ? 'text-red-600' : 'text-muted-foreground'}`}/>{d.status}</td>
                  <td className="px-3 py-2 flex gap-1">
                    <button onClick={() => statusCheck(d.id)} className="p-1 hover:bg-muted rounded" title="检测状态"><Wifi className="h-3.5 w-3.5"/></button>
                    <button onClick={() => edit(d)} className="p-1 hover:bg-muted rounded"><Edit className="h-3.5 w-3.5"/></button>
                    <button onClick={() => del(d.id)} className="p-1 hover:bg-muted rounded text-destructive"><Trash2 className="h-3.5 w-3.5"/></button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {devices.length === 0 && <p className="text-center text-muted-foreground py-8">暂无设备</p>}
      </div>

      {showForm && (
        <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50" onClick={() => reset()}>
          <div className="bg-card border rounded-lg p-6 w-full max-w-lg space-y-3 max-h-[80vh] overflow-auto" onClick={e => e.stopPropagation()}>
            <h3 className="font-bold text-lg">{editingId ? "编辑设备" : "添加设备"}</h3>
            <div className="grid grid-cols-2 gap-3">
              {input("名称*", "name", { required: true })}
              {input("IP地址*", "ip", { required: true })}
              <label className="block text-sm"><span className="text-muted-foreground">分组</span>
                <select className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.group} onChange={e => setForm(f=>({...f,group:e.target.value}))}>
                  {GROUPS.map(g=><option key={g}>{g}</option>)}</select></label>
              <label className="block text-sm"><span className="text-muted-foreground">类型</span>
                <select className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.type||form.device_type} onChange={e=>setForm(f=>({...f,device_type:e.target.value,type:e.target.value}))}>
                  {TYPES.map(t=><option key={t}>{t}</option>)}</select></label>
              <label className="block text-sm"><span className="text-muted-foreground">厂商</span>
                <select className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.vendor} onChange={e=>setForm(f=>({...f,vendor:e.target.value}))}>
                  {VENDORS.map(v=><option key={v}>{v}</option>)}</select></label>
              <label className="block text-sm"><span className="text-muted-foreground">巡检模式</span>
                <select className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.inspection_mode} onChange={e=>setForm(f=>({...f,inspection_mode:e.target.value}))}>
                  {MODES.map(m=><option key={m}>{m}</option>)}</select></label>
              {input("SSH用户名", "ssh_username")}
              {input("SSH密码", "ssh_password", { type: "password" })}
              {input("SSH端口", "ssh_port", { type: "number" })}
              {input("型号", "model")}
              <label className="block text-sm"><span className="text-muted-foreground">数据库类型</span>
                <select className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.db_type||""} onChange={e=>setForm(f=>({...f,db_type:e.target.value||null}))}>
                  <option value="">无</option>{DB_TYPES.map(t=><option key={t}>{t}</option>)}</select></label>
              {form.db_type && <>
                {input("DB端口", "db_port", { type: "number" })}
                {input("DB用户名", "db_username")}
                {input("DB密码", "db_password", { type: "password" })}
                {input("OS用户", "db_os_user")}
              </>}
            </div>
            <div className="flex gap-2 pt-2">
              <button onClick={submit} className="bg-primary text-primary-foreground px-4 py-1.5 rounded-md text-sm">{editingId?"更新":"添加"}</button>
              <button onClick={reset} className="border px-4 py-1.5 rounded-md text-sm">取消</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
