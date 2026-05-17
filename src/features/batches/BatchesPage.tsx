import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Link } from "react-router-dom";
import { Plus, Play, Pause, Square, RotateCcw, Eye } from "lucide-react";
import type { InspectionBatch, Device } from "@/types";

const STATUS_COLORS: Record<string, string> = { pending:"bg-yellow-100 text-yellow-800", running:"bg-blue-100 text-blue-800", completed:"bg-green-100 text-green-800", failed:"bg-red-100 text-red-800", paused:"bg-gray-100 text-gray-800", stopped:"bg-orange-100 text-orange-800" };

export default function BatchesPage() {
  const [batches, setBatches] = useState<InspectionBatch[]>([]);
  const [devices, setDevices] = useState<Device[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState({ name: "", mode: "ssh", device_ids: [] as number[], auto_start: true });

  const load = () => { invoke<InspectionBatch[]>("list_batches", { status: null }).then(setBatches).catch(console.error); };
  useEffect(load, []);
  useEffect(() => { invoke<Device[]>("list_devices", { group: null, status: null, vendor: null }).then(setDevices).catch(console.error); }, []);

  const create = async () => {
    await invoke("create_batch", { data: form }); load(); setShowForm(false);
    setForm({ name: "", mode: "ssh", device_ids: [], auto_start: true });
  };
  const action = async (batchId: number, cmd: string) => {
    await invoke(cmd, { batchId }); load();
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold">巡检批次</h2>
        <button onClick={() => setShowForm(true)} className="flex items-center gap-1 bg-primary text-primary-foreground px-3 py-1.5 rounded-md text-sm"><Plus className="h-4 w-4"/>创建批次</button>
      </div>

      <div className="space-y-3">
        {batches.map(b => (
          <div key={b.id} className="border rounded-lg p-4 bg-card">
            <div className="flex items-center justify-between mb-2">
              <div>
                <Link to={`/batches/${b.id}`} className="font-bold hover:text-primary">{b.name || `批次#${b.id}`}</Link>
                <span className="ml-2 text-xs text-muted-foreground">{b.mode}</span>
                <span className={`ml-2 text-xs rounded px-1.5 py-0.5 ${STATUS_COLORS[b.status] || ""}`}>{b.status}</span>
              </div>
              <div className="flex gap-1">
                <Link to={`/batches/${b.id}`} className="p-1 hover:bg-muted rounded"><Eye className="h-4 w-4"/></Link>
                {b.status === "pending" && <button onClick={() => action(b.id, "run_batch")} className="p-1 hover:bg-muted rounded text-green-600"><Play className="h-4 w-4"/></button>}
                {b.status === "running" && <button onClick={() => action(b.id, "pause_batch")} className="p-1 hover:bg-muted rounded text-yellow-600"><Pause className="h-4 w-4"/></button>}
                {(b.status === "running" || b.status === "paused") && <button onClick={() => action(b.id, "stop_batch")} className="p-1 hover:bg-muted rounded text-red-600"><Square className="h-4 w-4"/></button>}
                {b.status !== "running" && b.status !== "pending" && <button onClick={() => action(b.id, "restart_batch")} className="p-1 hover:bg-muted rounded"><RotateCcw className="h-4 w-4"/></button>}
              </div>
            </div>
            <div className="flex gap-4 text-xs text-muted-foreground">
              <span>{b.device_ids?.length || 0} 台设备</span>
              <span>{b.triggered_by}</span>
              <span>创建: {b.created_at}</span>
              {b.records && <span>{b.records.length} 条记录</span>}
            </div>
          </div>
        ))}
      </div>

      {showForm && (
        <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50" onClick={() => setShowForm(false)}>
          <div className="bg-card border rounded-lg p-6 w-full max-w-lg space-y-3 max-h-[80vh] overflow-auto" onClick={e => e.stopPropagation()}>
            <h3 className="font-bold text-lg">创建批次</h3>
            <label className="block"><span className="text-sm text-muted-foreground">名称</span>
              <input className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.name} onChange={e=>setForm(f=>({...f,name:e.target.value}))} placeholder="留空自动生成" /></label>
            <label className="block"><span className="text-sm text-muted-foreground">模式</span>
              <select className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.mode} onChange={e=>setForm(f=>({...f,mode:e.target.value}))}>
                <option value="ssh">ssh</option><option value="offline">offline</option><option value="mixed">mixed</option></select></label>
            <div>
              <p className="text-sm text-muted-foreground mb-1">选择设备 ({form.device_ids.length})</p>
              <div className="border rounded p-2 max-h-48 overflow-auto space-y-0.5">
                {devices.map(d => (
                  <label key={d.id} className="flex items-center gap-2 text-sm cursor-pointer hover:bg-muted px-1 py-0.5 rounded">
                    <input type="checkbox" checked={form.device_ids.includes(d.id)} onChange={() => setForm(f => f.device_ids.includes(d.id) ? {...f, device_ids: f.device_ids.filter(i=>i!==d.id)} : {...f, device_ids: [...f.device_ids, d.id]})} />
                    <span>{d.name}</span><span className="text-xs text-muted-foreground">{d.ip}</span>
                    <span className={`text-xs ml-auto ${d.status === 'online' ? 'text-green-600' : 'text-red-600'}`}>{d.status}</span>
                  </label>
                ))}
              </div>
            </div>
            <div className="flex gap-2">
              <button onClick={create} className="bg-primary text-primary-foreground px-4 py-1.5 rounded-md text-sm">创建</button>
              <button onClick={()=>setShowForm(false)} className="border px-4 py-1.5 rounded-md text-sm">取消</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
