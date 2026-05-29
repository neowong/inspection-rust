# 项目优化设计文档

基于 2026-05-29 代码审计，对网络设备巡检系统（Rust + Tauri v2）进行四阶段优化。

## Phase 1: 安全加固

### 1.1 CSP 策略

**文件**: `src-tauri/tauri.conf.json`

将 `"csp": null` 替换为明确策略：

```
default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'
```

- `script-src 'self'` — 禁止内联脚本，防 XSS
- `style-src 'unsafe-inline'` — TailwindCSS JIT 需要内联 style
- `img-src data:` — 支持 base64 内联图标

### 1.2 Fernet 密钥管理

**文件**: `src-tauri/src/services/crypto.rs`

**当前问题**: `MASTER_PASSWORD` 硬编码在源码第 1 行，任何获取源码的人可解密全部加密字段。

**方案**: 首次启动自动生成，持久化到本地文件。

```
密钥文件路径: ~/.local/share/inspection-rust/.key
```

实现要点：
- `fn load_or_create_key() -> String`: 检查 `.key` 文件是否存在；不存在则 `fernet::Fernet::generate_key()` 生成并写入（权限 0600）；存在则读取
- `static FERNET_KEY: OnceLock<String>` 全局缓存
- `static FERNET_INSTANCE: OnceLock<fernet::Fernet>` 避免每次加解密重建实例
- 移除硬编码 `MASTER_PASSWORD` 常量

## Phase 2: 性能优化

### 2.1 Mutex 锁拆分 — run_batch

**文件**: `src-tauri/src/commands/inspections.rs` 第 505-600 行

**当前问题**: `run_batch` 在整个批次执行期间（可能数十秒）持有 `state.db.lock()`，阻塞所有其他 Tauri command 对 DB 的访问。

**改造方案**:

```
1. 获锁 → 读取 batch 信息 + device 列表 → 释放锁
2. 更新 batch 状态为 "running"（短暂获锁 → 释放）
3. 对每台设备:
   a. 创建 inspection_record（短暂获锁 → 释放）
   b. 执行 SSH 命令（锁外，耗时操作）
   c. 更新 record 结果（短暂获锁 → 释放）
4. 更新 batch 为 completed（短暂获锁 → 释放）
```

同理改造 `check_device_status_inner`（devices.rs 第 349-398 行），TCP 超时检测移到锁外。

### 2.2 异步化 — run_batch

**当前问题**: `run_batch` 是同步 `#[tauri::command]` 函数，SSH 操作阻塞 tokio 工作线程。

**改造**: 改为 `async fn`，SSH 调用用 `tokio::task::spawn_blocking` 包装。

```rust
#[tauri::command]
pub async fn run_batch(batch_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    // ... 锁操作 ...
    let result = tokio::task::spawn_blocking(move || {
        execute_ssh_for_device(device, commands)
    }).await.map_err(|e| e.to_string())?;
    // ... 锁操作写入结果 ...
}
```

### 2.3 reqwest Client 复用

**文件**: `src-tauri/src/services/ai_inspection.rs` 第 84、160 行

**当前问题**: 每次 AI API 调用新建 `reqwest::Client`，无法复用 TCP 连接和 TLS 会话。

**改造**:

```rust
use std::sync::OnceLock;

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| reqwest::Client::new())
}
```

## Phase 3: 代码质量

### 3.1 后端去重

**行映射函数统一** — 将以下函数从各 command 模块移至 `db/models.rs`：
- `device_from_row` (devices.rs:22-39, inspections.rs:74-91, reports.rs:51-67)
- `template_from_row` (templates.rs, inspections.rs)
- `batch_from_row` (inspections.rs)
- `record_from_row` (inspections.rs, reports.rs)
- `command_from_row` (templates.rs)

各模块改为 `use crate::db::models::*` 引用。

**now_str() 统一** — 从 `inspections.rs:127` 和 `reports.rs:119` 提取到 `db/mod.rs`：

```rust
pub fn now_str() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}
```

### 3.2 前端公共抽取

**新文件 `src/lib/status.ts`**:
```ts
export function batchStatusColor(status: string): "pending" | "running" | "completed" | "failed" | "stopped" {
  // 从 InspectionPage.tsx:106 和 ReportsPage.tsx:11 提取
}
```

**新文件 `src/lib/constants.ts`**:
```ts
export const VENDORS = ["H3C", "华为", "思科", "锐捷"] as const;
```

**性能修正**:
- `ReportsPage.tsx:71-90`: `useCallback` → `useMemo`，返回计算值而非函数
- `DevicesPage.tsx:62`: `filteredDevices` 包裹 `useMemo`
- `TemplatesPage.tsx:77-84`: `filteredTemplates` / `filteredCommands` 包裹 `useMemo`

### 3.3 错误处理统一

**新文件 `src/hooks/useInvoke.ts`**:

```ts
import { invoke } from "@tauri-apps/api/core";
import { useState, useCallback } from "react";

export function useInvoke<T>(command: string) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const execute = useCallback(async (args?: Record<string, unknown>): Promise<T | null> => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<T>(command, args);
      return result;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
      return null;
    } finally {
      setLoading(false);
    }
  }, [command]);

  return { execute, loading, error };
}
```

替换各页面的 `.catch(console.error)` 模式，让用户能看到错误信息。

### 3.4 类型收紧

**文件**: `src/types/index.ts`

```ts
export interface InspectionBatch {
  // ...
  status: "pending" | "running" | "completed" | "failed" | "stopped" | "paused" | "waiting" | "in_progress";
}

export interface InspectionRecordSummary {
  // ...
  status: "pending" | "running" | "completed" | "failed" | "skipped";
  ai_status: "none" | "pending" | "completed" | "failed";
}

export interface InspectionRecord {
  // 统一用 | null，不用 ?
  ai_result: string | null;
  ai_analysis: string | null;
  ai_suggestions: string | null;
  command_judgments: string | null;
  summary_judgment: string | null;
  report_path: string | null;
  error_message: string | null;
  started_at: string | null;
  completed_at: string | null;
}
```

## Phase 4: 构建优化

### 4.1 Cargo.toml

```toml
[lib]
crate-type = ["cdylib", "rlib"]  # 移除 staticlib

[dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "sync"] }  # 最小化
serde_yaml → serde_yml = "0.1"  # 迁移至非废弃版
# 移除 log = "0.4"（tracing 已涵盖）
```

### 4.2 vite.config.ts

```ts
build: {
  sourcemap: false,
  rollupOptions: {
    output: {
      manualChunks: {
        react: ["react", "react-dom", "react-router-dom"],
        tauri: ["@tauri-apps/api"],
      },
    },
  },
},
```

### 4.3 tsconfig.json

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "noUncheckedIndexedAccess": true
  }
}
```

### 4.4 .gitignore 补充

```
.claude/
*.tsbuildinfo
coverage/
```

## 变更影响总结

| 维度 | 新增文件 | 修改文件 | 预估改动 |
|------|---------|---------|---------|
| Phase 1 | 0 | 2 | ~50 行 |
| Phase 2 | 0 | 3 | ~200 行重构 |
| Phase 3 | 4 | 8 | ~300 行（含新增 -150 行重复） |
| Phase 4 | 0 | 4 | ~30 行配置 |
