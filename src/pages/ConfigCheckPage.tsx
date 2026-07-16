import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import Button from "../components/ui/Button";
import Card from "../components/ui/Card";

interface ConfigFileType {
  id: string;
  name: string;
  extensions: string[];
  comment_patterns: string[];
}

interface Device {
  id: number;
  name: string;
  ip: string;
  vendor: string;
}

interface AnalysisResult {
  analysis: {
    summary?: string;
    risk_level?: string;
    issues?: Array<{
      category: string;
      severity: string;
      line_hint: string;
      description: string;
      suggestion: string;
    }>;
    optimizations?: string[];
  };
  raw_response: string;
  stats: {
    original_lines: number;
    removed_lines: number;
    analyzed_lines: number;
    config_type: string;
  };
}

const RISK_COLORS: Record<string, string> = {
  low: "text-[hsl(var(--success))]",
  medium: "text-[hsl(var(--warning))]",
  high: "text-[hsl(var(--danger))]",
  critical: "text-[hsl(var(--danger))]",
};

const RISK_LABELS: Record<string, string> = {
  low: "低风险",
  medium: "中风险",
  high: "高风险",
  critical: "严重",
};

const SEVERITY_COLORS: Record<string, string> = {
  info: "bg-[hsl(var(--info)_/_0.1)] text-[hsl(var(--info))]",
  warning: "bg-[hsl(var(--warning)_/_0.1)] text-[hsl(var(--warning))]",
  critical: "bg-[hsl(var(--danger)_/_0.1)] text-[hsl(var(--danger))]",
};

const CATEGORY_LABELS: Record<string, string> = {
  security: "安全",
  performance: "性能",
  compatibility: "兼容性",
  best_practice: "最佳实践",
  risk: "风险",
};

