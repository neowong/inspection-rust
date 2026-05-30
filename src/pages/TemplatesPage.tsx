import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronRight, ChevronDown, Pencil, Trash2 } from "lucide-react";
import type { InspectionTemplate, CommandPool } from "../types";
import Toolbar from "../components/Toolbar";
import SearchInput from "../components/SearchInput";
import DataTable from "../components/DataTable";
import Modal from "../components/Modal";
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
  const [cmdModal, setCmdModal] = useState(false);
  const [editingCmd, setEditingCmd] = useState<CommandPool | null>(null);
  const [cmdForm, setCmdForm] = useState<CommandForm>(EMPTY_COMMAND_FORM);
  const [confirmDeleteCmd, setConfirmDeleteCmd] = useState<number | null>(null);
  const [cmdSaving, setCmdSaving] = useState(false);
  const [cmdSaveError, setCmdSaveError] = useState<string | null>(null);

  const loadTemplates = () => {
    invoke<InspectionTemplate[]>("list_templates", { vendor: templateVendor || undefined })
      .then(setTemplates).catch(console.error);
  };

  const loadCommands = () => {
    invoke<CommandPool[]>("list_commands", {
      vendor: cmdVendor || undefined,
    }).then(setCommands).catch(console.error);
  };

  useEffect(() => { loadTemplates(); }, [templateVendor]);
  useEffect(() => { loadCommands(); }, [cmdVendor]);

  const filteredTemplates = useMemo(() => templates.filter((t) =>
    !templateSearch || t.name.toLowerCase().includes(templateSearch.toLowerCase())
  ), [templates, templateSearch]);

  const filteredCommands = useMemo(() => commands.filter((c) =>
    !cmdSearch || c.command.toLowerCase().includes(cmdSearch.toLowerCase()) || (c.description && c.description.toLowerCase().includes(cmdSearch.toLowerCase()))
  ), [commands, cmdSearch]);

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

  // Command handlers
  const openAddCmd = () => {
    setEditingCmd(null);
    setCmdForm(EMPTY_COMMAND_FORM);
    setCmdSaveError(null);
    setCmdModal(true);
  };

  const openEditCmd = (c: CommandPool) => {
    setEditingCmd(c);
    setCmdSaveError(null);
    setCmdForm({
      vendor: c.vendor,
      command: c.command,
      description: c.description || "",
      category: c.category || "general",
    });
    setCmdModal(true);
  };

  const handleSaveCommand = () => {
    if (!cmdForm.command.trim()) {
      setCmdSaveError("请输入命令文本");
      return;
    }
    setCmdSaving(true);
    setCmdSaveError(null);
    const promise = editingCmd
      ? invoke<CommandPool>("update_command", { commandId: editingCmd.id, data: { ...cmdForm } })
      : invoke<CommandPool>("create_command", { data: { ...cmdForm } });
    promise
      .then(() => { setCmdModal(false); setCmdForm(EMPTY_COMMAND_FORM); setEditingCmd(null); loadCommands(); })
      .catch((e) => setCmdSaveError(typeof e === "string" ? e : JSON.stringify(e)))
      .finally(() => setCmdSaving(false));
  };

  const handleDeleteCmd = (id: number) => {
    invoke<void>("delete_command", { commandId: id })
      .then(() => { setConfirmDeleteCmd(null); loadCommands(); })
      .catch(console.error);
  };

  return (
    <div className="space-y-6">
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-3 bg-[hsl(var(--bg-content))] shadow-sm relative">
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
            {
              key: "actions", header: "操作", width: "140px", render: (r) => (
                <div className="flex gap-1" onClick={(e) => e.stopPropagation()}>
                  <Button size="sm" variant="ghost" onClick={() => openEditTemplate(r)}>编辑</Button>
                  <Button size="sm" variant="ghost" onClick={() => setConfirmDeleteTemplate(r.id)}>删除</Button>
                </div>
              ),
            },
          ]}
          data={filteredTemplates}
          rowKey={(r) => r.id}
          onRowClick={(r) => setSelectedTemplate(r)}
          onRowDoubleClick={(r) => openEditTemplate(r)}
          selectedKey={selectedTemplate?.id}
          emptyText="暂无模板"
        />
      </div>

      {/* Command Pool Section */}
      <div>
        <h2 className="text-lg font-semibold text-[hsl(var(--text-primary))] mb-3">命令库</h2>
        <Toolbar>
          <Button onClick={openAddCmd} size="sm">添加命令</Button>
          <SearchInput value={cmdSearch} onChange={setCmdSearch} placeholder="搜索命令..." />
        </Toolbar>

        {/* Vendor tabs */}
        <div className="flex gap-1 mb-3 border-b border-[hsl(var(--border))] pb-0">
          {["全部", ...VENDORS].map((v) => (
            <button
              key={v}
              onClick={() => setCmdVendor(v === "全部" ? "" : v)}
              className={`px-3 py-1.5 text-xs font-medium rounded-t-md transition-colors -mb-px ${
                (v === "全部" && !cmdVendor) || v === cmdVendor
                  ? "bg-[hsl(var(--bg-card))] text-[hsl(var(--accent))] border border-b-transparent border-[hsl(var(--border))]"
                  : "text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))]"
              }`}
            >
              {v}
            </button>
          ))}
        </div>

        {/* Grouped commands */}
        <CommandList
          commands={filteredCommands}
          onEdit={openEditCmd}
          onDelete={(id) => setConfirmDeleteCmd(id)}
        />
      </div>

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
                setTemplateForm({ ...templateForm, vendor: newVendor, command_ids: [] });
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
            <Button onClick={handleSaveCommand} loading={cmdSaving}>{editingCmd ? "保存" : "添加"}</Button>
          </div>
        }
      >
        <div className="space-y-3">
          {cmdSaveError && (
            <div className="p-2 bg-[hsl(var(--danger)_/_0.1)] border border-[hsl(var(--danger)_/_0.3)] rounded text-sm text-[hsl(var(--danger))]">
              {cmdSaveError}
            </div>
          )}
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

