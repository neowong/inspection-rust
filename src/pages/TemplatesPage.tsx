import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import Toolbar from "../components/Toolbar";
import SearchInput from "../components/SearchInput";
import Modal from "../components/Modal";
import ContextMenu, { ContextMenuItem } from "../components/ContextMenu";
import type { InspectionTemplate, CommandPool } from "../types";

const VENDORS = ["H3C", "华为", "思科", "深信服", "锐捷", "Linux", "CentOS", "Ubuntu", "openEuler", "MySQL", "PostgreSQL", "Oracle", "其它"];
const TEMPLATE_TYPES = [
  { value: "ssh", label: "SSH" },
  { value: "offline", label: "离线" },
];

interface TemplateForm {
  name: string;
  vendor: string;
  model: string;
  device_type: string;
  type: string;
  description: string;
}

const EMPTY_FORM: TemplateForm = {
  name: "",
  vendor: "H3C",
  model: "",
  device_type: "",
  type: "ssh",
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
        type: selected.type,
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
          template_type: newForm.type,
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
          template_type: tpl.type,
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
          template_type: form.type,
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

  if (loading) return <div className="p-4 text-gray-500 text-sm">加载中...</div>;

  return (
    <div className="flex gap-3 h-full">
      {/* ====== Left Panel: Template List ====== */}
      <div className="w-48 shrink-0 flex flex-col bg-white rounded border border-gray-200">
        <div className="p-2 border-b border-gray-100">
          <button
            className="w-full px-3 py-1.5 text-xs bg-blue-500 text-white rounded hover:bg-blue-600"
            onClick={() => {
              setNewForm({ ...EMPTY_FORM, vendor: selected?.vendor || "H3C" });
              setNewModalOpen(true);
            }}
          >
            + 新建模板
          </button>
        </div>
        <div className="flex-1 overflow-auto">
          {templates.length === 0 ? (
            <p className="p-3 text-xs text-gray-400 text-center">暂无模板</p>
          ) : (
            templates.map((tpl) => (
              <div
                key={tpl.id}
                className={`px-3 py-2 cursor-pointer border-b border-gray-50 text-xs hover:bg-blue-50/50 ${
                  selectedId === tpl.id ? "bg-blue-100 text-blue-800 font-medium" : "text-gray-700"
                }`}
                onClick={() => selectTemplate(tpl.id)}
                onContextMenu={(e) => onContextMenu(e, tpl)}
              >
                <div className="truncate">{tpl.name}</div>
                <div className="text-[10px] text-gray-400 mt-0.5 flex justify-between">
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
          <div className="flex-1 flex items-center justify-center text-gray-400 text-sm">
            请从左侧选择一个模板，或点击「新建模板」
          </div>
        ) : (
          <>
            {/* Template info header */}
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold text-gray-800 truncate">
                {selected.name}
              </h2>
              <div className="flex gap-1">
                <button
                  className="px-2 py-0.5 text-[11px] border border-gray-300 rounded hover:bg-gray-100"
                  onClick={handleAutoGenerate}
                >
                  自动生成命令
                </button>
                <button
                  className="px-2 py-0.5 text-[11px] border border-gray-300 rounded hover:bg-gray-100"
                  onClick={handleGenerateReport}
                >
                  生成报告模板
                </button>
                {dirty && (
                  <button
                    className="px-3 py-0.5 text-[11px] bg-green-500 text-white rounded hover:bg-green-600 disabled:opacity-50"
                    disabled={saving || !form.name.trim()}
                    onClick={handleSave}
                  >
                    {saving ? "保存中..." : "保存"}
                  </button>
                )}
              </div>
            </div>

            {/* Form fields */}
            <div className="grid grid-cols-3 gap-2">
              <FormField label="模板名称 *">
                <input
                  className="form-input"
                  value={form.name}
                  onChange={(e) => updateForm({ name: e.target.value })}
                  placeholder="例如: H3C交换机巡检模板"
                />
              </FormField>
              <FormField label="厂商">
                <select
                  className="form-input"
                  value={form.vendor}
                  onChange={(e) => updateForm({ vendor: e.target.value })}
                >
                  {VENDORS.map((v) => (
                    <option key={v} value={v}>{v}</option>
                  ))}
                </select>
              </FormField>
              <FormField label="型号">
                <input
                  className="form-input"
                  value={form.model}
                  onChange={(e) => updateForm({ model: e.target.value })}
                  placeholder="例如: S5130-52S-EI"
                />
              </FormField>
              <FormField label="设备类型">
                <input
                  className="form-input"
                  value={form.device_type}
                  onChange={(e) => updateForm({ device_type: e.target.value })}
                  placeholder="例如: switch / router / server"
                />
              </FormField>
              <FormField label="模板类型">
                <select
                  className="form-input"
                  value={form.type}
                  onChange={(e) => updateForm({ type: e.target.value })}
                >
                  {TEMPLATE_TYPES.map((t) => (
                    <option key={t.value} value={t.value}>{t.label}</option>
                  ))}
                </select>
              </FormField>
              <FormField label="描述">
                <input
                  className="form-input"
                  value={form.description}
                  onChange={(e) => updateForm({ description: e.target.value })}
                  placeholder="模板用途说明"
                />
              </FormField>
            </div>

            {/* Additional info */}
            <div className="text-[10px] text-gray-400 flex gap-4">
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
                <h3 className="text-xs font-semibold text-gray-600">
                  巡检命令 ({templateCommands.length})
                </h3>
              </div>
              <div className="flex-1 overflow-auto border border-gray-200 rounded">
                {templateCommands.length === 0 ? (
                  <p className="p-4 text-xs text-gray-400 text-center">
                    暂无命令，请从右侧命令库中添加，或点击「自动生成命令」
                  </p>
                ) : (
                  <table className="w-full text-xs">
                    <thead className="bg-gray-50 sticky top-0">
                      <tr>
                        <th className="text-left px-2 py-1.5 border-b border-gray-200 font-medium text-gray-500 w-8">#</th>
                        <th className="text-left px-2 py-1.5 border-b border-gray-200 font-medium text-gray-500">命令</th>
                        <th className="text-left px-2 py-1.5 border-b border-gray-200 font-medium text-gray-500 w-16">分类</th>
                        <th className="text-left px-2 py-1.5 border-b border-gray-200 font-medium text-gray-500 w-20">厂商</th>
                        <th className="text-left px-2 py-1.5 border-b border-gray-200 font-medium text-gray-500 w-16">操作</th>
                      </tr>
                    </thead>
                    <tbody>
                      {templateCommands.map((cmd, idx) => (
                        <tr key={cmd.id} className="border-b border-gray-100 hover:bg-gray-50">
                          <td className="px-2 py-1 text-gray-400">{idx + 1}</td>
                          <td className="px-2 py-1">
                            <code className="text-[11px] bg-gray-100 px-1 py-0.5 rounded">{cmd.command}</code>
                            {cmd.description && (
                              <span className="text-gray-400 ml-1">- {cmd.description}</span>
                            )}
                          </td>
                          <td className="px-2 py-1 text-gray-500">{cmd.category || "-"}</td>
                          <td className="px-2 py-1 text-gray-500">{cmd.vendor}</td>
                          <td className="px-2 py-1">
                            <button
                              className="px-1.5 py-0.5 text-[10px] text-red-500 border border-red-200 rounded hover:bg-red-50"
                              onClick={() => handleRemoveCommand(cmd.id)}
                            >
                              移除
                            </button>
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
      <div className="w-56 shrink-0 flex flex-col bg-white rounded border border-gray-200">
        <div className="p-2 border-b border-gray-100 space-y-2">
          <h3 className="text-xs font-semibold text-gray-600">命令库</h3>
          <select
            className="form-input text-[11px]"
            value={vendorFilter}
            onChange={(e) => setVendorFilter(e.target.value)}
          >
            <option value="all">全部厂商</option>
            {VENDORS.map((v) => (
              <option key={v} value={v}>{v}</option>
            ))}
          </select>
          <SearchInput
            value={cmdSearch}
            onChange={setCmdSearch}
            placeholder="搜索命令..."
          />
        </div>
        <div className="flex-1 overflow-auto">
          {filteredCommands.length === 0 ? (
            <p className="p-3 text-xs text-gray-400 text-center">无匹配命令</p>
          ) : (
            filteredCommands.map((cmd) => {
              const alreadyAdded = templateCommandIds.includes(cmd.id);
              return (
                <div
                  key={cmd.id}
                  className={`px-2 py-1.5 border-b border-gray-50 text-xs hover:bg-gray-50 ${
                    alreadyAdded ? "bg-green-50/50" : ""
                  }`}
                >
                  <code className="text-[11px] bg-gray-100 px-1 py-0.5 rounded block truncate">
                    {cmd.command}
                  </code>
                  <div className="flex items-center justify-between mt-0.5">
                    <span className="text-[10px] text-gray-400">
                      {cmd.vendor}
                      {cmd.category ? ` · ${cmd.category}` : ""}
                    </span>
                    {!selected ? (
                      <span className="text-[10px] text-gray-300">请先选择模板</span>
                    ) : alreadyAdded ? (
                      <span className="text-[10px] text-green-500">已加入</span>
                    ) : (
                      <button
                        className="px-1.5 py-0.5 text-[10px] bg-blue-500 text-white rounded hover:bg-blue-600"
                        onClick={() => handleAddCommand(cmd.id)}
                      >
                        加入
                      </button>
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
        title="新建模板"
        width="max-w-lg"
        onClose={() => setNewModalOpen(false)}
        footer={
          <>
            <button
              className="px-3 py-1 text-xs border border-gray-300 rounded hover:bg-gray-100"
              onClick={() => setNewModalOpen(false)}
            >
              取消
            </button>
            <button
              className="px-3 py-1 text-xs bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50"
              disabled={!newForm.name.trim()}
              onClick={handleCreate}
            >
              创建
            </button>
          </>
        }
      >
        <div className="grid grid-cols-2 gap-3">
          <FormField label="模板名称 *">
            <input
              className="form-input"
              value={newForm.name}
              onChange={(e) => setNewForm({ ...newForm, name: e.target.value })}
              placeholder="例如: H3C交换机巡检模板"
            />
          </FormField>
          <FormField label="厂商">
            <select
              className="form-input"
              value={newForm.vendor}
              onChange={(e) => setNewForm({ ...newForm, vendor: e.target.value })}
            >
              {VENDORS.map((v) => (
                <option key={v} value={v}>{v}</option>
              ))}
            </select>
          </FormField>
          <FormField label="型号">
            <input
              className="form-input"
              value={newForm.model}
              onChange={(e) => setNewForm({ ...newForm, model: e.target.value })}
              placeholder="例如: S5130-52S-EI"
            />
          </FormField>
          <FormField label="设备类型">
            <input
              className="form-input"
              value={newForm.device_type}
              onChange={(e) => setNewForm({ ...newForm, device_type: e.target.value })}
              placeholder="例如: switch / router"
            />
          </FormField>
          <FormField label="模板类型">
            <select
              className="form-input"
              value={newForm.type}
              onChange={(e) => setNewForm({ ...newForm, type: e.target.value })}
            >
              {TEMPLATE_TYPES.map((t) => (
                <option key={t.value} value={t.value}>{t.label}</option>
              ))}
            </select>
          </FormField>
          <FormField label="描述">
            <input
              className="form-input"
              value={newForm.description}
              onChange={(e) => setNewForm({ ...newForm, description: e.target.value })}
              placeholder="模板用途说明"
            />
          </FormField>
        </div>
      </Modal>

      {/* Rename Modal */}
      <Modal
        open={renameModalOpen}
        title="重命名模板"
        onClose={() => setRenameModalOpen(false)}
        footer={
          <>
            <button
              className="px-3 py-1 text-xs border border-gray-300 rounded hover:bg-gray-100"
              onClick={() => setRenameModalOpen(false)}
            >
              取消
            </button>
            <button
              className="px-3 py-1 text-xs bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50"
              disabled={!renameName.trim()}
              onClick={handleRename}
            >
              确认
            </button>
          </>
        }
      >
        <FormField label="模板名称">
          <input
            className="form-input"
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
            <button
              className="px-3 py-1 text-xs border border-gray-300 rounded hover:bg-gray-100"
              onClick={() => setDeleteModalOpen(false)}
            >
              取消
            </button>
            <button
              className="px-3 py-1 text-xs bg-red-500 text-white rounded hover:bg-red-600"
              onClick={handleDelete}
            >
              确认删除
            </button>
          </>
        }
      >
        <p className="text-sm text-gray-700">
          确定要删除模板「{ctxTemplate?.name}」吗？此操作不可撤销。
          {ctxTemplate && (ctxTemplate.device_count ?? 0) > 0 && (
            <span className="text-red-500 block mt-1">
              注意：该模板当前被 {ctxTemplate.device_count} 台设备引用。
            </span>
          )}
        </p>
      </Modal>

      {/* Form input style */}
      <style>{`
        .form-input {
          width: 100%;
          padding: 4px 8px;
          font-size: 12px;
          border: 1px solid #d1d5db;
          border-radius: 4px;
          outline: none;
          background: #fff;
        }
        .form-input:focus {
          border-color: #3b82f6;
          box-shadow: 0 0 0 1px #3b82f6;
        }
      `}</style>
    </div>
  );
}

function FormField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-xs text-gray-600">{label}</span>
      {children}
    </label>
  );
}
