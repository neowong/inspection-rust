import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AiModelConfig } from "../types";
import Card from "../components/ui/Card";
import Input, { Select } from "../components/ui/Input";
import Button from "../components/ui/Button";
import DataTable from "../components/DataTable";
import Modal from "../components/Modal";
import StatusBadge from "../components/StatusBadge";

interface SystemSettings {
  report_max_output_lines: number;
}

interface ConfigForm {
  name: string;
  provider: string;
  model_id: string;
  api_key: string;
  base_url: string;
}

const EMPTY_FORM: ConfigForm = { name: "", provider: "deepseek", model_id: "deepseek-chat", api_key: "", base_url: "" };
const PROVIDERS = ["deepseek", "openai", "anthropic"];
const PROVIDER_LABELS: Record<string, string> = {
  deepseek: "DeepSeek",
  openai: "OpenAI",
  anthropic: "Anthropic",
};

export default function SettingsPage() {
  // System settings state
  const [reportMaxOutputLines, setReportMaxOutputLines] = useState(100);
  const [settingsLoading, setSettingsLoading] = useState(false);
  const [settingsSaved, setSettingsSaved] = useState(false);

  // AI config state
  const [configs, setConfigs] = useState<AiModelConfig[]>([]);
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<AiModelConfig | null>(null);
  const [form, setForm] = useState<ConfigForm>(EMPTY_FORM);
  const [deleteConfirm, setDeleteConfirm] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [shakeFields, setShakeFields] = useState<Set<string>>(new Set());

  const triggerShake = (field: string) => {
    setShakeFields((prev) => new Set(prev).add(field));
    setTimeout(() => setShakeFields((prev) => {
      const next = new Set(prev);
      next.delete(field);
      return next;
    }), 600);
  };

  // Load system settings
  useEffect(() => {
    invoke<SystemSettings>("get_settings")
      .then((s) => setReportMaxOutputLines(s.report_max_output_lines))
      .catch(console.error);
  }, []);

  // Load AI configs
  const loadConfigs = () => {
    invoke<AiModelConfig[]>("list_ai_configs").then(setConfigs).catch(console.error);
  };

  useEffect(() => { loadConfigs(); }, []);

  // Save system settings
  const handleSaveSettings = () => {
    setSettingsLoading(true);
    setSettingsSaved(false);
    invoke<void>("update_settings", { reportMaxOutputLines })
      .then(() => {
        setSettingsSaved(true);
        setTimeout(() => setSettingsSaved(false), 2000);
      })
      .catch(console.error)
      .finally(() => setSettingsLoading(false));
  };

  // AI config handlers
  const openAdd = () => {
    setEditing(null);
    setForm(EMPTY_FORM);
    setSaveError(null);
    setModalOpen(true);
  };

  const openEdit = (cfg: AiModelConfig) => {
    setEditing(cfg);
    setForm({
      name: cfg.name,
      provider: cfg.provider,
      model_id: cfg.model_id,
      api_key: "",
      base_url: cfg.base_url || "",
    });
    setSaveError(null);
    setModalOpen(true);
  };

  const handleSave = () => {
    if (!form.name.trim()) { triggerShake("name"); return; }
    if (!form.model_id.trim()) { triggerShake("model_id"); return; }
    if (!editing && !form.api_key.trim()) { triggerShake("api_key"); return; }

    const data: Record<string, unknown> = {
      name: form.name,
      provider: form.provider,
      model_id: form.model_id,
    };
    if (form.api_key) data.api_key_encrypted = form.api_key;
    if (form.base_url) data.base_url = form.base_url;

    setSaving(true);
    setSaveError(null);

    const promise = editing
      ? invoke<AiModelConfig>("update_ai_config", { configId: editing.id, data })
      : invoke<AiModelConfig>("create_ai_config", { data });

    promise
      .then(() => {
        setModalOpen(false);
        loadConfigs();
      })
      .catch((e) => {
        setSaveError(typeof e === "string" ? e : JSON.stringify(e));
      })
      .finally(() => setSaving(false));
  };

  const handleDelete = (id: number) => {
    invoke<void>("delete_ai_config", { configId: id })
      .then(() => { setDeleteConfirm(null); loadConfigs(); })
      .catch(console.error);
  };

  const handleActivate = (id: number) => {
    invoke<void>("activate_ai_config", { configId: id }).then(loadConfigs).catch(console.error);
  };

  const handleDeactivate = (id: number) => {
    invoke<void>("deactivate_ai_config", { configId: id }).then(loadConfigs).catch(console.error);
  };

  return (
    <div className="space-y-6">
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm relative">
        <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">系统设置</h1>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1">配置系统运行参数和 AI 模型</p>
      </div>

      {/* System Settings Section */}
      <Card className="max-w-lg">
        <h2 className="text-base font-semibold text-[hsl(var(--text-primary))] mb-4">基本设置</h2>
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-[hsl(var(--text-primary))] mb-1.5">
              报告最大输出行数
            </label>
            <Input
              type="number"
              value={reportMaxOutputLines}
              onChange={(e) => setReportMaxOutputLines(Number(e.target.value))}
              min={10}
              max={10000}
            />
            <p className="text-xs text-[hsl(var(--text-tertiary))] mt-1">
              生成报告时每台设备命令输出的最大行数，超出部分将被截断
            </p>
          </div>
          <div className="flex items-center gap-3">
            <Button onClick={handleSaveSettings} loading={settingsLoading}>
              保存设置
            </Button>
            {settingsSaved && (
              <span className="text-sm text-[hsl(var(--success))]">设置已保存</span>
            )}
          </div>
        </div>
      </Card>

      {/* AI Config Section */}
      <Card>
        <div className="flex items-center justify-between mb-4">
          <div>
            <h2 className="text-base font-semibold text-[hsl(var(--text-primary))]">AI 模型配置</h2>
            <p className="text-xs text-[hsl(var(--text-tertiary))] mt-0.5">管理用于巡检分析的 AI 模型连接</p>
          </div>
          <Button onClick={openAdd} size="sm">添加配置</Button>
        </div>

        <DataTable<AiModelConfig>
          columns={[
            { key: "name", header: "名称", render: (r) => r.name },
            { key: "provider", header: "Provider", render: (r) => r.provider },
            { key: "model_id", header: "模型", render: (r) => r.model_id },
            { key: "base_url", header: "Base URL", render: (r) => r.base_url || "-" },
            {
              key: "is_active", header: "状态", width: "80px", render: (r) => (
                <StatusBadge status={r.is_active ? "online" : "offline"} />
              ),
            },
            {
              key: "actions", header: "操作", width: "180px", render: (r) => (
                <div className="flex gap-1" onClick={(e) => e.stopPropagation()}>
                  <Button size="sm" variant="ghost" onClick={() => openEdit(r)}>编辑</Button>
                  {r.is_active
                    ? <Button size="sm" variant="ghost" onClick={() => handleDeactivate(r.id)}>停用</Button>
                    : <Button size="sm" variant="ghost" onClick={() => handleActivate(r.id)}>激活</Button>
                  }
                  <Button size="sm" variant="ghost" onClick={() => setDeleteConfirm(r.id)}>删除</Button>
                </div>
              ),
            },
          ]}
          data={configs}
          rowKey={(r) => r.id}
          emptyText="暂无 AI 配置，点击上方按钮添加"
        />
      </Card>

      {/* AI Config Modal */}
      <Modal
        open={modalOpen}
        title={editing ? "编辑 AI 配置" : "添加 AI 配置"}
        width="max-w-md"
        onClose={() => setModalOpen(false)}
        footer={
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => setModalOpen(false)}>取消</Button>
            <Button onClick={handleSave} loading={saving}>{editing ? "保存" : "添加"}</Button>
          </div>
        }
      >
        <div className="space-y-3">
          {saveError && (
            <div className="p-2 bg-[hsl(var(--danger)_/_0.1)] border border-[hsl(var(--danger)_/_0.3)] rounded text-sm text-[hsl(var(--danger))]">
              {saveError}
            </div>
          )}
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">名称</label>
            <Input value={form.name} className={shakeFields.has("name") ? "animate-shake" : ""} onChange={(e) => { setForm({ ...form, name: e.target.value }); setSaveError(null); }} placeholder="例如: OpenAI GPT-4" />
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">Provider</label>
            <Select value={form.provider} onChange={(e) => setForm({ ...form, provider: e.target.value })}>
              {PROVIDERS.map((p) => <option key={p} value={p}>{PROVIDER_LABELS[p] || p}</option>)}
            </Select>
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">Model ID</label>
            <Input value={form.model_id} className={shakeFields.has("model_id") ? "animate-shake" : ""} onChange={(e) => { setForm({ ...form, model_id: e.target.value }); setSaveError(null); }} placeholder={form.provider === "deepseek" ? "deepseek-chat" : form.provider === "openai" ? "gpt-4o" : "claude-sonnet-4-20250514"} />
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">API Key</label>
            <Input type="password" value={form.api_key} className={shakeFields.has("api_key") ? "animate-shake" : ""} onChange={(e) => { setForm({ ...form, api_key: e.target.value }); setSaveError(null); }} placeholder={editing ? "留空则不修改" : "输入 API Key"} />
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">Base URL（可选）</label>
            <Input value={form.base_url} onChange={(e) => setForm({ ...form, base_url: e.target.value })} placeholder="https://api.openai.com/v1" />
          </div>
        </div>
      </Modal>

      {/* Delete Confirm Modal */}
      <Modal
        open={deleteConfirm !== null}
        title="确认删除"
        width="max-w-sm"
        onClose={() => setDeleteConfirm(null)}
        footer={
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => setDeleteConfirm(null)}>取消</Button>
            <Button variant="danger" onClick={() => handleDelete(deleteConfirm!)}>删除</Button>
          </div>
        }
      >
        <p>确定要删除此 AI 配置吗？此操作不可恢复。</p>
      </Modal>
    </div>
  );
}
