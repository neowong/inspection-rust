import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import Toolbar from "../components/Toolbar";
import DataTable from "../components/DataTable";
import StatusBadge from "../components/StatusBadge";
import Modal from "../components/Modal";
import Button from "../components/ui/Button";
import Input from "../components/ui/Input";
import { Select } from "../components/ui/Input";
import type { AiModelConfig } from "../types";

const PROVIDERS = [
  { value: "openai", label: "OpenAI" },
  { value: "anthropic", label: "Anthropic" },
];

interface ConfigForm {
  name: string;
  provider: string;
  model_id: string;
  api_key: string;
  base_url: string;
}

const EMPTY_FORM: ConfigForm = {
  name: "",
  provider: "openai",
  model_id: "",
  api_key: "",
  base_url: "",
};

function formatTime(ts: string | null) {
  if (!ts) return "-";
  return ts.replace("T", " ").substring(0, 19);
}

export default function AiConfigPage() {
  const [configs, setConfigs] = useState<AiModelConfig[]>([]);
  const [loading, setLoading] = useState(true);
  const [form, setForm] = useState<ConfigForm>(EMPTY_FORM);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<string | null>(null);

  // Delete confirmation
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<AiModelConfig | null>(null);

  const loadConfigs = useCallback(async () => {
    try {
      const list = await invoke<AiModelConfig[]>("list_ai_configs");
      setConfigs(list);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadConfigs(); }, [loadConfigs]);

  const handleSave = async () => {
    if (!form.name.trim() || !form.model_id.trim() || !form.api_key.trim()) return;
    setSaving(true);
    setTestResult(null);
    try {
      await invoke("create_ai_config", {
        name: form.name.trim(),
        provider: form.provider,
        model_id: form.model_id.trim(),
        api_key: form.api_key.trim(),
        base_url: form.base_url.trim() || undefined,
      });
      setForm(EMPTY_FORM);
      await loadConfigs();
    } catch (e) {
      console.error(e);
    } finally {
      setSaving(false);
    }
  };

  const handleTestConnection = async () => {
    if (!form.model_id.trim() || !form.api_key.trim()) {
      setTestResult("请先填写模型ID和API密钥");
      return;
    }
    setTesting(true);
    setTestResult(null);
    try {
      // Test via a simple chat invocation — backend handles this
      await invoke("test_ai_connection", {
        provider: form.provider,
        model_id: form.model_id.trim(),
        api_key: form.api_key.trim(),
        base_url: form.base_url.trim() || undefined,
      });
      setTestResult("连接成功");
    } catch (e: any) {
      setTestResult(typeof e === "string" ? e : (e?.message || e?.toString?.() || "连接失败"));
    } finally {
      setTesting(false);
    }
  };

  const handleActivate = async (config: AiModelConfig) => {
    try {
      await invoke("activate_ai_config", { id: config.id });
      await loadConfigs();
    } catch (e) {
      console.error(e);
    }
  };

  const handleDeactivate = async (config: AiModelConfig) => {
    try {
      await invoke("deactivate_ai_config", { id: config.id });
      await loadConfigs();
    } catch (e) {
      console.error(e);
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await invoke("delete_ai_config", { id: deleteTarget.id });
      setDeleteOpen(false);
      setDeleteTarget(null);
      await loadConfigs();
    } catch (e) {
      console.error(e);
    }
  };

  const columns = useMemo(() => [
    {
      key: "name",
      header: "名称",
      width: "140px",
      render: (c: AiModelConfig) => <span className="font-medium">{c.name}</span>,
    },
    {
      key: "provider",
      header: "提供商",
      width: "80px",
      render: (c: AiModelConfig) => (
        <span className={`inline-block px-1.5 py-0.5 rounded border text-[11px] font-medium ${
          c.provider === "openai"
            ? "bg-emerald-500/15 text-emerald-400 border-emerald-500/30"
            : "bg-purple-500/15 text-purple-400 border-purple-500/30"
        }`}>
          {c.provider === "openai" ? "OpenAI" : "Anthropic"}
        </span>
      ),
    },
    {
      key: "model_id",
      header: "模型ID",
      width: "160px",
      render: (c: AiModelConfig) => <code className="text-[11px] bg-transparent">{c.model_id}</code>,
    },
    {
      key: "active",
      header: "状态",
      width: "60px",
      render: (c: AiModelConfig) => (
        <StatusBadge status={c.is_active ? "online" : "offline"} />
      ),
    },
    {
      key: "created_at",
      header: "创建时间",
      width: "140px",
      render: (c: AiModelConfig) => <span className="text-[hsl(var(--text-secondary))]">{formatTime(c.created_at)}</span>,
    },
    {
      key: "actions",
      header: "操作",
      width: "140px",
      render: (c: AiModelConfig) => (
        <div className="flex gap-1">
          {c.is_active ? (
            <Button variant="secondary" size="sm" onClick={() => handleDeactivate(c)}>停用</Button>
          ) : (
            <Button size="sm" className="bg-emerald-500 hover:bg-emerald-600" onClick={() => handleActivate(c)}>启用</Button>
          )}
          <Button variant="danger" size="sm" onClick={() => { setDeleteTarget(c); setDeleteOpen(true); }}>删除</Button>
        </div>
      ),
    },
  ], [configs]);

  if (loading) return <div className="p-4 text-[hsl(var(--text-secondary))] text-sm">加载中...</div>;

  return (
    <div className="flex flex-col gap-4 h-full overflow-auto">
      <h1 className="text-xl font-semibold text-[hsl(var(--text-primary))]">AI 配置</h1>

      {/* Form Section */}
      <div className="bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg p-4">
        <h2 className="text-sm font-semibold mb-3 text-[hsl(var(--text-primary))]">添加 AI 模型配置</h2>
        <div className="grid grid-cols-2 gap-3 mb-3">
          <FormField label="配置名称 *">
            <Input size="sm" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="例如: 生产环境GPT-4o" />
          </FormField>
          <FormField label="提供商">
            <Select size="sm" value={form.provider} onChange={(e) => setForm({ ...form, provider: e.target.value })}>
              {PROVIDERS.map((p) => (
                <option key={p.value} value={p.value}>{p.label}</option>
              ))}
            </Select>
          </FormField>
          <FormField label="模型 ID *">
            <Input size="sm" value={form.model_id} onChange={(e) => setForm({ ...form, model_id: e.target.value })} placeholder="例如: gpt-4o 或 claude-sonnet-4-20250514" />
          </FormField>
          <FormField label="API 密钥 *">
            <Input size="sm" type="password" value={form.api_key} onChange={(e) => setForm({ ...form, api_key: e.target.value })} placeholder="sk-... 或 sk-ant-..." />
          </FormField>
          <FormField label="Base URL (可选)">
            <Input size="sm" value={form.base_url} onChange={(e) => setForm({ ...form, base_url: e.target.value })} placeholder="默认使用官方地址" />
          </FormField>
        </div>

        {/* Test result */}
        {testResult && (
          <div className={`mb-3 px-3 py-1.5 rounded text-xs ${
            testResult === "连接成功"
              ? "bg-[hsl(var(--success)/0.1)] text-[hsl(var(--success))] border border-[hsl(var(--success)/0.3)]"
              : "bg-[hsl(var(--danger)/0.1)] text-[hsl(var(--danger))] border border-[hsl(var(--danger)/0.3)]"
          }`}>
            {testResult}
          </div>
        )}

        <div className="flex gap-2">
          <Button variant="secondary" size="sm" disabled={testing || !form.model_id.trim() || !form.api_key.trim()} loading={testing}>测试连接</Button>
          <Button size="sm" disabled={saving || !form.name.trim() || !form.model_id.trim() || !form.api_key.trim()} loading={saving}>保存配置</Button>
        </div>
      </div>

      {/* Config List */}
      <div className="flex-1 min-h-0 flex flex-col gap-2">
        <Toolbar>
          <span className="text-xs text-[hsl(var(--text-secondary))]">共 {configs.length} 个配置</span>
        </Toolbar>

        <DataTable
          columns={columns}
          data={configs}
          rowKey={(c) => c.id}
          emptyText="暂无 AI 配置，请在上方表单中添加"
        />
      </div>

      {/* Delete Confirmation Modal */}
      <Modal
        open={deleteOpen}
        title="删除 AI 配置"
        onClose={() => { setDeleteOpen(false); setDeleteTarget(null); }}
        footer={
          <>
            <Button variant="secondary" size="sm" onClick={() => { setDeleteOpen(false); setDeleteTarget(null); }}>取消</Button>
            <Button variant="danger" size="sm" onClick={handleDelete}>确认删除</Button>
          </>
        }
      >
        <p className="text-sm text-[hsl(var(--text-primary))]">
          确定要删除配置「{deleteTarget?.name}」吗？此操作不可撤销。
        </p>
      </Modal>

    </div>
  );
}

function FormField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-xs text-[hsl(var(--text-secondary))]">{label}</span>
      {children}
    </label>
  );
}
