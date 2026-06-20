import React, { useState, useEffect, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronRight, ChevronDown, Pencil, Trash2, Copy, Star, GripVertical, Lock } from "lucide-react";
import type {
  InspectionTemplate, CommandPool, ReportTemplate, TemplateCommandConfig,
  ReportTemplateConfig, TableColumn, DeviceField,
} from "../types";
import { DEFAULT_REPORT_CONFIG } from "../types";
import { useShakeValidation } from "../hooks/useShakeValidation";
import { friendlyError } from "../lib/utils";
import Toolbar from "../components/Toolbar";
import SearchInput from "../components/SearchInput";
import DataTable from "../components/DataTable";
import Modal from "../components/Modal";
import Button from "../components/ui/Button";
import Input, { Select } from "../components/ui/Input";
import { VENDORS, CATEGORIES } from "../lib/constants";

type TabKey = "templates" | "commands" | "reports";

const TABS: { key: TabKey; label: string }[] = [
  { key: "templates", label: "巡检模板" },
  { key: "commands", label: "命令库" },
  { key: "reports", label: "报告模板" },
];

// ============================================================
// Forms
// ============================================================

interface TemplateForm {
  name: string;
  vendor: string;
  model: string;
  device_type: string;
  description: string;
  commands: TemplateCommandConfig[];
  report_template_id: number | null;
}

const getEmptyTemplateForm = (): TemplateForm => ({
  name: "", vendor: "H3C", model: "", device_type: "", description: "", commands: [], report_template_id: null,
});

interface CommandForm {
  vendor: string;
  command: string;
  description: string;
  category: string;
}

const getEmptyCommandForm = (): CommandForm => ({
  vendor: "H3C", command: "", description: "", category: "general",
});

interface ReportForm {
  name: string;
  vendor: string;
  description: string;
  config: ReportTemplateConfig;
}

const EMPTY_REPORT_FORM = (): ReportForm => ({
  name: "", vendor: "", description: "",
  config: JSON.parse(JSON.stringify(DEFAULT_REPORT_CONFIG)),
});

// ============================================================
// TemplatesPage
// ============================================================

