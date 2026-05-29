import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { InspectionTemplate, CommandPool } from "../types";
import Toolbar from "../components/Toolbar";
import SearchInput from "../components/SearchInput";
import DataTable from "../components/DataTable";
import Modal from "../components/Modal";
import ContextMenu, { type ContextMenuItem } from "../components/ContextMenu";
import Button from "../components/ui/Button";
import Input, { Select } from "../components/ui/Input";
import { VENDORS, CATEGORIES } from "../lib/constants";

interface TemplateForm {
  name: string;
  vendor: string;
  model: string;
  device_type: string;
  description: string;
  command_ids: number[];
}

const EMPTY_TEMPLATE_FORM: TemplateForm = {
  name: "", vendor: "H3C", model: "", device_type: "", description: "", command_ids: [],
};

interface CommandForm {
  vendor: string;
  command: string;
  description: string;
  category: string;
}

const EMPTY_COMMAND_FORM: CommandForm = {
  vendor: "H3C", command: "", description: "", category: "general",
};

export default function TemplatesPage() {
  // Template state
  const [templates, setTemplates] = useState<InspectionTemplate[]>([]);
  const [templateSearch, setTemplateSearch] = useState("");
  const [templateVendor, setTemplateVendor] = useState("");
  const [selectedTemplate, setSelectedTemplate] = useState<InspectionTemplate | null>(null);
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number } | null>(null);
  const [templateModal, setTemplateModal] = useState(false);
  const [editingTemplate, setEditingTemplate] = useState<InspectionTemplate | null>(null);
  const [templateForm, setTemplateForm] = useState<TemplateForm>(EMPTY_TEMPLATE_FORM);
  const [confirmDeleteTemplate, setConfirmDeleteTemplate] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  // Command pool state
  const [commands, setCommands] = useState<CommandPool[]>([]);
  const [cmdSearch, setCmdSearch] = useState("");
  const [cmdVendor, setCmdVendor] = useState("");
  const [cmdCategory, setCmdCategory] = useState("");
  const [cmdModal, setCmdModal] = useState(false);
  const [editingCmd, setEditingCmd] = useState<CommandPool | null>(null);
  const [cmdForm, setCmdForm] = useState<CommandForm>(EMPTY_COMMAND_FORM);
  const [confirmDeleteCmd, setConfirmDeleteCmd] = useState<number | null>(null);
  const [cmdCtxMenu, setCmdCtxMenu] = useState<{ x: number; y: number } | null>(null);
  const [selectedCmd, setSelectedCmd] = useState<CommandPool | null>(null);

  const loadTemplates = () => {
    invoke<InspectionTemplate[]>("list_templates", { vendor: templateVendor || undefined })
      .then(setTemplates).catch(console.error);
  };

  const loadCommands = () => {
    invoke<CommandPool[]>("list_commands", {
      vendor: cmdVendor || undefined,
      category: cmdCategory || undefined,
    }).then(setCommands).catch(console.error);
  };

  useEffect(() => { loadTemplates(); }, [templateVendor]);
  useEffect(() => { loadCommands(); }, [cmdVendor, cmdCategory]);

  // Filter templates
  const filteredTemplates = useMemo(() => templates.filter((t) =>
    !templateSearch || t.name.toLowerCase().includes(templateSearch.toLowerCase())
  ), [templates, templateSearch]);

  // Filter commands
  const filteredCommands = useMemo(() => commands.filter((c) =>
    !cmdSearch || c.command.toLowerCase().includes(cmdSearch.toLowerCase()) || (c.description && c.description.toLowerCase().includes(cmdSearch.toLowerCase()))
  ), [commands, cmdSearch]);

  // Filter commands by template vendor in modal
  const vendorFilteredCommands = useMemo(() => commands.filter((c) =>
    c.vendor === templateForm.vendor
  ), [commands, templateForm.vendor]);

  // Template handlers
  const openAddTemplate = () => {
    setEditingTemplate(null);
    setTemplateForm(EMPTY_TEMPLATE_FORM);
    setTemplateModal(true);
  };

  const openEditTemplate = (t: InspectionTemplate) => {
    setEditingTemplate(t);
    setTemplateForm({
      name: t.name,
      vendor: t.vendor,
      model: t.model || "",
      device_type: t.device_type || "",
      description: t.description || "",
      command_ids: t.config?.command_ids || [],
    });
    setTemplateModal(true);
  };

  const handleSaveTemplate = () => {
    if (!templateForm.name.trim()) {
      setSaveError("请输入模板名称");
      return;
    }

    const data: Record<string, unknown> = {
      name: templateForm.name,
      vendor: templateForm.vendor,
      config: JSON.stringify({ command_ids: templateForm.command_ids }),
    };
    if (templateForm.model) data.model = templateForm.model;
    if (templateForm.device_type) data.device_type = templateForm.device_type;
    if (templateForm.description) data.description = templateForm.description;

    setSaving(true);
    setSaveError(null);

    const promise = editingTemplate
      ? invoke<InspectionTemplate>("update_template", { templateId: editingTemplate.id, data })
      : invoke<InspectionTemplate>("create_template", { data });

    promise
      .then(() => {
        setTemplateModal(false);
        loadTemplates();
      })
      .catch((e) => {
        setSaveError(typeof e === "string" ? e : JSON.stringify(e));
      })
      .finally(() => setSaving(false));
  };

  const handleDeleteTemplate = (id: number) => {
    invoke<void>("delete_template", { templateId: id })
      .then(() => { setConfirmDeleteTemplate(null); loadTemplates(); })
      .catch(console.error);
  };

  const handleAutoGenerate = (t: InspectionTemplate) => {
    invoke<{ command_ids: number[] }>("auto_generate_template", {
      vendor: t.vendor,
      model: t.model || undefined,
      device_type: t.device_type || undefined,
    }).then(() => loadTemplates()).catch(console.error);
  };

  // Command handlers
  const openAddCmd = () => {
    setEditingCmd(null);
    setCmdForm(EMPTY_COMMAND_FORM);
    setCmdModal(true);
  };

  const openEditCmd = (c: CommandPool) => {
    setEditingCmd(c);
    setCmdForm({
      vendor: c.vendor,
      command: c.command,
      description: c.description || "",
      category: c.category || "general",
    });
    setCmdModal(true);
  };

  const handleSaveCommand = () => {
    if (!cmdForm.command.trim()) return;
    const promise = editingCmd
      ? invoke<CommandPool>("update_command", { commandId: editingCmd.id, data: { ...cmdForm } })
      : invoke<CommandPool>("create_command", { data: { ...cmdForm } });
    promise
      .then(() => { setCmdModal(false); setCmdForm(EMPTY_COMMAND_FORM); setEditingCmd(null); loadCommands(); })
      .catch(console.error);
  };

  const handleDeleteCmd = (id: number) => {
    invoke<void>("delete_command", { commandId: id })
      .then(() => { setConfirmDeleteCmd(null); loadCommands(); })
      .catch(console.error);
  };

  const handleCmdCtx = (e: React.MouseEvent, c: CommandPool) => {
    e.preventDefault();
    setSelectedCmd(c);
    setCmdCtxMenu({ x: e.clientX, y: e.clientY });
  };

  const cmdCtxItems: ContextMenuItem[] = selectedCmd
    ? [
        { label: "编辑", action: () => openEditCmd(selectedCmd) },
        { label: "", separator: true },
        { label: "删除", danger: true, action: () => setConfirmDeleteCmd(selectedCmd.id) },
      ]
    : [];

  const handleTemplateCtx = (e: React.MouseEvent, t: InspectionTemplate) => {
    e.preventDefault();
    setSelectedTemplate(t);
    setCtxMenu({ x: e.clientX, y: e.clientY });
  };

  const ctxItems: ContextMenuItem[] = selectedTemplate
    ? [
        { label: "编辑", action: () => openEditTemplate(selectedTemplate) },
        { label: "", separator: true },
        { label: "自动生成命令", action: () => handleAutoGenerate(selectedTemplate) },
        { label: "", separator: true },
        { label: "删除", danger: true, action: () => setConfirmDeleteTemplate(selectedTemplate.id) },
      ]
    : [];

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">巡检模板</h1>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1">管理巡检模板和命令库</p>
      </div>

      {/* Templates Section */}
      <div>
        <h2 className="text-lg font-semibold text-[hsl(var(--text-primary))] mb-3">模板列表</h2>
        <Toolbar>
          <Button onClick={openAddTemplate} size="sm">添加模板</Button>
          <Select className="w-28" value={templateVendor} onChange={(e) => setTemplateVendor(e.target.value)}>
            <option value="">全部厂商</option>
            {VENDORS.map((v) => <option key={v} value={v}>{v}</option>)}
          </Select>
          <SearchInput value={templateSearch} onChange={setTemplateSearch} placeholder="搜索模板..." />
        </Toolbar>
        <DataTable<InspectionTemplate>
          columns={[
            { key: "name", header: "名称", render: (r) => r.name },
            { key: "vendor", header: "厂商", render: (r) => r.vendor },
            {
              key: "command_count", header: "命令数", width: "80px", render: (r) =>
                String((r.config?.command_ids || []).length),
            },
            { key: "description", header: "描述", render: (r) => r.description || "-" },
            {
              key: "updated_at", header: "更新时间", render: (r) =>
                new Date(r.updated_at).toLocaleString("zh-CN"),
            },
          ]}
          data={filteredTemplates}
          rowKey={(r) => r.id}
          onRowClick={(r) => setSelectedTemplate(r)}
          onRowDoubleClick={(r) => openEditTemplate(r)}
          onContextMenu={handleTemplateCtx}
          selectedKey={selectedTemplate?.id}
          emptyText="暂无模板"
        />
      </div>

      {/* Command Pool Section */}
      <div>
        <h2 className="text-lg font-semibold text-[hsl(var(--text-primary))] mb-3">命令库</h2>
        <Toolbar>
          <Button onClick={openAddCmd} size="sm">添加命令</Button>
          <Select className="w-28" value={cmdVendor} onChange={(e) => setCmdVendor(e.target.value)}>
            <option value="">全部厂商</option>
            {VENDORS.map((v) => <option key={v} value={v}>{v}</option>)}
          </Select>
          <Select className="w-28" value={cmdCategory} onChange={(e) => setCmdCategory(e.target.value)}>
            <option value="">全部分类</option>
            {CATEGORIES.map((c) => <option key={c} value={c}>{c}</option>)}
          </Select>
          <SearchInput value={cmdSearch} onChange={setCmdSearch} placeholder="搜索命令..." />
        </Toolbar>
        <DataTable<CommandPool>
          columns={[
            { key: "vendor", header: "厂商", width: "80px", render: (r) => r.vendor },
            { key: "command", header: "命令", render: (r) => <code className="text-xs bg-[hsl(var(--bg-hover))] px-1.5 py-0.5 rounded">{r.command}</code> },
            { key: "description", header: "描述", render: (r) => r.description || "-" },
            { key: "category", header: "分类", width: "100px", render: (r) => r.category || "-" },
          ]}
          data={filteredCommands}
          rowKey={(r) => r.id}
          onRowDoubleClick={(r) => openEditCmd(r)}
          onContextMenu={handleCmdCtx}
          emptyText="暂无命令"
        />
      </div>

      <ContextMenu
        items={ctxItems}
        visible={ctxMenu !== null}
        x={ctxMenu?.x ?? 0}
        y={ctxMenu?.y ?? 0}
        onClose={() => setCtxMenu(null)}
      />

      <ContextMenu
        items={cmdCtxItems}
        visible={cmdCtxMenu !== null}
        x={cmdCtxMenu?.x ?? 0}
        y={cmdCtxMenu?.y ?? 0}
        onClose={() => setCmdCtxMenu(null)}
      />

      {/* Template Modal */}
      <Modal
        open={templateModal}
        title={editingTemplate ? "编辑模板" : "添加模板"}
        width="max-w-lg"
        onClose={() => setTemplateModal(false)}
        footer={
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => setTemplateModal(false)}>取消</Button>
            <Button onClick={handleSaveTemplate} loading={saving}>{editingTemplate ? "保存" : "添加"}</Button>
          </div>
        }
      >
        <div className="space-y-3">
          {saveError && (
            <div className="p-2 bg-[hsl(var(--danger)_/_0.1)] border border-[hsl(var(--danger)_/_0.3)] rounded text-sm text-[hsl(var(--danger))]">
              {saveError}
            </div>
          )}
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">名称</label>
              <Input value={templateForm.name} onChange={(e) => { setTemplateForm({ ...templateForm, name: e.target.value }); setSaveError(null); }} />
            </div>
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">厂商</label>
              <Select value={templateForm.vendor} onChange={(e) => {
                const newVendor = e.target.value;
                // 切换厂商时清空已选命令（不同厂商的命令 ID 不同）
                setTemplateForm({
                  ...templateForm,
                  vendor: newVendor,
                  command_ids: [],
                });
              }}>
                {VENDORS.map((v) => <option key={v} value={v}>{v}</option>)}
              </Select>
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">型号（可选）</label>
              <Input value={templateForm.model} onChange={(e) => setTemplateForm({ ...templateForm, model: e.target.value })} />
            </div>
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">设备类型（可选）</label>
              <Input value={templateForm.device_type} onChange={(e) => setTemplateForm({ ...templateForm, device_type: e.target.value })} />
            </div>
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">描述（可选）</label>
            <Input value={templateForm.description} onChange={(e) => setTemplateForm({ ...templateForm, description: e.target.value })} />
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-2">选择命令 ({templateForm.vendor})</label>
            <div className="max-h-48 overflow-y-auto border border-[hsl(var(--border))] rounded-md p-2 space-y-1">
              {vendorFilteredCommands.length === 0 && <p className="text-xs text-[hsl(var(--text-tertiary))]">暂无 {templateForm.vendor} 命令，请先在命令库中添加</p>}
              {vendorFilteredCommands.map((cmd) => {
                const checked = templateForm.command_ids.includes(cmd.id);
                return (
                  <label key={cmd.id} className="flex items-center gap-2 cursor-pointer hover:bg-[hsl(var(--bg-hover))] rounded px-1 py-0.5">
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() => {
                        setTemplateForm({
                          ...templateForm,
                          command_ids: checked
                            ? templateForm.command_ids.filter((id) => id !== cmd.id)
                            : [...templateForm.command_ids, cmd.id],
                        });
                      }}
                      className="accent-[hsl(var(--accent))]"
                    />
                    <span className="text-xs">
                      <code className="bg-[hsl(var(--bg-hover))] px-1 rounded">{cmd.command}</code>
                      {cmd.description && <span className="text-[hsl(var(--text-tertiary))] ml-1">— {cmd.description}</span>}
                    </span>
                  </label>
                );
              })}
            </div>
          </div>
        </div>
      </Modal>

      {/* Delete Template Confirm */}
      <Modal
        open={confirmDeleteTemplate !== null}
        title="确认删除"
        width="max-w-sm"
        onClose={() => setConfirmDeleteTemplate(null)}
        footer={
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => setConfirmDeleteTemplate(null)}>取消</Button>
            <Button variant="danger" onClick={() => handleDeleteTemplate(confirmDeleteTemplate!)}>删除</Button>
          </div>
        }
      >
        <p>确定要删除此模板吗？此操作不可恢复。</p>
      </Modal>

      {/* Command Modal */}
      <Modal
        open={cmdModal}
        title={editingCmd ? "编辑命令" : "添加命令"}
        width="max-w-md"
        onClose={() => { setCmdModal(false); setEditingCmd(null); }}
        footer={
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => { setCmdModal(false); setEditingCmd(null); }}>取消</Button>
            <Button onClick={handleSaveCommand}>{editingCmd ? "保存" : "添加"}</Button>
          </div>
        }
      >
        <div className="space-y-3">
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">厂商</label>
            <Select value={cmdForm.vendor} onChange={(e) => setCmdForm({ ...cmdForm, vendor: e.target.value })}>
              {VENDORS.map((v) => <option key={v} value={v}>{v}</option>)}
            </Select>
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">命令文本</label>
            <Input value={cmdForm.command} onChange={(e) => setCmdForm({ ...cmdForm, command: e.target.value })} placeholder="display version" />
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">描述（可选）</label>
            <Input value={cmdForm.description} onChange={(e) => setCmdForm({ ...cmdForm, description: e.target.value })} />
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">分类</label>
            <Select value={cmdForm.category} onChange={(e) => setCmdForm({ ...cmdForm, category: e.target.value })}>
              {CATEGORIES.map((c) => <option key={c} value={c}>{c}</option>)}
            </Select>
          </div>
        </div>
      </Modal>

      {/* Delete Command Confirm */}
      <Modal
        open={confirmDeleteCmd !== null}
        title="确认删除"
        width="max-w-sm"
        onClose={() => setConfirmDeleteCmd(null)}
        footer={
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => setConfirmDeleteCmd(null)}>取消</Button>
            <Button variant="danger" onClick={() => handleDeleteCmd(confirmDeleteCmd!)}>删除</Button>
          </div>
        }
      >
        <p>确定要删除此命令吗？此操作不可恢复。</p>
      </Modal>
    </div>
  );
}
