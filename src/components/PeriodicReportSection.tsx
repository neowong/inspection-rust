import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import Button from "./ui/Button";
import Card from "./ui/Card";

interface PeriodicReport {
  id: number;
  report_type: string;
  period_start: string;
  period_end: string;
  status: string;
  device_ids: number[];
  report_path: string | null;
  ai_summary: string | null;
  stats_json: string | null;
  error_message: string | null;
  created_at: string;
  updated_at: string;
}

interface Device {
  id: number;
  name: string;
  ip: string;
  vendor: string;
}

const REPORT_TYPES = [
  { value: "weekly", label: "周报" },
  { value: "monthly", label: "月报" },
  { value: "quarterly", label: "季报" },
  { value: "yearly", label: "年报" },
];

export default function PeriodicReportSection() {
  const [reports, setReports] = useState<PeriodicReport[]>([]);
  const [devices, setDevices] = useState<Device[]>([]);
  const [loading, setLoading] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 生成表单状态
  const [reportType, setReportType] = useState("monthly");
  const [periodStart, setPeriodStart] = useState("");
  const [periodEnd, setPeriodEnd] = useState("");
  const [selectedDeviceIds, setSelectedDeviceIds] = useState<number[]>([]);
  const [showDeviceSelector, setShowDeviceSelector] = useState(false);

  // 详情展开
  const [expandedId, setExpandedId] = useState<number | null>(null);

  const loadReports = useCallback(async () => {
    setLoading(true);
    try {
      const result = await invoke<PeriodicReport[]>("list_periodic_reports", {});
      setReports(result);
    } catch (e) {
      console.error("加载周期报告列表失败:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  const loadDevices = useCallback(async () => {
    try {
      const result = await invoke<Device[]>("list_devices", {});
      setDevices(result);
    } catch (e) {
      console.error("加载设备列表失败:", e);
    }
  }, []);

  useEffect(() => {
    loadReports();
    loadDevices();
  }, [loadReports, loadDevices]);

  // 根据报告类型自动计算建议的时间范围
  const suggestPeriod = useCallback(async (type: string) => {
    try {
      const [start, end] = await invoke<[string, string]>("suggest_period_range", {
        reportType: type,
      });
      setPeriodStart(start);
      setPeriodEnd(end);
    } catch (e) {
      console.error("获取建议时间范围失败:", e);
    }
  }, []);

  useEffect(() => {
    suggestPeriod(reportType);
  }, [reportType, suggestPeriod]);

  const handleGenerate = async () => {
    if (!periodStart || !periodEnd) {
      setError("请选择时间范围");
      return;
    }

    setGenerating(true);
    setError(null);

    try {
      await invoke("generate_periodic_report", {
        reportType,
        periodStart,
        periodEnd,
        deviceIds: selectedDeviceIds.length > 0 ? selectedDeviceIds : null,
      });
      await loadReports();
    } catch (e) {
      setError(String(e));
    } finally {
      setGenerating(false);
    }
  };

  const handleDownload = async (reportId: number) => {
    try {
      await invoke("download_periodic_report", { reportId });
    } catch (e) {
      console.error("下载失败:", e);
    }
  };

  const handleDelete = async (reportId: number) => {
    if (!confirm("确定要删除此报告吗？")) return;

    try {
      await invoke("delete_periodic_report", { reportId });
      await loadReports();
    } catch (e) {
      console.error("删除失败:", e);
    }
  };

  const toggleDevice = (deviceId: number) => {
    setSelectedDeviceIds((prev) =>
      prev.includes(deviceId)
        ? prev.filter((id) => id !== deviceId)
        : [...prev, deviceId]
    );
  };

  const getReportTypeLabel = (type: string) => {
    return REPORT_TYPES.find((t) => t.value === type)?.label || type;
  };

  const getStatusLabel = (status: string) => {
    switch (status) {
      case "completed": return "已完成";
      case "generating": return "生成中";
      case "failed": return "失败";
      default: return status;
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case "completed": return "var(--success)";
      case "generating": return "var(--info)";
      case "failed": return "var(--danger)";
      default: return "var(--text-secondary)";
    }
  };

  const parseStats = (statsJson: string | null) => {
    if (!statsJson) return null;
    try {
      return JSON.parse(statsJson);
    } catch {
      return null;
    }
  };

  return (
    <div className="space-y-4">
      {/* 生成表单 */}
      <Card>
        <div className="p-4">
          <h2 className="text-lg font-semibold mb-4">生成周期报告</h2>

          {/* 报告类型选择 */}
          <div className="flex gap-2 mb-4">
            {REPORT_TYPES.map((type) => (
              <button
                key={type.value}
                onClick={() => setReportType(type.value)}
                className={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
                  reportType === type.value
                    ? "bg-[hsl(var(--accent))] text-white"
                    : "bg-[hsl(var(--bg-secondary))] text-[hsl(var(--text-primary))] hover:bg-[hsl(var(--bg-hover))]"
                }`}
              >
                {type.label}
              </button>
            ))}
          </div>

          {/* 时间范围 */}
          <div className="grid grid-cols-2 gap-4 mb-4">
            <div>
              <label className="block text-sm font-medium mb-1">开始日期</label>
              <input
                type="date"
                value={periodStart}
                onChange={(e) => setPeriodStart(e.target.value)}
                className="w-full px-3 py-2 border rounded-lg bg-[hsl(var(--bg-input))] border-[hsl(var(--border))]"
              />
            </div>
            <div>
              <label className="block text-sm font-medium mb-1">结束日期</label>
              <input
                type="date"
                value={periodEnd}
                onChange={(e) => setPeriodEnd(e.target.value)}
                className="w-full px-3 py-2 border rounded-lg bg-[hsl(var(--bg-input))] border-[hsl(var(--border))]"
              />
            </div>
          </div>

          {/* 设备选择 */}
          <div className="mb-4">
            <div className="flex items-center gap-2 mb-2">
              <label className="text-sm font-medium">设备范围</label>
              <button
                onClick={() => setShowDeviceSelector(!showDeviceSelector)}
                className="text-sm text-[hsl(var(--accent))] hover:underline"
              >
                {selectedDeviceIds.length > 0
                  ? `已选择 ${selectedDeviceIds.length} 台设备`
                  : "全部设备"}
              </button>
            </div>

            {showDeviceSelector && (
              <div className="border rounded-lg p-3 max-h-48 overflow-y-auto bg-[hsl(var(--bg-secondary))]">
                <div className="flex items-center gap-2 mb-2">
                  <button
                    onClick={() => setSelectedDeviceIds([])}
                    className="text-xs text-[hsl(var(--accent))] hover:underline"
                  >
                    清空
                  </button>
                  <button
                    onClick={() => setSelectedDeviceIds(devices.map((d) => d.id))}
                    className="text-xs text-[hsl(var(--accent))] hover:underline"
                  >
                    全选
                  </button>
                </div>
                {devices.map((device) => (
                  <label
                    key={device.id}
                    className="flex items-center gap-2 py-1 cursor-pointer hover:bg-[hsl(var(--bg-hover))] rounded px-2"
                  >
                    <input
                      type="checkbox"
                      checked={selectedDeviceIds.includes(device.id)}
                      onChange={() => toggleDevice(device.id)}
                      className="rounded"
                    />
                    <span className="text-sm">{device.name}</span>
                    <span className="text-xs text-[hsl(var(--text-secondary))]">
                      {device.ip} ({device.vendor})
                    </span>
                  </label>
                ))}
              </div>
            )}
          </div>

          {/* 错误提示 */}
          {error && (
            <div className="mb-4 p-3 bg-[hsl(var(--danger)_/_0.1)] text-[hsl(var(--danger))] rounded-lg text-sm">
              {error}
            </div>
          )}

          {/* 生成按钮 */}
          <Button
            onClick={handleGenerate}
            loading={generating}
            disabled={generating || !periodStart || !periodEnd}
          >
            {generating ? "生成中..." : "生成周期报告"}
          </Button>
        </div>
      </Card>

      {/* 历史报告列表 */}
      <Card>
        <div className="p-4">
          <h2 className="text-lg font-semibold mb-4">历史报告</h2>

          {loading ? (
            <div className="text-center py-8 text-[hsl(var(--text-secondary))]">
              加载中...
            </div>
          ) : reports.length === 0 ? (
            <div className="text-center py-8 text-[hsl(var(--text-secondary))]">
              暂无周期报告
            </div>
          ) : (
            <div className="space-y-3">
              {reports.map((report) => {
                const stats = parseStats(report.stats_json);
                const isExpanded = expandedId === report.id;

                return (
                  <div
                    key={report.id}
                    className="border rounded-lg overflow-hidden"
                    style={{ borderColor: "hsl(var(--border))" }}
                  >
                    {/* 报告头部 */}
                    <div
                      className="flex items-center justify-between p-3 cursor-pointer hover:bg-[hsl(var(--bg-hover))]"
                      onClick={() => setExpandedId(isExpanded ? null : report.id)}
                    >
                      <div className="flex items-center gap-3">
                        <span className="text-lg font-medium">
                          {getReportTypeLabel(report.report_type)}
                        </span>
                        <span className="text-sm text-[hsl(var(--text-secondary))]">
                          {report.period_start} ~ {report.period_end}
                        </span>
                      </div>

                      <div className="flex items-center gap-3">
                        <span
                          className="text-sm font-medium"
                          style={{ color: getStatusColor(report.status) }}
                        >
                          {getStatusLabel(report.status)}
                        </span>
                        <span className="text-xs text-[hsl(var(--text-secondary))]">
                          {new Date(report.created_at).toLocaleDateString()}
                        </span>

                        {report.status === "completed" && (
                          <Button
                            size="sm"
                            variant="secondary"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleDownload(report.id);
                            }}
                          >
                            下载
                          </Button>
                        )}

                        <Button
                          size="sm"
                          variant="danger"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleDelete(report.id);
                          }}
                        >
                          删除
                        </Button>
                      </div>
                    </div>

                    {/* 展开的详情 */}
                    {isExpanded && stats && (
                      <div className="p-4 bg-[hsl(var(--bg-secondary))] border-t">
                        {/* 统计概览 */}
                        <div className="grid grid-cols-4 gap-4 mb-4">
                          <div className="text-center">
                            <div className="text-2xl font-bold text-[hsl(var(--accent))]">
                              {stats.overview?.total_devices || 0}
                            </div>
                            <div className="text-xs text-[hsl(var(--text-secondary))]">
                              设备数
                            </div>
                          </div>
                          <div className="text-center">
                            <div className="text-2xl font-bold text-[hsl(var(--accent))]">
                              {stats.overview?.total_inspections || 0}
                            </div>
                            <div className="text-xs text-[hsl(var(--text-secondary))]">
                              巡检次数
                            </div>
                          </div>
                          <div className="text-center">
                            <div className="text-2xl font-bold text-[hsl(var(--success))]">
                              {stats.overview?.status_counts?.ok || 0}
                            </div>
                            <div className="text-xs text-[hsl(var(--text-secondary))]">
                              正常
                            </div>
                          </div>
                          <div className="text-center">
                            <div className="text-2xl font-bold text-[hsl(var(--warning))]">
                              {stats.overview?.status_counts?.warning || 0}
                            </div>
                            <div className="text-xs text-[hsl(var(--text-secondary))]">
                              警告
                            </div>
                          </div>
                        </div>

                        {/* 健康分数 */}
                        <div className="flex items-center gap-2 mb-4">
                          <span className="text-sm font-medium">健康分数:</span>
                          <div className="flex-1 bg-[hsl(var(--bg-primary))] rounded-full h-2">
                            <div
                              className="h-2 rounded-full"
                              style={{
                                width: `${stats.overview?.health_score || 0}%`,
                                backgroundColor:
                                  (stats.overview?.health_score || 0) >= 80
                                    ? "hsl(var(--success))"
                                    : (stats.overview?.health_score || 0) >= 60
                                    ? "hsl(var(--warning))"
                                    : "hsl(var(--danger))",
                              }}
                            />
                          </div>
                          <span className="text-sm font-medium">
                            {stats.overview?.health_score || 0}/100
                          </span>
                        </div>

                        {/* AI 总结 */}
                        {report.ai_summary && (
                          <div className="p-3 bg-[hsl(var(--bg-primary))] rounded-lg">
                            <div className="text-sm font-medium mb-2">AI 分析:</div>
                            <div className="text-sm text-[hsl(var(--text-secondary))] whitespace-pre-wrap">
                              {report.ai_summary}
                            </div>
                          </div>
                        )}

                        {/* 错误信息 */}
                        {report.error_message && (
                          <div className="mt-3 p-3 bg-[hsl(var(--danger)_/_0.1)] text-[hsl(var(--danger))] rounded-lg text-sm">
                            错误: {report.error_message}
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}
