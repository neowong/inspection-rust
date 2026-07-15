import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Shield, ShieldCheck, ShieldAlert, ShieldOff, Search, Target, AlertTriangle,
  WifiOff, ChevronDown, ChevronRight, ExternalLink, BugPlay,
  Monitor, Globe, List, Zap, Database, Download, CheckCircle2,
} from "lucide-react";
import Button from "../components/ui/Button";

/* ── 类型 ── */
interface CveItem {
  cve_id: string;
  summary: string;
  cvss_score: number;
  severity: string;
  fix_version: string | null;
  exploit_available: boolean;
}

interface CveDetail {
  product: string;
  version: string;
  total_cves: number;
  max_cvss: number;
  cves: CveItem[];
}

interface PortBanner {
  port: number; service: string; product: string; version: string; banner: string;
}

interface VulnResult {
  ip: string;
  os_info: string;
  total_ports: number;
  total_cves: number;
  max_cvss: number;
  overall: string;
  summary: string;
  banners: PortBanner[];
  cve_details: CveDetail[];
  cve_api_ok: boolean;
  nuclei_enabled: boolean;
  nuclei_findings: any[];
}

/* ── 样式 ── */
const OVERALL_STYLES: Record<string, string> = {
  ok:       "bg-[hsl(var(--success)/0.12)] text-[hsl(var(--success))] border border-[hsl(var(--success)/0.25)]",
  info:     "bg-[hsl(var(--info)/0.12)] text-[hsl(var(--info))] border border-[hsl(var(--info)/0.25)]",
  warning:  "bg-[hsl(var(--warning)/0.12)] text-[hsl(var(--warning))] border border-[hsl(var(--warning)/0.25)]",
  critical: "bg-[hsl(var(--danger)/0.12)] text-[hsl(var(--danger))] border border-[hsl(var(--danger)/0.25)]",
};

const SEV_STYLES: Record<string, string> = {
  critical: "text-[hsl(var(--danger))] bg-[hsl(var(--danger)/0.1)] border border-[hsl(var(--danger)/0.2)]",
  high:     "text-[hsl(var(--warning))] bg-[hsl(var(--warning)/0.1)] border border-[hsl(var(--warning)/0.2)]",
  medium:   "text-[hsl(var(--info))] bg-[hsl(var(--info)/0.1)] border border-[hsl(var(--info)/0.2)]",
  low:      "text-[hsl(var(--accent))] bg-[hsl(var(--accent)/0.1)] border border-[hsl(var(--accent)/0.2)]",
};

/* 端口服务映射 */
function serviceInfo(port: number): { name: string; color: string } {
  const m: Record<number, [string, string]> = {
    22: ["SSH", "#E85D2C"], 80: ["HTTP", "#2563EB"], 443: ["HTTPS", "#2563EB"],
    21: ["FTP", "#F59E0B"], 23: ["Telnet", "#8B5CF6"], 25: ["SMTP", "#06B6D4"],
    53: ["DNS", "#10B981"], 110: ["POP3", "#EC4899"], 143: ["IMAP", "#EC4899"],
    3306: ["MySQL", "#DC9D00"], 5432: ["PostgreSQL", "#336791"],
    6379: ["Redis", "#DC382D"], 27017: ["MongoDB", "#47A248"],
    3389: ["RDP", "#A855F7"], 5900: ["VNC", "#A855F7"],
    8080: ["HTTP", "#2563EB"], 8443: ["HTTPS-Alt", "#2563EB"],
    9090: ["HTTP-Alt", "#2563EB"],
    139: ["NetBIOS", "#64748B"], 445: ["SMB", "#64748B"],
    44333: ["HTTPS-Alt", "#2563EB"],
  };
  const found = m[port];
  return found ? { name: found[0], color: found[1] } : { name: `Port ${port}`, color: "#64748B" };
}

