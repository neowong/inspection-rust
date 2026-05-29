import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AiModelConfig } from "../types";
import Toolbar from "../components/Toolbar";
import DataTable from "../components/DataTable";
import Modal from "../components/Modal";
import Button from "../components/ui/Button";
import Input, { Select } from "../components/ui/Input";
import StatusBadge from "../components/StatusBadge";

interface ConfigForm {
  name: string;
  provider: string;
  model_id: string;
  api_key: string;
  base_url: string;
}

const EMPTY_FORM: ConfigForm = { name: "", provider: "openai", model_id: "", api_key: "", base_url: "" };
const PROVIDERS = ["openai", "anthropic"];

export default function AiConfigPage() {
  const [configs, setConfigs] = useState<AiModelConfig[]>([]);
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<AiModelConfig | null>(null);
  const [form, setForm] = useState<ConfigForm>(EMPTY_FORM);
  const [deleteConfirm, setDeleteConfirm] = useState<number | null>(null);

  const loadConfigs = () => {
    invoke<AiModelConfig[]>("list_ai_configs").then(setConfigs).catch(console.error);
  };

  useEffect(() => { loadConfigs(); }, []);

  const openAdd = () => {
    setEditing(null);
    setForm(EMPTY_FORM);
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
    setModalOpen(true);
  };

  const handleSave = () => {
    const data: Record<string, unknown> = {
      name: form.name,
      provider: form.provider,
      model_id: form.model_id,
    };
    if (form.api_key) data.api_key = form.api_key;
    if (form.base_url) data.base_url = form.base_url;

    const promise = editing
      ? invoke<AiModelConfig>("update_ai_config", { configId: editing.id, data })
      : invoke<AiModelConfig>("create_ai_config", { data });

    promise.then(() => { setModalOpen(false); loadConfigs(); }).catch(console.error);
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
      <div>
        <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">AI 配置</h1>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1">管理 AI 模型连接配置</p>
      </div>

      <Toolbar>
        <Button onClick={openAdd} variant="primary" size="sm">添加配置</Button>
      </Toolbar>

      <DataTable<AiModelConfig>
        columns={[
          { key: "name", header: "名称", render: (r) => r.name },
          { key: "provider", header: "Provider", render: (r) => r.provider },
          { key: "model_id", header: "模型", render: (r) => r.model_id },
          { key: "base_url", header: "Base URL", render: (r) => r.base_url || "-" },
          {
            key: "is_active", header: "状态", render: (r) => (
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
        emptyText="暂无 AI 配置"
      />

      <Modal
        open={modalOpen}
        title={editing ? "编辑 AI 配置" : "添加 AI 配置"}
        width="max-w-md"
        onClose={() => setModalOpen(false)}
        footer={
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => setModalOpen(false)}>取消</Button>
            <Button onClick={handleSave}>{editing ? "保存" : "添加"}</Button>
          </div>
        }
      >
        <div className="space-y-3">
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">名称</label>
            <Input value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="例如: OpenAI GPT-4" />
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">Provider</label>
            <Select value={form.provider} onChange={(e) => setForm({ ...form, provider: e.target.value })}>
              {PROVIDERS.map((p) => <option key={p} value={p}>{p}</option>)}
            </Select>
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">Model ID</label>
            <Input value={form.model_id} onChange={(e) => setForm({ ...form, model_id: e.target.value })} placeholder="例如: gpt-4o" />
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">API Key</label>
            <Input type="password" value={form.api_key} onChange={(e) => setForm({ ...form, api_key: e.target.value })} placeholder={editing ? "留空则不修改" : "输入 API Key"} />
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">Base URL（可选）</label>
            <Input value={form.base_url} onChange={(e) => setForm({ ...form, base_url: e.target.value })} placeholder="https://api.openai.com/v1" />
          </div>
        </div>
      </Modal>

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
