import { useEffect, useState } from "react";
import { useParams, Link } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft, Brain, FileText, Download } from "lucide-react";

export default function BatchDetailPage() {
  const { id } = useParams();
  const [batch, setBatch] = useState<any>(null);

  useEffect(() => { if (id) invoke("get_batch", { batchId: parseInt(id) }).then(setBatch).catch(console.error); }, [id]);

  if (!batch) return <div className="text-muted-foreground">加载中...</div>;

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <Link to="/batches" className="text-muted-foreground hover:text-foreground"><ArrowLeft className="h-4 w-4"/></Link>
        <h2 className="text-2xl font-bold">{batch.name}</h2>
        <span className="text-xs border rounded px-1.5 py-0.5">{batch.mode}</span>
        <span className="text-sm text-muted-foreground">{batch.status}</span>
      </div>

      <div className="flex gap-2">
        <button onClick={() => invoke("analyze_batch", { batchId: batch.id })} className="flex items-center gap-1 border px-3 py-1.5 rounded-md text-sm hover:bg-muted"><Brain className="h-4 w-4"/>AI分析</button>
        <button onClick={() => invoke("generate_batch_reports", { batchId: batch.id })} className="flex items-center gap-1 border px-3 py-1.5 rounded-md text-sm hover:bg-muted"><FileText className="h-4 w-4"/>批量报告</button>
        <button onClick={() => invoke("download_batch_report", { batchId: batch.id }).then((r: any) => { if (r.success) window.alert("报告: " + r.path); })} className="flex items-center gap-1 border px-3 py-1.5 rounded-md text-sm hover:bg-muted"><Download className="h-4 w-4"/>下载综合报告</button>
      </div>

      <div className="border rounded-lg overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-muted"><tr>{["记录ID","设备ID","状态","AI状态","错误","报告","操作"].map(h => <th key={h} className="text-left px-3 py-2">{h}</th>)}</tr></thead>
          <tbody>
            {(batch.records || []).map((r: any) => (
              <tr key={r.id} className="border-t hover:bg-muted/50">
                <td className="px-3 py-2">{r.id}</td><td className="px-3 py-2">{r.device_id}</td>
                <td className="px-3 py-2">{r.status}</td><td className="px-3 py-2">{r.ai_status}</td>
                <td className="px-3 py-2 text-xs text-red-600">{r.error_message || "-"}</td>
                <td className="px-3 py-2">{r.report_path ? <span className="text-green-600 text-xs">已生成</span> : "-"}</td>
                <td className="px-3 py-2 flex gap-1">
                  <button onClick={() => invoke("analyze_record", { recordId: r.id })} className="text-xs hover:bg-muted px-1 rounded">分析</button>
                  <button onClick={() => invoke("generate_report", { recordId: r.id })} className="text-xs hover:bg-muted px-1 rounded">报告</button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
