import { useState, useEffect, useRef } from "react";
import { CheckCircle2, XCircle, Plug, Loader2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { Select, SpinInput } from "../components/ui/Input";

const TOOLS = [
  { key: "subnet", label: "子网计算器" },
  { key: "scanner", label: "存活扫描" },
  { key: "port", label: "端口扫描" },
  { key: "trace", label: "路由跟踪" },
  { key: "web", label: "WEB检测" },
  { key: "snmp", label: "SNMP" },
  { key: "tftp", label: "TFTP 服务" },
  { key: "syslog", label: "Syslog" },
] as const;

type ToolKey = (typeof TOOLS)[number]["key"];

// ---- Types ------------------------------------------------------------------

interface LiveHostResult {
  ip: string;
  alive: boolean;
  response_time_ms: number | null;
}

interface PortScanResult {
  port: number;
  open: boolean;
  service: string;
}

interface WebCheckResult {
  url: string;
  final_url: string;
  status_code: number | null;
  response_time_ms: number;
  error: string | null;
  content_type: string | null;
  content_length: number | null;
}

interface SnmpResult {
  oid: string;
  value: string | null;
  value_type: string | null;
  error: string | null;
  response_time_ms: number;
  raw_hex: string | null;
}

// ---- Subnet Calculator ------------------------------------------------------

interface SubnetResult {
  ipInt: number;
  maskInt: number;
  network: number[];
  broadcast: number[];
  firstHost: number[];
  lastHost: number[];
  hostCount: string;
  subnetMask: number[];
  wildcard: number[];
  cidr: number;
  ipBinary: string;
  maskBinary: string;
}

function ipToInt(octets: number[]): number {
  const [a = 0, b = 0, c = 0, d = 0] = octets;
  return ((a << 24) | (b << 16) | (c << 8) | d) >>> 0;
}

function intToIp(n: number): number[] {
  return [(n >>> 24) & 0xff, (n >>> 16) & 0xff, (n >>> 8) & 0xff, n & 0xff];
}

function ipToBinary(n: number): string {
  return ((n >>> 24) & 0xff).toString(2).padStart(8, "0") +
    "." + ((n >>> 16) & 0xff).toString(2).padStart(8, "0") +
    "." + ((n >>> 8) & 0xff).toString(2).padStart(8, "0") +
    "." + (n & 0xff).toString(2).padStart(8, "0");
}

// ── IPv6 helpers ──

/** 展开简写 IPv6 地址为完整 8 组 hex，返回 BigInt */
function ipv6ToBigInt(addr: string): bigint | null {
  try {
    addr = addr.trim();
    if (addr.includes("::")) {
      const [left, right] = addr.split("::");
      const leftGroups = left ? left.split(":").filter(Boolean) : [];
      const rightGroups = right ? right.split(":").filter(Boolean) : [];
      const missing = 8 - leftGroups.length - rightGroups.length;
      if (missing < 0) return null;
      const middle = Array(missing).fill("0");
      addr = [...leftGroups, ...middle, ...rightGroups].join(":");
    }
    const groups = addr.split(":");
    if (groups.length !== 8) return null;
    let result = 0n;
    for (const g of groups) {
      const val = parseInt(g, 16);
      if (isNaN(val) || val < 0 || val > 0xffff) return null;
      result = (result << 16n) | BigInt(val);
    }
    return result;
  } catch { return null; }
}

/** 128-bit BigInt → 简写格式 IPv6 字符串 */
function bigIntToIpv6(n: bigint): string {
  const groups: number[] = [];
  for (let i = 0; i < 8; i++) {
    groups.unshift(Number(n & 0xffffn));
    n >>= 16n;
  }
  // 找最长的连续零段做 ::
  let bestStart = -1, bestLen = 0;
  for (let i = 0; i < 8; i++) {
    if (groups[i] === 0) {
      let j = i;
      while (j < 8 && groups[j] === 0) j++;
      if (j - i > bestLen) { bestStart = i; bestLen = j - i; }
      i = j;
    }
  }
  if (bestLen >= 2) {
    const parts = groups.map(g => g.toString(16));
    return [...parts.slice(0, bestStart), "", ...parts.slice(bestStart + bestLen)].join(":");
  }
  return groups.map(g => g.toString(16)).join(":");
}

function calcSubnet(ip: string, cidrStr: string): SubnetResult | null {
  const cidr = parseInt(cidrStr, 10);
  if (isNaN(cidr) || cidr < 0) return null;

  const ipTrimmed = ip.trim();

  // ── IPv4 ──
  if (ipTrimmed.includes(".") && !ipTrimmed.includes(":")) {
    if (cidr > 32) return null;
    const parts = ipTrimmed.split(".").map(Number);
    if (parts.length !== 4 || parts.some(p => isNaN(p) || p < 0 || p > 255)) return null;
    const ipInt = ipToInt(parts);
    const maskInt = cidr === 0 ? 0 : (~0 << (32 - cidr)) >>> 0;
    const wildcardInt = ~maskInt >>> 0;
    const networkInt = (ipInt & maskInt) >>> 0;
    const broadcastInt = (networkInt | wildcardInt) >>> 0;
    let firstHost: number[], lastHost: number[], hostCount: string;
    if (cidr >= 31) {
      firstHost = [0, 0, 0, 0]; lastHost = [0, 0, 0, 0];
      hostCount = cidr === 32 ? "1 (单主机)" : "2 (点对点链路)";
    } else {
      firstHost = intToIp(networkInt + 1);
      lastHost = intToIp(broadcastInt - 1);
      hostCount = (Math.pow(2, 32 - cidr) - 2).toLocaleString();
    }
    return {
      ipInt, maskInt,
      network: intToIp(networkInt),
      broadcast: intToIp(broadcastInt),
      firstHost, lastHost, hostCount,
      subnetMask: intToIp(maskInt),
      wildcard: intToIp(wildcardInt),
      cidr,
      ipBinary: ipToBinary(ipInt),
      maskBinary: ipToBinary(maskInt),
    };
  }

  // ── IPv6 ──
  if (ipTrimmed.includes(":")) {
    if (cidr > 128) return null;
    const ipBig = ipv6ToBigInt(ipTrimmed);
    if (ipBig === null) return null;
    const maskBig = cidr === 0 ? 0n : (~0n << BigInt(128 - cidr)) & ((1n << 128n) - 1n);
    const networkBig = ipBig & maskBig;
    const lastBig = networkBig | (~maskBig & ((1n << 128n) - 1n));

    let hostCount: string;
    if (128 - cidr > 52) {
      hostCount = "≈ " + (2n ** BigInt(128 - cidr)).toLocaleString();
    } else {
      hostCount = (2n ** BigInt(128 - cidr)).toLocaleString();
    }

    return {
      network: bigIntToIpv6(networkBig).split(":") as any,
      broadcast: bigIntToIpv6(lastBig).split(":") as any,
      firstHost: cidr >= 127 ? ["—"] : bigIntToIpv6(networkBig + 1n).split(":") as any,
      lastHost: cidr >= 127 ? ["—"] : bigIntToIpv6(lastBig - 1n).split(":") as any,
      hostCount,
      subnetMask: bigIntToIpv6(maskBig).split(":") as any,
      wildcard: bigIntToIpv6(~maskBig & ((1n << 128n) - 1n)).split(":") as any,
      cidr,
      ipInt: 0, maskInt: 0,
      ipBinary: "", maskBinary: "",
    };
  }

  return null;
}

function SubnetCalc() {
  const [ip, setIp] = useState("");
  const [cidr, setCidr] = useState("");
  const [result, setResult] = useState<SubnetResult | null>(null);
  const [error, setError] = useState("");

  const isV6 = ip.trim().includes(":");
  const maxCidr = isV6 ? 128 : 32;

  const handleCalc = () => {
    setError("");
    const r = calcSubnet(ip.trim(), cidr.trim());
    if (!r) { setError(`请输入有效的 IP 地址和 CIDR 前缀（IPv4: 0-32 / IPv6: 0-128）`); setResult(null); return; }
    setResult(r);
  };

  const gridClass = "grid grid-cols-2 gap-x-6 gap-y-2 text-sm";
  const fmt = (arr: string[] | number[]) => isV6 ? arr.join(":") : arr.join(".");

  return (
    <div className="space-y-6">
      <div className="flex items-end gap-3 flex-wrap">
        <div>
          <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">IP 地址</label>
          <input
            type="text" placeholder="192.168.1.0 或 2001:db8::1"
            value={ip} onChange={e => setIp(e.target.value)}
            onKeyDown={e => e.key === "Enter" && handleCalc()}
            className="w-72 px-3 py-2 rounded-lg border border-[hsl(var(--border))] bg-[hsl(var(--bg-input))] text-sm focus:outline-none focus:ring-2 focus:ring-[hsl(var(--accent)_/_0.4)]"
            style={{ imeMode: "disabled" }}
          />
        </div>
        <span className="text-lg font-bold text-[hsl(var(--text-secondary))] pb-2">/</span>
        <div>
          <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">CIDR</label>
          <input
            type="number" min={0} max={maxCidr} placeholder={isV6 ? "64" : "24"}
            value={cidr} onChange={e => setCidr(e.target.value)}
            onKeyDown={e => e.key === "Enter" && handleCalc()}
            className="w-20 px-3 py-2 rounded-lg border border-[hsl(var(--border))] bg-[hsl(var(--bg-input))] text-sm focus:outline-none focus:ring-2 focus:ring-[hsl(var(--accent)_/_0.4)]"
          />
        </div>
        <button
          onClick={handleCalc}
          className="px-5 py-2 rounded-lg text-sm font-medium text-white bg-[hsl(var(--accent))] hover:opacity-90 transition-opacity"
        >
          计算
        </button>
      </div>

      {error && <p className="text-sm text-[hsl(var(--danger))]">{error}</p>}

      {result && (
        <div className="space-y-4">
          <div className={gridClass}>
            <div className="text-[hsl(var(--text-secondary))]">网络地址</div>
            <div className="font-mono font-semibold break-all">{fmt(result.network)}</div>
            {!isV6 && <><div className="text-[hsl(var(--text-secondary))]">广播地址</div>
            <div className="font-mono font-semibold">{fmt(result.broadcast)}</div></>}
            <div className="text-[hsl(var(--text-secondary))]">可用范围</div>
            <div className="font-mono break-all">{fmt(result.firstHost)} — {fmt(result.lastHost)}</div>
            <div className="text-[hsl(var(--text-secondary))]">{isV6 ? "地址总数" : "可用主机数"}</div>
            <div className="font-mono">{result.hostCount}</div>
            <div className="text-[hsl(var(--text-secondary))]">子网掩码</div>
            <div className="font-mono break-all">{fmt(result.subnetMask)}</div>
            {!isV6 && <><div className="text-[hsl(var(--text-secondary))]">反掩码 (通配符)</div>
            <div className="font-mono">{fmt(result.wildcard)}</div></>}
          </div>

          {!isV6 && <details className="text-xs">
            <summary className="cursor-pointer font-medium text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))] transition-colors">
              二进制
            </summary>
            <div className="mt-2 space-y-1 font-mono text-xs">
              <div><span className="text-[hsl(var(--text-tertiary))]">IP:　　</span>{result.ipBinary}</div>
              <div><span className="text-[hsl(var(--text-tertiary))]">掩码:　</span>{result.maskBinary}</div>
              <div className="pt-1 text-[hsl(var(--text-tertiary))]">
                {result.ipBinary.split("").map((ch, i) => (
                  <span key={i} className={ch === "1" && result.maskBinary[i] === "1" ? "text-[hsl(var(--success))]" : ""}>{ch}</span>
                ))}
                <span className="ml-2">← 网络位</span>
              </div>
            </div>
          </details>
          }
        </div>
      )}
    </div>
  );
}

// ---- Shared styles ----------------------------------------------------------

const inputClass = "px-3 py-2 rounded-lg border border-[hsl(var(--border))] bg-[hsl(var(--bg-input))] text-sm focus:outline-none focus:ring-2 focus:ring-[hsl(var(--accent)_/_0.4)]";
const btnClass = "px-5 py-2 rounded-lg text-sm font-medium text-white bg-[hsl(var(--accent))] hover:opacity-90 transition-opacity disabled:opacity-50 disabled:cursor-not-allowed";
const labelClass = "block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1";

// ---- Alive Scanner ----------------------------------------------------------

function LiveScanner() {
  const [subnet, setSubnet] = useState("192.168.1.0/24");
  const [timeout, setTimeout_] = useState("2000");
  const [scanning, setScanning] = useState(false);
  const [results, setResults] = useState<LiveHostResult[] | null>(null);
  const [error, setError] = useState("");
  // 保存当前监听器的 unlisten，组件卸载时清理，防止事件监听器泄漏
  const unlistenRef = useRef<(() => void) | null>(null);
  // 组件卸载时若仍有未结束的扫描，清理监听器，避免向已卸载组件 setState
  useEffect(() => () => { unlistenRef.current?.(); }, []);

  const handleScan = async () => {
    setError("");
    setResults([]);  // 先清空，准备接收实时结果
    setScanning(true);

    // 监听实时结果事件——每扫到一个 IP 就追加到列表
    const unlisten = await listen<LiveHostResult>("live-scan-result", (event) => {
      setResults(prev => [...(prev ?? []), event.payload]);
    });
    unlistenRef.current = unlisten;

    try {
      const data = await invoke<LiveHostResult[]>("scan_live_hosts", {
        subnet: subnet.trim(),
        timeoutMs: parseInt(timeout, 10) || 2000,
      });
      // 扫描完成后用排序结果替换
      setResults(data);
    } catch (e: any) {
      setError(typeof e === "string" ? e : e?.message || String(e));
    } finally {
      unlisten();
      unlistenRef.current = null;
      setScanning(false);
    }
  };

  const alive = results?.filter(r => r.alive) ?? [];
  const dead = results?.filter(r => !r.alive) ?? [];

  return (
    <div className="space-y-6">
      <p className="text-xs text-[hsl(var(--text-tertiary))]">基于系统 ping + TCP 后备(135/445)探测，结果仅供参考。</p>
      <div className="flex items-end gap-3 flex-wrap">
        <div>
          <label className={labelClass}>网段 (CIDR)</label>
          <input
            type="text" placeholder="192.168.1.0/24"
            value={subnet} onChange={e => setSubnet(e.target.value)}
            onKeyDown={e => e.key === "Enter" && handleScan()}
            className={`w-44 ${inputClass}`}
            style={{ imeMode: "disabled" }}
          />
        </div>
        <div>
          <label className={labelClass}>超时 (ms)</label>
          <SpinInput
            min={500} max={10000} step={500}
            value={timeout} onChange={(v) => setTimeout_(String(v))}
            className="w-24"
          />
        </div>
        <button onClick={handleScan} disabled={scanning} className={btnClass}>
          {scanning ? `扫描中... (${results?.length ?? 0})` : "开始扫描"}
        </button>
      </div>

      {scanning && (
        <div className="flex items-center gap-2 text-sm text-[hsl(var(--text-secondary))]">
          <Loader2 size={16} className="animate-spin" />
          已发现 {results?.filter(r => r.alive).length ?? 0} 个在线 / 扫描 {results?.length ?? 0} 个 ...
        </div>
      )}

      {error && <p className="text-sm text-[hsl(var(--danger))]">{error}</p>}

      {results && (
        <div className="space-y-3">
          <div className="flex gap-4 text-sm">
            <span className="flex items-center gap-1.5">
              <span className="w-3 h-3 rounded-full bg-[hsl(var(--success))]" />
              存活 <span className="font-semibold">{alive.length}</span>
            </span>
            <span className="flex items-center gap-1.5">
              <span className="w-3 h-3 rounded-full bg-[hsl(var(--text-tertiary))]" />
              离线 <span className="font-semibold">{dead.length}</span>
            </span>
            <span className="text-[hsl(var(--text-tertiary))]">共 {results.length} 个 IP</span>
          </div>

          {alive.length > 0 && (
            <div className="overflow-hidden rounded-lg border border-[hsl(var(--border))]">
              <table className="w-full text-sm">
                <thead>
                  <tr className="bg-[hsl(var(--muted))] text-left">
                    <th className="px-4 py-2 font-medium text-[hsl(var(--text-secondary))]">IP 地址</th>
                    <th className="px-4 py-2 font-medium text-[hsl(var(--text-secondary))]">响应时间</th>
                    <th className="px-4 py-2 font-medium text-[hsl(var(--text-secondary))]">状态</th>
                  </tr>
                </thead>
                <tbody>
                  {alive.map(r => (
                    <tr key={r.ip} className="border-t border-[hsl(var(--border))]">
                      <td className="px-4 py-2 font-mono">{r.ip}</td>
                      <td className="px-4 py-2 font-mono text-[hsl(var(--text-secondary))]">
                        {r.response_time_ms != null ? `${r.response_time_ms.toFixed(1)} ms` : "-"}
                      </td>
                      <td className="px-4 py-2">
                        <span className="inline-flex items-center gap-1 text-[hsl(var(--success))]">
                          <CheckCircle2 size={14} /> 在线
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {dead.length > 0 && (
            <details className="text-xs">
              <summary className="cursor-pointer text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-secondary))]">
                离线主机 ({dead.length})
              </summary>
              <div className="mt-2 flex flex-wrap gap-1.5">
                {dead.map(r => (
                  <span key={r.ip} className="px-2 py-0.5 rounded bg-[hsl(var(--muted))] font-mono text-[hsl(var(--text-tertiary))]">
                    {r.ip}
                  </span>
                ))}
              </div>
            </details>
          )}
        </div>
      )}
    </div>
  );
}

// ---- Port Scanner -----------------------------------------------------------

const TCP_PORT_PRESETS: Record<string, string> = {
  "常用端口": "22,80,443,8080,8443",
  "Web 端口": "80,443,8080,8443,9090,3000,5000,8000",
  "数据库端口": "3306,5432,1433,1521,6379,27017,5236",
  "SMB/文件共享": "135,139,445,2049",
  "FTP": "20,21,990",
  "邮件服务": "25,110,143,465,587,993,995",
  "远程登录": "22,23,3389,5900,5985",
  "基础设施": "53,88,123,389,636,464,623,161,162",
  "全端口 (1-1000)": "1-1000",
  "全端口 (1-5000)": "1-5000",
};

const UDP_PORT_PRESETS: Record<string, string> = {
  "常用UDP": "53,67,123,161,514,1900,5353",
  "DNS+DHCP+NTP": "53,67,68,123",
  "SNMP+管理": "161,162,514,623",
  "发现服务": "1900,5353,5683",
  "SMB/NetBIOS": "137,138",
  "Syslog/TFTP": "514,69",
  "全端口 (1-500)": "1-500",
};

interface UdpPortResult {
  port: number;
  open: boolean;
  filtered: boolean;
  service: string;
  detail: string;
}

type ScanType = "tcp" | "udp";

function PortScanner() {
  const [scanType, setScanType] = useState<ScanType>("tcp");
  const [ip, setIp] = useState("192.168.1.1");
  const [ports, setPorts] = useState("5000");
  const [timeout, setTimeout_] = useState("2000");
  const [scanning, setScanning] = useState(false);
  const [tcpResults, setTcpResults] = useState<PortScanResult[] | null>(null);
  const [udpResults, setUdpResults] = useState<UdpPortResult[] | null>(null);
  const [error, setError] = useState("");

  const results = scanType === "tcp" ? tcpResults : udpResults;

  // 实时接收端口扫描结果
  useEffect(() => {
    const unlistenTcp = listen<PortScanResult>("port-scan-result", (e) => {
      setTcpResults(prev => [...(prev ?? []), e.payload]);
    });
    const unlistenUdp = listen<UdpPortResult>("udp-scan-result", (e) => {
      setUdpResults(prev => [...(prev ?? []), e.payload]);
    });
    return () => {
      unlistenTcp.then(fn => fn());
      unlistenUdp.then(fn => fn());
    };
  }, []);

  const handleScan = async () => {
    setError("");
    setTcpResults([]);
    setUdpResults([]);
    setScanning(true);
    const t = parseInt(timeout, 10) || 2000;
    try {
      if (scanType === "tcp") {
        await invoke<void>("scan_ports", {
          ip: ip.trim(), ports: ports.trim(), timeoutMs: t,
        });
      } else {
        await invoke<void>("scan_udp_ports", {
          ip: ip.trim(), ports: ports.trim(), timeoutMs: t,
        });
      }
    } catch (e: any) {
      setError(typeof e === "string" ? e : e?.message || String(e));
    } finally {
      setScanning(false);
    }
  };

  const presets = scanType === "tcp" ? TCP_PORT_PRESETS : UDP_PORT_PRESETS;

  const tcpOpen = (tcpResults ?? []).filter(r => r.open).length;
  const tcpClosed = (tcpResults ?? []).filter(r => !r.open).length;
  const udpOpen = (udpResults ?? []).filter(r => r.open).length;
  const udpFiltered = (udpResults ?? []).filter(r => r.filtered).length;
  const udpClosed = (udpResults ?? []).filter(r => !r.open && !r.filtered).length;

  return (
    <div className="space-y-6">
      {/* Scan type toggle */}
      <div className="flex gap-2">
        <button
          onClick={() => { setScanType("tcp"); setTcpResults(null); setUdpResults(null); }}
          className={`px-3 py-1.5 rounded text-xs font-medium transition-colors ${
            scanType === "tcp"
              ? "bg-[hsl(var(--accent))] text-white"
              : "bg-[hsl(var(--muted))] text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--border))]"
          }`}
        >
          TCP 扫描
        </button>
        <button
          onClick={() => { setScanType("udp"); setTcpResults(null); setUdpResults(null); }}
          className={`px-3 py-1.5 rounded text-xs font-medium transition-colors ${
            scanType === "udp"
              ? "bg-[hsl(var(--accent))] text-white"
              : "bg-[hsl(var(--muted))] text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--border))]"
          }`}
        >
          UDP 扫描
        </button>
      </div>

      <div className="flex items-end gap-3 flex-wrap">
        <div>
          <label className={labelClass}>目标 IP</label>
          <input
            type="text" placeholder="192.168.1.1"
            value={ip} onChange={e => setIp(e.target.value)}
            onKeyDown={e => e.key === "Enter" && handleScan()}
            className={`w-40 ${inputClass}`}
            style={{ imeMode: "disabled" }}
          />
        </div>
        <div>
          <label className={labelClass}>端口 (逗号/范围)</label>
          <input
            type="text" placeholder="22,80,443 或 1-1000"
            value={ports} onChange={e => setPorts(e.target.value)}
            onKeyDown={e => e.key === "Enter" && handleScan()}
            className={`w-56 ${inputClass}`}
          />
        </div>
        <div>
          <label className={labelClass}>超时 (ms)</label>
          <SpinInput
            min={500} max={10000} step={500}
            value={timeout} onChange={(v) => setTimeout_(String(v))}
            className="w-24"
          />
        </div>
        <button onClick={handleScan} disabled={scanning} className={btnClass}>
          {scanning ? "扫描中..." : "开始扫描"}
        </button>
      </div>

      <div className="flex gap-2 flex-wrap">
        {Object.entries(presets).map(([label, preset]) => (
          <button
            key={label}
            onClick={() => setPorts(preset)}
            className="px-2.5 py-1 rounded text-xs font-medium bg-[hsl(var(--muted))] text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--border))] transition-colors"
          >
            {label}
          </button>
        ))}
      </div>

      {error && <p className="text-sm text-[hsl(var(--danger))]">{error}</p>}

      {results && results.length > 0 && (
        <div className="space-y-3">
          {scanType === "tcp" && (
            <div className="flex gap-4 text-sm">
              <span className="flex items-center gap-1.5">
                <CheckCircle2 size={14} className="text-[hsl(var(--success))]" />
                开放 <span className="font-semibold">{tcpOpen}</span>
              </span>
              <span className="flex items-center gap-1.5">
                <XCircle size={14} className="text-[hsl(var(--text-tertiary))]" />
                关闭 <span className="font-semibold">{tcpClosed}</span>
              </span>
            </div>
          )}
          {scanType === "udp" && (
            <div className="flex gap-4 text-sm">
              <span className="flex items-center gap-1.5">
                <CheckCircle2 size={14} className="text-[hsl(var(--success))]" />
                开放/响应 <span className="font-semibold">{udpOpen}</span>
              </span>
              <span className="flex items-center gap-1.5">
                <span className="w-3.5 h-3.5 rounded-full bg-[hsl(var(--accent)_/_0.4)]" />
                开放/无响应 <span className="font-semibold">{udpFiltered}</span>
              </span>
              <span className="flex items-center gap-1.5">
                <XCircle size={14} className="text-[hsl(var(--danger))]" />
                关闭 <span className="font-semibold">{udpClosed}</span>
              </span>
            </div>
          )}

          <div className="overflow-hidden rounded-lg border border-[hsl(var(--border))]">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-[hsl(var(--muted))] text-left">
                  <th className="px-4 py-2 font-medium text-[hsl(var(--text-secondary))] w-24">端口</th>
                  <th className="px-4 py-2 font-medium text-[hsl(var(--text-secondary))]">服务</th>
                  <th className="px-4 py-2 font-medium text-[hsl(var(--text-secondary))] w-24">状态</th>
                  {scanType === "udp" && <th className="px-4 py-2 font-medium text-[hsl(var(--text-secondary))]">详情</th>}
                </tr>
              </thead>
              <tbody>
                {scanType === "tcp" && (results as PortScanResult[]).map(r => (
                  <tr key={r.port} className="border-t border-[hsl(var(--border))]">
                    <td className="px-4 py-2 font-mono">{r.port}</td>
                    <td className="px-4 py-2 text-[hsl(var(--text-secondary))]">{r.service}</td>
                    <td className="px-4 py-2">
                      {r.open ? (
                        <span className="inline-flex items-center gap-1 text-[hsl(var(--success))]">
                          <CheckCircle2 size={14} /> 开放
                        </span>
                      ) : (
                        <span className="text-[hsl(var(--text-tertiary))]">关闭</span>
                      )}
                    </td>
                  </tr>
                ))}
                {scanType === "udp" && (results as UdpPortResult[]).map(r => (
                  <tr key={r.port} className="border-t border-[hsl(var(--border))]">
                    <td className="px-4 py-2 font-mono">{r.port}</td>
                    <td className="px-4 py-2 text-[hsl(var(--text-secondary))]">{r.service}</td>
                    <td className="px-4 py-2">
                      {r.open ? (
                        <span className="inline-flex items-center gap-1 text-[hsl(var(--success))]">
                          <CheckCircle2 size={14} /> 开放
                        </span>
                      ) : r.filtered ? (
                        <span className="inline-flex items-center gap-1 text-[hsl(var(--text-primary))]">
                          <span className="w-2.5 h-2.5 rounded-full bg-[hsl(var(--accent)_/_0.5)]" />
                          开放
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 text-[hsl(var(--danger))]">
                          <XCircle size={14} /> 关闭
                        </span>
                      )}
                    </td>
                    <td className="px-4 py-2 text-xs text-[hsl(var(--text-secondary))]">{r.detail}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}

// ---- Traceroute -------------------------------------------------------------

interface TraceHop {
  hop: number;
  ip: string | null;
  region: string;
  rtt_ms: number | null;
}

function Traceroute() {
  const [target, setTarget] = useState("8.8.8.8");
  const [maxHops, setMaxHops] = useState("30");
  const [timeout, setTimeout_] = useState("1000");
  const [tracing, setTracing] = useState(false);
  const [hops, setHops] = useState<TraceHop[] | null>(null);
  const [error, setError] = useState("");
  const [hasIpDb, setHasIpDb] = useState(true); // 默认 true，避免闪烁
  const [downloading, setDownloading] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [downloadError, setDownloadError] = useState("");

  // 检查离线 IP 归属地库是否已加载
  useEffect(() => {
    invoke<boolean>("has_ip_db").then(setHasIpDb).catch(() => setHasIpDb(false));
  }, []);

  // 监听下载进度事件
  useEffect(() => {
    if (!downloading) return;
    const unlisten = listen<{ percent: number }>("ip-db-download-progress", (e) => {
      setDownloadProgress(e.payload.percent);
    });
    return () => { unlisten.then(fn => fn()); };
  }, [downloading]);

  const handleDownload = async () => {
    setDownloadError("");
    setDownloading(true);
    setDownloadProgress(0);
    try {
      await invoke<string>("download_ip_db");
      setHasIpDb(true);
    } catch (e: any) {
      setDownloadError(typeof e === "string" ? e : e?.message || String(e));
    } finally {
      setDownloading(false);
      setDownloadProgress(0);
    }
  };

  const handleTrace = async () => {
    setError("");
    setHops([]);
    setTracing(true);
    try {
      await invoke<void>("trace_route", {
        target: target.trim(),
        maxHops: parseInt(maxHops, 10) || 30,
        timeoutMs: parseInt(timeout, 10) || 1000,
      });
    } catch (e: any) {
      setError(typeof e === "string" ? e : e?.message || String(e));
    } finally {
      // 确保 tracing 一定被复位（trace-done 事件可能丢失）
      setTracing(false);
    }
  };

  // 实时接收每跳结果
  useEffect(() => {
    const unlistenHop = listen<TraceHop>("trace-hop", (e) => {
      setHops(prev => [...(prev ?? []), e.payload]);
    });
    const unlistenDone = listen("trace-done", () => {
      setTracing(false);
    });
    return () => {
      unlistenHop.then(fn => fn());
      unlistenDone.then(fn => fn());
    };
  }, []);

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-end gap-3">
        <div>
          <label className={labelClass}>目标 IP / 域名</label>
          <input
            type="text" placeholder="如 8.8.8.8 或 www.baidu.com"
            value={target} onChange={(e) => setTarget(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && !tracing && handleTrace()}
            className={`w-64 ${inputClass}`}
            style={{ imeMode: "disabled" }}
          />
        </div>
        <div>
          <label className={labelClass}>最大跳数</label>
          <input type="number" value={maxHops} onChange={(e) => setMaxHops(e.target.value)} className={`w-24 ${inputClass}`} />
        </div>
        <div>
          <label className={labelClass}>每跳超时(ms)</label>
          <input type="number" value={timeout} onChange={(e) => setTimeout_(e.target.value)} className={`w-28 ${inputClass}`} />
        </div>
        <button onClick={handleTrace} disabled={tracing} className={btnClass}>
          {tracing ? "跟踪中..." : "开始跟踪"}
        </button>
      </div>

      {!hasIpDb && (
        <div className="rounded-lg border border-[hsl(var(--warning)_/_0.3)] bg-[hsl(var(--warning)_/_0.08)] px-4 py-3 text-sm space-y-2">
          <div className="flex items-center justify-between">
            <div>
              <p className="font-medium text-[hsl(var(--warning))]">IP 归属地库未安装</p>
              <p className="text-[hsl(var(--text-secondary))] mt-0.5">
                路由跟踪可正常使用，但节点归属地无法解析。下载离线 IP 库即可启用。
              </p>
            </div>
            <button onClick={handleDownload} disabled={downloading} className={btnClass + " shrink-0"}>
              {downloading ? `下载中 ${downloadProgress}%` : "一键下载"}
            </button>
          </div>
          {downloading && (
            <div className="w-full bg-[hsl(var(--bg-hover))] rounded-full h-1.5">
              <div className="bg-[hsl(var(--accent))] h-1.5 rounded-full transition-all" style={{ width: `${downloadProgress}%` }} />
            </div>
          )}
          {downloadError && (
            <p className="text-xs text-[hsl(var(--danger))]">{downloadError}</p>
          )}
          <p className="text-xs text-[hsl(var(--text-tertiary))]">
            自动下载 ip2region_v4.xdb（~11MB）到程序目录。也可手动下载：
            <a href="https://github.com/lionsoul2014/ip2region/raw/master/data/ip2region_v4.xdb" target="_blank" rel="noopener noreferrer" className="text-[hsl(var(--accent))] underline ml-1">GitHub</a>
          </p>
        </div>
      )}

      {tracing && (
        <p className="text-sm text-[hsl(var(--text-tertiary))]">路由跟踪中，最多 {maxHops} 跳，请耐心等待（可能需要数十秒）...</p>
      )}

      {error && (
        <div className="rounded-md border border-[hsl(var(--danger)_/_0.3)] bg-[hsl(var(--danger)_/_0.1)] px-3 py-2 text-sm text-[hsl(var(--danger))]">
          {error}
        </div>
      )}

      {hops && hops.length > 0 && (
        <div className="overflow-hidden rounded-lg border border-[hsl(var(--border))]">
          <table className="w-full text-sm">
            <thead className="bg-[hsl(var(--bg-hover))]">
              <tr>
                <th className="px-4 py-2 text-left font-medium text-[hsl(var(--text-secondary))] w-16">跳数</th>
                <th className="px-4 py-2 text-left font-medium text-[hsl(var(--text-secondary))] w-44">节点 IP</th>
                <th className="px-4 py-2 text-left font-medium text-[hsl(var(--text-secondary))]">归属地</th>
                <th className="px-4 py-2 text-right font-medium text-[hsl(var(--text-secondary))] w-28">延迟(ms)</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-[hsl(var(--border-light))]">
              {hops.map((h) => (
                <tr key={h.hop} className="hover:bg-[hsl(var(--bg-hover))]">
                  <td className="px-4 py-2 text-[hsl(var(--text-tertiary))]">{h.hop}</td>
                  <td className="px-4 py-2 font-mono text-[hsl(var(--text-primary))]">
                    {h.ip ?? <span className="text-[hsl(var(--text-tertiary))]">*</span>}
                  </td>
                  <td className="px-4 py-2 text-[hsl(var(--text-secondary))]">
                    {h.region || <span className="text-[hsl(var(--text-tertiary))]">-</span>}
                  </td>
                  <td className="px-4 py-2 text-right tabular-nums text-[hsl(var(--text-secondary))]">
                    {h.rtt_ms != null ? h.rtt_ms.toFixed(1) : <span className="text-[hsl(var(--text-tertiary))]">*</span>}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {hops && hops.length === 0 && !error && (
        <p className="text-sm text-[hsl(var(--text-tertiary))]">未获取到跟踪结果</p>
      )}

      <p className="text-xs text-[hsl(var(--text-tertiary))]">
        调用系统 traceroute（Linux）/ tracert（Windows）
        {hasIpDb && "，归属地由离线 ip2region 库解析"}
        。Linux 需先安装 traceroute：sudo apt install traceroute
      </p>
    </div>
  );
}

// ---- Web Checker ------------------------------------------------------------

function WebChecker() {
  const [urls, setUrls] = useState("https://www.baidu.com");
  const [timeout, setTimeout_] = useState("10");
  const [scanning, setScanning] = useState(false);
  const [results, setResults] = useState<WebCheckResult[] | null>(null);
  const [error, setError] = useState("");

  const handleCheck = async () => {
    setError("");
    setResults(null);
    setScanning(true);
    const urlList = urls.split("\n").map(s => s.trim()).filter(Boolean);
    if (urlList.length === 0) { setError("请输入至少一个URL"); setScanning(false); return; }
    try {
      const data = await invoke<WebCheckResult[]>("check_web_urls", {
        urls: urlList,
        timeoutSecs: parseInt(timeout, 10) || 10,
      });
      setResults(data);
    } catch (e: any) {
      setError(typeof e === "string" ? e : e?.message || String(e));
    } finally {
      setScanning(false);
    }
  };

  const statusColor = (code: number | null) => {
    if (!code) return "text-[hsl(var(--text-tertiary))]";
    if (code >= 200 && code < 300) return "text-[hsl(var(--success))]";
    if (code >= 300 && code < 400) return "text-[hsl(var(--warning))]";
    return "text-[hsl(var(--danger))]";
  };

  return (
    <div className="space-y-6">
      <div className="flex items-end gap-3 flex-wrap">
        <div className="flex-1 min-w-80">
          <label className={labelClass}>URL (每行一个)</label>
          <textarea
            placeholder={"https://www.baidu.com\nhttps://github.com"}
            value={urls} onChange={e => setUrls(e.target.value)}
            rows={4}
            className={`w-full ${inputClass} resize-y font-mono text-xs`}
          />
        </div>
        <div>
          <label className={labelClass}>超时 (秒)</label>
          <SpinInput
            min={1} max={60} step={1}
            value={timeout} onChange={(v) => setTimeout_(String(v))}
            className="w-20"
          />
        </div>
        <button onClick={handleCheck} disabled={scanning} className={btnClass}>
          {scanning ? "检测中..." : "开始检测"}
        </button>
      </div>

      {error && <p className="text-sm text-[hsl(var(--danger))]">{error}</p>}

      {results && (
        <div className="overflow-hidden rounded-lg border border-[hsl(var(--border))]">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-[hsl(var(--muted))] text-left">
                <th className="px-4 py-2 font-medium text-[hsl(var(--text-secondary))]">URL</th>
                <th className="px-4 py-2 font-medium text-[hsl(var(--text-secondary))] w-20">状态码</th>
                <th className="px-4 py-2 font-medium text-[hsl(var(--text-secondary))] w-24">响应时间</th>
                <th className="px-4 py-2 font-medium text-[hsl(var(--text-secondary))]">详情</th>
              </tr>
            </thead>
            <tbody>
              {results.map((r, i) => (
                <tr key={i} className="border-t border-[hsl(var(--border))]">
                  <td className="px-4 py-2 max-w-60 truncate font-mono" title={r.url}>
                    {r.url}
                  </td>
                  <td className={`px-4 py-2 font-mono font-semibold ${statusColor(r.status_code)}`}>
                    {r.status_code ?? "ERR"}
                  </td>
                  <td className="px-4 py-2 font-mono text-[hsl(var(--text-secondary))]">
                    {r.response_time_ms}ms
                  </td>
                  <td className="px-4 py-2 text-xs text-[hsl(var(--text-secondary))]">
                    {r.error ? (
                      <span className="text-[hsl(var(--danger))]">{r.error}</span>
                    ) : (
                      <span>
                        {r.final_url !== r.url && (
                          <span className="text-[hsl(var(--text-tertiary))]">→ {r.final_url}</span>
                        )}
                        {r.content_type && (
                          <span className="ml-2 text-[hsl(var(--text-tertiary))]">{r.content_type.split(";")[0]}</span>
                        )}
                        {r.content_length != null && (
                          <span className="ml-2 text-[hsl(var(--text-tertiary))]">
                            {(r.content_length / 1024).toFixed(1)}KB
                          </span>
                        )}
                      </span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

// ---- SNMP Checker -----------------------------------------------------------

const COMMON_OIDS: Record<string, string> = {
  "系统描述": "1.3.6.1.2.1.1.1.0",
  "系统名称": "1.3.6.1.2.1.1.5.0",
  "运行时间": "1.3.6.1.2.1.1.3.0",
  "联系信息": "1.3.6.1.2.1.1.4.0",
  "位置": "1.3.6.1.2.1.1.6.0",
  "接口数量": "1.3.6.1.2.1.2.1.0",
  "接口表": "1.3.6.1.2.1.2.2",
};

const AUTH_PROTOCOLS = ["MD5", "SHA1", "SHA256"] as const;
const PRIV_PROTOCOLS = ["none", "DES", "AES128"] as const;

function SnmpChecker() {
  const [version, setVersion] = useState<"v2c" | "v3">("v2c");
  const [ip, setIp] = useState("");
  const [community, setCommunity] = useState("public");
  // v3
  const [username, setUsername] = useState("");
  const [authProto, setAuthProto] = useState("SHA1");
  const [authPass, setAuthPass] = useState("");
  const [privProto, setPrivProto] = useState("DES");
  const [privPass, setPrivPass] = useState("");
  // shared
  const [oid, setOid] = useState("1.3.6.1.2.1.1.1.0");
  const [timeout, setTimeout_] = useState("5");
  const [scanning, setScanning] = useState(false);
  const [result, setResult] = useState<SnmpResult | null>(null);
  const [error, setError] = useState("");

  const handleGet = async () => {
    setError("");
    setResult(null);
    setScanning(true);
    const t = parseInt(timeout, 10) || 5;
    try {
      let data: SnmpResult;
      if (version === "v2c") {
        data = await invoke<SnmpResult>("snmp_get", {
          ip: ip.trim(),
          community: community.trim() || "public",
          oid: oid.trim(),
          timeoutSecs: t,
        });
      } else {
        data = await invoke<SnmpResult>("snmp_v3_get", {
          ip: ip.trim(),
          username: username.trim(),
          authProtocol: authProto,
          authPassword: authPass,
          privProtocol: privProto,
          privPassword: privPass,
          oid: oid.trim(),
          timeoutSecs: t,
        });
      }
      setResult(data);
    } catch (e: any) {
      setError(typeof e === "string" ? e : e?.message || String(e));
    } finally {
      setScanning(false);
    }
  };

  return (
    <div className="space-y-6">
      {/* Version toggle */}
      <div className="flex gap-2">
        <button
          onClick={() => setVersion("v2c")}
          className={`px-3 py-1.5 rounded text-xs font-medium transition-colors ${
            version === "v2c"
              ? "bg-[hsl(var(--accent))] text-white"
              : "bg-[hsl(var(--muted))] text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--border))]"
          }`}
        >
          SNMP v2c
        </button>
        <button
          onClick={() => setVersion("v3")}
          className={`px-3 py-1.5 rounded text-xs font-medium transition-colors ${
            version === "v3"
              ? "bg-[hsl(var(--accent))] text-white"
              : "bg-[hsl(var(--muted))] text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--border))]"
          }`}
        >
          SNMP v3
        </button>
      </div>

      <div className="flex items-end gap-3 flex-wrap">
        <div>
          <label className={labelClass}>目标 IP</label>
          <input
            type="text" placeholder="192.168.1.1"
            value={ip} onChange={e => setIp(e.target.value)}
            onKeyDown={e => e.key === "Enter" && handleGet()}
            className={`w-40 ${inputClass}`}
            style={{ imeMode: "disabled" }}
          />
        </div>

        {version === "v2c" && (
          <div>
            <label className={labelClass}>Community</label>
            <input
              type="text" placeholder="public"
              value={community} onChange={e => setCommunity(e.target.value)}
              onKeyDown={e => e.key === "Enter" && handleGet()}
              className={`w-32 ${inputClass}`}
            />
          </div>
        )}

        {version === "v3" && (
          <>
            <div>
              <label className={labelClass}>用户名</label>
              <input
                type="text" placeholder="snmpuser"
                value={username} onChange={e => setUsername(e.target.value)}
                onKeyDown={e => e.key === "Enter" && handleGet()}
                className={`w-32 ${inputClass}`}
              />
            </div>
            <div>
              <label className={labelClass}>认证协议</label>
              <Select value={authProto} onChange={e => setAuthProto(e.target.value)} className="w-24">
                {AUTH_PROTOCOLS.map(p => <option key={p} value={p}>{p}</option>)}
              </Select>
            </div>
            <div>
              <label className={labelClass}>认证密码</label>
              <input
                type="password" placeholder="auth密码"
                value={authPass} onChange={e => setAuthPass(e.target.value)}
                onKeyDown={e => e.key === "Enter" && handleGet()}
                className={`w-32 ${inputClass}`}
              />
            </div>
            <div>
              <label className={labelClass}>加密协议</label>
              <Select value={privProto} onChange={e => setPrivProto(e.target.value)} className="w-24">
                {PRIV_PROTOCOLS.map(p => <option key={p} value={p}>{p === "none" ? "无加密" : p}</option>)}
              </Select>
            </div>
            <div>
              <label className={labelClass}>加密密码</label>
              <input
                type="password" placeholder="priv密码"
                value={privPass} onChange={e => setPrivPass(e.target.value)}
                onKeyDown={e => e.key === "Enter" && handleGet()}
                className={`w-32 ${inputClass}`}
              />
            </div>
          </>
        )}

        <div>
          <label className={labelClass}>OID</label>
          <input
            type="text" placeholder="1.3.6.1.2.1.1.1.0"
            value={oid} onChange={e => setOid(e.target.value)}
            onKeyDown={e => e.key === "Enter" && handleGet()}
            className={`w-56 ${inputClass} font-mono text-xs`}
            style={{ imeMode: "disabled" }}
          />
        </div>
        <div>
          <label className={labelClass}>超时 (秒)</label>
          <SpinInput
            min={1} max={30} step={1}
            value={timeout} onChange={(v) => setTimeout_(String(v))}
            className="w-20"
          />
        </div>
        <button onClick={handleGet} disabled={scanning} className={btnClass}>
          {scanning ? "查询中..." : "GET"}
        </button>
      </div>

      <div className="flex gap-2 flex-wrap">
        {Object.entries(COMMON_OIDS).map(([label, oidVal]) => (
          <button
            key={label}
            onClick={() => setOid(oidVal)}
            className={`px-2.5 py-1 rounded text-xs font-medium transition-colors ${
              oid === oidVal
                ? "bg-[hsl(var(--accent)_/_0.15)] text-[hsl(var(--accent))]"
                : "bg-[hsl(var(--muted))] text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--border))]"
            }`}
          >
            {label}
          </button>
        ))}
      </div>

      {error && <p className="text-sm text-[hsl(var(--danger))]">{error}</p>}

      {result && (
        <div className="rounded-lg border border-[hsl(var(--border))] overflow-hidden">
          <table className="w-full text-sm">
            <tbody>
              <tr className="border-b border-[hsl(var(--border))]">
                <td className="px-4 py-2.5 bg-[hsl(var(--muted))] text-[hsl(var(--text-secondary))] w-24">版本</td>
                <td className="px-4 py-2.5 font-mono text-[hsl(var(--text-secondary))]">{version === "v2c" ? "SNMP v2c" : "SNMP v3"}</td>
              </tr>
              <tr className="border-b border-[hsl(var(--border))]">
                <td className="px-4 py-2.5 bg-[hsl(var(--muted))] text-[hsl(var(--text-secondary))]">OID</td>
                <td className="px-4 py-2.5 font-mono text-xs">{result.oid || oid}</td>
              </tr>
              <tr className="border-b border-[hsl(var(--border))]">
                <td className="px-4 py-2.5 bg-[hsl(var(--muted))] text-[hsl(var(--text-secondary))]">类型</td>
                <td className="px-4 py-2.5 font-mono">{result.value_type || "-"}</td>
              </tr>
              <tr className="border-b border-[hsl(var(--border))]">
                <td className="px-4 py-2.5 bg-[hsl(var(--muted))] text-[hsl(var(--text-secondary))]">值</td>
                <td className="px-4 py-2.5 font-mono">
                  {result.error ? (
                    <span className="text-[hsl(var(--danger))]">{result.error}</span>
                  ) : (
                    <span className="break-all">{result.value ?? "-"}</span>
                  )}
                </td>
              </tr>
              <tr className="border-b border-[hsl(var(--border))]">
                <td className="px-4 py-2.5 bg-[hsl(var(--muted))] text-[hsl(var(--text-secondary))]">响应时间</td>
                <td className="px-4 py-2.5 font-mono text-[hsl(var(--text-secondary))]">{result.response_time_ms}ms</td>
              </tr>
              {result.raw_hex && (
                <tr>
                  <td className="px-4 py-2.5 bg-[hsl(var(--muted))] text-[hsl(var(--text-secondary))]">原始数据</td>
                  <td className="px-4 py-2.5 font-mono text-xs text-[hsl(var(--text-tertiary))] break-all">{result.raw_hex}</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      {!result && !error && (
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <div className="w-12 h-12 rounded-full bg-[hsl(var(--muted))] flex items-center justify-center mb-4">
            <Plug size={20} className="text-[hsl(var(--text-tertiary))]" />
          </div>
          <h3 className="text-sm font-medium text-[hsl(var(--text-secondary))] mb-1">SNMP 检测</h3>
          <p className="text-xs text-[hsl(var(--text-tertiary))] max-w-md">
            支持 SNMP v2c 和 v3。v2c 使用 Community 字符串认证；v3 使用 USM 安全模型，支持 MD5/SHA1/SHA256 认证和 DES/AES128 加密。
          </p>
        </div>
      )}
    </div>
  );
}

// ---- TFTP Server ------------------------------------------------------------

function TftpServer() {
  const [filePath, setFilePath] = useState("");
  const [port, setPort] = useState("69");
  const [running, setRunning] = useState(false);
  const [logs, setLogs] = useState<{ msg: string; type: string }[]>([]);
  const [progress, setProgress] = useState<{ ip: string; bytes: number; total: number; done: boolean } | null>(null);
  const [speed, setSpeed] = useState("");
  const prevBytesRef = useRef(0);
  const prevTimeRef = useRef(0);

  useEffect(() => {
    const unlistens: (() => void)[] = [];
    listen<{ msg: string; type: string }>("tftp-log", (e) => {
      setLogs(prev => [...prev.slice(-199), e.payload]);
    }).then(f => unlistens.push(f));
    listen<{ ip: string; bytes: number; total: number; done: boolean }>("tftp-progress", (e) => {
      const now = Date.now();
      const prevT = prevTimeRef.current;
      const prevB = prevBytesRef.current;
      if (prevT > 0 && prevB > 0 && e.payload.bytes > prevB) {
        const dt = (now - prevT) / 1000;
        if (dt > 0.2) {
          const ds = e.payload.bytes - prevB;
          const spd = ds / dt;
          setSpeed(spd >= 1048576 ? `${(spd/1048576).toFixed(1)} MB/s`
            : spd >= 1024 ? `${(spd/1024).toFixed(0)} KB/s`
            : `${spd.toFixed(0)} B/s`);
          prevBytesRef.current = e.payload.bytes;
          prevTimeRef.current = now;
        }
      }
      setProgress(e.payload);
    }).then(f => unlistens.push(f));
    return () => unlistens.forEach(f => f());
  }, []);

  const start = async () => {
    if (!filePath.trim()) { alert("请选择 TFTP 根目录"); return; }
    try {
      await invoke("start_tftp_server", { filePath: filePath.trim(), port: parseInt(port) || 69 });
      setRunning(true); setProgress(null); setSpeed(""); prevBytesRef.current = 0; prevTimeRef.current = Date.now();
    } catch (e: any) { alert(typeof e === "string" ? e : e?.message || "启动失败"); }
  };
  const stop = async () => {
    try { await invoke("stop_tftp_server"); } catch {}
    setRunning(false); setProgress(null);
  };

  const fmtSize = (b: number) => b >= 1048576 ? `${(b/1048576).toFixed(1)} MB` : b >= 1024 ? `${(b/1024).toFixed(0)} KB` : `${b} B`;

  return (
    <div className="space-y-4">
      <div className="flex items-end gap-3 flex-wrap">
        <div className="flex-1 min-w-[250px]">
          <label className={labelClass}>TFTP 根目录</label>
          <div className="flex gap-2">
            <input value={filePath} readOnly
              className="flex-1 px-3 py-2 rounded-lg border border-[hsl(var(--border))] bg-[hsl(var(--bg-primary))] text-sm text-[hsl(var(--text-primary))] cursor-default"
              placeholder="未选择目录" />
            <button onClick={async () => {
              const selected = await open({ multiple: false, directory: true });
              if (selected) setFilePath(selected as string);
            }} className="px-3 py-2 rounded-lg text-xs border border-[hsl(var(--border))] hover:bg-[hsl(var(--bg-hover))] shrink-0">
              选择目录
            </button>
          </div>
        </div>
        <div className="w-20">
          <label className={labelClass}>端口</label>
          <input value={port} onChange={e => setPort(e.target.value)}
            className="w-full px-3 py-2 rounded-lg border border-[hsl(var(--border))] bg-[hsl(var(--bg-primary))] text-sm" />
        </div>
        <div className="flex gap-2">
          {!running
            ? <button onClick={start} className={btnClass}>▶ 启动</button>
            : <button onClick={stop} className="px-5 py-2 rounded-lg text-sm font-medium text-white bg-red-500 hover:opacity-90">⏹ 停止</button>}
        </div>
        {running && <span className="text-xs text-green-500">● 运行中</span>}
      </div>
      {progress && !progress.done && (
        <div className="p-4 border-2 border-[hsl(var(--accent)_/_0.3)] rounded-xl bg-[hsl(var(--accent)_/_0.04)]">
          <div className="flex items-center justify-between mb-2">
            <p className="text-sm font-medium text-[hsl(var(--text-primary))]">
              {progress.total > 0 ? '↓' : '↑'} 传输中 — {progress.ip}
            </p>
            <p className="text-xs text-[hsl(var(--text-secondary))]">
              {progress.total > 0 ? `${Math.round((progress.bytes / progress.total) * 100)}%` : ''} {speed}
            </p>
          </div>
          <div className="w-full bg-[hsl(var(--border))] rounded-full h-4 overflow-hidden">
            <div className="bg-[hsl(var(--accent))] h-4 rounded-full transition-all duration-300 flex items-center justify-end pr-2"
              style={{ width: progress.total > 0 ? `${Math.max((progress.bytes / progress.total) * 100, 2)}%` : '100%', animation: progress.total === 0 ? 'pulse 2s infinite' : undefined }}>
              {progress.total > 0 && progress.bytes / progress.total > 0.15 &&
                <span className="text-[10px] text-white font-medium">{Math.round((progress.bytes / progress.total) * 100)}%</span>}
            </div>
          </div>
          <p className="text-xs text-[hsl(var(--text-tertiary))] mt-1.5">
            {fmtSize(progress.bytes)}{progress.total > 0 ? ` / ${fmtSize(progress.total)}` : ' 已接收'}
          </p>
        </div>
      )}
      <div className="max-h-48 overflow-y-auto border border-[hsl(var(--border))] rounded-lg p-3 bg-[hsl(var(--bg-primary))] text-xs font-mono space-y-1">
        {logs.length === 0 && <p className="text-[hsl(var(--text-tertiary))]">等待传输...</p>}
        {logs.map((l, i) => (
          <p key={i} className={l.type === "error" ? "text-red-500" : l.type === "success" ? "text-green-500" : "text-[hsl(var(--text-secondary))]"}>
            {l.msg}
          </p>
        ))}
      </div>
    </div>
  );
}

// ---- Syslog Receiver --------------------------------------------------------

function SyslogReceiver() {
  const [port, setPort] = useState("514");
  const [running, setRunning] = useState(false);
  const [messages, setMessages] = useState<{ time: string; ip: string; msg: string }[]>([]);
  const [count, setCount] = useState(0);

  useEffect(() => {
    const unlistenRef = { current: undefined as (() => void) | undefined };
    listen<{ time: string; ip: string; msg: string }>("syslog-msg", (e) => {
      setMessages(prev => [...prev.slice(-499), e.payload]);
      setCount(c => c + 1);
    }).then(f => { unlistenRef.current = f; });
    return () => unlistenRef.current?.();
  }, []);

  const start = async () => {
    try {
      await invoke("start_syslog_server", { port: parseInt(port) || 514 });
      setRunning(true);
    } catch (e: any) { alert(typeof e === "string" ? e : e?.message || "启动失败"); }
  };
  const stop = async () => {
    try { await invoke("stop_syslog_server"); } catch {}
    setRunning(false);
  };

  return (
    <div className="space-y-4">
      <div className="flex items-end gap-3 flex-wrap">
        <div className="w-24">
          <label className={labelClass}>端口</label>
          <input value={port} onChange={e => setPort(e.target.value)}
            className="w-full px-3 py-2 rounded-lg border border-[hsl(var(--border))] bg-[hsl(var(--bg-primary))] text-sm" />
        </div>
        <div className="flex gap-2">
          {!running
            ? <button onClick={start} className={btnClass}>▶ 启动</button>
            : <button onClick={stop} className="px-5 py-2 rounded-lg text-sm font-medium text-white bg-red-500 hover:opacity-90">⏹ 停止</button>}
          <button onClick={() => { setMessages([]); setCount(0); }} className="px-3 py-2 rounded-lg text-xs border border-[hsl(var(--border))]">清空</button>
        </div>
        {running && <span className="text-xs text-green-500">● 运行中</span>}
        <span className="text-xs text-[hsl(var(--text-secondary))] ml-auto">已接收 {count} 条</span>
      </div>
      <div className="max-h-80 overflow-y-auto border border-[hsl(var(--border))] rounded-lg bg-[hsl(var(--bg-primary))] text-xs font-mono">
        {messages.length === 0 && <p className="p-4 text-[hsl(var(--text-tertiary))] text-center">等待日志...</p>}
        {messages.map((m, i) => (
          <div key={i} className="flex gap-2 px-3 py-1 border-b border-[hsl(var(--border-light))] hover:bg-[hsl(var(--bg-hover))]">
            <span className="text-[hsl(var(--text-tertiary))] shrink-0">{m.time}</span>
            <span className="text-[hsl(var(--accent))] shrink-0">{m.ip}</span>
            <span className="text-[hsl(var(--text-primary))] truncate">{m.msg}</span>
          </div>
        ))}
      </div>
    </div>
  );
}


// ---- Main page ---------------------------------------------------------------

export default function ToolsPage() {
  const [active, setActive] = useState<ToolKey>("subnet");

  return (
    <div>
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm">
        <h1 className="text-lg font-bold">工具箱</h1>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 mb-6 border-b border-[hsl(var(--border))]">
        {TOOLS.map(t => (
          <button
            key={t.key}
            onClick={() => setActive(t.key)}
            className={`px-4 py-2.5 text-sm font-medium transition-colors border-b-2 -mb-[1px] ${
              active === t.key
                ? "border-[hsl(var(--accent))] text-[hsl(var(--accent))]"
                : "border-transparent text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-secondary))]"
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* Panel — use hidden to preserve state across tab switches */}
      <div hidden={active !== "subnet"}><SubnetCalc /></div>
      <div hidden={active !== "scanner"}><LiveScanner /></div>
      <div hidden={active !== "port"}><PortScanner /></div>
      <div hidden={active !== "trace"}><Traceroute /></div>
      <div hidden={active !== "web"}><WebChecker /></div>
      <div hidden={active !== "snmp"}><SnmpChecker /></div>
      <div hidden={active !== "tftp"}><TftpServer /></div>
      <div hidden={active !== "syslog"}><SyslogReceiver /></div>
    </div>
  );
}
