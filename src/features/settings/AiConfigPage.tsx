import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus, Trash2, Zap, ZapOff } from "lucide-react";
import type { AiModelConfig } from "@/types";

export default function AiConfigPage() {
  const [configs, setConfigs] = useState<AiModelConfig[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState({ name: "", provider: "openai", model_id: "", api_key: "", base_url: "" });

  const load = () => { invoke<AiModelConfig[]>("list_ai_configs").then(setConfigs).catch(console.error); };
  useEffect(load, []);

  const create = async () => {
    await invoke("create_ai_config", { data: form }); load(); setShowForm(false);
    setForm({ name: "", provider: "openai", model_id: "", api_key: "", base_url: "" });
  };
  const toggle = async (id: number, active: boolean) => {
    await invoke(active ? "deactivate_ai_config" : "activate_ai_config", { configId: id }); load();
  };
  const del = async (id: number) => { await invoke("delete_ai_config", { configId: id }); load(); };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold">AI 模型配置</h2>
        <button onClick={() => setShowForm(true)} className="flex items-center gap-1 bg-primary text-primary-foreground px-3 py-1.5 rounded-md text-sm"><Plus className="h-4 w-4"/>添加配置</button>
      </div>

      <div className="space-y-3">
        {configs.map(c => (
          <div key={c.id} className="border rounded-lg p-4 bg-card flex items-center justify-between">
            <div>
              <span className="font-bold">{c.name}</span>
              <span className="ml-2 text-xs text-muted-foreground">{c.provider}</span>
              <span className="ml-2 text-xs font-mono">{c.model_id}</span>
              {c.base_url && <span className="ml-2 text-xs text-muted-foreground">{c.base_url}</span>}
              <span className={`ml-2 text-xs rounded px-1.5 py-0.5 ${c.is_active ? 'bg-green-100 text-green-700' : 'bg-gray-100'}`}>{c.is_active ? '已激活' : '未激活'}</span>
            </div>
            <div className="flex gap-1">
              <button onClick={() => toggle(c.id, c.is_active)} className="p-1 hover:bg-muted rounded">
                {c.is_active ? <ZapOff className="h-4 w-4 text-yellow-600"/> : <Zap className="h-4 w-4 text-green-600"/>}
              </button>
              <button onClick={() => del(c.id)} className="p-1 hover:bg-muted rounded text-destructive"><Trash2 className="h-4 w-4"/></button>
            </div>
          </div>
        ))}
        {configs.length === 0 && <p className="text-center text-muted-foreground py-8">暂无 AI 配置</p>}
      </div>

      {showForm && (
        <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50" onClick={()=>setShowForm(false)}>
          <div className="bg-card border rounded-lg p-6 w-full max-w-md space-y-3" onClick={e=>e.stopPropagation()}>
            <h3 className="font-bold text-lg">添加 AI 配置</h3>
            <label className="block"><span className="text-sm text-muted-foreground">名称</span>
              <input className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.name} onChange={e=>setForm(f=>({...f,name:e.target.value}))} /></label>
            <label className="block"><span className="text-sm text-muted-foreground">提供商</span>
              <select className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.provider} onChange={e=>setForm(f=>({...f,provider:e.target.value}))}>
                <option value="openai">OpenAI 兼容</option><option value="anthropic">Anthropic</option></select></label>
            <label className="block"><span className="text-sm text-muted-foreground">模型 ID</span>
              <input className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.model_id} onChange={e=>setForm(f=>({...f,model_id:e.target.value}))} placeholder="gpt-4o / claude-sonnet-4-6"/></label>
            <label className="block"><span className="text-sm text-muted-foreground">API Key</span>
              <input className="w-full border rounded px-2 py-1 mt-0.5 text-sm" type="password" value={form.api_key} onChange={e=>setForm(f=>({...f,api_key:e.target.value}))} /></label>
            <label className="block"><span className="text-sm text-muted-foreground">Base URL (可选)</span>
              <input className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.base_url} onChange={e=>setForm(f=>({...f,base_url:e.target.value}))} placeholder="留空使用默认" /></label>
            <div className="flex gap-2">
              <button onClick={create} className="bg-primary text-primary-foreground px-4 py-1.5 rounded-md text-sm">添加</button>
              <button onClick={()=>setShowForm(false)} className="border px-4 py-1.5 rounded-md text-sm">取消</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