export default function TemplatesPage() {
  const [tab, setTab] = useState<TabKey>("templates");

  // Template state
  const [templates, setTemplates] = useState<InspectionTemplate[]>([]);
  const [templateSearch, setTemplateSearch] = useState("");
  const [templateVendor, setTemplateVendor] = useState("");
  const [selectedTemplate, setSelectedTemplate] = useState<InspectionTemplate | null>(null);
  const [templateModal, setTemplateModal] = useState(false);
  const [editingTemplate, setEditingTemplate] = useState<InspectionTemplate | null>(null);
  const [templateForm, setTemplateForm] = useState<TemplateForm>(getEmptyTemplateForm());
  const [confirmDeleteTemplate, setConfirmDeleteTemplate] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const { shakeFields, triggerShake } = useShakeValidation();

  // Command pool state
  const [commands, setCommands] = useState<CommandPool[]>([]);
  const [cmdSearch, setCmdSearch] = useState("");
  const [cmdVendor, setCmdVendor] = useState("");
  const [cmdModal, setCmdModal] = useState(false);
  const [editingCmd, setEditingCmd] = useState<CommandPool | null>(null);
  const [cmdForm, setCmdForm] = useState<CommandForm>(getEmptyCommandForm());
  const [confirmDeleteCmd, setConfirmDeleteCmd] = useState<number | null>(null);
  const [cmdSaving, setCmdSaving] = useState(false);
  const [cmdSaveError, setCmdSaveError] = useState<string | null>(null);

  // Report template state
  const [reportTemplates, setReportTemplates] = useState<ReportTemplate[]>([]);
  const [reportModalOpen, setReportModalOpen] = useState(false);
  const [editingReport, setEditingReport] = useState<ReportTemplate | null>(null);
  const [reportForm, setReportForm] = useState<ReportForm>(EMPTY_REPORT_FORM());
  const [reportSaving, setReportSaving] = useState(false);
  const [reportSaveError, setReportSaveError] = useState<string | null>(null);
  const [confirmDeleteReport, setConfirmDeleteReport] = useState<number | null>(null);

  const cmdListRef = useRef<HTMLDivElement>(null);
  const autoScrollRaf = useRef<number | null>(null);
  const SCROLL_ZONE = 50;
  const MAX_SCROLL_SPEED = 8;

  const handleDragAutoScroll = (e: React.DragEvent, container: HTMLElement | null) => {
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const y = e.clientY - rect.top;
    if (autoScrollRaf.current) { cancelAnimationFrame(autoScrollRaf.current); autoScrollRaf.current = null; }
    let speed = 0;
    if (y < SCROLL_ZONE && container.scrollTop > 0) {
      speed = -((SCROLL_ZONE - y) / SCROLL_ZONE) * MAX_SCROLL_SPEED;
    } else if (y > rect.height - SCROLL_ZONE && container.scrollTop < container.scrollHeight - container.clientHeight) {
      speed = ((y - (rect.height - SCROLL_ZONE)) / SCROLL_ZONE) * MAX_SCROLL_SPEED;
    }
    if (speed !== 0) {
      const scroll = () => {
        if ((speed < 0 && container.scrollTop <= 0) || (speed > 0 && container.scrollTop >= container.scrollHeight - container.clientHeight)) {
          autoScrollRaf.current = null;
          return;
        }
        container.scrollTop += speed;
        autoScrollRaf.current = requestAnimationFrame(scroll);
      };
      autoScrollRaf.current = requestAnimationFrame(scroll);
    }
  };
  const stopAutoScroll = () => {
    if (autoScrollRaf.current) { cancelAnimationFrame(autoScrollRaf.current); autoScrollRaf.current = null; }
  };
  // 组件卸载时清理动画帧，防止内存泄漏
  useEffect(() => () => { stopAutoScroll(); }, []);

  const loadTemplates = () => {
    invoke<InspectionTemplate[]>("list_templates", { vendor: templateVendor || undefined })
      .then(setTemplates).catch(console.error);
  };
  const loadCommands = () => {
    invoke<CommandPool[]>("list_commands", { vendor: cmdVendor || undefined })
      .then(setCommands).catch(console.error);
  };
  const loadReportTemplates = () => {
    invoke<ReportTemplate[]>("list_report_templates")
      .then(setReportTemplates).catch(console.error);
  };

  useEffect(() => { loadTemplates(); }, [templateVendor]);
  useEffect(() => { loadCommands(); }, [cmdVendor]);
  useEffect(() => { loadReportTemplates(); }, []);

  const filteredTemplates = useMemo(() => templates.filter((t) =>
    !templateSearch || t.name.toLowerCase().includes(templateSearch.toLowerCase())
  ), [templates, templateSearch]);

  const filteredCommands = useMemo(() => commands.filter((c) =>
    !cmdSearch || c.command.toLowerCase().includes(cmdSearch.toLowerCase()) || (c.description && c.description.toLowerCase().includes(cmdSearch.toLowerCase()))
  ), [commands, cmdSearch]);

  const vendorFilteredCommands = useMemo(() => commands.filter((c) =>
    c.vendor === templateForm.vendor
  ), [commands, templateForm.vendor]);

  // ----- Template handlers -----
  const openAddTemplate = () => {
    setEditingTemplate(null);
    setTemplateForm(getEmptyTemplateForm());
    setTemplateModal(true);
  };
  const openEditTemplate = (t: InspectionTemplate) => {
    setEditingTemplate(t);
    setTemplateForm({
      name: t.name, vendor: t.vendor, model: t.model || "",
      device_type: t.device_type || "", description: t.description || "",
      commands: t.config?.commands || [],
      report_template_id: t.report_template_id ?? null,
    });
    setTemplateModal(true);
  };
  const handleSaveTemplate = () => {
    if (!templateForm.name.trim()) { triggerShake("template_name"); return; }
    const data: Record<string, unknown> = {
      name: templateForm.name,
      vendor: templateForm.vendor,
      config: JSON.stringify({ commands: templateForm.commands }),
    };
    if (templateForm.model) data.model = templateForm.model;
    if (templateForm.device_type) data.device_type = templateForm.device_type;
    if (templateForm.description) data.description = templateForm.description;
    data.report_template_id = templateForm.report_template_id;

    setSaving(true); setSaveError(null);
    const promise = editingTemplate
      ? invoke<InspectionTemplate>("update_template", { templateId: editingTemplate.id, data })
      : invoke<InspectionTemplate>("create_template", { data });
    promise
      .then(() => { setTemplateModal(false); loadTemplates(); })
      .catch((e) => { setSaveError(friendlyError(e)); triggerShake("template_name"); })
      .finally(() => setSaving(false));
  };
  const handleDeleteTemplate = (id: number) => {
    invoke<void>("delete_template", { templateId: id })
      .then(() => { setConfirmDeleteTemplate(null); loadTemplates(); })
      .catch(console.error);
  };

  // ----- Command handlers -----
  const openAddCmd = () => { setEditingCmd(null); setCmdForm(getEmptyCommandForm()); setCmdSaveError(null); setCmdModal(true); };
  const openEditCmd = (c: CommandPool) => {
    setEditingCmd(c); setCmdSaveError(null);
    setCmdForm({ vendor: c.vendor, command: c.command, description: c.description || "", category: c.category || "general" });
    setCmdModal(true);
  };
  const handleSaveCommand = () => {
    if (!cmdForm.command.trim()) { triggerShake("cmd_command"); return; }
    setCmdSaving(true); setCmdSaveError(null);
    const promise = editingCmd
      ? invoke<CommandPool>("update_command", { commandId: editingCmd.id, data: { ...cmdForm } })
      : invoke<CommandPool>("create_command", { data: { ...cmdForm } });
    promise
      .then(() => { setCmdModal(false); setCmdForm(getEmptyCommandForm()); setEditingCmd(null); loadCommands(); })
      .catch((e) => { setCmdSaveError(friendlyError(e)); triggerShake("cmd_command"); })
      .finally(() => setCmdSaving(false));
  };
  const handleDeleteCmd = (id: number) => {
    invoke<void>("delete_command", { commandId: id })
      .then(() => { setConfirmDeleteCmd(null); loadCommands(); })
      .catch(console.error);
  };

  // ----- Report template handlers -----
  const openNewReport = () => {
    setEditingReport(null);
    setReportForm(EMPTY_REPORT_FORM());
    setReportSaveError(null);
    setReportModalOpen(true);
  };
  const openEditReport = (rt: ReportTemplate) => {
    setEditingReport(rt);
    let config: ReportTemplateConfig = JSON.parse(JSON.stringify(DEFAULT_REPORT_CONFIG));
    if (rt.config_json) {
      try {
        const parsed = JSON.parse(rt.config_json);
        if (parsed && typeof parsed === "object" && parsed.command_table) {
          config = { ...DEFAULT_REPORT_CONFIG, ...parsed };
        }
      } catch { /* fall back to default */ }
    }
    setReportForm({
      name: rt.name,
      vendor: rt.vendor || "",
      description: rt.description || "",
      config,
    });
    setReportSaveError(null);
    setReportModalOpen(true);
  };
  const handleCopyReport = (rt: ReportTemplate) => {
    invoke<ReportTemplate>("create_report_template", {
      data: {
        name: rt.name + " (副本)",
        vendor: rt.vendor,
        description: rt.description,
        config_json: rt.config_json,
      },
    }).then(() => loadReportTemplates()).catch(console.error);
  };
  const handleSetDefault = (id: number) => {
    invoke<void>("update_report_template", { templateId: id, data: { is_default: 1 } })
      .then(() => loadReportTemplates()).catch(console.error);
  };
  const handleSaveReport = () => {
    if (!reportForm.name.trim()) { triggerShake("report_name"); return; }
    setReportSaving(true); setReportSaveError(null);
    const data: Record<string, unknown> = {
      name: reportForm.name,
      vendor: reportForm.vendor || null,
      description: reportForm.description,
      config_json: JSON.stringify(reportForm.config),
    };
    const promise = editingReport
      ? invoke<ReportTemplate>("update_report_template", { templateId: editingReport.id, data })
      : invoke<ReportTemplate>("create_report_template", { data });
    promise
      .then(() => { setReportModalOpen(false); loadReportTemplates(); })
      .catch((e) => { setReportSaveError(friendlyError(e)); triggerShake("report_name"); })
      .finally(() => setReportSaving(false));
  };
  const handleDeleteReport = (id: number) => {
    invoke<void>("delete_report_template", { templateId: id })
      .then(() => { setConfirmDeleteReport(null); loadReportTemplates(); })
      .catch(console.error);
  };

  // ----- Render -----

  return (
    <div className="space-y-6">
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-0 bg-[hsl(var(--bg-content))] shadow-sm relative">
        <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">巡检模板</h1>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1 mb-3">管理巡检模板、命令库和报告模板</p>
        <div className="flex gap-0 border-b border-[hsl(var(--border))]">
          {TABS.map((t) => (
            <button
              key={t.key}
              onClick={() => setTab(t.key)}
              className={`px-4 py-2 text-sm font-medium transition-colors -mb-px border-b-2 ${
                tab === t.key
                  ? "text-[hsl(var(--accent))] border-[hsl(var(--accent))]"
                  : "text-[hsl(var(--text-secondary))] border-transparent hover:text-[hsl(var(--text-primary))]"
              }`}
            >
              {t.label}
            </button>
          ))}
        </div>
      </div>

      {/* ===== Tab: 巡检模板 ===== */}
      {tab === "templates" && (
        <div>
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
              { key: "command_count", header: "命令数", width: "80px", render: (r) => String((r.config?.commands || []).length) },
              { key: "report_template", header: "报告模板", width: "140px", render: (r) => {
                const rt = reportTemplates.find(t => t.id === r.report_template_id);
                return rt ? rt.name : <span className="text-[hsl(var(--text-tertiary))]">跟随默认</span>;
              }},
              { key: "description", header: "描述", render: (r) => r.description || "-" },
              { key: "updated_at", header: "更新时间", render: (r) => new Date(r.updated_at).toLocaleString("zh-CN") },
              { key: "actions", header: "操作", width: "140px", render: (r) => (
                <div className="flex gap-1" onClick={(e) => e.stopPropagation()}>
                  <Button size="sm" variant="ghost" onClick={() => openEditTemplate(r)}>编辑</Button>
                  <Button size="sm" variant="ghost" onClick={() => setConfirmDeleteTemplate(r.id)}>删除</Button>
                </div>
              )},
            ]}
            data={filteredTemplates}
            rowKey={(r) => r.id}
            onRowClick={(r) => setSelectedTemplate(r)}
            onRowDoubleClick={(r) => openEditTemplate(r)}
            selectedKey={selectedTemplate?.id}
            emptyText="暂无模板"
          />
        </div>
      )}

      {/* ===== Tab: 命令库 ===== */}
      {tab === "commands" && (
        <div>
          <Toolbar>
            <Button onClick={openAddCmd} size="sm">添加命令</Button>
            <SearchInput value={cmdSearch} onChange={setCmdSearch} placeholder="搜索命令..." />
          </Toolbar>
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
              >{v}</button>
            ))}
          </div>
          <CommandList
            commands={filteredCommands}
            onEdit={openEditCmd}
            onDelete={(id) => setConfirmDeleteCmd(id)}
          />
        </div>
      )}

      {/* ===== Tab: 报告模板 ===== */}
      {tab === "reports" && (
        <div>
          <Toolbar>
            <Button onClick={openNewReport} size="sm">新建模板</Button>
          </Toolbar>
          <DataTable<ReportTemplate>
            columns={[
              { key: "name", header: "名称", render: (r) => (
                <div className="flex items-center gap-1">
                  {r.is_default ? <Star size={12} className="text-[hsl(var(--warning))]" /> : null}
                  <span>{r.name}</span>
                </div>
              )},
              { key: "vendor", header: "厂商", render: (r) => r.vendor || "通用" },
              { key: "description", header: "描述", render: (r) => r.description || "-" },
              { key: "updated_at", header: "更新时间", render: (r) => new Date(r.updated_at).toLocaleString("zh-CN") },
              { key: "actions", header: "操作", width: "200px", render: (r) => (
                <div className="flex gap-0.5" onClick={(e) => e.stopPropagation()}>
                  <Button size="sm" variant="ghost" onClick={() => openEditReport(r)}>编辑</Button>
                  {!r.is_default && (
                    <Button size="sm" variant="ghost" onClick={() => handleSetDefault(r.id)}>设为默认</Button>
                  )}
                  <Button size="sm" variant="ghost" onClick={() => handleCopyReport(r)}><Copy size={12} /></Button>
                  {!r.is_default && (
                    <Button size="sm" variant="ghost" onClick={() => setConfirmDeleteReport(r.id)}><Trash2 size={12} /></Button>
                  )}
                </div>
              )},
            ]}
            data={reportTemplates}
            rowKey={(r) => r.id}
            onRowDoubleClick={(r) => openEditReport(r)}
            emptyText="暂无报告模板"
          />
        </div>
      )}

      {/* ===== Inspection Template Modal ===== */}
      {tab === "templates" && (
        <Modal
          open={templateModal}
          title={editingTemplate ? "编辑模板" : "添加模板"}
          width="max-w-2xl"
          onClose={() => setTemplateModal(false)}
          footer={
            <div className="flex gap-2">
              <Button variant="secondary" onClick={() => setTemplateModal(false)}>取消</Button>
              <Button onClick={handleSaveTemplate} loading={saving}>{editingTemplate ? "保存" : "添加"}</Button>
            </div>
          }
        >
          <div className="space-y-3">
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">名称</label>
                <Input value={templateForm.name} className={shakeFields.has("template_name") ? "animate-shake" : ""}
                  onChange={(e) => { setTemplateForm({ ...templateForm, name: e.target.value }); setSaveError(null); }} />
                {saveError && <p className="mt-1 text-xs text-[hsl(var(--danger))]">{saveError}</p>}
              </div>
              <div>
                <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">厂商</label>
                <Select value={templateForm.vendor} onChange={(e) => {
                  setTemplateForm({ ...templateForm, vendor: e.target.value, commands: [] });
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
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">关联报告模板（可选）</label>
              <Select
                value={templateForm.report_template_id ?? ""}
                onChange={(e) => setTemplateForm({ ...templateForm, report_template_id: e.target.value ? Number(e.target.value) : null })}
              >
                <option value="">跟随默认（按厂商自动匹配）</option>
                {reportTemplates.map((rt) => (
                  <option key={rt.id} value={rt.id}>{rt.name}{rt.is_default ? " (默认)" : ""}{rt.vendor ? ` · ${rt.vendor}` : ""}</option>
                ))}
              </Select>
            </div>
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-2">
                已选命令 ({templateForm.commands.length}) <span className="text-[10px] text-[hsl(var(--text-tertiary))]">拖拽排序，静态信息不进入报告明细</span>
              </label>
              <div ref={cmdListRef} className="max-h-56 overflow-y-auto border border-[hsl(var(--border))] rounded-md p-2 space-y-1 mb-3"
                onDragOver={(e) => { e.preventDefault(); handleDragAutoScroll(e, cmdListRef.current); }}
                onDrop={stopAutoScroll}
                onDragEnd={stopAutoScroll}
              >
                {templateForm.commands.length === 0 && <p className="text-xs text-[hsl(var(--text-tertiary))]">未选择命令</p>}
                {templateForm.commands.map((spec, idx) => {
                  const cmd = commands.find(c => c.id === spec.command_id);
                  if (!cmd) return null;
                  const updateSpec = (patch: Partial<TemplateCommandConfig>) => {
                    const next = [...templateForm.commands];
                    next[idx] = { ...spec, ...patch };
                    setTemplateForm({ ...templateForm, commands: next });
                  };
                  return (
                    <div
                      key={spec.command_id}
                      draggable
                      onDragStart={(e) => { e.dataTransfer.setData("text/plain", String(idx)); e.dataTransfer.effectAllowed = "move"; e.currentTarget.style.opacity = "0.3"; }}
                      onDragEnd={(e) => { e.currentTarget.style.opacity = ""; }}
                      onDragOver={(e) => { e.preventDefault(); e.currentTarget.style.borderColor = "hsl(var(--accent))"; }}
                      onDragLeave={(e) => { e.currentTarget.style.borderColor = ""; }}
                      onDrop={(e) => {
                        e.preventDefault(); e.currentTarget.style.borderColor = "";
                        const fromIdx = parseInt(e.dataTransfer.getData("text/plain"));
                        if (isNaN(fromIdx) || fromIdx === idx) return;
                        const next = [...templateForm.commands];
                        const moved = next.splice(fromIdx, 1)[0];
                        if (moved !== undefined) next.splice(idx, 0, moved);
                        setTemplateForm({ ...templateForm, commands: next });
                      }}
                      className="bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded px-2 py-1.5 cursor-grab active:cursor-grabbing"
                    >
                      <div className="flex items-center gap-2">
                        <GripVertical size={14} className="text-[hsl(var(--text-tertiary))] shrink-0" />
                        <span className="text-[11px] text-[hsl(var(--text-tertiary))] w-5 text-right">{idx + 1}</span>
                        <code className="text-xs bg-[hsl(var(--bg-hover))] px-1 rounded">{cmd.command}</code>
                        {cmd.description && <span className="text-[11px] text-[hsl(var(--text-tertiary))] truncate">— {cmd.description}</span>}
                        <button type="button"
                          onClick={() => setTemplateForm({ ...templateForm, commands: templateForm.commands.filter(c => c.command_id !== spec.command_id) })}
                          className="ml-auto shrink-0 text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--danger))] text-xs">×</button>
                      </div>
                      <div className="mt-1.5 flex flex-wrap items-center gap-2 pl-7 text-[11px]">
                        <Select className="h-6 w-24 text-[11px]" value={spec.purpose} onChange={(e) => {
                          const purpose = e.target.value as "inspection" | "static_info";
                          updateSpec({ purpose, show_in_report: purpose !== "static_info" });
                        }}>
                          <option value="inspection">巡检项</option>
                          <option value="static_info">静态信息</option>
                        </Select>
                        <label className="flex items-center gap-1 text-[hsl(var(--text-secondary))]">
                          <input type="checkbox" checked={spec.show_in_report} onChange={(e) => updateSpec({ show_in_report: e.target.checked })} className="accent-[hsl(var(--accent))]" />
                          显示到报告
                        </label>
                        {spec.purpose === "static_info" && (
                          <div className="flex flex-wrap gap-1">
                            {["sysname", "model", "serial_number", "manufacturing_date"].map((field) => (
                              <label key={field} className="flex items-center gap-1 text-[hsl(var(--text-secondary))]">
                                <input type="checkbox" checked={spec.extract_fields.includes(field)} onChange={(e) => {
                                  const fields = e.target.checked
                                    ? [...spec.extract_fields, field]
                                    : spec.extract_fields.filter(f => f !== field);
                                  updateSpec({ extract_fields: fields });
                                }} className="accent-[hsl(var(--accent))]" />
                                {field}
                              </label>
                            ))}
                          </div>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-2">可选命令 ({templateForm.vendor})</label>
              <div className="max-h-36 overflow-y-auto border border-[hsl(var(--border))] rounded-md p-2 space-y-1">
                {vendorFilteredCommands.length === 0 && <p className="text-xs text-[hsl(var(--text-tertiary))]">暂无 {templateForm.vendor} 命令，请先在命令库中添加</p>}
                {vendorFilteredCommands.filter(cmd => !templateForm.commands.some(c => c.command_id === cmd.id)).map((cmd) => (
                  <label key={cmd.id} className="flex items-center gap-2 cursor-pointer hover:bg-[hsl(var(--bg-hover))] rounded px-1 py-0.5">
                    <input type="checkbox" checked={false}
                      onChange={() => setTemplateForm({
                        ...templateForm,
                        commands: [...templateForm.commands, { command_id: cmd.id, purpose: "inspection", show_in_report: true, extract_fields: [] }]
                      })}
                      className="accent-[hsl(var(--accent))]" />
                    <span className="text-xs">
                      <code className="bg-[hsl(var(--bg-hover))] px-1 rounded">{cmd.command}</code>
                      {cmd.description && <span className="text-[hsl(var(--text-tertiary))] ml-1">— {cmd.description}</span>}
                    </span>
                  </label>
                ))}
              </div>
            </div>
          </div>
        </Modal>
      )}

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

      {/* ===== Command Modal ===== */}
      {tab === "commands" && (
        <Modal
          open={cmdModal}
          title={editingCmd ? "编辑命令" : "添加命令"}
          width="max-w-lg"
          onClose={() => { setCmdModal(false); setEditingCmd(null); }}
          footer={
            <div className="flex gap-2">
              <Button variant="secondary" onClick={() => { setCmdModal(false); setEditingCmd(null); }}>取消</Button>
              <Button onClick={handleSaveCommand} loading={cmdSaving}>{editingCmd ? "保存" : "添加"}</Button>
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
              <Input
                value={cmdForm.command}
                className={shakeFields.has("cmd_command") ? "animate-shake" : ""}
                onChange={(e) => { setCmdForm({ ...cmdForm, command: e.target.value }); if (cmdSaveError) setCmdSaveError(null); }}
                placeholder="display version"
              />
              {cmdSaveError && <p className="mt-1 text-xs text-[hsl(var(--danger))]">{cmdSaveError}</p>}
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
      )}

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

      {/* ===== Report Template Editor (split pane) ===== */}
      <Modal
        open={reportModalOpen}
        title={editingReport ? "编辑报告模板" : "新建报告模板"}
        width="max-w-6xl"
        onClose={() => setReportModalOpen(false)}
        footer={
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => setReportModalOpen(false)}>取消</Button>
            <Button onClick={handleSaveReport} loading={reportSaving}>{editingReport ? "保存" : "创建"}</Button>
          </div>
        }
      >
        <ReportTemplateEditor
          form={reportForm}
          onChange={setReportForm}
          shakeName={shakeFields.has("report_name")}
          saveError={reportSaveError}
          onErrorClear={() => setReportSaveError(null)}
        />
      </Modal>

      <Modal
        open={confirmDeleteReport !== null}
        title="确认删除"
        width="max-w-sm"
        onClose={() => setConfirmDeleteReport(null)}
        footer={
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => setConfirmDeleteReport(null)}>取消</Button>
            <Button variant="danger" onClick={() => handleDeleteReport(confirmDeleteReport!)}>删除</Button>
          </div>
        }
      >
        <p>确定要删除此报告模板吗？此操作不可恢复。</p>
      </Modal>
    </div>
  );
}

// ============================================================
// Report Template Editor — 列定义驱动 + A4 实时预览
// ============================================================

const SAMPLE_DEVICE = {
  name: "SW-CORE-01",
  ip: "192.168.1.1",
  vendor: "H3C",
  model: "S6850-56HF",
  sn: "210235A1A1234567",
  mfg_date: "2024-08-12",
  inspect_time: "2026-06-13 09:30:00",
};

const SAMPLE_ROWS = [
  { item: "查看设备版本", cmd: "display version",
    output: "H3C Comware Software, Version 7.1.075 R6628P12\nUptime: 60 days 4 hours",
    status: "ok", finding: "", suggestion: "" },
  { item: "查看 CPU 使用率", cmd: "display cpu-usage",
    output: "Slot 0 CPU 0 usage:\n  in last 5 seconds: 78%\n  in last 1 minute:  72%",
    status: "warning", finding: "CPU 使用率偏高 (78%)",
    suggestion: "建议关注 CPU 负载趋势，必要时检查 CPU 占用进程" },
  { item: "查看内存使用率", cmd: "display memory-usage",
    output: "System Total Memory(MB):   8192\nMemory Used(MB):           3686 (45%)",
    status: "ok", finding: "", suggestion: "" },
  { item: "查看接口状态", cmd: "display interface brief",
    output: "GE1/0/1   UP    1000Mbps  full\nGE1/0/2   DOWN  --        --",
    status: "info", finding: "GE1/0/2 处于 DOWN 状态", suggestion: "确认是否为预期未使用接口" },
];

const STATUS_DEF: Record<string, { label: string; bg: string; color: string }> = {
  ok:       { label: "正常", bg: "#E2F0D9", color: "#385723" },
  info:     { label: "提示", bg: "#DEEBF7", color: "#1F4E79" },
  warning:  { label: "注意", bg: "#FFF2CC", color: "#806000" },
  critical: { label: "严重", bg: "#FBE5D6", color: "#843C0C" },
};

function ReportTemplateEditor({
  form, onChange, shakeName, saveError, onErrorClear,
}: {
  form: ReportForm;
  onChange: (f: ReportForm) => void;
  shakeName: boolean;
  saveError: string | null;
  onErrorClear: () => void;
}) {
  const update = (patch: Partial<ReportForm>) => onChange({ ...form, ...patch });
  const updateConfig = (patch: Partial<ReportTemplateConfig>) =>
    onChange({ ...form, config: { ...form.config, ...patch } });

  return (
    <div className="grid grid-cols-12 gap-4" style={{ maxHeight: "75vh" }}>
      {/* 左侧表单 */}
      <div className="col-span-5 overflow-y-auto pr-2 space-y-3" style={{ maxHeight: "75vh" }}>
        {/* 元信息 */}
        <div className="space-y-2 pb-3 border-b border-[hsl(var(--border-light))]">
          <div className="grid grid-cols-2 gap-2">
            <div>
              <label className="block text-[11px] font-medium text-[hsl(var(--text-secondary))] mb-1">模板名称</label>
              <Input
                value={form.name}
                className={shakeName ? "animate-shake" : ""}
                onChange={(e) => { update({ name: e.target.value }); onErrorClear(); }}
                placeholder="如：H3C 月度巡检"
              />
            </div>
            <div>
              <label className="block text-[11px] font-medium text-[hsl(var(--text-secondary))] mb-1">厂商</label>
              <Select value={form.vendor} onChange={(e) => update({ vendor: e.target.value })}>
                <option value="">通用</option>
                {VENDORS.map((v) => <option key={v} value={v}>{v}</option>)}
              </Select>
            </div>
          </div>
          {saveError && <p className="text-[11px] text-[hsl(var(--danger))]">{saveError}</p>}
          <div>
            <label className="block text-[11px] font-medium text-[hsl(var(--text-secondary))] mb-1">描述</label>
            <Input value={form.description} onChange={(e) => update({ description: e.target.value })} placeholder="模板用途说明" />
          </div>
        </div>

        <CollapsibleSection title="封面" defaultOpen>
          <div className="space-y-2">
            <Field label="主标题（支持 {{vendor}}）">
              <Input value={form.config.cover.title}
                onChange={(e) => updateConfig({ cover: { ...form.config.cover, title: e.target.value } })} />
            </Field>
            <Field label="副标题">
              <Input value={form.config.cover.subtitle}
                onChange={(e) => updateConfig({ cover: { ...form.config.cover, subtitle: e.target.value } })} />
            </Field>
            <Field label="主色调">
              <div className="flex items-center gap-2">
                <input type="color" value={form.config.cover.primary_color}
                  onChange={(e) => updateConfig({ cover: { ...form.config.cover, primary_color: e.target.value } })}
                  className="h-7 w-10 rounded border border-[hsl(var(--border))]" />
                <Input value={form.config.cover.primary_color}
                  onChange={(e) => updateConfig({ cover: { ...form.config.cover, primary_color: e.target.value } })} />
              </div>
            </Field>
          </div>
        </CollapsibleSection>

        <CollapsibleSection title="设备信息" defaultOpen>
          <label className="flex items-center gap-2 text-[11px] mb-2">
            <input type="checkbox" checked={form.config.device_info.enabled}
              onChange={(e) => updateConfig({ device_info: { ...form.config.device_info, enabled: e.target.checked } })}
              className="accent-[hsl(var(--accent))]" />
            启用此区块
          </label>
          {form.config.device_info.enabled && (
            <>
              <Field label="布局">
                <Select value={form.config.device_info.layout}
                  onChange={(e) => updateConfig({ device_info: { ...form.config.device_info, layout: e.target.value as "two_column" | "table" } })}>
                  <option value="two_column">两列（标签|值）</option>
                  <option value="table">横向表格</option>
                </Select>
              </Field>
              <div className="space-y-1">
                <label className="text-[11px] font-medium text-[hsl(var(--text-secondary))]">字段（拖拽调整顺序）</label>
                <DraggableList
                  items={form.config.device_info.fields}
                  onReorder={(fields) => updateConfig({ device_info: { ...form.config.device_info, fields } })}
                  renderItem={(f, i) => (
                    <div className="flex items-center gap-2">
                      <input type="checkbox" checked={f.visible}
                        onChange={(e) => {
                          const fields = [...form.config.device_info.fields];
                          fields[i] = { ...f, visible: e.target.checked };
                          updateConfig({ device_info: { ...form.config.device_info, fields } });
                        }}
                        onClick={(e) => e.stopPropagation()}
                        className="accent-[hsl(var(--accent))] shrink-0" />
                      <input type="text" value={f.label}
                        onChange={(e) => {
                          const fields = [...form.config.device_info.fields];
                          fields[i] = { ...f, label: e.target.value };
                          updateConfig({ device_info: { ...form.config.device_info, fields } });
                        }}
                        onClick={(e) => e.stopPropagation()}
                        className="flex-1 min-w-0 text-[11px] px-1.5 py-0.5 rounded border border-[hsl(var(--border))] bg-[hsl(var(--bg-app))]" />
                      <span className="text-[10px] text-[hsl(var(--text-tertiary))] shrink-0">{f.key}</span>
                    </div>
                  )}
                  itemKey={(f) => f.key}
                />
              </div>
            </>
          )}
        </CollapsibleSection>

        <CollapsibleSection title="巡检明细表" defaultOpen>
          <div className="space-y-1.5">
            <label className="text-[11px] font-medium text-[hsl(var(--text-secondary))]">列定义（拖拽调整顺序，宽度为百分比）</label>
            <DraggableList
              items={form.config.command_table.columns}
              onReorder={(columns) => updateConfig({ command_table: { ...form.config.command_table, columns } })}
              renderItem={(col, i) => (
                <div className="flex items-center gap-2">
                  <input type="checkbox" checked={col.visible}
                    onChange={(e) => {
                      const columns = [...form.config.command_table.columns];
                      columns[i] = { ...col, visible: e.target.checked };
                      updateConfig({ command_table: { ...form.config.command_table, columns } });
                    }}
                    onClick={(e) => e.stopPropagation()}
                    className="accent-[hsl(var(--accent))] shrink-0" />
                  <input type="text" value={col.label}
                    onChange={(e) => {
                      const columns = [...form.config.command_table.columns];
                      columns[i] = { ...col, label: e.target.value };
                      updateConfig({ command_table: { ...form.config.command_table, columns } });
                    }}
                    onClick={(e) => e.stopPropagation()}
                    className="flex-1 min-w-0 text-[11px] px-1.5 py-0.5 rounded border border-[hsl(var(--border))] bg-[hsl(var(--bg-app))]" />
                  <span className="text-[10px] text-[hsl(var(--text-tertiary))] shrink-0">{col.key}</span>
                  <input type="number" min={4} max={80} value={col.width}
                    onChange={(e) => {
                      const columns = [...form.config.command_table.columns];
                      columns[i] = { ...col, width: Number(e.target.value) || 10 };
                      updateConfig({ command_table: { ...form.config.command_table, columns } });
                    }}
                    onClick={(e) => e.stopPropagation()}
                    className="w-12 text-[11px] px-1 py-0.5 rounded border border-[hsl(var(--border))] bg-[hsl(var(--bg-app))]" />
                  <span className="text-[10px] text-[hsl(var(--text-tertiary))] shrink-0">%</span>
                </div>
              )}
              itemKey={(col) => col.key}
            />
            <Field label="输出截断行数（0 = 不截断）">
              <Input type="number" value={String(form.config.command_table.output_max_lines)}
                onChange={(e) => updateConfig({ command_table: { ...form.config.command_table, output_max_lines: Math.max(0, Number(e.target.value) || 0) } })} />
            </Field>
          </div>
        </CollapsibleSection>

        <CollapsibleSection title="巡检总结" defaultOpen={false}>
          <label className="flex items-center gap-2 text-[11px] mb-2">
            <input type="checkbox" checked={form.config.summary.enabled}
              onChange={(e) => updateConfig({ summary: { ...form.config.summary, enabled: e.target.checked } })}
              className="accent-[hsl(var(--accent))]" />
            启用此区块
          </label>
          {form.config.summary.enabled && (
            <div className="space-y-2">
              <Field label="区块标题">
                <Input value={form.config.summary.title}
                  onChange={(e) => updateConfig({ summary: { ...form.config.summary, title: e.target.value } })} />
              </Field>
              <label className="flex items-center gap-2 text-[11px]">
                <input type="checkbox" checked={form.config.summary.show_problem_table}
                  onChange={(e) => updateConfig({ summary: { ...form.config.summary, show_problem_table: e.target.checked } })}
                  className="accent-[hsl(var(--accent))]" />
                显示问题汇总表（只列 警告 / 严重）
              </label>
            </div>
          )}
        </CollapsibleSection>

        <CollapsibleSection title="页眉/页脚" defaultOpen={false}>
          <div className="space-y-2">
            <Field label="页眉（支持 {{vendor}}）">
              <Input value={form.config.header}
                onChange={(e) => updateConfig({ header: e.target.value })} />
            </Field>
            <Field label="页脚（支持 {{page}} {{total}}）">
              <Input value={form.config.footer}
                onChange={(e) => updateConfig({ footer: e.target.value })} />
            </Field>
          </div>
        </CollapsibleSection>
      </div>

      {/* 右侧 A4 预览 */}
      <div className="col-span-7 flex flex-col" style={{ maxHeight: "75vh" }}>
        <div className="px-3 py-1.5 text-[11px] font-medium bg-[hsl(var(--bg-hover))] border border-[hsl(var(--border))] rounded-t-md text-[hsl(var(--text-secondary))]">
          实时预览（示例数据，按 A4 比例缩放展示）
        </div>
        <div className="flex-1 overflow-auto bg-[hsl(var(--bg-app))] border border-t-0 border-[hsl(var(--border))] rounded-b-md p-4">
          <div className="min-w-full flex justify-center">
            <DocxPreview config={form.config} />
          </div>
        </div>
      </div>
    </div>
  );
}

// ----- 表单辅助组件 -----

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <label className="block text-[11px] font-medium text-[hsl(var(--text-secondary))] mb-1">{label}</label>
      {children}
    </div>
  );
}

function CollapsibleSection({ title, defaultOpen, children }: { title: string; defaultOpen: boolean; children: React.ReactNode }) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className="border border-[hsl(var(--border))] rounded-md">
      <button type="button" onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-1.5 px-2.5 py-1.5 bg-[hsl(var(--bg-hover))] hover:bg-[hsl(var(--bg-active))] rounded-t-md text-left">
        {open ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
        <span className="text-[12px] font-medium text-[hsl(var(--text-primary))]">{title}</span>
      </button>
      {open && <div className="p-2.5">{children}</div>}
    </div>
  );
}

function DraggableList<T>({
  items, onReorder, renderItem, itemKey,
}: {
  items: T[];
  onReorder: (items: T[]) => void;
  renderItem: (item: T, idx: number) => React.ReactNode;
  itemKey: (item: T) => string | number;
}) {
  return (
    <div className="space-y-1">
      {items.map((item, i) => (
        <div
          key={itemKey(item)}
          draggable
          onDragStart={(e) => {
            e.dataTransfer.setData("text/plain", String(i));
            e.dataTransfer.effectAllowed = "move";
            e.currentTarget.style.opacity = "0.4";
          }}
          onDragEnd={(e) => { e.currentTarget.style.opacity = ""; }}
          onDragOver={(e) => { e.preventDefault(); e.currentTarget.style.borderColor = "hsl(var(--accent))"; }}
          onDragLeave={(e) => { e.currentTarget.style.borderColor = ""; }}
          onDrop={(e) => {
            e.preventDefault();
            e.currentTarget.style.borderColor = "";
            const fromIdx = parseInt(e.dataTransfer.getData("text/plain"));
            if (isNaN(fromIdx) || fromIdx === i) return;
            const next = [...items];
            const [moved] = next.splice(fromIdx, 1);
            if (moved !== undefined) next.splice(i, 0, moved);
            onReorder(next);
          }}
          className="flex items-center gap-1.5 px-2 py-1.5 bg-[hsl(var(--bg-card))] border border-[hsl(var(--border))] rounded cursor-grab active:cursor-grabbing"
        >
          <GripVertical size={12} className="text-[hsl(var(--text-tertiary))] shrink-0" />
          <div className="flex-1 min-w-0">{renderItem(item, i)}</div>
        </div>
      ))}
    </div>
  );
}

// ----- A4 预览 -----

function applyVars(s: string, device: typeof SAMPLE_DEVICE): string {
  return s
    .replace(/\{\{vendor\}\}/g, device.vendor)
    .replace(/\{\{device_name\}\}/g, device.name)
    .replace(/\{\{page\}\}/g, "1")
    .replace(/\{\{total\}\}/g, "3");
}

function DocxPreview({ config }: { config: ReportTemplateConfig }) {
  const dev = SAMPLE_DEVICE;
  const accent = config.cover.primary_color;
  const title = applyVars(config.cover.title, dev);
  const headerText = applyVars(config.header, dev);
  const footerText = applyVars(config.footer, dev);
  const visibleColumns = config.command_table.columns.filter((c) => c.visible);
  const totalW = visibleColumns.reduce((acc, c) => acc + Math.max(c.width, 1), 0) || 100;
  const visibleFields: DeviceField[] = config.device_info.fields.filter((f) => f.visible);

  const valueOf = (key: DeviceField["key"]) => {
    switch (key) {
      case "name": return dev.name;
      case "ip": return dev.ip;
      case "vendor": return dev.vendor;
      case "model": return dev.model;
      case "sn": return dev.sn;
      case "mfg_date": return dev.mfg_date;
      case "inspect_time": return dev.inspect_time;
      default: return "";
    }
  };

  // CLI 提示符：真实 sysname 来自设备配置；预览中用 aHope 模拟，不使用设备名称
  const promptOf = (): string => {
    const v = dev.vendor.toLowerCase();
    const sysname = "aHope";
    if (v.includes("cisco") || v.includes("思科") || v.includes("ruijie") || v.includes("锐捷")) {
      return `${sysname}>`;
    }
    return `<${sysname}>`;
  };

  const cellFor = (col: TableColumn, row: typeof SAMPLE_ROWS[number], idx: number): React.ReactNode => {
    switch (col.key) {
      case "seq": return idx + 1;
      case "item": return row.item;
      case "output": {
        const text = `${promptOf()}${row.cmd}\n${row.output}`;
        return <pre style={{ margin: 0, fontFamily: "Consolas, monospace", fontSize: 10, whiteSpace: "pre-wrap" }}>{text}</pre>;
      }
      case "ai_judgment": {
        const m = STATUS_DEF[row.status];
        const lines: string[] = [];
        if (m) lines.push(`【${m.label}】`);
        if (row.finding) lines.push(row.finding);
        if (row.suggestion) lines.push(`建议：${row.suggestion}`);
        return (
          <div style={{ color: m?.color, fontWeight: 500 }}>
            {lines.map((line, k) => <div key={k}>{line}</div>)}
          </div>
        );
      }
      default: return "";
    }
  };

  const problems = SAMPLE_ROWS.filter((r) => r.status === "warning" || r.status === "critical");

  return (
    <div style={{ width: "min(100%, 210mm)", display: "flex", justifyContent: "center" }}>
      <div
        style={{
          width: "210mm",
          minHeight: "297mm",
          background: "white",
          boxShadow: "0 2px 12px rgba(0,0,0,0.08)",
          padding: "20mm 18mm",
          boxSizing: "border-box",
          color: "#222",
          fontFamily: '"FangSong", "STFangsong", "仿宋", serif',
          fontSize: 11,
          transformOrigin: "top center",
          transform: "scale(min(0.78, calc((100vw - 520px) / 794)))",
        }}
      >
      {/* 页眉 */}
      {headerText.trim() && (
        <div style={{ textAlign: "center", fontSize: 10, color: "#666", borderBottom: "1px solid #ddd", paddingBottom: 4, marginBottom: 12 }}>
          {headerText.replace(/\{\{[^}]+\}\}/g, "")}
        </div>
      )}

      {/* 封面 */}
      <div style={{ textAlign: "center", padding: "60px 0 40px" }}>
        <div style={{ fontSize: 28, fontWeight: 700, color: accent }}>{title}</div>
        {config.cover.subtitle && (
          <div style={{ fontSize: 16, color: "#777", marginTop: 12 }}>{applyVars(config.cover.subtitle, dev)}</div>
        )}
        <div style={{ marginTop: 80, fontSize: 14 }}>设备：{dev.name}</div>
        <div style={{ marginTop: 6, fontSize: 12, color: "#888" }}>生成日期：{dev.inspect_time.slice(0, 10)}</div>
      </div>

      <div style={{ height: 1, background: "#eee", margin: "20px 0" }} />

      {/* 设备信息 */}
      {config.device_info.enabled && visibleFields.length > 0 && (
        <>
          <SectionHeading text="基本信息" color={accent} />
          {config.device_info.layout === "table" ? (
            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 11, border: "1.2px solid #000" }}>
              <thead>
                <tr>
                  {visibleFields.map((f) => (
                    <th key={f.key} style={{ background: "#F2F2F2", padding: "8px 6px", border: "0.6px solid #000", fontWeight: 600, textAlign: "center", verticalAlign: "middle" }}>{f.label}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                <tr>
                  {visibleFields.map((f) => (
                    <td key={f.key} style={{ padding: "8px 6px", border: "0.6px solid #000", textAlign: "center", verticalAlign: "middle" }}>{valueOf(f.key)}</td>
                  ))}
                </tr>
              </tbody>
            </table>
          ) : (
            // 仿模板 4 列布局：标签 | 值 | 标签 | 值
            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 11, border: "1.2px solid #000" }}>
              <tbody>
                {(() => {
                  const rows: React.ReactNode[] = [];
                  for (let i = 0; i < visibleFields.length; i += 2) {
                    const f1 = visibleFields[i];
                    if (!f1) continue;
                    const f2 = visibleFields[i + 1];
                    rows.push(
                      <tr key={i}>
                        <td style={{ background: "#F2F2F2", padding: "8px", border: "0.6px solid #000", fontWeight: 600, width: "20%", verticalAlign: "middle" }}>{f1.label}</td>
                        <td style={{ padding: "8px", border: "0.6px solid #000", width: "32%", verticalAlign: "middle" }}>{valueOf(f1.key)}</td>
                        <td style={{ background: "#F2F2F2", padding: "8px", border: "0.6px solid #000", fontWeight: 600, width: "15%", verticalAlign: "middle" }}>{f2 ? f2.label : ""}</td>
                        <td style={{ padding: "8px", border: "0.6px solid #000", verticalAlign: "middle" }}>{f2 ? valueOf(f2.key) : ""}</td>
                      </tr>
                    );
                  }
                  return rows;
                })()}
              </tbody>
            </table>
          )}
        </>
      )}

      {/* 巡检明细 */}
      <div style={{ marginTop: 16 }}>
        <SectionHeading text="巡检记录" color={accent} />
        {visibleColumns.length === 0 ? (
          <div style={{ color: "#888", fontSize: 11 }}>未启用任何列</div>
        ) : (
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 10, tableLayout: "fixed", border: "1.2px solid #000" }}>
            <colgroup>
              {visibleColumns.map((c, i) => (
                <col key={i} style={{ width: `${(Math.max(c.width, 1) / totalW) * 100}%` }} />
              ))}
            </colgroup>
            <thead>
              <tr>
                {visibleColumns.map((c) => (
                  <th key={c.key} style={{ background: "white", color: "#000", padding: "8px 6px", border: "0.6px solid #000", fontWeight: 700, textAlign: "center", verticalAlign: "middle" }}>
                    {c.label}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {SAMPLE_ROWS.map((row, i) => (
                <tr key={i}>
                  {visibleColumns.map((col) => {
                    const fill = col.key === "ai_judgment" ? STATUS_DEF[row.status]?.bg : undefined;
                    const isOutput = col.key === "output";
                    return (
                      <td key={col.key} style={{
                        padding: "6px",
                        border: "0.6px solid #000",
                        background: fill,
                        verticalAlign: isOutput ? "top" : "middle",
                        textAlign: isOutput ? "left" : "center",
                        wordBreak: "break-word",
                      }}>
                        {cellFor(col, row, i)}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* 总结 */}
      {config.summary.enabled && (
        <div style={{ marginTop: 16 }}>
          <SectionHeading text={config.summary.title || "巡检总结"} color={accent} />
          <div style={{ fontSize: 11, fontWeight: 600 }}>整体状态：警告</div>
          <div style={{ fontSize: 11, marginTop: 4, lineHeight: 1.6 }}>
            设备整体运行基本正常，CPU 使用率偏高需关注；GE1/0/2 接口未启用，建议确认是否为预期。
          </div>
          {config.summary.show_problem_table && problems.length > 0 && (
            <>
              <div style={{ fontWeight: 700, fontSize: 12, marginTop: 12, marginBottom: 6 }}>问题汇总</div>
              <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 10, border: "1.2px solid #000" }}>
                <thead>
                  <tr>
                    {["状态", "巡检项目", "发现", "建议"].map((h) => (
                      <th key={h} style={{ background: "white", color: "#000", padding: "8px 6px", border: "0.6px solid #000", fontWeight: 700, textAlign: "center", verticalAlign: "middle" }}>{h}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {problems.map((p, i) => {
                    const m = STATUS_DEF[p.status];
                    const cellBase = { padding: "6px", border: "0.6px solid #000", textAlign: "center" as const, verticalAlign: "middle" as const };
                    return (
                      <tr key={i}>
                        <td style={{ ...cellBase, background: m?.bg, color: m?.color, fontWeight: 600 }}>{m?.label}</td>
                        <td style={cellBase}>{p.item}</td>
                        <td style={cellBase}>{p.finding}</td>
                        <td style={cellBase}>{p.suggestion}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </>
          )}
        </div>
      )}

        {/* 页脚 */}
        {footerText.trim() && (
          <div style={{ textAlign: "center", fontSize: 10, color: "#666", borderTop: "1px solid #ddd", paddingTop: 4, marginTop: 24 }}>
            {footerText}
          </div>
        )}
      </div>
    </div>
  );
}

function SectionHeading({ text, color }: { text: string; color: string }) {
  return (
    <div style={{
      fontSize: 14, fontWeight: 700, color,
      marginTop: 14, marginBottom: 8,
      lineHeight: 1.5,
      fontFamily: '"FangSong", "STFangsong", "仿宋", serif',
    }}>{text}</div>
  );
}

// ============================================================
// Command List
// ============================================================

const CATEGORY_LABELS: Record<string, string> = {
  version: "版本信息", clock: "系统时钟", cpu: "CPU", memory: "内存",
  hardware: "硬件信息", storage: "存储", interface: "接口", vlan: "VLAN", log: "日志",
  protocol: "协议", vpn: "VPN", ha: "高可用", security: "安全策略", wireless: "无线", general: "通用",
  system: "系统信息", disk: "磁盘", network: "网络", service: "服务", process: "进程", schedule: "定时任务",
};

function CommandList({
  commands, onEdit, onDelete,
}: {
  commands: CommandPool[];
  onEdit: (c: CommandPool) => void;
  onDelete: (id: number) => void;
}) {
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  const toggle = (cat: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(cat)) next.delete(cat); else next.add(cat);
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
    return [...map.entries()].sort(([a], [b]) => {
      const ia = CATEGORIES.indexOf(a as typeof CATEGORIES[number]);
      const ib = CATEGORIES.indexOf(b as typeof CATEGORIES[number]);
      return (ia === -1 ? 99 : ia) - (ib === -1 ? 99 : ib);
    });
  }, [commands]);

  if (commands.length === 0) {
    return <div className="text-center py-8 text-sm text-[hsl(var(--text-tertiary))]">暂无命令</div>;
  }

  return (
    <div className="space-y-1">
      {grouped.map(([cat, cmds]) => {
        const open = !collapsed.has(cat);
        return (
          <div key={cat} className="rounded-lg border border-[hsl(var(--border))] overflow-hidden">
            <button onClick={() => toggle(cat)}
              className="w-full flex items-center gap-2 px-3 py-2 bg-[hsl(var(--bg-hover))] hover:bg-[hsl(var(--bg-active))] transition-colors text-left">
              {open ? <ChevronDown size={14} className="text-[hsl(var(--text-tertiary))]" /> : <ChevronRight size={14} className="text-[hsl(var(--text-tertiary))]" />}
              <span className="text-xs font-medium text-[hsl(var(--text-primary))]">{CATEGORY_LABELS[cat] || cat}</span>
              <span className="text-[11px] text-[hsl(var(--text-tertiary))] ml-auto">{cmds.length} 条</span>
            </button>
            {open && (
              <div className="divide-y divide-[hsl(var(--border-light))]">
                {cmds.map((cmd) => (
                  <div key={cmd.id} className="flex items-center gap-3 px-4 py-2 hover:bg-[hsl(var(--bg-hover))] transition-colors group">
                    <code className="flex-1 text-xs bg-[hsl(var(--bg-hover))] px-2 py-1 rounded font-mono text-[hsl(var(--text-primary))]">{cmd.command}</code>
                    {cmd.needs_root && <span title="需要 root 权限"><Lock size={12} className="text-[hsl(var(--warning))] shrink-0" /></span>}
                    {cmd.description && <span className="text-xs text-[hsl(var(--text-tertiary))] max-w-[200px] truncate hidden sm:block">{cmd.description}</span>}
                    <div className="flex gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
                      <button onClick={() => onEdit(cmd)} className="p-1 rounded hover:bg-[hsl(var(--bg-active))] text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--accent))]" title="编辑">
                        <Pencil size={13} />
                      </button>
                      <button onClick={() => onDelete(cmd.id)} className="p-1 rounded hover:bg-[hsl(var(--danger)_/_0.1)] text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--danger))]" title="删除">
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
