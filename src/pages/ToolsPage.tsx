import { useState } from "react";
import { Wrench } from "lucide-react";

const TOOLS = [
  { key: "subnet", label: "子网计算器" },
  { key: "scanner", label: "存活扫描" },
  { key: "port", label: "端口扫描" },
  { key: "web", label: "WEB检测" },
  { key: "snmp", label: "SNMP" },
] as const;

type ToolKey = (typeof TOOLS)[number]["key"];

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

function calcSubnet(ip: string, cidrStr: string): SubnetResult | null {
  const cidr = parseInt(cidrStr, 10);
  if (isNaN(cidr) || cidr < 0 || cidr > 32) return null;
  const parts = ip.split(".").map(Number);
  if (parts.length !== 4 || parts.some(p => isNaN(p) || p < 0 || p > 255)) return null;

  const ipInt = ipToInt(parts);
  const maskInt = cidr === 0 ? 0 : (~0 << (32 - cidr)) >>> 0;
  const wildcardInt = ~maskInt >>> 0;
  const networkInt = (ipInt & maskInt) >>> 0;
  const broadcastInt = (networkInt | wildcardInt) >>> 0;

  let firstHost: number[], lastHost: number[], hostCount: string;
  if (cidr >= 31) {
    firstHost = [0, 0, 0, 0];
    lastHost = [0, 0, 0, 0];
    hostCount = cidr === 32 ? "1 (单主机)" : "2 (点对点链路)";
  } else {
    firstHost = intToIp(networkInt + 1);
    lastHost = intToIp(broadcastInt - 1);
    const count = Math.pow(2, 32 - cidr) - 2;
    hostCount = count.toLocaleString();
  }

  return {
    ipInt,
    maskInt,
    network: intToIp(networkInt),
    broadcast: intToIp(broadcastInt),
    firstHost,
    lastHost,
    hostCount,
    subnetMask: intToIp(maskInt),
    wildcard: intToIp(wildcardInt),
    cidr,
    ipBinary: ipToBinary(ipInt),
    maskBinary: ipToBinary(maskInt),
  };
}

function SubnetCalc() {
  const [ip, setIp] = useState("");
  const [cidr, setCidr] = useState("");
  const [result, setResult] = useState<SubnetResult | null>(null);
  const [error, setError] = useState("");

  const handleCalc = () => {
    setError("");
    const r = calcSubnet(ip.trim(), cidr.trim());
    if (!r) { setError("请输入有效的 IP 地址和 CIDR 前缀（0-32）"); setResult(null); return; }
    setResult(r);
  };

  const gridClass = "grid grid-cols-2 gap-x-6 gap-y-2 text-sm";

  return (
    <div className="space-y-6">
      <div className="flex items-end gap-3 flex-wrap">
        <div>
          <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">IP 地址</label>
          <input
            type="text" placeholder="192.168.1.0"
            value={ip} onChange={e => setIp(e.target.value)}
            onKeyDown={e => e.key === "Enter" && handleCalc()}
            className="w-36 px-3 py-2 rounded-lg border border-[hsl(var(--border))] bg-[hsl(var(--bg-input))] text-sm focus:outline-none focus:ring-2 focus:ring-[hsl(var(--accent)_/_0.4)]"
          />
        </div>
        <span className="text-lg font-bold text-[hsl(var(--text-secondary))] pb-2">/</span>
        <div>
          <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">CIDR</label>
          <input
            type="number" min={0} max={32} placeholder="24"
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
            <div className="font-mono font-semibold">{result.network.join(".")}</div>
            <div className="text-[hsl(var(--text-secondary))]">广播地址</div>
            <div className="font-mono font-semibold">{result.broadcast.join(".")}</div>
            <div className="text-[hsl(var(--text-secondary))]">可用范围</div>
            <div className="font-mono">{result.firstHost.join(".")} — {result.lastHost.join(".")}</div>
            <div className="text-[hsl(var(--text-secondary))]">可用主机数</div>
            <div className="font-mono">{result.hostCount}</div>
            <div className="text-[hsl(var(--text-secondary))]">子网掩码</div>
            <div className="font-mono">{result.subnetMask.join(".")}</div>
            <div className="text-[hsl(var(--text-secondary))]">反掩码 (通配符)</div>
            <div className="font-mono">{result.wildcard.join(".")}</div>
          </div>

          <details className="text-xs">
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
        </div>
      )}
    </div>
  );
}

// ---- Placeholder tabs --------------------------------------------------------

function PlaceholderTab({ title, desc }: { title: string; desc: string }) {
  return (
    <div className="flex flex-col items-center justify-center py-20 text-center">
      <div className="w-12 h-12 rounded-full bg-[hsl(var(--muted))] flex items-center justify-center mb-4">
        <Wrench size={20} className="text-[hsl(var(--text-tertiary))]" />
      </div>
      <h3 className="text-sm font-medium text-[hsl(var(--text-secondary))] mb-1">{title}</h3>
      <p className="text-xs text-[hsl(var(--text-tertiary))]">{desc}</p>
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

      {/* Panel */}
      {active === "subnet" && <SubnetCalc />}
      {active === "scanner" && <PlaceholderTab title="存活主机扫描" desc="扫描指定网段内存活的主机（即将上线）" />}
      {active === "port" && <PlaceholderTab title="端口扫描" desc="检测指定 IP 的开放 TCP 端口（即将上线）" />}
      {active === "web" && <PlaceholderTab title="WEB 状态码检测" desc="批量检测 HTTP/HTTPS 站点状态（即将上线）" />}
      {active === "snmp" && <PlaceholderTab title="SNMP 检测" desc="检测 SNMP 服务可达性（即将上线）" />}
    </div>
  );
}