export default function VulnScanPage() {
  const [ip, setIp] = useState("");
  const [fullScan, setFullScan] = useState(false);
  const [customPorts, setCustomPorts] = useState("");
  const [cveDbStatus, setCveDbStatus] = useState<{ count: number; db_size: number } | null>(null);
  const [downloadingDb, setDownloadingDb] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [progress, setProgress] = useState("");
  const [phase, setPhase] = useState<"" | "scan" | "cve">("");
  const [estimatedTime, setEstimatedTime] = useState(10);
  const [result, setResult] = useState<VulnResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [infoMsg, setInfoMsg] = useState<string | null>(null);
  const [expandedCves, setExpandedCves] = useState<Set<string>>(new Set());
  const [nucleiReady, setNucleiReady] = useState(false);
  const [downloadingNuclei, setDownloadingNuclei] = useState(false);
  const [showAllPorts, setShowAllPorts] = useState(false);
  const resultRef = useRef<HTMLDivElement>(null);

  // 全扫时获取预估时长 + CVE 数据库状态 + nuclei 状态
  useEffect(() => {
    invoke<number>("estimate_scan_time", { fullScan }).then(setEstimatedTime).catch(() => {});
    invoke<{ count: number; db_size: number }>("get_cve_db_info")
      .then(setCveDbStatus).catch(() => setCveDbStatus(null));
    invoke<{ installed: boolean }>("get_nuclei_status")
      .then(r => setNucleiReady(r.installed)).catch(() => {});
  }, [fullScan]);
  useEffect(() => {
    invoke<number>("estimate_scan_time", { fullScan }).then(setEstimatedTime).catch(() => {});
    invoke<{ count: number; db_size: number }>("get_cve_db_info")
      .then(setCveDbStatus).catch(() => setCveDbStatus(null));
  }, [fullScan]);

  // 检查本地 CVE 库
  const handleDownloadNuclei = async () => {
    setDownloadingNuclei(true);
    try {
      await invoke("download_nuclei");
      setNucleiReady(true);
      setInfoMsg("nuclei 安装完成，现在扫描将自动进行漏洞验证");
    } catch (e) {
      setError(typeof e === "string" ? e : "nuclei 下载失败");
    } finally {
      setDownloadingNuclei(false);
    }
  };

  const handleDownloadDb = async () => {
    setDownloadingDb(true);
    try {
      await invoke("download_cve_db");
      const info = await invoke<{ count: number; db_size: number }>("get_cve_db_info");
      setCveDbStatus(info);
    } catch (e) {
      setError(typeof e === "string" ? e : "CVE 数据库下载失败");
    } finally {
      setDownloadingDb(false);
    }
  };

  const handleScan = async () => {
    if (!ip.trim()) { setError("请输入 IP 地址"); return; }
    setScanning(true);
    setError(null);
    setResult(null);
    setPhase("scan");
    setProgress(customPorts.trim()
      ? `第一阶段：扫描指定端口（${customPorts.trim()}）...`
      : fullScan
        ? `第一阶段：全端口扫描（1-65535，预计约 ${estimatedTime} 秒）...`
        : "第一阶段：扫描常见端口（Top 88）...");
    try {
      const r = await invoke<VulnResult>("scan_ip_vulns", { ip: ip.trim(), fullScan, customPorts: customPorts.trim() });
      setPhase("cve");
      setProgress("第二阶段：查询 CVE 数据库，匹配已知漏洞...");
      // 这里不需要额外调用，scan_ip_vulns 已经包含了 CVE 查询
      setProgress("");
      setPhase("");
      setResult(r);
      setTimeout(() => resultRef.current?.scrollIntoView({ behavior: "smooth", block: "start" }), 100);
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
    } finally {
      setScanning(false);
      setProgress("");
      setPhase("");
    }
  };

  const toggleCve = (key: string) => {
    setExpandedCves(prev => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key); else next.add(key);
      return next;
    });
  };

  const OverallIcon = result?.overall === "critical" ? ShieldOff
    : result?.overall === "warning" ? ShieldAlert
    : result?.overall === "ok" ? ShieldCheck : Shield;

  const dispPorts = result?.banners.filter((_, i) => showAllPorts || i < 10) ?? [];
  const hiddenPorts = (result?.banners.length ?? 0) - dispPorts.length;

  return (
    <div className="space-y-5">
      {/* Sticky Header */}
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm relative">
        <div className="flex items-center gap-3">
          <BugPlay className="text-[hsl(var(--accent))]" size={22} />
          <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">漏洞扫描</h1>
          <span className="px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-[hsl(var(--accent)/0.12)] text-[hsl(var(--accent))] border border-[hsl(var(--accent)/0.25)]">
            CVE 匹配
          </span>
        </div>
      </div>

      {/* Goal */}
      <div className="rounded-xl border p-4" style={{
        backgroundColor: "hsl(var(--info)/0.06)", borderColor: "hsl(var(--info)/0.2)",
      }}>
        <div className="flex items-start gap-3">
          <Target size={18} className="text-[hsl(var(--info))] shrink-0 mt-0.5" />
          <div className="text-sm text-[hsl(var(--text-secondary))] leading-relaxed">
            <span className="font-semibold text-[hsl(var(--text-primary))]">功能定位：</span>
            全端口扫描 → 服务版本识别 → CVE 数据库匹配已知漏洞。支持安装验证引擎后对目标发送探测包，确认漏洞真实存在。<br />
            <span className="text-xs text-[hsl(var(--text-tertiary))] mt-2 block space-y-0.5">
              <span className="flex items-center gap-1.5"><Database size={12} /> 优先使用本地 CVE 库，查不到时自动联网查询</span>
              <span className="flex items-center gap-1.5">
                {nucleiReady ? <CheckCircle2 size={12} className="text-[hsl(var(--success))]" /> : <Zap size={12} />}
                <span>{nucleiReady ? "漏洞验证引擎已就绪，将发探测包验证漏洞真实性" : "基于版本号匹配（如需验证漏洞真实性，请下载验证引擎）"}</span>
              </span>
              <span className="flex items-center gap-1.5"><Zap size={12} /> 两阶段扫描：端口扫描 → CVE 匹配。快速模式 Top 88，全端口模式 1-65535</span>
            </span>
          </div>
        </div>
      </div>

      {/* 资源状态栏：CVE 库 + 漏洞验证引擎 */}
      <div className="flex items-center gap-3 px-4 py-2 rounded-lg border text-xs flex-wrap"
        style={{ borderColor: "hsl(var(--border-light))", backgroundColor: "hsl(var(--bg-card))" }}>
        {/* CVE 库 */}
        <span className="flex items-center gap-1.5">
          <Database size={12} className="text-[hsl(var(--text-tertiary))]" />
          <span className="text-[hsl(var(--text-secondary))] mr-1">CVE 库</span>
          {cveDbStatus && cveDbStatus.count > 100 ? (
            <span className="text-[hsl(var(--success))] flex items-center gap-1"><CheckCircle2 size={10} /> {cveDbStatus.count.toLocaleString()} 条</span>
          ) : (
            <>
              <span className="text-[hsl(var(--warning))]">未下载</span>
              <Button size="sm" variant="secondary" onClick={handleDownloadDb} loading={downloadingDb} className="text-[10px] px-1.5 py-0.5">
                <Download size={10} className="mr-0.5" />下载
              </Button>
            </>
          )}
        </span>
        <span className="text-[hsl(var(--text-tertiary))]">|</span>
        {/* 漏洞验证引擎 */}
        <span className="flex items-center gap-1.5">
          <BugPlay size={12} className={nucleiReady ? "text-[hsl(var(--success))]" : "text-[hsl(var(--text-tertiary))]"} />
          <span className="text-[hsl(var(--text-secondary))] mr-1">验证引擎</span>
          {nucleiReady ? (
            <span className="text-[hsl(var(--success))] flex items-center gap-1"><CheckCircle2 size={10} /> 已就绪</span>
          ) : (
            <>
              <span className="text-[hsl(var(--warning))]">未安装</span>
              <Button size="sm" variant="secondary" onClick={handleDownloadNuclei} loading={downloadingNuclei} className="text-[10px] px-1.5 py-0.5">
                <Download size={10} className="mr-0.5" />安装（~60MB）
              </Button>
            </>
          )}
        </span>
      </div>

      {/* 扫描区 */}
      <div className="flex flex-wrap gap-3 items-end">
        <div className="min-w-[160px] max-w-[200px]">
          <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">目标 IP</label>
          <input value={ip} onChange={e => setIp(e.target.value)}
            onKeyDown={e => e.key === "Enter" && handleScan()}
            placeholder="192.168.1.1"
            className="w-full h-9 rounded-md border border-[hsl(var(--border))] bg-[hsl(var(--bg-card))] px-3 text-sm text-[hsl(var(--text-primary))] placeholder:text-[hsl(var(--text-tertiary))] focus:outline-none focus:ring-2 focus:ring-[hsl(var(--accent)/0.2)] focus:border-[hsl(var(--accent))] transition-all"
            style={{ imeMode: "disabled" as any }}
          />
        </div>
        <div className="min-w-[200px] max-w-[280px]">
          <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">指定端口（可选，逗号/空格/横杠分隔）</label>
          <input value={customPorts} onChange={e => setCustomPorts(e.target.value)}
            placeholder="22,80,443,8080-8090"
            className="w-full h-9 rounded-md border border-[hsl(var(--border))] bg-[hsl(var(--bg-card))] px-3 text-xs font-mono text-[hsl(var(--text-primary))] placeholder:text-[hsl(var(--text-tertiary))] focus:outline-none focus:ring-2 focus:ring-[hsl(var(--accent)/0.2)] focus:border-[hsl(var(--accent))] transition-all"
            style={{ imeMode: "disabled" as any }}
          />
        </div>
        <div className="flex items-center gap-2 mb-1">
          <label className="flex items-center gap-1.5 text-xs cursor-pointer select-none group"
            style={{ color: fullScan ? "hsl(var(--accent))" : "hsl(var(--text-secondary))" }}>
            <input type="checkbox" checked={fullScan} onChange={e => { setFullScan(e.target.checked); if (e.target.checked) setCustomPorts(""); }}
              className="rounded border-[hsl(var(--border))] text-[hsl(var(--accent))] focus:ring-[hsl(var(--accent))]" />
            全端口 1-65535
            {fullScan && (
              <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-[hsl(var(--warning)/0.1)] text-[hsl(var(--warning))] border border-[hsl(var(--warning)/0.2)] ml-1">
                ~{estimatedTime}s
              </span>
            )}
          </label>
          <Button onClick={handleScan} loading={scanning}>
            <Search size={14} className="mr-1" />开始扫描
          </Button>
        </div>
      </div>

      {progress && (
        <div className="flex items-center gap-3 text-xs" style={{ color: "hsl(var(--text-tertiary))" }}>
          {/* 阶段指示 */}
          <div className="flex items-center gap-1.5">
            <span className={`w-2 h-2 rounded-full ${phase === "scan" ? "bg-[hsl(var(--accent))] animate-pulse-dot" : phase === "cve" ? "bg-[hsl(var(--success))]" : "bg-[hsl(var(--border))]"}`} />
            <span className={phase === "scan" ? "text-[hsl(var(--accent))] font-medium" : ""}>端口扫描</span>
          </div>
          <div className="w-4 h-px" style={{ backgroundColor: "hsl(var(--border))" }} />
          <div className="flex items-center gap-1.5">
            <span className={`w-2 h-2 rounded-full ${phase === "cve" ? "bg-[hsl(var(--accent))] animate-pulse-dot" : "bg-[hsl(var(--border))]"}`} />
            <span className={phase === "cve" ? "text-[hsl(var(--accent))] font-medium" : ""}>CVE 匹配</span>
          </div>
          <span className="ml-1">{progress}</span>
        </div>
      )}

      {error && (
        <div className="p-3 rounded-lg border text-xs" style={{
          backgroundColor: "hsl(var(--danger)/0.08)", borderColor: "hsl(var(--danger)/0.25)", color: "hsl(var(--danger))"
        }}>
          <AlertTriangle size={14} className="inline mr-1 align-text-top" />{error}
        </div>
      )}
      {infoMsg && (
        <div className="p-3 rounded-lg border text-xs" style={{
          backgroundColor: "hsl(var(--info)/0.08)", borderColor: "hsl(var(--info)/0.25)", color: "hsl(var(--info))"
        }}>
          <AlertTriangle size={14} className="inline mr-1 align-text-top" />{infoMsg}
          <button onClick={() => setInfoMsg(null)} className="float-right text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-primary))]">✕</button>
        </div>
      )}

      {/* 扫描中动画 */}
      {scanning && (
        <div className="flex flex-col items-center justify-center py-16 border border-dashed border-[hsl(var(--border-light))] rounded-xl">
          <div className="relative mb-4">
            <div className="w-16 h-16 rounded-full border-2 border-[hsl(var(--accent)/0.15)] flex items-center justify-center animate-spin" style={{ animationDuration: "3s" }}>
              <div className="w-10 h-10 rounded-full border-2 border-[hsl(var(--accent)/0.25)] flex items-center justify-center animate-spin" style={{ animationDuration: "2s", animationDirection: "reverse" }}>
                <Search size={16} className="text-[hsl(var(--accent))]" />
              </div>
            </div>
          </div>
          <p className="text-sm text-[hsl(var(--accent))] font-medium">正在扫描端口并查询 CVE 数据库</p>
          <div className="flex gap-1 mt-2">
            {[0,1,2].map(i => (
              <div key={i} className="w-6 h-1 rounded-full bg-[hsl(var(--accent)/0.3)]"
                style={{ animation: `pulse-bar 1.5s ease-in-out ${i * 0.3}s infinite` }} />
            ))}
          </div>
        </div>
      )}

      {/* 结果区 */}
      {result && (
        <div ref={resultRef} className="space-y-4 animate-in">
          {/* 总体状态 */}
          <div className={`rounded-xl border p-4 ${OVERALL_STYLES[result.overall] || OVERALL_STYLES.info}`}>
            <div className="flex items-center gap-2 mb-2">
              <OverallIcon size={20} />
              <span className="text-sm font-semibold">
                {result.overall === "critical" ? "存在严重漏洞"
                  : result.overall === "warning" ? "存在高危漏洞"
                  : result.overall === "info" ? "存在中低危漏洞"
                  : result.total_ports === 0 ? "未发现开放端口"
                  : "未发现已知漏洞"}
              </span>
            </div>
            <p className="text-sm text-[hsl(var(--text-secondary))]">{result.summary}</p>
            <div className="flex flex-wrap items-center gap-x-3 gap-y-1 mt-2 text-xs text-[hsl(var(--text-tertiary))]">
              <span className="flex items-center gap-1"><Globe size={12} />{result.ip}</span>
              <span>·</span>
              <span className="flex items-center gap-1"><Monitor size={12} />{result.os_info}</span>
              <span>·</span>
              <span><List size={12} className="inline mr-0.5" />开放端口 {result.total_ports}</span>
              <span>·</span>
              <span>CVE 数量 {result.total_cves}</span>
              {result.max_cvss > 0 && <><span>·</span><span>最高 CVSS {result.max_cvss.toFixed(1)}</span></>}
            </div>
          </div>

          {/* 端口与服务 */}
          {result.banners.length > 0 && (
            <div>
              <h3 className="text-xs font-semibold text-[hsl(var(--text-tertiary))] uppercase tracking-wider mb-2">
                开放端口 ({result.total_ports})
              </h3>
              <div className="flex flex-wrap gap-1.5">
                {dispPorts.map((b, i) => {
                  const info = serviceInfo(b.port);
                  return (
                    <div key={i} className="group relative px-2.5 py-1.5 rounded-lg border text-xs flex items-center gap-1.5 transition-colors hover:bg-[hsl(var(--bg-hover))]"
                      style={{ borderColor: "hsl(var(--border))" }}>
                      <span className="font-mono font-semibold" style={{ color: info.color }}>{b.port}</span>
                      <span className="text-[hsl(var(--text-secondary))]">{info.name}</span>
                      {b.product && <span className="text-[hsl(var(--text-tertiary))] hidden sm:inline">{b.product} {b.version}</span>}
                      {b.banner && (
                        <span className="absolute bottom-full left-0 mb-1 px-2 py-1 rounded text-[10px] text-[hsl(var(--text-primary))] bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-10 shadow-sm">
                          {b.banner}
                        </span>
                      )}
                    </div>
                  );
                })}
                {hiddenPorts > 0 && (
                  <button onClick={() => setShowAllPorts(!showAllPorts)}
                    className="px-2.5 py-1.5 rounded-lg border border-dashed text-xs text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--accent))] transition-colors">
                    {showAllPorts ? "收起" : `+${hiddenPorts} 个`}
                  </button>
                )}
              </div>
            </div>
          )}

          {/* CVE 详情 */}
          {result.cve_details.length > 0 && (
            <div>
              <h3 className="text-xs font-semibold text-[hsl(var(--text-tertiary))] uppercase tracking-wider mb-2">
                漏洞详情
              </h3>
              <div className="space-y-2">
                {result.cve_details.map((detail) => (
                  <div key={detail.product + detail.version} className="border border-[hsl(var(--border))] rounded-xl overflow-hidden">
                    <button onClick={() => toggleCve(detail.product + detail.version)}
                      className="w-full flex items-center gap-2 px-4 py-2.5 text-left hover:bg-[hsl(var(--bg-hover))] transition-colors">
                      {expandedCves.has(detail.product + detail.version)
                        ? <ChevronDown size={14} className="shrink-0 text-[hsl(var(--text-tertiary))]" />
                        : <ChevronRight size={14} className="shrink-0 text-[hsl(var(--text-tertiary))]" />}
                      <span className="text-sm font-medium text-[hsl(var(--text-primary))]">{detail.product}</span>
                      <span className="text-xs font-mono text-[hsl(var(--text-tertiary))]">{detail.version}</span>
                      <div className="flex gap-1 ml-auto">
                        {detail.total_cves > 0 ? (
                          <>
                            {detail.max_cvss >= 9.0
                              ? <span className="text-[10px] px-1.5 py-0.5 rounded font-medium text-white" style={{ backgroundColor: "hsl(var(--danger))" }}>CRITICAL</span>
                              : detail.max_cvss >= 7.0
                              ? <span className="text-[10px] px-1.5 py-0.5 rounded font-medium text-white" style={{ backgroundColor: "hsl(var(--warning))" }}>HIGH</span>
                              : <span className="text-[10px] px-1.5 py-0.5 rounded font-medium" style={{ backgroundColor: "hsl(var(--info)/0.15)", color: "hsl(var(--info))" }}>MEDIUM</span>}
                            <span className="text-[10px] px-1.5 py-0.5 rounded font-medium bg-[hsl(var(--bg-hover))] text-[hsl(var(--text-secondary))]">{detail.total_cves} CVEs</span>
                          </>
                        ) : (
                          <span className="text-[10px] px-1.5 py-0.5 rounded font-medium text-[hsl(var(--success))]" style={{ backgroundColor: "hsl(var(--success)/0.1)" }}>无已知漏洞</span>
                        )}
                      </div>
                    </button>
                    {expandedCves.has(detail.product + detail.version) && detail.cves.length > 0 && (
                      <div className="border-t border-[hsl(var(--border-light))] divide-y divide-[hsl(var(--border-light))]">
                        {detail.cves.map((cve) => (
                          <div key={cve.cve_id} className="px-4 py-2.5">
                            <div className="flex items-start gap-2">
                              <div className={`text-[10px] px-1.5 py-0.5 rounded font-medium shrink-0 mt-0.5 ${SEV_STYLES[cve.severity] || ""}`}>
                                {cve.cvss_score.toFixed(1)}
                              </div>
                              <div className="flex-1 min-w-0">
                                <div className="flex items-center gap-1.5 flex-wrap">
                                  <a href={`https://cve.mitre.org/cgi-bin/cvename.cgi?name=${cve.cve_id}`}
                                    target="_blank" rel="noopener noreferrer"
                                    className="text-xs font-mono font-semibold text-[hsl(var(--accent))] hover:underline inline-flex items-center gap-1">
                                    {cve.cve_id}<ExternalLink size={10} />
                                  </a>
                                  {cve.exploit_available && (
                                    <span className="text-[10px] px-1 py-0.5 rounded font-medium"
                                      style={{ backgroundColor: "hsl(var(--danger)/0.1)", color: "hsl(var(--danger))" }}>EXP 已公开</span>
                                  )}
                                  {cve.fix_version && (
                                    <span className="text-[10px] px-1 py-0.5 rounded font-medium"
                                      style={{ backgroundColor: "hsl(var(--success)/0.1)", color: "hsl(var(--success))" }}>
                                      修复版本: {cve.fix_version}
                                    </span>
                                  )}
                                </div>
                                <p className="text-xs text-[hsl(var(--text-secondary))] mt-0.5 leading-relaxed">{cve.summary}</p>
                              </div>
                            </div>
                          </div>
                        ))}
                      </div>
                    )}
                    {expandedCves.has(detail.product + detail.version) && detail.cves.length === 0 && (
                      <div className="px-4 py-3 text-xs text-[hsl(var(--text-tertiary))] border-t border-[hsl(var(--border-light))]">
                        该产品版本未匹配到已知 CVE
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* 空结果 */}
          {result.total_cves === 0 && !result.cve_api_ok && (
            <div className="text-center py-8 text-sm border border-dashed rounded-xl"
              style={{ borderColor: "hsl(var(--warning)/0.3)", color: "hsl(var(--text-secondary))" }}>
              <WifiOff size={32} className="mx-auto mb-2 text-[hsl(var(--warning))] opacity-50" />
              <p className="font-medium text-[hsl(var(--warning))]">CVE 数据库不可达</p>
              <p className="text-xs mt-1">当前环境无网络连接，仅显示端口扫描结果</p>
              <p className="text-xs mt-0.5">请检查网络后重新扫描</p>
            </div>
          )}
          {result.total_cves === 0 && result.cve_api_ok && (
            <div className="text-center py-8 text-sm text-[hsl(var(--text-tertiary))] border border-dashed border-[hsl(var(--border-light))] rounded-xl">
              <ShieldCheck size={32} className="mx-auto mb-2 text-[hsl(var(--success))] opacity-50" />
              <p>未发现已知 CVE 漏洞</p>
              <p className="text-xs mt-1">扫描服务版本均为安全版本，未匹配到公开漏洞</p>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
