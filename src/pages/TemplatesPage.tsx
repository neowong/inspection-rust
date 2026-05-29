import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import Toolbar from "../components/Toolbar";
import SearchInput from "../components/SearchInput";
import Modal from "../components/Modal";
import ContextMenu, { ContextMenuItem } from "../components/ContextMenu";
import Button from "../components/ui/Button";
import Input from "../components/ui/Input";
import { Select } from "../components/ui/Input";
import type { InspectionTemplate, CommandPool } from "../types";

const VENDORS = ["H3C", "华为", "思科", "锐捷"];
interface TemplateForm {
  name: string;
  vendor: string;
  model: string;
  device_type: string;
  description: string;
}

const EMPTY_FORM: TemplateForm = {
  name: "",
  vendor: "H3C",
  model: "",
  device_type: "",
  description: "",
};

function formatTime(ts: string | null) {
  if (!ts) return "-";
  return ts.replace("T", " ").substring(0, 19);
}

export default function TemplatesPage() {
  const [templates, setTemplates] = useState<InspectionTemplate[]>([]);
  const [commands, setCommands] = useState<CommandPool[]>([]);
  const [loading, setLoading] = useState(true);

  // Selection
  const [selectedId, setSelectedId] = useState<number | null>(null);

  // Context menu
  const [ctxVisible, setCtxVisible] = useState(false);
  const [ctxPos, setCtxPos] = useState({ x: 0, y: 0 });
  const [ctxTemplate, setCtxTemplate] = useState<InspectionTemplate | null>(null);

  // Center panel: form state
  const [form, setForm] = useState<TemplateForm>(EMPTY_FORM);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);

  // Right panel: command pool filters
  const [vendorFilter, setVendorFilter] = useState<string>("all");
  const [cmdSearch, setCmdSearch] = useState("");

  // Modals
  const [newModalOpen, setNewModalOpen] = useState(false);
  const [newForm, setNewForm] = useState<TemplateForm>(EMPTY_FORM);

  const [renameModalOpen, setRenameModalOpen] = useState(false);
  const [renameName, setRenameName] = useState("");

  const [deleteModalOpen, setDeleteModalOpen] = useState(false);

  // Load data
  const loadData = useCallback(async () => {
    try {
      const [tpls, cmds] = await Promise.all([
        invoke<InspectionTemplate[]>("list_templates"),
        invoke<CommandPool[]>("list_commands"),
      ]);
      setTemplates(tpls);
      setCommands(cmds);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadData(); }, [loadData]);

  // Get selected template
  const selected = useMemo(
    () => templates.find((t) => t.id === selectedId) ?? null,
    [templates, selectedId]
  );

  // Command IDs from selected template's config
  const templateCommandIds: number[] = useMemo(() => {
    if (!selected?.config?.command_ids) return [];
    return selected.config.command_ids;
  }, [selected]);

  // Commands referenced by the selected template
  const templateCommands = useMemo(() => {
    const idSet = new Set(templateCommandIds);
    return commands.filter((c) => idSet.has(c.id));
  }, [commands, templateCommandIds]);

  // When selection changes, populate form
  useEffect(() => {
    if (selected) {
      setForm({
        name: selected.name,
        vendor: selected.vendor,
        model: selected.model || "",
        device_type: selected.device_type || "",
        description: selected.description || "",
      });
      setDirty(false);
    } else {
      setForm(EMPTY_FORM);
      setDirty(false);
    }
  }, [selectedId]); // eslint-disable-line react-hooks/exhaustive-deps

  // Filtered commands for right panel
  const filteredCommands = useMemo(() => {
    let list = commands;
    if (vendorFilter !== "all") {
      list = list.filter((c) => c.vendor === vendorFilter);
    }
    if (cmdSearch.trim()) {
      const kw = cmdSearch.trim().toLowerCase();
      list = list.filter(
        (c) =>
          c.command.toLowerCase().includes(kw) ||
          (c.description && c.description.toLowerCase().includes(kw)) ||
          (c.category && c.category.toLowerCase().includes(kw))
      );
    }
    return list;
  }, [commands, vendorFilter, cmdSearch]);

  // Form change handler
  const updateForm = (patch: Partial<TemplateForm>) => {
    setForm((prev) => ({ ...prev, ...patch }));
    setDirty(true);
  };

  // Select template
  const selectTemplate = (id: number) => {
    if (dirty) {
      if (!window.confirm("当前模板有未保存的修改，是否放弃？")) return;
    }
    setSelectedId(id);
  };

  // --- Context menu ---
  const onContextMenu = useCallback((e: React.MouseEvent, tpl: InspectionTemplate) => {
    e.preventDefault();
    setCtxPos({ x: e.clientX, y: e.clientY });
    setCtxTemplate(tpl);
    setCtxVisible(true);
  }, []);

  const ctxItems: ContextMenuItem[] = useMemo(() => {
    const t = ctxTemplate;
    if (!t) return [];
    return [
      { label: "编辑模板", action: () => selectTemplate(t.id) },
      { label: "复制模板", action: () => handleCopy(t) },
      { label: "重命名", action: () => { setRenameName(t.name); setRenameModalOpen(true); } },
      { label: "-", separator: true },
      { label: "删除模板", danger: true, action: () => { setDeleteModalOpen(true); } },
    ] as ContextMenuItem[];
  }, [ctxTemplate]); // eslint-disable-line react-hooks/exhaustive-deps

  // --- Actions ---
  const handleCreate = async () => {
    if (!newForm.name.trim()) return;
    try {
      await invoke("create_template", {
        data: {
          name: newForm.name.trim(),
          vendor: newForm.vendor,
          model: newForm.model.trim() || null,
          device_type: newForm.device_type.trim() || null,
          template_type: "ssh",
          description: newForm.description.trim() || null,
          config: { command_ids: [] },
        },
      });
      setNewModalOpen(false);
      setNewForm(EMPTY_FORM);
      await loadData();
    } catch (e) {
      console.error(e);
      alert(String(e));
    }
  };

  const handleCopy = async (tpl: InspectionTemplate) => {
    try {
      await invoke("create_template", {
        data: {
          name: tpl.name + " (副本)",
          vendor: tpl.vendor,
          model: tpl.model || null,
          device_type: tpl.device_type || null,
          template_type: "ssh",
          description: tpl.description || null,
          config: tpl.config || { command_ids: [] },
        },
      });
      await loadData();
    } catch (e) {
      console.error(e);
      alert(String(e));
    }
  };

  const handleRename = async () => {
    if (!renameName.trim() || !ctxTemplate) return;
    try {
      await invoke("update_template", {
        templateId: ctxTemplate.id,
        data: { name: renameName.trim() },
      });
      setRenameModalOpen(false);
      await loadData();
    } catch (e) {
      console.error(e);
      alert(String(e));
    }
  };

  const handleDelete = async () => {
    if (!ctxTemplate) return;
    try {
      await invoke("delete_template", { templateId: ctxTemplate.id });
      setDeleteModalOpen(false);
      if (selectedId === ctxTemplate.id) setSelectedId(null);
      setCtxTemplate(null);
      await loadData();
    } catch (e) {
      console.error(e);
      alert(String(e));
    }
  };

  // Save current template
  const handleSave = async () => {
    if (!selected || !form.name.trim()) return;
    setSaving(true);
    try {
      await invoke("update_template", {
        templateId: selected.id,
        data: {
          name: form.name.trim(),
          vendor: form.vendor,
          model: form.model.trim() || null,
          device_type: form.device_type.trim() || null,
          template_type: "ssh",
          description: form.description.trim() || null,
        },
      });
      setDirty(false);
      await loadData();
    } catch (e) {
      console.error(e);
      alert(String(e));
    } finally {
      setSaving(false);
    }
  };

  // Add command to template
  const handleAddCommand = async (cmdId: number) => {
    if (!selected) return;
    if (templateCommandIds.includes(cmdId)) return;
    const newIds = [...templateCommandIds, cmdId];
    try {
      await invoke("update_template", {
        templateId: selected.id,
        data: { config: { command_ids: newIds } },
      });
      await loadData();
      // Re-select to refresh
      setSelectedId(selected.id);
    } catch (e) {
      console.error(e);
      alert(String(e));
    }
  };

  // Remove command from template
  const handleRemoveCommand = async (cmdId: number) => {
    if (!selected) return;
    const newIds = templateCommandIds.filter((id) => id !== cmdId);
    try {
      await invoke("update_template", {
        templateId: selected.id,
        data: { config: { command_ids: newIds } },
      });
      await loadData();
      setSelectedId(selected.id);
    } catch (e) {
      console.error(e);
      alert(String(e));
    }
  };

  // Auto-generate from command pool
  const handleAutoGenerate = async () => {
    if (!selected) return;
    try {
      const result = await invoke<{ config: { command_ids: number[] } }>("auto_generate_template", {
        vendor: selected.vendor,
        model: selected.model || null,
        deviceType: selected.device_type || null,
      });
      if (result.config?.command_ids) {
        await invoke("update_template", {
          templateId: selected.id,
          data: { config: result.config },
        });
        await loadData();
        setSelectedId(selected.id);
      }
    } catch (e) {
      console.error(e);
      alert(String(e));
    }
  };

  // Generate report template
  const handleGenerateReport = async () => {
    if (!selected) return;
    try {
      await invoke("generate_report_template", { templateId: selected.id });
      alert("报告模板已生成");
      await loadData();
    } catch (e) {
      console.error(e);
      alert(String(e));
    }
  };

  if (loading) return <div className="p-4 text-[hsl(var(--text-secondary))] text-sm">加载中...</div>;

  return (
    <div className="flex gap-3 h-full">
      {/* ====== Left Panel: Template List ====== */}
      <div className="w-48 shrink-0 flex flex-col bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg">
        <div className="p-2 border-b border-[hsl(var(--border-light))]">
          <Button size="sm" className="w-full"
            onClick={() => {
              setNewForm({ ...EMPTY_FORM, vendor: selected?.vendor || "H3C" });
              setNewModalOpen(true);
            }}
          >
            + 新建模板
          </Button>
        </div>
        <div className="flex-1 overflow-auto">
          {templates.length === 0 ? (
            <p className="p-3 text-xs text-[hsl(var(--text-tertiary))] text-center">暂无模板</p>
          ) : (
            templates.map((tpl) => (
              <div
                key={tpl.id}
                className={`px-3 py-2 cursor-pointer border-b border-[hsl(var(--border-light))] text-xs hover:bg-[hsl(var(--bg-hover))] ${
                  selectedId === tpl.id ? "bg-[hsl(var(--accent-subtle))] text-[hsl(var(--accent))] font-medium" : "text-[hsl(var(--text-primary))]"
                }`}
                onClick={() => selectTemplate(tpl.id)}
                onContextMenu={(e) => onContextMenu(e, tpl)}
              >
                <div className="truncate">{tpl.name}</div>
                <div className="text-[10px] text-[hsl(var(--text-tertiary))] mt-0.5 flex justify-between">
                  <span>{tpl.vendor}</span>
                  <span>{tpl.device_count ?? 0} 设备</span>
                </div>
              </div>
            ))
          )}
        </div>
      </div>

      {/* ====== Center Panel: Template Detail ====== */}
      <div className="flex-1 min-w-0 flex flex-col gap-3">
        {!selected ? (
          <div className="flex-1 flex items-center justify-center text-[hsl(var(--text-tertiary))] text-sm">
            请从左侧选择一个模板，或点击「新建模板」
          </div>
        ) : (
          <>
            {/* Template info header */}
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold text-[hsl(var(--text-primary))] truncate">
                {selected.name}
              </h2>
              <div className="flex gap-1">
                <Button variant="secondary" size="sm" onClick={handleAutoGenerate}>
                  自动生成命令
                </Button>
                <Button variant="secondary" size="sm" onClick={handleGenerateReport}>
                  生成报告模板
                </Button>
                {dirty && (
                  <Button size="sm" loading={saving} disabled={saving || !form.name.trim()} onClick={handleSave}>
                    {saving ? "保存中..." : "保存"}
                  </Button>
                )}
              </div>
            </div>

            {/* Form fields */}
            <div className="grid grid-cols-3 gap-2">
              <FormField label="模板名称 *">
                <Input size="sm"
                  value={form.name}
                  onChange={(e) => updateForm({ name: e.target.value })}
                  placeholder="例如: H3C交换机巡检模板"
                />
              </FormField>
              <FormField label="厂商">
                <Select size="sm"
                  value={form.vendor}
                  onChange={(e) => updateForm({ vendor: e.target.value })}
                >
                  {VENDORS.map((v) => (
                    <option key={v} value={v}>{v}</option>
                  ))}
                </Select>
              </FormField>
              <FormField label="型号">
                <Input size="sm"
                  value={form.model}
                  onChange={(e) => updateForm({ model: e.target.value })}
                  placeholder="例如: S5130-52S-EI"
                />
              </FormField>
              <FormField label="设备类型">
                <Input size="sm"
                  value={form.device_type}
                  onChange={(e) => updateForm({ device_type: e.target.value })}
                  placeholder="例如: switch / router / server"
                />
              </FormField>
              <FormField label="描述">
                <Input size="sm"
                  value={form.description}
                  onChange={(e) => updateForm({ description: e.target.value })}
                  placeholder="模板用途说明"
                />
              </FormField>
            </div>

            {/* Additional info */}
            <div className="text-[10px] text-[hsl(var(--text-tertiary))] flex gap-4">
              <span>创建: {formatTime(selected.created_at)}</span>
              <span>更新: {formatTime(selected.updated_at)}</span>
              <span>关联设备: {selected.device_count ?? 0}</span>
              {selected.report_template_id && (
                <span>报告模板ID: {selected.report_template_id}</span>
              )}
            </div>

            {/* Command list in template */}
            <div className="flex-1 min-h-0 flex flex-col">
              <div className="flex items-center justify-between mb-1">
                <h3 className="text-xs font-semibold text-[hsl(var(--text-secondary))]">
                  巡检命令 ({templateCommands.length})
                </h3>
              </div>
              <div className="flex-1 overflow-auto border border-[hsl(var(--border))] rounded">
                {templateCommands.length === 0 ? (
                  <p className="p-4 text-xs text-[hsl(var(--text-tertiary))] text-center">
                    暂无命令，请从右侧命令库中添加，或点击「自动生成命令」
                  </p>
                ) : (
                  <table className="w-full text-xs">
                    <thead className="bg-[hsl(var(--bg-hover))] sticky top-0">
                      <tr>
                        <th className="text-left px-2 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))] w-8">#</th>
                        <th className="text-left px-2 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))]">命令</th>
                        <th className="text-left px-2 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))] w-16">分类</th>
                        <th className="text-left px-2 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))] w-20">厂商</th>
                        <th className="text-left px-2 py-1.5 border-b border-[hsl(var(--border))] font-medium text-[hsl(var(--text-secondary))] w-16">操作</th>
                      </tr>
                    </thead>
                    <tbody>
                      {templateCommands.map((cmd, idx) => (
                        <tr key={cmd.id} className="border-b border-[hsl(var(--border-light))] hover:bg-[hsl(var(--bg-hover))]">
                          <td className="px-2 py-1 text-[hsl(var(--text-tertiary))]">{idx + 1}</td>
                          <td className="px-2 py-1">
                            <code className="text-[11px] bg-[hsl(var(--bg-hover))] px-1 py-0.5 rounded">{cmd.command}</code>
                            {cmd.description && (
                              <span className="text-[hsl(var(--text-tertiary))] ml-1">- {cmd.description}</span>
                            )}
                          </td>
                          <td className="px-2 py-1 text-[hsl(var(--text-secondary))]">{cmd.category || "-"}</td>
                          <td className="px-2 py-1 text-[hsl(var(--text-secondary))]">{cmd.vendor}</td>
                          <td className="px-2 py-1">
                            <Button variant="danger" size="sm"
                              onClick={() => handleRemoveCommand(cmd.id)}
                            >
                              移除
                            </Button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </div>
            </div>
          </>
        )}
      </div>

      {/* ====== Right Panel: Command Pool Browser ====== */}
      <div className="w-56 shrink-0 flex flex-col bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded-lg">
        <div className="p-2 border-b border-[hsl(var(--border-light))] space-y-2">
          <h3 className="text-xs font-semibold text-[hsl(var(--text-secondary))]">命令库</h3>
          <Select size="sm"
            value={vendorFilter}
            onChange={(e) => setVendorFilter(e.target.value)}
          >
            <option value="all">全部厂商</option>
            {VENDORS.map((v) => (
              <option key={v} value={v}>{v}</option>
            ))}
          </Select>
          <SearchInput
            value={cmdSearch}
            onChange={setCmdSearch}
            placeholder="搜索命令..."
          />
        </div>
        <div className="flex-1 overflow-auto">
          {filteredCommands.length === 0 ? (
            <p className="p-3 text-xs text-[hsl(var(--text-tertiary))] text-center">无匹配命令</p>
          ) : (
            filteredCommands.map((cmd) => {
              const alreadyAdded = templateCommandIds.includes(cmd.id);
              return (
                <div
                  key={cmd.id}
                  className={`px-2 py-1.5 border-b border-[hsl(var(--border-light))] text-xs hover:bg-[hsl(var(--bg-hover))] ${
                    alreadyAdded ? "bg-[hsl(var(--success)/0.1)]" : ""
                  }`}
                >
                  <code className="text-[11px] bg-[hsl(var(--bg-hover))] px-1 py-0.5 rounded block truncate">
                    {cmd.command}
                  </code>
                  <div className="flex items-center justify-between mt-0.5">
                    <span className="text-[10px] text-[hsl(var(--text-tertiary))]">
                      {cmd.vendor}
                      {cmd.category ? ` · ${cmd.category}` : ""}
                    </span>
                    {!selected ? (
                      <span className="text-[10px] text-[hsl(var(--text-tertiary))]">请先选择模板</span>
                    ) : alreadyAdded ? (
                      <span className="text-[10px] text-[hsl(var(--success))]">已加入</span>
                    ) : (
                      <Button size="sm"
                        onClick={() => handleAddCommand(cmd.id)}
                      >
                        加入
                      </Button>
                    )}
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>

      {/* ====== Modals ====== */}

      {/* Context Menu */}
      <ContextMenu
        items={ctxItems}
        visible={ctxVisible}
        x={ctxPos.x}
        y={ctxPos.y}
        onClose={() => setCtxVisible(false)}
      />

      {/* New Template Modal */}
      <Modal
        open={newModalOpen}
        title="新建巡检模板"
        width="max-w-md"
        onClose={() => setNewModalOpen(false)}
        footer={
          <>
            <Button variant="secondary" size="sm" onClick={() => setNewModalOpen(false)}>
              取消
            </Button>
            <Button size="sm" disabled={!newForm.name.trim()} onClick={handleCreate}>
              创建模板
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          {/* 模板名称 — 必填，突出 */}
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-primary))] mb-1.5">
              模板名称 <span className="text-[hsl(var(--danger))]">*</span>
            </label>
            <Input
              value={newForm.name}
              onChange={(e) => setNewForm({ ...newForm, name: e.target.value })}
              placeholder="例如: H3C 核心交换机巡检模板"
              autoFocus
            />
            {!newForm.name.trim() && (
              <p className="text-[11px] text-[hsl(var(--text-tertiary))] mt-1">输入模板名称后即可创建</p>
            )}
          </div>

          {/* 选填信息 — 收起，用分割线隔开 */}
          <div className="pt-3 border-t border-[hsl(var(--border-light))]">
            <p className="text-[11px] text-[hsl(var(--text-tertiary))] mb-3">以下为选填信息，可稍后在模板详情中修改</p>
            <div className="grid grid-cols-2 gap-3">
              <FormField label="厂商">
                <Select size="sm"
                  value={newForm.vendor}
                  onChange={(e) => setNewForm({ ...newForm, vendor: e.target.value })}
                >
                  {VENDORS.map((v) => (
                    <option key={v} value={v}>{v}</option>
                  ))}
                </Select>
              </FormField>
              <FormField label="设备类型">
                <Input size="sm"
                  value={newForm.device_type}
                  onChange={(e) => setNewForm({ ...newForm, device_type: e.target.value })}
                  placeholder="switch / router / firewall"
                />
              </FormField>
              <FormField label="型号">
                <Input size="sm"
                  value={newForm.model}
                  onChange={(e) => setNewForm({ ...newForm, model: e.target.value })}
                  placeholder="例如: S5130-52S-EI"
                />
              </FormField>
              <FormField label="描述">
                <Input size="sm"
                  value={newForm.description}
                  onChange={(e) => setNewForm({ ...newForm, description: e.target.value })}
                  placeholder="简要说明模板用途"
                />
              </FormField>
            </div>
          </div>
        </div>
      </Modal>

      {/* Rename Modal */}
      <Modal
        open={renameModalOpen}
        title="重命名模板"
        onClose={() => setRenameModalOpen(false)}
        footer={
          <>
            <Button variant="secondary" size="sm" onClick={() => setRenameModalOpen(false)}>
              取消
            </Button>
            <Button size="sm" disabled={!renameName.trim()} onClick={handleRename}>
              确认
            </Button>
          </>
        }
      >
        <FormField label="模板名称">
          <Input size="sm"
            value={renameName}
            onChange={(e) => setRenameName(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") handleRename(); }}
            autoFocus
          />
        </FormField>
      </Modal>

      {/* Delete Confirmation Modal */}
      <Modal
        open={deleteModalOpen}
        title="删除模板"
        onClose={() => setDeleteModalOpen(false)}
        footer={
          <>
            <Button variant="secondary" size="sm" onClick={() => setDeleteModalOpen(false)}>
              取消
            </Button>
            <Button variant="danger" size="sm" onClick={handleDelete}>
              确认删除
            </Button>
          </>
        }
      >
        <p className="text-sm text-[hsl(var(--text-primary))]">
          确定要删除模板「{ctxTemplate?.name}」吗？此操作不可撤销。
          {ctxTemplate && (ctxTemplate.device_count ?? 0) > 0 && (
            <span className="text-[hsl(var(--danger))] block mt-1">
              注意：该模板当前被 {ctxTemplate.device_count} 台设备引用。
            </span>
          )}
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