// ============================================================
// Command List (collapsible by category)
// ============================================================

const CATEGORY_LABELS: Record<string, string> = {
  version: "版本信息",
  clock: "系统时钟",
  cpu: "CPU",
  memory: "内存",
  hardware: "硬件信息",
  interface: "接口",
  vlan: "VLAN",
  log: "日志",
  protocol: "协议",
  general: "通用",
};

function CommandList({
  commands,
  onEdit,
  onDelete,
}: {
  commands: CommandPool[];
  onEdit: (c: CommandPool) => void;
  onDelete: (id: number) => void;
}) {
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  const toggle = (cat: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(cat)) next.delete(cat);
      else next.add(cat);
      return next;
    });
  };

  const grouped = useMemo(() => {
    const map = new Map<string, CommandPool[]>();
    for (const cmd of commands) {
      const cat = cmd.category || "general";
      if (!map.has(cat)) map.set(cat, []);
      map.get(cat)!.push(cmd);
    }
    // Sort categories
    const ordered = [...map.entries()].sort(([a], [b]) => {
      const ia = CATEGORIES.indexOf(a as typeof CATEGORIES[number]);
      const ib = CATEGORIES.indexOf(b as typeof CATEGORIES[number]);
      return (ia === -1 ? 99 : ia) - (ib === -1 ? 99 : ib);
    });
    return ordered;
  }, [commands]);

  if (commands.length === 0) {
    return (
      <div className="text-center py-8 text-sm text-[hsl(var(--text-tertiary))]">
        暂无命令
      </div>
    );
  }

  return (
    <div className="space-y-1">
      {grouped.map(([cat, cmds]) => {
        const open = !collapsed.has(cat);
        return (
          <div key={cat} className="rounded-lg border border-[hsl(var(--border))] overflow-hidden">
            <button
              onClick={() => toggle(cat)}
              className="w-full flex items-center gap-2 px-3 py-2 bg-[hsl(var(--bg-hover))] hover:bg-[hsl(var(--bg-active))] transition-colors text-left"
            >
              {open ? <ChevronDown size={14} className="text-[hsl(var(--text-tertiary))]" /> : <ChevronRight size={14} className="text-[hsl(var(--text-tertiary))]" />}
              <span className="text-xs font-medium text-[hsl(var(--text-primary))]">
                {CATEGORY_LABELS[cat] || cat}
              </span>
              <span className="text-[11px] text-[hsl(var(--text-tertiary))] ml-auto">
                {cmds.length} 条
              </span>
            </button>
            {open && (
              <div className="divide-y divide-[hsl(var(--border-light))]">
                {cmds.map((cmd) => (
                  <div
                    key={cmd.id}
                    className="flex items-center gap-3 px-4 py-2 hover:bg-[hsl(var(--bg-hover))] transition-colors group"
                  >
                    <code className="flex-1 text-xs bg-[hsl(var(--bg-hover))] px-2 py-1 rounded font-mono text-[hsl(var(--text-primary))]">
                      {cmd.command}
                    </code>
                    {cmd.description && (
                      <span className="text-xs text-[hsl(var(--text-tertiary))] max-w-[200px] truncate hidden sm:block">
                        {cmd.description}
                      </span>
                    )}
                    <div className="flex gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
                      <button
                        onClick={() => onEdit(cmd)}
                        className="p-1 rounded hover:bg-[hsl(var(--bg-active))] text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--accent))]"
                        title="编辑"
                      >
                        <Pencil size={13} />
                      </button>
                      <button
                        onClick={() => onDelete(cmd.id)}
                        className="p-1 rounded hover:bg-[hsl(var(--danger)_/_0.1)] text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--danger))]"
                        title="删除"
                      >
                        <Trash2 size={13} />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
