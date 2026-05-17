import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Upload, Trash2, Eye, Download } from "lucide-react";
import type { ReportTemplate } from "@/types";

export default function ReportTemplatesPage() {
  const [templates, setTemplates] = useState<ReportTemplate[]>([]);

  useEffect(() => {
    invoke<ReportTemplate[]>("list_report_templates").then(setTemplates).catch(console.error);
  }, []);

  const upload = async () => {
    const name = prompt("模板名称:");
    const path = prompt("文件路径:");
    if (name && path) {
      await invoke("upload_template", { filePath: path, name, vendor: null });
      invoke<ReportTemplate[]>("list_report_templates").then(setTemplates);
    }
  };
  const del = async (id: number) => { await invoke("delete_template", { templateId: id }); invoke<ReportTemplate[]>("list_report_templates").then(setTemplates); };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold">报告模板</h2>
        <button onClick={upload} className="flex items-center gap-1 bg-primary text-primary-foreground px-3 py-1.5 rounded-md text-sm"><Upload className="h-4 w-4"/>上传模板</button>
      </div>

      <div className="border rounded-lg overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-muted"><tr>{["名称","厂商","文件路径","创建时间","操作"].map(h => <th key={h} className="text-left px-3 py-2">{h}</th>)}</tr></thead>
          <tbody>
            {templates.map(t => (
              <tr key={t.id} className="border-t hover:bg-muted/50">
                <td className="px-3 py-2 font-medium">{t.name}</td><td className="px-3 py-2">{t.vendor || "-"}</td>
                <td className="px-3 py-2 text-xs text-muted-foreground font-mono">{t.file_path}</td>
                <td className="px-3 py-2 text-xs text-muted-foreground">{t.created_at}</td>
                <td className="px-3 py-2 flex gap-1">
                  <button onClick={() => invoke("preview_template", { templateId: t.id })} className="p-1 hover:bg-muted rounded"><Eye className="h-3.5 w-3.5"/></button>
                  <button onClick={() => invoke("download_template", { templateId: t.id }).then((r: any) => { if (r.success) window.alert("文件: " + r.path); })} className="p-1 hover:bg-muted rounded"><Download className="h-3.5 w-3.5"/></button>
                  <button onClick={() => del(t.id)} className="p-1 hover:bg-muted rounded text-destructive"><Trash2 className="h-3.5 w-3.5"/></button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {templates.length === 0 && <p className="text-center text-muted-foreground py-8">暂无报告模板</p>}
      </div>
    </div>
  );
}
