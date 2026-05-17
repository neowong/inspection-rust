import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus, Pause, Play, Trash2 } from "lucide-react";
import type { ScheduledTask } from "@/types";

export default function ScheduledTasksPage() {
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState({ name: "", cron_expression: "0 9 * * *", device_ids: [] as number[] });
  const [devices, setDevices] = useState<any[]>([]);

  const load = () => { invoke<ScheduledTask[]>("list_tasks").then(setTasks).catch(console.error); };
  useEffect(load, []);
  useEffect(() => { invoke("list_devices", { group: null, status: null, vendor: null }).then(setDevices).catch(console.error); }, []);

  const create = async () => {
    await invoke("create_task", { data: form }); load(); setShowForm(false);
    setForm({ name: "", cron_expression: "0 9 * * *", device_ids: [] });
  };
  const toggle = async (id: number, enabled: boolean) => {
    await invoke(enabled ? "pause_task" : "resume_task", { taskId: id }); load();
  };
  const del = async (id: number) => { await invoke("delete_task", { taskId: id }); load(); };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold">定时任务</h2>
        <button onClick={() => setShowForm(true)} className="flex items-center gap-1 bg-primary text-primary-foreground px-3 py-1.5 rounded-md text-sm"><Plus className="h-4 w-4"/>创建任务</button>
      </div>

      <div className="space-y-3">
        {tasks.map(t => (
          <div key={t.id} className="border rounded-lg p-4 bg-card flex items-center justify-between">
            <div>
              <span className="font-bold">{t.name}</span>
              <span className="ml-2 text-xs font-mono text-muted-foreground">{t.cron_expression}</span>
              <span className={`ml-2 text-xs rounded px-1.5 py-0.5 ${t.enabled ? 'bg-green-100 text-green-700' : 'bg-gray-100 text-gray-600'}`}>{t.enabled ? '运行中' : '已暂停'}</span>
              <span className="ml-2 text-xs text-muted-foreground">{t.device_ids?.length || 0} 台设备</span>
            </div>
            <div className="flex gap-1">
              <button onClick={() => toggle(t.id, t.enabled)} className="p-1 hover:bg-muted rounded">
                {t.enabled ? <Pause className="h-4 w-4 text-yellow-600"/> : <Play className="h-4 w-4 text-green-600"/>}
              </button>
              <button onClick={() => del(t.id)} className="p-1 hover:bg-muted rounded text-destructive"><Trash2 className="h-4 w-4"/></button>
            </div>
          </div>
        ))}
        {tasks.length === 0 && <p className="text-center text-muted-foreground py-8">暂无定时任务</p>}
      </div>

      {showForm && (
        <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50" onClick={()=>setShowForm(false)}>
          <div className="bg-card border rounded-lg p-6 w-full max-w-md space-y-3" onClick={e=>e.stopPropagation()}>
            <h3 className="font-bold text-lg">创建定时任务</h3>
            <label className="block"><span className="text-sm text-muted-foreground">名称</span>
              <input className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.name} onChange={e=>setForm(f=>({...f,name:e.target.value}))} /></label>
            <label className="block"><span className="text-sm text-muted-foreground">Cron 表达式</span>
              <input className="w-full border rounded px-2 py-1 mt-0.5 text-sm font-mono" value={form.cron_expression} onChange={e=>setForm(f=>({...f,cron_expression:e.target.value}))} placeholder="0 9 * * *" /></label>
            <div>
              <p className="text-sm text-muted-foreground mb-1">选择设备 ({form.device_ids.length})</p>
              <div className="border rounded p-2 max-h-40 overflow-auto">
                {devices.map((d: any) => (
                  <label key={d.id} className="flex items-center gap-2 text-sm cursor-pointer hover:bg-muted px-1 py-0.5 rounded">
                    <input type="checkbox" checked={form.device_ids.includes(d.id)} onChange={() => setForm(f => f.device_ids.includes(d.id) ? {...f,device_ids:f.device_ids.filter(i=>i!==d.id)} : {...f,device_ids:[...f.device_ids,d.id]})}/>
                    <span>{d.name}</span></label>
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
