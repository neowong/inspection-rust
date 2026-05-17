import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus, Trash2, Edit } from "lucide-react";
import type { CommandPool } from "@/types";

const VENDORS = ["H3C","华为","思科","深信服","锐捷","Linux","CentOS","Ubuntu","openEuler","MySQL","PostgreSQL","Oracle","其它"];
const CATEGORIES = ["version","clock","disk","cpu","memory","hardware","power","fan","env","interface","protocol","log","vlan","general","数据库","连接","复制","权限","性能","存储","版本","引擎"];

export default function CommandsPage() {
  const [commands, setCommands] = useState<CommandPool[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [form, setForm] = useState({ vendor: "H3C", command: "", description: "", category: "general", command_type: "ssh" });
  const [filterVendor, setFilterVendor] = useState("");

  const load = () => {
    invoke<CommandPool[]>("list_commands", { vendor: filterVendor || null }).then(setCommands).catch(console.error);
  };
  useEffect(load, [filterVendor]);

  const reset = () => { setForm({ vendor: "H3C", command: "", description: "", category: "general", command_type: "ssh" }); setEditingId(null); setShowForm(false); };
  const submit = async () => {
    await invoke(editingId ? "update_command" : "create_command", editingId ? { commandId: editingId, data: form } : { data: form });
    reset(); load();
  };
  const edit = (c: CommandPool) => {
    setForm({ vendor: c.vendor, command: c.command, description: c.description || "", category: c.category || "general", command_type: c.command_type });
    setEditingId(c.id); setShowForm(true);
  };
  const del = async (id: number) => { if (confirm("确认删除？")) { await invoke("delete_command", { commandId: id }); load(); } };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold">命令库</h2>
        <button onClick={() => setShowForm(true)} className="flex items-center gap-1 bg-primary text-primary-foreground px-3 py-1.5 rounded-md text-sm"><Plus className="h-4 w-4"/>添加命令</button>
      </div>

      <select className="border rounded px-2 py-1 text-sm" value={filterVendor} onChange={e => setFilterVendor(e.target.value)}>
        <option value="">全部厂商</option>{VENDORS.map(v => <option key={v}>{v}</option>)}
      </select>

      <div className="border rounded-lg overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-muted"><tr>{["厂商","类型","命令","描述","类别","操作"].map(h => <th key={h} className="text-left px-3 py-2">{h}</th>)}</tr></thead>
          <tbody>
            {commands.map(c => (
              <tr key={c.id} className="border-t hover:bg-muted/50">
                <td className="px-3 py-2">{c.vendor}</td>
                <td className="px-3 py-2"><span className="text-xs bg-blue-50 rounded px-1 py-0.5">{c.command_type}</span></td>
                <td className="px-3 py-2 font-mono text-xs">{c.command}</td>
                <td className="px-3 py-2 text-muted-foreground text-xs">{c.description || "-"}</td>
                <td className="px-3 py-2">{c.category || "-"}</td>
                <td className="px-3 py-2 flex gap-1">
                  <button onClick={() => edit(c)} className="p-1 hover:bg-muted rounded"><Edit className="h-3.5 w-3.5"/></button>
                  <button onClick={() => del(c.id)} className="p-1 hover:bg-muted rounded text-destructive"><Trash2 className="h-3.5 w-3.5"/></button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {commands.length === 0 && <p className="text-center text-muted-foreground py-8">暂无命令</p>}
      </div>

      {showForm && (
        <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50" onClick={reset}>
          <div className="bg-card border rounded-lg p-6 w-full max-w-md space-y-3" onClick={e => e.stopPropagation()}>
            <h3 className="font-bold text-lg">{editingId ? "编辑命令" : "添加命令"}</h3>
            <label className="block"><span className="text-sm text-muted-foreground">厂商</span>
              <select className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.vendor} onChange={e => setForm(f => ({...f, vendor: e.target.value}))}>
                {VENDORS.map(v=><option key={v}>{v}</option>)}</select></label>
            <label className="block"><span className="text-sm text-muted-foreground">命令*</span>
              <input className="w-full border rounded px-2 py-1 mt-0.5 text-sm font-mono" value={form.command} onChange={e => setForm(f => ({...f, command: e.target.value}))} /></label>
            <label className="block"><span className="text-sm text-muted-foreground">描述</span>
              <input className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.description} onChange={e => setForm(f => ({...f, description: e.target.value}))} /></label>
            <label className="block"><span className="text-sm text-muted-foreground">类别</span>
              <select className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.category} onChange={e => setForm(f=>({...f,category:e.target.value}))}>
                {CATEGORIES.map(c=><option key={c}>{c}</option>)}</select></label>
            <label className="block"><span className="text-sm text-muted-foreground">类型</span>
              <select className="w-full border rounded px-2 py-1 mt-0.5 text-sm" value={form.command_type} onChange={e => setForm(f=>({...f,command_type:e.target.value}))}>
                <option value="ssh">ssh</option><option value="db">db</option></select></label>
            <div className="flex gap-2 pt-2">
              <button onClick={submit} className="bg-primary text-primary-foreground px-4 py-1.5 rounded-md text-sm">{editingId ? "更新" : "添加"}</button>
              <button onClick={reset} className="border px-4 py-1.5 rounded-md text-sm">取消</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
