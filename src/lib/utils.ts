import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/** 解析巡检记录的 command_outputs JSON 为命令-输出数组 */
export function parseCommandOutputs(json: string | null | undefined): { command: string; content: string }[] {
  if (!json) return [];
  try {
    const parsed = JSON.parse(json);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      return Object.entries(parsed).map(([command, content]) => ({
        command,
        content: typeof content === "string" ? content : JSON.stringify(content),
      }));
    }
    if (Array.isArray(parsed)) return parsed;
    return [{ command: "output", content: json }];
  } catch {
    return [{ command: "output", content: json }];
  }
}

/** 解析巡检记录的 ai_result JSON */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function parseAiResult(json: string | null | undefined): any {
  if (!json) return null;
  try {
    return JSON.parse(json);
  } catch {
    return null;
  }
}

/** 将后端错误转为友好中文提示 */
export function friendlyError(e: unknown): string {
  // JSON.stringify(undefined) 返回 undefined，会导致后续 .includes() 抛出 TypeError
  const raw = typeof e === "string" ? e : JSON.stringify(e ?? "未知错误");
  // 后端 check_unique 自定义消息
  if (raw.includes("设备名称") && raw.includes("已存在")) return "设备名称已存在，请换一个";
  if (raw.includes("IP 地址") && raw.includes("已存在")) return "该 IP 地址已被其他设备使用";
  // SQLite UNIQUE 约束
  if (raw.includes("UNIQUE constraint failed: devices.name")) return "设备名称已存在，请换一个";
  if (raw.includes("UNIQUE constraint failed: devices.ip")) return "该 IP 地址已被其他设备使用";
  if (raw.includes("UNIQUE constraint failed: inspection_templates.name")) return "模板名称已存在，请换一个";
  if (raw.includes("UNIQUE constraint failed: command_pool")) return "该厂商下已存在相同命令";
  if (raw.includes("UNIQUE constraint failed")) return "数据重复，请检查输入";
  // 其他约束
  if (raw.includes("NOT NULL constraint failed")) return "请填写所有必填项";
  if (raw.includes("FOREIGN KEY constraint failed")) return "关联数据不存在，请检查";
  return raw;
}

/** 在状态栏后显示一个临时提示标签（8 秒后自动消失） */
export function showStatusHint(
  text: string,
  level: "info" | "warn" | "error" | "success" = "info",
  durationMs?: number,
) {
  window.dispatchEvent(
    new CustomEvent("statusbar-hint", { detail: { text, level, durationMs } }),
  );
}
