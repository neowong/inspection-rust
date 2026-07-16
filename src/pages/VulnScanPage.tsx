import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Target, BugPlay, Download, CheckCircle2, AlertTriangle, ShieldCheck, ShieldAlert, ExternalLink } from "lucide-react";
import Button from "../components/ui/Button";

export default function VulnScanPage() {
  const [cve, setCve] = useState("");
  const [ip, setIp] = useState("");
  const [port, setPort] = useState("80");
  const [verifying, setVerifying] = useState(false);
  const [result, setResult] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);
  const [nucleiReady, setNucleiReady] = useState(false);
  const [downloading, setDownloading] = useState(false);

  useEffect(() => {
    invoke<{ installed: boolean }>("get_nuclei_status")
      .then(r => setNucleiReady(r.installed)).catch(() => {});
  }, []);

  const handleVerify = async () => {
    if (!cve.trim() || !ip.trim()) { setError("请输入 CVE ID 和目标 IP"); return; }
    let raw = cve.trim().toUpperCase();
    if (!raw.startsWith("CVE-")) raw = "CVE-" + raw;
    if (!/^CVE-\d{4}-\d{4,}$/.test(raw)) {
      setError("CVE 编号格式不正确，示例：CVE-2021-23017");
      return;
    }
    if (!nucleiReady) { setError("请先安装验证引擎"); return; }
    setVerifying(true); setError(null); setResult(null);
    try {
      const r = await invoke("verify_specific_cve", {
        target: ip.trim(), cve_id: raw, port: Number(port) || 0,
      });
      setResult(r);
      setCve(raw);
    } catch (e) {
      setError(typeof e === "string" ? e : "验证请求失败");
    } finally { setVerifying(false); }
  };

  const handleDownload = async () => {
    setDownloading(true);
    try {
      await invoke("download_nuclei");
      const info = await invoke<{ installed: boolean }>("get_nuclei_status");
      setNucleiReady(info.installed);
    } catch (e) {
      setError(typeof e === "string" ? e : "下载失败");
    } finally { setDownloading(false); }
  };

  return (
    <div className="space-y-5">
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm relative">
        <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">漏洞检测</h1>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1">
          根据外部安全扫描报告的 CVE 编号，验证目标漏洞是否已修复（需安装验证引擎）
        </p>
      </div>

      {/* 引擎状态 */}
      <div className="flex items-center gap-3 px-4 py-2 rounded-lg border text-xs flex-wrap"
        style={{ borderColor: "hsl(var(--border-light))", backgroundColor: "hsl(var(--bg-card))" }}>
        <BugPlay size={14} className={nucleiReady ? "text-[hsl(var(--success))]" : "text-[hsl(var(--text-tertiary))]"} />
        <span className="text-[hsl(var(--text-secondary))]">验证引擎</span>
        {nucleiReady ? (
          <span className="text-[hsl(var(--success))] flex items-center gap-1"><CheckCircle2 size={12} /> 已就绪</span>
        ) : (
          <>
            <span className="text-[hsl(var(--warning))]">未安装</span>
            <Button size="sm" variant="secondary" onClick={handleDownload} loading={downloading}>
              <Download size={12} className="mr-1" />安装（~60MB）
            </Button>
          </>
        )}
      </div>

      {/* CVE 验证表单 */}
      <div className="rounded-xl border p-5" style={{ borderColor: "hsl(var(--border))", backgroundColor: "hsl(var(--bg-card))" }}>
        <div className="flex items-center gap-2 mb-3">
          <Target size={16} className="text-[hsl(var(--accent))]" />
          <span className="text-sm font-semibold text-[hsl(var(--text-primary))]">CVE 验证</span>
        </div>
        <p className="text-[11px] text-[hsl(var(--text-tertiary))] mb-3">
          支持简写：<code className="text-[hsl(var(--accent))] bg-[hsl(var(--accent)/0.08)] px-1 rounded">2021-23017</code> → <code className="text-[hsl(var(--accent))] bg-[hsl(var(--accent)/0.08)] px-1 rounded">CVE-2021-23017</code>，大小写不敏感
        </p>
        <div className="grid grid-cols-4 gap-3 items-end">
          <div className="col-span-1">
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">CVE 编号</label>
            <input value={cve} onChange={e => setCve(e.target.value)}
              onKeyDown={e => e.key === "Enter" && handleVerify()}
              placeholder="CVE-2021-23017" autoFocus
              className="w-full h-9 rounded-md border border-[hsl(var(--border))] bg-[hsl(var(--bg-input))] px-3 text-sm font-mono text-[hsl(var(--text-primary))] placeholder:text-[hsl(var(--text-tertiary))] focus:outline-none focus:ring-2 focus:ring-[hsl(var(--accent)/0.2)] focus:border-[hsl(var(--accent))] transition-all" />
          </div>
          <div className="col-span-1">
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">目标 IP</label>
            <input value={ip} onChange={e => setIp(e.target.value)}
              onKeyDown={e => e.key === "Enter" && handleVerify()}
              placeholder="192.168.1.1"
              className="w-full h-9 rounded-md border border-[hsl(var(--border))] bg-[hsl(var(--bg-input))] px-3 text-sm text-[hsl(var(--text-primary))] placeholder:text-[hsl(var(--text-tertiary))] focus:outline-none focus:ring-2 focus:ring-[hsl(var(--accent)/0.2)] focus:border-[hsl(var(--accent))] transition-all"
              style={{ imeMode: "disabled" as any }} />
          </div>
          <div className="col-span-1">
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">端口</label>
            <input value={port} onChange={e => setPort(e.target.value)}
              placeholder="80"
              className="w-full h-9 rounded-md border border-[hsl(var(--border))] bg-[hsl(var(--bg-input))] px-3 text-sm text-[hsl(var(--text-primary))] placeholder:text-[hsl(var(--text-tertiary))] focus:outline-none focus:ring-2 focus:ring-[hsl(var(--accent)/0.2)] focus:border-[hsl(var(--accent))] transition-all" />
          </div>
          <div className="col-span-1">
            <Button onClick={handleVerify} loading={verifying} disabled={!nucleiReady} className="w-full">
              <BugPlay size={14} className="mr-1" />验证
            </Button>
          </div>
        </div>
      </div>

      {error && (
        <div className="p-3 rounded-lg border text-xs" style={{
          backgroundColor: "hsl(var(--danger)/0.08)", borderColor: "hsl(var(--danger)/0.25)", color: "hsl(var(--danger))"
        }}>
          <AlertTriangle size={14} className="inline mr-1 align-text-top" />{error}
        </div>
      )}

      {/* 验证结果 */}
      {result && (
        <div className="rounded-xl border p-4 animate-in" style={{
          borderColor: result.found ? "hsl(var(--danger)/0.3)" : "hsl(var(--success)/0.3)",
          backgroundColor: result.found ? "hsl(var(--danger)/0.05)" : "hsl(var(--success)/0.05)"
        }}>
          <div className="flex items-center gap-2 mb-2">
            {result.found ? (
              <ShieldAlert size={22} className="text-[hsl(var(--danger))]" />
            ) : (
              <ShieldCheck size={22} className="text-[hsl(var(--success))]" />
            )}
            <div>
              <span className={`text-sm font-semibold ${result.found ? "text-[hsl(var(--danger))]" : "text-[hsl(var(--success))]"}`}>
                {result.found ? "漏洞仍存在" : "未检测到该漏洞"}
              </span>
              <p className="text-xs text-[hsl(var(--text-secondary))] mt-0.5">{result.message}</p>
            </div>
          </div>
          {result.findings?.map((f: any, i: number) => (
            <div key={i} className="flex items-start gap-2 mt-2 px-3 py-2 rounded border text-xs"
              style={{ borderColor: "hsl(var(--danger)/0.2)", backgroundColor: "hsl(var(--danger)/0.05)" }}>
              <ExternalLink size={12} className="shrink-0 mt-0.5 text-[hsl(var(--danger))]" />
              <div>
                <span className="font-semibold text-[hsl(var(--text-primary))]">{f.info?.name || f.templateID || result.cve_id}</span>
                {f.info?.severity && <span className="ml-1.5 text-[10px] px-1 py-0.5 rounded" style={{
                  backgroundColor: f.info.severity === "high" || f.info.severity === "critical" ? "hsl(var(--danger)/0.15)" : "hsl(var(--warning)/0.15)",
                  color: f.info.severity === "high" || f.info.severity === "critical" ? "hsl(var(--danger))" : "hsl(var(--warning))",
                }}>{f.info.severity.toUpperCase()}</span>}
                <p className="text-[hsl(var(--text-secondary))] mt-0.5">{f.info?.description || f.matcher_name || ""}</p>
                {f.matched && <p className="text-[hsl(var(--text-tertiary))] mt-0.5 font-mono truncate max-w-xl">{f.matched}</p>}
              </div>
            </div>
          ))}
          {!result.found && (
            <p className="text-xs text-[hsl(var(--text-tertiary))] mt-2">验证引擎未能触发该漏洞，可能已被修复、不存在或无法从网络层面检测。</p>
          )}
        </div>
      )}
    </div>
  );
}
