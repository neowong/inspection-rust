import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-shell";
import { Network, Mail, Download, RefreshCw } from "lucide-react";
import Card from "../components/ui/Card";


export default function AboutPage() {
  // 版本号（从后端获取，编译时嵌入）
  const [currentVersion, setCurrentVersion] = useState("");

  // 版本检查
  const [updateInfo, setUpdateInfo] = useState<{ version: string; url: string } | null>(null);
  const [checking, setChecking] = useState(false);
  const [checkDone, setCheckDone] = useState(false);

  // 启动时获取版本号并检查更新
  useEffect(() => {
    const init = async () => {
      try {
        const ver = await invoke<string>("get_app_version");
        setCurrentVersion(ver);
        const result = await invoke<{ version: string; url: string } | null>("check_update", {
          currentVersion: ver,
        });
        setUpdateInfo(result);
        setCheckDone(true);
      } catch {
        // 静默忽略
      }
    };
    init();
  }, []);

  const checkUpdate = async () => {
    setChecking(true);
    setCheckDone(false);
    try {
      const ver = currentVersion || await invoke<string>("get_app_version");
      if (!currentVersion) setCurrentVersion(ver);
      const result = await invoke<{ version: string; url: string } | null>("check_update", {
        currentVersion: ver,
      });
      setUpdateInfo(result);
      setCheckDone(true);
    } catch {
      // 静默忽略
    } finally {
      setChecking(false);
    }
  };

  return (
    <div className="space-y-5">
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm relative">
        <h1 className="text-lg font-bold">关于</h1>
        <p className="text-xs text-[hsl(var(--text-tertiary))] mt-0.5">项目介绍</p>
      </div>

      {/* 项目信息 */}
      <Card>
        <div className="flex items-start gap-4">
          <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl bg-[hsl(var(--accent)_/_0.12)] text-[hsl(var(--accent))]">
            <Network size={30} />
          </div>
          <div className="min-w-0 flex-1">
            <h2 className="text-xl font-bold text-[hsl(var(--text-primary))]">AI巡检助手</h2>
            <p className="mt-1 text-sm leading-relaxed text-[hsl(var(--text-secondary))]">
              AI巡检助手 是面向运维工程师的桌面巡检工具，用于集中管理网络设备与服务器、维护巡检命令模板、批量执行 SSH 巡检、调用 AI 生成评判结论，并输出可编辑的 DOCX 巡检报告。
            </p>
            <div className="mt-3 flex flex-wrap gap-2 text-xs text-[hsl(var(--text-secondary))]">
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">设备巡检</span>
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">静态信息采集</span>
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">AI 分析</span>
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">DOCX 报告</span>
              <span className="rounded-full bg-[hsl(var(--bg-hover))] px-2 py-1">网络工具箱</span>
            </div>
          </div>
        </div>

        {/* 版本信息与更新检查 */}
        <div className="mt-4 pt-4 border-t border-[hsl(var(--border))] flex items-center justify-between">
          <div className="flex items-center gap-3">
            <span className="text-sm text-[hsl(var(--text-secondary))]">版本</span>
            <span className="text-sm font-medium text-[hsl(var(--text-primary))]">v{currentVersion || "..."}</span>
          </div>
          <div className="flex items-center gap-3">
            {updateInfo && (
              <span className="text-xs text-[hsl(var(--accent))]">
                🆕 v{updateInfo.version} 已发布
              </span>
            )}
            {checkDone && !updateInfo && (
              <span className="text-xs text-[hsl(var(--text-tertiary))]">已是最新版本</span>
            )}
            <button
              onClick={checkUpdate}
              disabled={checking}
              className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-[hsl(var(--accent))] bg-[hsl(var(--accent)_/_0.1)] rounded-lg hover:bg-[hsl(var(--accent)_/_0.15)] transition-colors disabled:opacity-50"
            >
              <RefreshCw size={12} className={checking ? "animate-spin" : ""} />
              {checking ? "检查中..." : "检查更新"}
            </button>
          </div>
        </div>

        {/* 新版本下载提示 */}
        {updateInfo && (
          <div className="mt-3 flex items-center justify-between rounded-lg bg-[hsl(var(--accent)_/_0.08)] border border-[hsl(var(--accent)_/_0.2)] px-4 py-3">
            <div>
              <p className="text-sm font-medium text-[hsl(var(--text-primary))]">
                新版本 v{updateInfo.version} 可用
              </p>
              <p className="text-xs text-[hsl(var(--text-tertiary))] mt-0.5">
                建议更新以获取最新功能和修复
              </p>
            </div>
            <button
              onClick={() => open(updateInfo.url)}
              className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-white bg-[hsl(var(--accent))] rounded-lg hover:opacity-90 transition-opacity"
            >
              <Download size={12} />
              前往下载
            </button>
          </div>
        )}
      </Card>

      {/* 联系方式 */}
      <Card className="border-l-4 border-l-[hsl(var(--accent))]">
        <div className="flex items-start gap-6">
          <div className="flex-1">
            <div className="flex items-center gap-2 text-sm font-semibold text-[hsl(var(--text-primary))]">
              <Mail size={18} className="text-[hsl(var(--accent))]" />
              联系我们
            </div>
            <p className="mt-2 text-sm text-[hsl(var(--text-secondary))]">
              遇到问题或有功能建议，欢迎通过以下方式联系：
            </p>
            <div className="mt-3 space-y-2">
              <div className="flex items-center gap-2">
                <Mail size={14} className="text-[hsl(var(--accent))]" />
                <span className="text-sm font-medium text-[hsl(var(--text-primary))]">neowong2005@gmail.com</span>
              </div>
              <div className="flex items-center gap-2">
                <svg className="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor" style={{ color: "hsl(var(--accent))" }}>
                  <path d="M8.5,13.5a1,1,0,1,1,1-1A1,1,0,0,1,8.5,13.5Zm5,0a1,1,0,1,1,1-1A1,1,0,0,1,13.5,13.5ZM12,2A10,10,0,1,0,22,12,10,10,0,0,0,12,2Zm0,18a8,8,0,1,1,8-8A8,8,0,0,1,12,20Z"/>
                </svg>
                <span className="text-sm font-medium text-[hsl(var(--text-primary))]">微信扫码添加</span>
              </div>
            </div>
          </div>
          <div className="shrink-0 text-center">
            <img src="/weixin.png" alt="微信二维码" className="h-32 w-32 rounded-lg border border-[hsl(var(--border))] object-contain" />
            <p className="mt-2 text-xs text-[hsl(var(--text-tertiary))]">扫码添加微信</p>
          </div>
        </div>
      </Card>
    </div>
  );
}