export default function ConfigCheckPage() {
  const [configTypes, setConfigTypes] = useState<ConfigFileType[]>([]);
  const [devices, setDevices] = useState<Device[]>([]);
  const [configType, setConfigType] = useState("generic");
  const [content, setContent] = useState("");
  const [filename, setFilename] = useState("");
  const [analyzing, setAnalyzing] = useState(false);
  const [result, setResult] = useState<AnalysisResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  // SSH 远程读取
  const [showSshModal, setShowSshModal] = useState(false);
  const [selectedDeviceId, setSelectedDeviceId] = useState<number | null>(null);
  const [remoteFilePath, setRemoteFilePath] = useState("/etc/nginx/nginx.conf");
  const [reading, setReading] = useState(false);

  // 清理统计
  const [cleanStats, setCleanStats] = useState<{
    original_lines: number;
    removed_lines: number;
    remaining_lines: number;
    reduction_percent: number;
  } | null>(null);

  useEffect(() => {
    invoke<ConfigFileType[]>("get_config_file_types").then(setConfigTypes).catch(console.error);
    invoke<Device[]>("list_devices", {}).then(setDevices).catch(console.error);
  }, []);

  // 内容变化时自动清理统计
  const handleContentChange = useCallback(async (newContent: string) => {
    setContent(newContent);
    if (newContent.trim()) {
      try {
        const stats = await invoke<{
          original_lines: number;
          removed_lines: number;
          remaining_lines: number;
          reduction_percent: number;
        }>("clean_config_content", {
          content: newContent,
          configType,
          keepEmptyLines: false,
        });
        setCleanStats(stats);
      } catch {
        setCleanStats(null);
      }
    } else {
      setCleanStats(null);
    }
  }, [configType]);

  // 配置类型变化时重新统计
  useEffect(() => {
    if (content.trim()) {
      handleContentChange(content);
    }
  }, [configType, content, handleContentChange]);

  const handleAnalyze = async () => {
    if (!content.trim()) {
      setError("请输入或上传配置文件内容");
      return;
    }

    setAnalyzing(true);
    setError(null);
    setResult(null);

    try {
      const res = await invoke<AnalysisResult>("analyze_config", {
        content,
        configType,
        filename: filename || null,
      });
      setResult(res);
    } catch (e) {
      setError(String(e));
    } finally {
      setAnalyzing(false);
    }
  };

  const handleFileUpload = () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".conf,.cnf,.ini,.yml,.yaml,.cfg,.txt";
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;
      setFilename(file.name);
      const text = await file.text();
      await handleContentChange(text);

      // 自动检测配置类型
      const ext = file.name.split(".").pop()?.toLowerCase() || "";
      const nameLower = file.name.toLowerCase();
      if (nameLower.includes("nginx")) setConfigType("nginx");
      else if (nameLower.includes("apache") || nameLower.includes("httpd")) setConfigType("apache");
      else if (nameLower.includes("zabbix")) setConfigType("zabbix");
      else if (nameLower.includes("mysql") || nameLower.includes("mariadb")) setConfigType("mysql");
      else if (nameLower.includes("redis")) setConfigType("redis");
      else if (nameLower.includes("sshd")) setConfigType("ssh");
      else if (nameLower.includes("docker-compose") || nameLower.includes("compose")) setConfigType("docker");
      else if (ext === "service" || ext === "timer") setConfigType("systemd");
    };
    input.click();
  };

  const handleRemoteRead = async () => {
    if (!selectedDeviceId || !remoteFilePath.trim()) {
      setError("请选择设备并输入文件路径");
      return;
    }

    setReading(true);
    setError(null);

    try {
      const res = await invoke<{
        content: string;
        file_path: string;
        config_type: string;
        device_id: number;
      }>("read_remote_config", {
        deviceId: selectedDeviceId,
        filePath: remoteFilePath,
      });

      setContent(res.content);
      setConfigType(res.config_type);
      setFilename(res.file_path);
      setShowSshModal(false);

      // 触发清理统计
      await handleContentChange(res.content);
    } catch (e) {
      setError(String(e));
    } finally {
      setReading(false);
    }
  };

  return (
    <div className="space-y-6">
      {/* 页面标题 */}
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm">
        <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">配置检查</h1>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1">
          通过 AI 分析配置文件，发现安全隐患、性能问题和最佳实践违规
        </p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* 左侧：输入区 */}
        <div className="space-y-4">
          <Card>
            <div className="p-4">
              <div className="flex items-center justify-between mb-4">
                <h2 className="text-lg font-semibold">配置文件输入</h2>
                <div className="flex gap-2">
                  <Button size="sm" variant="secondary" onClick={handleFileUpload}>
                    上传文件
                  </Button>
                  <Button size="sm" variant="secondary" onClick={() => setShowSshModal(true)}>
                    SSH 远程读取
                  </Button>
                </div>
              </div>

              {/* 配置类型选择 */}
              <div className="mb-4">
                <label className="block text-sm font-medium mb-1">配置文件类型</label>
                <div className="flex flex-wrap gap-2">
                  {configTypes.map((ct) => (
                    <button
                      key={ct.id}
                      onClick={() => setConfigType(ct.id)}
                      className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
                        configType === ct.id
                          ? "bg-[hsl(var(--accent))] text-white"
                          : "bg-[hsl(var(--bg-secondary))] hover:bg-[hsl(var(--bg-hover))]"
                      }`}
                    >
                      {ct.name}
                    </button>
                  ))}
                </div>
              </div>

              {/* 文件名（可选） */}
              {filename && (
                <div className="mb-3 text-xs text-[hsl(var(--text-tertiary))]">
                  文件: {filename}
                </div>
              )}

              {/* 内容输入 */}
              <div className="mb-4">
                <div className="flex items-center justify-between mb-1">
                  <label className="text-sm font-medium">配置内容</label>
                  {cleanStats && (
                    <span className="text-xs text-[hsl(var(--text-tertiary))]">
                      {cleanStats.original_lines} 行 → {cleanStats.remaining_lines} 行
                      （去除 {cleanStats.removed_lines} 行注释，减少 {cleanStats.reduction_percent}%）
                    </span>
                  )}
                </div>
                <textarea
                  value={content}
                  onChange={(e) => handleContentChange(e.target.value)}
                  placeholder="粘贴配置文件内容..."
                  className="w-full h-[400px] px-3 py-2 border rounded-lg bg-[hsl(var(--bg-input))] border-[hsl(var(--border))] font-mono text-sm resize-none"
                  style={{ imeMode: "disabled" }}
                />
              </div>

              {/* 错误提示 */}
              {error && (
                <div className="mb-4 p-3 bg-[hsl(var(--danger)_/_0.1)] text-[hsl(var(--danger))] rounded-lg text-sm">
                  {error}
                </div>
              )}

              {/* 分析按钮 */}
              <Button
                onClick={handleAnalyze}
                loading={analyzing}
                disabled={analyzing || !content.trim()}
                className="w-full"
              >
                {analyzing ? "AI 分析中..." : "开始分析"}
              </Button>
            </div>
          </Card>
        </div>

        {/* 右侧：结果区 */}
        <div className="space-y-4">
          {!result && !analyzing && (
            <Card>
              <div className="p-4 flex items-center justify-center h-[400px] text-[hsl(var(--text-tertiary))]">
                <div className="text-center">
                  <div className="text-4xl mb-2">🔍</div>
                  <p>输入配置文件并点击「开始分析」</p>
                  <p className="text-xs mt-1">支持 Nginx、MySQL、Redis、Zabbix 等常见配置</p>
                </div>
              </div>
            </Card>
          )}

          {analyzing && (
            <Card>
              <div className="p-4 flex items-center justify-center h-[400px]">
                <div className="text-center">
                  <div className="w-8 h-8 border-2 border-[hsl(var(--accent))] border-t-transparent rounded-full animate-spin mx-auto mb-3" />
                  <p className="text-[hsl(var(--text-secondary))]">AI 正在分析配置文件...</p>
                  <p className="text-xs text-[hsl(var(--text-tertiary))] mt-1">请稍候，这可能需要几秒钟</p>
                </div>
              </div>
            </Card>
          )}

          {result && (
            <>
              {/* 统计卡片 */}
              <div className="grid grid-cols-3 gap-3">
                <Card>
                  <div className="p-3 text-center">
                    <div className="text-2xl font-bold text-[hsl(var(--accent))]">
                      {result.stats.analyzed_lines}
                    </div>
                    <div className="text-xs text-[hsl(var(--text-tertiary))]">分析行数</div>
                  </div>
                </Card>
                <Card>
                  <div className="p-3 text-center">
                    <div className="text-2xl font-bold text-[hsl(var(--warning))]">
                      {result.analysis.issues?.length || 0}
                    </div>
                    <div className="text-xs text-[hsl(var(--text-tertiary))]">发现问题</div>
                  </div>
                </Card>
                <Card>
                  <div className="p-3 text-center">
                    <div className={`text-2xl font-bold ${RISK_COLORS[result.analysis.risk_level || "low"]}`}>
                      {RISK_LABELS[result.analysis.risk_level || "low"]}
                    </div>
                    <div className="text-xs text-[hsl(var(--text-tertiary))]">风险等级</div>
                  </div>
                </Card>
              </div>

              {/* 总体评估 */}
              <Card>
                <div className="p-4">
                  <h3 className="text-sm font-semibold mb-2">总体评估</h3>
                  <p className="text-sm text-[hsl(var(--text-secondary))]">
                    {result.analysis.summary || "暂无评估"}
                  </p>
                </div>
              </Card>

              {/* 问题列表 */}
              {result.analysis.issues && result.analysis.issues.length > 0 && (
                <Card>
                  <div className="p-4">
                    <h3 className="text-sm font-semibold mb-3">发现的问题</h3>
                    <div className="space-y-3">
                      {result.analysis.issues.map((issue, index) => (
                        <div
                          key={index}
                          className="border border-[hsl(var(--border))] rounded-lg p-3"
                        >
                          <div className="flex items-center gap-2 mb-2">
                            <span className={`text-xs px-2 py-0.5 rounded-full ${SEVERITY_COLORS[issue.severity]}`}>
                              {issue.severity === "critical" ? "严重" : issue.severity === "warning" ? "警告" : "提示"}
                            </span>
                            <span className="text-xs text-[hsl(var(--text-tertiary))]">
                              {CATEGORY_LABELS[issue.category] || issue.category}
                            </span>
                            {issue.line_hint && (
                              <span className="text-xs text-[hsl(var(--text-tertiary))]">
                                · {issue.line_hint}
                              </span>
                            )}
                          </div>
                          <p className="text-sm mb-2">{issue.description}</p>
                          <div className="text-xs text-[hsl(var(--accent))] bg-[hsl(var(--accent)_/_0.05)] rounded px-2 py-1">
                            💡 {issue.suggestion}
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                </Card>
              )}

              {/* 优化建议 */}
              {result.analysis.optimizations && result.analysis.optimizations.length > 0 && (
                <Card>
                  <div className="p-4">
                    <h3 className="text-sm font-semibold mb-3">优化建议</h3>
                    <ul className="space-y-2">
                      {result.analysis.optimizations.map((opt, index) => (
                        <li key={index} className="flex items-start gap-2 text-sm">
                          <span className="text-[hsl(var(--accent))] mt-0.5">•</span>
                          <span>{opt}</span>
                        </li>
                      ))}
                    </ul>
                  </div>
                </Card>
              )}
            </>
          )}
        </div>
      </div>

      {/* SSH 远程读取模态框 */}
      {showSshModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="bg-[hsl(var(--bg-content))] rounded-lg shadow-xl w-full max-w-md">
            <div className="flex items-center justify-between px-5 py-3 border-b border-[hsl(var(--border))]">
              <h2 className="text-lg font-semibold">SSH 远程读取配置</h2>
              <button
                onClick={() => setShowSshModal(false)}
                className="w-8 h-8 flex items-center justify-center rounded-lg hover:bg-[hsl(var(--bg-hover))]"
              >
                ✕
              </button>
            </div>
            <div className="p-5 space-y-4">
              <div>
                <label className="block text-sm font-medium mb-1">选择设备</label>
                <select
                  value={selectedDeviceId || ""}
                  onChange={(e) => setSelectedDeviceId(Number(e.target.value) || null)}
                  className="w-full px-3 py-2 border rounded-lg bg-[hsl(var(--bg-input))] border-[hsl(var(--border))]"
                >
                  <option value="">请选择设备</option>
                  {devices.filter(d => d.ip).map((d) => (
                    <option key={d.id} value={d.id}>
                      {d.name} ({d.ip})
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="block text-sm font-medium mb-1">配置文件路径</label>
                <input
                  type="text"
                  value={remoteFilePath}
                  onChange={(e) => setRemoteFilePath(e.target.value)}
                  placeholder="/etc/nginx/nginx.conf"
                  className="w-full px-3 py-2 border rounded-lg bg-[hsl(var(--bg-input))] border-[hsl(var(--border))] font-mono text-sm"
                />
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {[
                    "/etc/nginx/nginx.conf",
                    "/etc/mysql/my.cnf",
                    "/etc/redis/redis.conf",
                    "/etc/zabbix/zabbix_server.conf",
                    "/etc/ssh/sshd_config",
                    "/etc/syslog.conf",
                  ].map((path) => (
                    <button
                      key={path}
                      onClick={() => setRemoteFilePath(path)}
                      className="px-2 py-0.5 text-xs bg-[hsl(var(--bg-secondary))] rounded hover:bg-[hsl(var(--bg-hover))]"
                    >
                      {path}
                    </button>
                  ))}
                </div>
              </div>

              {error && (
                <div className="p-3 bg-[hsl(var(--danger)_/_0.1)] text-[hsl(var(--danger))] rounded-lg text-sm">
                  {error}
                </div>
              )}

              <div className="flex justify-end gap-2">
                <Button variant="ghost" onClick={() => setShowSshModal(false)}>取消</Button>
                <Button onClick={handleRemoteRead} loading={reading} disabled={reading || !selectedDeviceId}>
                  {reading ? "读取中..." : "读取"}
                </Button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
