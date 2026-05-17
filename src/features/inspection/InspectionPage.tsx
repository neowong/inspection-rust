import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { InspectionBatch } from "@/types";

export default function InspectionPage() {
  const [batches, setBatches] = useState<InspectionBatch[]>([]);

  useEffect(() => {
    invoke<InspectionBatch[]>("list_batches", { status: null }).then(setBatches).catch(console.error);
  }, []);

  const records = batches.flatMap(b => (b.records || []).map(r => ({ ...r, batch_name: b.name })));

  return (
    <div className="space-y-4">
      <h2 className="text-2xl font-bold">巡检记录</h2>
      <div className="border rounded-lg overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-muted"><tr>
            {["记录ID","批次","设备ID","状态","AI状态","操作"].map(h => <th key={h} className="text-left px-3 py-2">{h}</th>)}
          </tr></thead>
          <tbody>
            {records.map((r: any) => (
              <tr key={`${r.batch_id}-${r.id}`} className="border-t hover:bg-muted/50">
                <td className="px-3 py-2">{r.id}</td><td className="px-3 py-2 text-xs">{r.batch_name || `批次#${r.batch_id}`}</td>
                <td className="px-3 py-2">{r.device_id}</td><td className="px-3 py-2">{r.status}</td>
                <td className="px-3 py-2">{r.ai_status}</td>
                <td className="px-3 py-2 flex gap-1">
                  <button onClick={() => invoke("analyze_record", { recordId: r.id })} className="text-xs border rounded px-1.5 py-0.5 hover:bg-muted">AI分析</button>
                  <button onClick={() => invoke("generate_report", { recordId: r.id })} className="text-xs border rounded px-1.5 py-0.5 hover:bg-muted">生成报告</button>
                  <button onClick={() => invoke("download_report", { recordId: r.id }).then((res: any) => { if (res.success) window.alert("报告路径: " + res.path); else window.alert(res.message); })} className="text-xs border rounded px-1.5 py-0.5 hover:bg-muted">下载</button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {records.length === 0 && <p className="text-center text-muted-foreground py-8">暂无巡检记录</p>}
      </div>
    </div>
  );
}
