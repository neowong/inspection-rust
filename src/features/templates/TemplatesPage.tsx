import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus, Trash2, Edit, Wand2 } from "lucide-react";
import type { InspectionTemplate, CommandPool } from "@/types";

export default function TemplatesPage() {
  const [templates, setTemplates] = useState<InspectionTemplate[]>([]);
  const [commands, setCommands] = useState<CommandPool[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [form, setForm] = useState({ name: "", vendor: "", model: "", device_type: "", template_type: "ssh", description: "", command_ids: [] as number[] });
  const [filterVendor, setFilterVendor] = useState("");

  const load = () => { invoke<InspectionTemplate[]>("list_templates", { vendor: filterVendor || null }).then(setTemplates).catch(console.error); };
  useEffect(load, [filterVendor]);
  useEffect(() => { invoke<CommandPool[]>("list_commands", { vendor: null }).then(setCommands).catch(console.error); }, []);

  const reset = () => { setForm({ name: "", vendor: "", model: "", device_type: "", template_type: "ssh", description: "", command_ids: [] }); setEditingId(null); setShowForm(false); };
  const submit = async () => {
    const data = { ...form, config: { command_ids: form.command_ids } };
    await invoke(editingId ? "update_template" : "create_template", editingId ? { templateId: editingId, data } : { data });
    reset(); load();
  };
  const edit = (t: InspectionTemplate) => {
    setForm({ name: t.name, vendor: t.vendor, model: t.model || "", device_type: t.device_type || "", template_type: t.type, description: t.description || "", command_ids: t.config?.command_ids || [] });
    setEditingId(t.id); setShowForm(true);
  };
  const del = async (id: number) => { if (confirm("确认删除？")) { await invoke("delete_template", { templateId: id }); load(); } };
  const toggleCmd = (id: number) => {
    setForm(f => f.command_ids.includes(id) ? { ...f, command_ids: f.command_ids.filter(c => c !== id) } : { ...f, command_ids: [...f.command_ids, id] });
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold">巡检模板</h2>
        <button onClick={() => setShowForm(true)} className="flex items-center gap-1 bg-primary text-primary-foreground px-3 py-1.5 rounded-md text-sm"><Plus className="h-4 w-4"/>创建模板</button>
      </div>

      <div className="flex gap-2">
        <select className="border rounded px-2 py-1 text-sm" value={filterVendor} onChange={e => setFilterVendor(e.target.value)}>
          <option value="">全部厂商</option>
          {["H3C","华为","思科","锐捷","Linux","CentOS","Ubuntu","MySQL","PostgreSQL","Oracle"].map(v => <option key={v}>{v}</option>)}
        </select>
      </div>

      <div className="border rounded-lg overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-muted"><tr>{["名称","厂商","型号","适用类型","命令数","关联设备","操作"].map(h => <th key={h} className="text-left px-3 py-2">{h}</th>)}</tr></thead>
          <tbody>
            {templates.map(t => (
              <tr key={t.id} className="border-t hover:bg-muted/50">
                <td className="px-3 py-2 font-medium">{t.name}</td><td className="px-3 py-2">{t.vendor}</td>
                <td className="px-3 py-2 text-muted-foreground">{t.model || "-"}</td><td className="px-3 py-2">{t.device_type || "-"}</td>
                <td className="px-3 py-2">{t.config?.command_ids?.length || 0}</td><td className="px-3 py-2">{t.device_count}</td>
                <td className="px-3 py-2 flex gap-1">
                  <button onClick={() => edit(t)} className="p-1 hover:bg-muted rounded"><Edit className="h-3.5 w-3.5"/></button>
                  <button onClick={() => del(t.id)} className="p-1 hover:bg-muted rounded text-destructive"><Trash2 className="h-3.5 w-3.5"/></button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {templates.length === 0 && <p className="text-center text-muted-foreground py-8">暂无模板</p>}
      </div>

      {showForm && (
        <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50" onClick={reset}>
          <div className="bg-card border rounded-lg p-6 w-full max-w-2xl space-y-3 max-h-[85vh] overflow-auto" onClick={e => e.stopPropagation()}>
            <h3 className="font-bold text-lg">{editingId ? "编辑模板" : "创建模板"}</h3>
            <div className="grid grid-cols-2 gap-3">
              <label className="block"><span className="text-sm text-muted-foreground">名称*</span>
                <input className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))} /></label>
              <label className="block"><span className="text-sm text-muted-foreground">厂商*</span>
                <input className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.vendor} onChange={e => setForm(f => ({ ...f, vendor: e.target.value }))} /></label>
              <label className="block"><span className="text-sm text-muted-foreground">型号</span>
                <input className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.model} onChange={e => setForm(f => ({ ...f, model: e.target.value }))} /></label>
              <label className="block"><span className="text-sm text-muted-foreground">设备类型</span>
                <input className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.device_type} onChange={e => setForm(f => ({ ...f, device_type: e.target.value }))} /></label>
            </div>
            <label className="block"><span className="text-sm text-muted-foreground">描述</span>
              <input className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.description} onChange={e => setForm(f => ({ ...f, description: e.target.value }))} /></label>
            <div>
              <p className="text-sm text-muted-foreground mb-1">选择命令 ({form.command_ids.length})</p>
              <div className="border rounded p-2 max-h-48 overflow-auto grid grid-cols-1 gap-0.5 text-sm">
                {commands.filter(c => c.vendor === form.vendor || !form.vendor).map(c => (
                  <label key={c.id} className="flex items-center gap-2 cursor-pointer hover:bg-muted px-1 py-0.5 rounded">
                    <input type="checkbox" checked={form.command_ids.includes(c.id)} onChange={() => toggleCmd(c.id)} className="h-3.5 w-3.5" />
                    <span className="text-xs font-mono text-muted-foreground">[{c.vendor}]</span>
                    <span className="text-xs">{c.command}</span>
                    {c.description && <span className="text-xs text-muted-foreground">- {c.description}</span>}
                  </label>
                ))}
              </div>
            </div>
            <div className="flex gap-2">
              <button onClick={submit} className="bg-primary text-primary-foreground px-4 py-1.5 rounded-md text-sm">{editingId ? "更新" : "创建"}</button>
              <button onClick={reset} className="border px-4 py-1.5 rounded-md text-sm">取消</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
