import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Download, Upload, FileText } from "lucide-react";

export default function OfflinePage() {
  const [imports, setImports] = useState<any[]>([]);
  const [devices, setDevices] = useState<any[]>([]);
  const [selectedIds, setSelectedIds] = useState<number[]>([]);

  useEffect(() => { invoke<any[]>("list_imports").then(setImports).catch(console.error); }, []);
  useEffect(() => { invoke("list_devices", { group: null, status: null, vendor: null }).then(setDevices).catch(console.error); }, []);

  const exportCmds = async (fmt: string) => {
    if (selectedIds.length === 0) { alert("请先选择设备"); return; }
    const res: any = await invoke("export_scripts", { deviceIds: selectedIds, format: fmt });
    if (res.success) {
      navigator.clipboard.writeText(res.content).then(() => alert("已复制到剪贴板！")).catch(() => alert("导出内容：\n" + res.content));
    }
  };

  const parseAndImport = async () => {
    const text = prompt("请粘贴巡检执行结果（JSON格式）:");
    if (!text) return;
    const filename = prompt("文件名:", "offline_result.txt") || "offline_result.txt";
    const res: any = await invoke("upload_result", { content: text, filename, batchName: null, batchId: null });
    if (res.success) {
      alert(`导入成功！批次 ${res.batch_id}，${res.record_count} 条记录`);
      invoke<any[]>("list_imports").then(setImports);
    } else {
      alert("导入失败: " + (res.error || ""));
    }
  };

  return (
    <div className="space-y-4">
      <h2 className="text-2xl font-bold">离线巡检</h2>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div className="border rounded-lg p-4 bg-card space-y-3">
          <h3 className="font-semibold flex items-center gap-2"><Download className="h-4 w-4"/>导出命令清单</h3>
          <div className="border rounded p-2 max-h-40 overflow-auto">
            {devices.map((d: any) => (
              <label key={d.id} className="flex items-center gap-2 text-sm cursor-pointer hover:bg-muted px-1 py-0.5 rounded">
                <input type="checkbox" checked={selectedIds.includes(d.id)} onChange={() => setSelectedIds(s => s.includes(d.id) ? s.filter(i=>i!==d.id) : [...s, d.id])}/>
                <span>{d.name}</span><span className="text-xs text-muted-foreground">{d.ip}</span>
              </label>
            ))}
          </div>
          <div className="flex gap-2">
            <button onClick={() => exportCmds("text")} className="text-sm border rounded px-3 py-1 hover:bg-muted">导出Text</button>
            <button onClick={() => exportCmds("json")} className="text-sm border rounded px-3 py-1 hover:bg-muted">导出JSON</button>
            <button onClick={() => exportCmds("csv")} className="text-sm border rounded px-3 py-1 hover:bg-muted">导出CSV</button>
            <button onClick={() => exportCmds("yaml")} className="text-sm border rounded px-3 py-1 hover:bg-muted">导出YAML</button>
          </div>
        </div>

        <div className="border rounded-lg p-4 bg-card space-y-3">
          <h3 className="font-semibold flex items-center gap-2"><Upload className="h-4 w-4"/>导入执行结果</h3>
          <p className="text-xs text-muted-foreground">支持 JSON 格式（包含 devices 数组），自动按 IP 匹配设备并创建巡检记录</p>
          <button onClick={parseAndImport} className="flex items-center gap-1 bg-primary text-primary-foreground px-3 py-1.5 rounded-md text-sm"><FileText className="h-4 w-4"/>粘贴并导入</button>
        </div>
      </div>

      <div>
        <h3 className="font-semibold mb-2">导入历史</h3>
        <div className="border rounded-lg overflow-hidden">
          <table className="w-full text-sm">
            <thead className="bg-muted"><tr>{["ID","文件名","模式","批次ID","日期"].map(h => <th key={h} className="text-left px-3 py-2">{h}</th>)}</tr></thead>
            <tbody>
              {imports.map((imp: any) => (
                <tr key={imp.id} className="border-t hover:bg-muted/50">
                  <td className="px-3 py-2">{imp.id}</td><td className="px-3 py-2">{imp.filename}</td>
                  <td className="px-3 py-2">{imp.mode}</td><td className="px-3 py-2">{imp.batch_id || "-"}</td>
                  <td className="px-3 py-2 text-muted-foreground text-xs">{imp.created_at}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {imports.length === 0 && <p className="text-center text-muted-foreground py-8">暂无导入记录</p>}
        </div>
      </div>
    </div>
  );
}
