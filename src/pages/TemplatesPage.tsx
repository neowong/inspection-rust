import React, { useState, useEffect, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronRight, ChevronDown, Pencil, Trash2, Upload, Copy, Star, Settings, GripVertical } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { InspectionTemplate, CommandPool, ReportTemplate, TemplateSection, TemplateConfig } from "../types";
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
// Report Section Builder
// ============================================================

function cloneSections(): TemplateSection[] {
  return [
    { type: "title", enabled: true, label: "报告标题", config: {} },
    { type: "basic_info", enabled: true, label: "基本信息", config: { fields: ["device_name", "device_ip", "vendor", "model", "sn", "manufacturing_date"] } },
    { type: "inspection_results", enabled: true, label: "巡检结果", config: { show_output: true, max_output_lines: 60 } },
    { type: "ai_analysis", enabled: true, label: "AI 分析总结", config: {} },
    { type: "overall_assessment", enabled: true, label: "总体评估", config: {} },
  ];
}

const SECTION_META: Record<string, { description: string; icon: string }> = {
  title: { description: "报告标题和生成时间", icon: "📋" },
  basic_info: { description: "设备名称、IP、厂商、型号、序列号、生产日期", icon: "📊" },
  inspection_results: { description: "逐命令巡检判断结果（序号/项目/结果/结论）", icon: "✅" },
  ai_analysis: { description: "AI 对巡检结果的整体分析文字", icon: "🤖" },
  overall_assessment: { description: "综合判断结论和处理建议", icon: "📝" },
};

const BASIC_INFO_FIELDS = [
  { key: "device_name", label: "设备名称" },
  { key: "device_ip", label: "IP 地址" },
  { key: "vendor", label: "厂商" },
  { key: "model", label: "型号" },
  { key: "sn", label: "序列号" },
  { key: "manufacturing_date", label: "生产日期" },
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
  command_ids: number[];
  report_template_id: number | null;
}

const EMPTY_TEMPLATE_FORM: TemplateForm = {
  name: "", vendor: "H3C", model: "", device_type: "", description: "", command_ids: [], report_template_id: null,
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
  const [templateForm, setTemplateForm] = useState<TemplateForm>(EMPTY_TEMPLATE_FORM);
  const [confirmDeleteTemplate, setConfirmDeleteTemplate] = useState<number | null>(null);
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

  // Report template state
  const [reportTemplates, setReportTemplates] = useState<ReportTemplate[]>([]);
  const [renderedPreview, setRenderedPreview] = useState<string | null>(null);
  const [confirmDeleteReport, setConfirmDeleteReport] = useState<number | null>(null);
  const [reportModalOpen, setReportModalOpen] = useState(false);
  const [editingReport, setEditingReport] = useState<ReportTemplate | null>(null);
  const [reportForm, setReportForm] = useState({
    name: "", vendor: "", format: "markdown" as "markdown" | "html",
    description: "", mode: "visual" as "visual" | "advanced",
    sections: cloneSections(),
    content: "",
  });
  const [expandedSection, setExpandedSection] = useState<string | null>(null);
  const [reportSaving, setReportSaving] = useState(false);
  const [reportSaveError, setReportSaveError] = useState<string | null>(null);
  const contentTextareaRef = useRef<HTMLTextAreaElement>(null);

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
      report_template_id: t.report_template_id ?? null,
    });
    setTemplateModal(true);
  };

  const handleSaveTemplate = () => {
    if (!templateForm.name.trim()) { triggerShake("template_name"); return; }

    const data: Record<string, unknown> = {
      name: templateForm.name,
      vendor: templateForm.vendor,
      config: JSON.stringify({ command_ids: templateForm.command_ids }),
    };
    if (templateForm.model) data.model = templateForm.model;
    if (templateForm.device_type) data.device_type = templateForm.device_type;
    if (templateForm.description) data.description = templateForm.description;
    if (templateForm.report_template_id !== null) data.report_template_id = templateForm.report_template_id;

    setSaving(true);
    setSaveError(null);

    const promise = editingTemplate
      ? invoke<InspectionTemplate>("update_template", { templateId: editingTemplate.id, data })
      : invoke<InspectionTemplate>("create_template", { data });

    promise
      .then(() => { setTemplateModal(false); loadTemplates(); })
      .catch((e) => { setSaveError(typeof e === "string" ? e : JSON.stringify(e)); })
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
    if (!cmdForm.command.trim()) { triggerShake("cmd_command"); return; }
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

  // Report template handlers
  const openNewReport = () => {
    setEditingReport(null);
    setReportForm({
      name: "", vendor: "", format: "markdown", description: "", mode: "visual",
      sections: cloneSections(),
      content: "",
    });
    setReportSaveError(null);
    setExpandedSection(null);
    setReportModalOpen(true);
  };

  const openEditReport = (rt: ReportTemplate) => {
    setEditingReport(rt);
    // Parse config_json for visual mode
    let sections = cloneSections();
    const mode = rt.mode || "visual";
    if (mode === "visual" && rt.config_json) {
      try {
        const cfg = JSON.parse(rt.config_json) as TemplateConfig;
        if (cfg.sections?.length) {
          sections = cfg.sections.map(s => ({
            type: s.type,
            enabled: s.enabled,
            label: s.label,
            config: { ...(s.config || {}) },
          } as TemplateSection));
        }
      } catch { /* use defaults */ }
    }
    setReportForm({
      name: rt.name,
      vendor: rt.vendor || "",
      format: rt.format,
      description: rt.description || "",
      mode,
      sections,
      content: rt.content || "",
    });
    setReportSaveError(null);
    setExpandedSection(null);
    setReportModalOpen(true);
  };

  const handleCopyReport = (rt: ReportTemplate) => {
    invoke<ReportTemplate>("create_report_template", {
      data: {
        name: rt.name + " (副本)",
        vendor: rt.vendor,
        content: rt.content,
        format: rt.format,
        description: rt.description,
        config_json: rt.config_json,
        mode: rt.mode,
      },
    })
      .then(() => loadReportTemplates())
      .catch(console.error);
  };

  const handleSetDefault = (id: number) => {
    // First unset all defaults, then set the new one
    invoke<void>("update_report_template", { templateId: id, data: { is_default: true } })
      .then(() => loadReportTemplates())
      .catch(console.error);
  };

  const handleSaveReport = () => {
    if (!reportForm.name.trim()) return;
    setReportSaving(true);
    setReportSaveError(null);

    const data: Record<string, unknown> = {
      name: reportForm.name,
      vendor: reportForm.vendor || null,
      format: reportForm.format,
      description: reportForm.description,
      mode: reportForm.mode,
    };

    if (reportForm.mode === "visual") {
      data.config_json = JSON.stringify({ sections: reportForm.sections });
      data.content = "";
    } else {
      data.content = reportForm.content;
      data.config_json = "";
    }

    const promise = editingReport
      ? invoke<ReportTemplate>("update_report_template", { templateId: editingReport.id, data })
      : invoke<ReportTemplate>("create_report_template", { data });

    promise
      .then(() => { setReportModalOpen(false); loadReportTemplates(); })
      .catch((e) => setReportSaveError(typeof e === "string" ? e : JSON.stringify(e)))
      .finally(() => setReportSaving(false));
  };

  const handleReportPreview = (id: number) => {
    invoke<string>("render_template_preview", { templateId: id })
      .then(setRenderedPreview)
      .catch(console.error);
  };

  const handleUploadReport = () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".md,.html,.txt";
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;
      const name = file.name.replace(/\.[^.]+$/, "");
      try {
        await invoke<ReportTemplate>("upload_template", {
          filePath: (file as unknown as { path: string }).path || file.name,
          name,
          vendor: null,
        });
        loadReportTemplates();
      } catch (err) {
        console.error(err);
      }
    };
    input.click();
  };

  const handleDeleteReport = (id: number) => {
    invoke<void>("delete_report_template", { templateId: id })
      .then(() => { setConfirmDeleteReport(null); loadReportTemplates(); })
      .catch(console.error);
  };

  // Section builder helpers
  const toggleSection = (index: number) => {
    const s = reportForm.sections[index];
    if (!s) return;
    setReportForm(prev => {
      const sections = [...prev.sections];
      sections[index] = { type: s.type, enabled: !s.enabled, label: s.label, config: { ...s.config } };
      return { ...prev, sections };
    });
  };

  const updateSectionConfig = (index: number, config: Record<string, unknown>) => {
    const s = reportForm.sections[index];
    if (!s) return;
    setReportForm(prev => {
      const sections = [...prev.sections];
      sections[index] = { type: s.type, enabled: s.enabled, label: s.label, config: { ...s.config, ...config } };
      return { ...prev, sections };
    });
  };

  const toggleSectionField = (index: number, field: string) => {
    const section = reportForm.sections[index];
    if (!section) return;
    const fields = (section.config.fields as string[]) || [];
    const newFields = fields.includes(field) ? fields.filter(f => f !== field) : [...fields, field];
    updateSectionConfig(index, { fields: newFields });
  };

  // ============================================================
  // Render
  // ============================================================

  return (
    <div className="space-y-6">
      <div className="sticky top-0 z-20 -mt-6 pt-6 pb-0 bg-[hsl(var(--bg-content))] shadow-sm relative">
        <h1 className="text-2xl font-bold text-[hsl(var(--text-primary))]">巡检模板</h1>
        <p className="text-sm text-[hsl(var(--text-secondary))] mt-1 mb-3">管理巡检模板、命令库和报告模板</p>

        {/* Tab bar */}
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

      {/* ====== Tab: 巡检模板 ====== */}
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
      )}

      {/* ====== Tab: 命令库 ====== */}
      {tab === "commands" && (
        <div>
          <Toolbar>
            <Button onClick={openAddCmd} size="sm">添加命令</Button>
            <SearchInput value={cmdSearch} onChange={setCmdSearch} placeholder="搜索命令..." />
          </Toolbar>

          {/* Vendor sub-tabs */}
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

          <CommandList
            commands={filteredCommands}
            onEdit={openEditCmd}
            onDelete={(id) => setConfirmDeleteCmd(id)}
          />
        </div>
      )}

      {/* ====== Tab: 报告模板 ====== */}
      {tab === "reports" && (
        <div>
          <Toolbar>
            <Button onClick={openNewReport} size="sm">新建模板</Button>
            <Button onClick={handleUploadReport} size="sm" variant="secondary">
              <Upload size={14} className="mr-1" />上传
            </Button>
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
              {
                key: "format", header: "格式", width: "80px", render: (r) => (
                  <span className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${
                    r.format === "html"
                      ? "bg-[hsl(var(--danger)_/_0.1)] text-[hsl(var(--danger))]"
                      : "bg-[hsl(var(--accent)_/_0.1)] text-[hsl(var(--accent))]"
                  }`}>{r.format === "html" ? "HTML" : "MD"}</span>
                ),
              },
              { key: "description", header: "描述", render: (r) => r.description || "-" },
              {
                key: "updated_at", header: "更新时间", render: (r) =>
                  new Date(r.updated_at).toLocaleString("zh-CN"),
              },
              {
                key: "actions", header: "操作", width: "200px", render: (r) => (
                  <div className="flex gap-0.5" onClick={(e) => e.stopPropagation()}>
                    <Button size="sm" variant="ghost" onClick={() => openEditReport(r)}>编辑</Button>
                    <Button size="sm" variant="ghost" onClick={() => handleReportPreview(r.id)}>预览</Button>
                    {!r.is_default && (
                      <Button size="sm" variant="ghost" onClick={() => handleSetDefault(r.id)}>默认</Button>
                    )}
                    <Button size="sm" variant="ghost" onClick={() => handleCopyReport(r)}><Copy size={12} /></Button>
                    {!r.is_default && (
                      <Button size="sm" variant="ghost" onClick={() => setConfirmDeleteReport(r.id)}><Trash2 size={12} /></Button>
                    )}
                  </div>
                ),
              },
            ]}
            data={reportTemplates}
            rowKey={(r) => r.id}
            onRowDoubleClick={(r) => openEditReport(r)}
            emptyText="暂无报告模板"
          />
        </div>
      )}

      {/* ====== Template Modal ====== */}
      {tab === "templates" && (
        <Modal
          open={templateModal}
          title={editingTemplate ? "编辑模板" : "添加模板"}
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
                <Input value={templateForm.name} className={shakeFields.has("template_name") ? "animate-shake" : ""} onChange={(e) => { setTemplateForm({ ...templateForm, name: e.target.value }); setSaveError(null); }} />
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
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">关联报告模板（可选）</label>
              <Select
                value={templateForm.report_template_id ?? ""}
                onChange={(e) => setTemplateForm({ ...templateForm, report_template_id: e.target.value ? Number(e.target.value) : null })}
              >
                <option value="">跟随默认</option>
                {reportTemplates.map((rt) => (
                  <option key={rt.id} value={rt.id}>{rt.name}{rt.is_default ? " (默认)" : ""}</option>
                ))}
              </Select>
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
      )}

      {/* Template Delete Confirm */}
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

      {/* ====== Command Modal ====== */}
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
              <Input value={cmdForm.command} className={shakeFields.has("cmd_command") ? "animate-shake" : ""} onChange={(e) => setCmdForm({ ...cmdForm, command: e.target.value })} placeholder="display version" />
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

      {/* Command Delete Confirm */}
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

      {/* Report Template Editor Modal — Split pane: builder + WYSIWYG */}
      <Modal
        open={reportModalOpen}
        title={editingReport ? "编辑报告模板" : "新建报告模板"}
        width="max-w-5xl"
        onClose={() => setReportModalOpen(false)}
        footer={
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => setReportModalOpen(false)}>取消</Button>
            <Button variant="secondary" onClick={() => {
              if (!editingReport) {
                setReportSaveError("请先保存模板后再预览");
                return;
              }
              handleReportPreview(editingReport.id);
            }}>预览真实数据</Button>
            <Button onClick={handleSaveReport} loading={reportSaving}>{editingReport ? "保存" : "创建"}</Button>
          </div>
        }
      >
        <div className="space-y-3" style={{ maxHeight: "72vh", overflowY: "auto" }}>
          {reportSaveError && (
            <div className="p-2 bg-[hsl(var(--danger)_/_0.1)] border border-[hsl(var(--danger)_/_0.3)] rounded text-sm text-[hsl(var(--danger))]">
              {reportSaveError}
            </div>
          )}

          {/* Top row: name + vendor + format + mode */}
          <div className="flex gap-3">
            <div className="flex-1">
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">模板名称</label>
              <Input value={reportForm.name} onChange={(e) => { setReportForm({ ...reportForm, name: e.target.value }); setReportSaveError(null); }} placeholder="如：标准巡检报告" />
            </div>
            <div className="w-28">
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">厂商</label>
              <Select value={reportForm.vendor} onChange={(e) => setReportForm({ ...reportForm, vendor: e.target.value })}>
                <option value="">通用</option>
                {VENDORS.map((v) => <option key={v} value={v}>{v}</option>)}
              </Select>
            </div>
            <div className="w-24">
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">格式</label>
              <Select value={reportForm.format} onChange={(e) => setReportForm({ ...reportForm, format: e.target.value as "markdown" | "html" })}>
                <option value="markdown">MD</option>
                <option value="html">HTML</option>
              </Select>
            </div>
            <div className="w-24">
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">模式</label>
              <Select value={reportForm.mode} onChange={(e) => setReportForm({ ...reportForm, mode: e.target.value as "visual" | "advanced" })}>
                <option value="visual">可视化</option>
                <option value="advanced">代码</option>
              </Select>
            </div>
          </div>
          <div>
            <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">描述</label>
            <Input value={reportForm.description} onChange={(e) => setReportForm({ ...reportForm, description: e.target.value })} placeholder="模板用途描述" />
          </div>

          {/* ---- VISUAL MODE: split pane ---- */}
          {reportForm.mode === "visual" && (
            <div className="flex gap-4" style={{ minHeight: "420px" }}>
              {/* Left: Building blocks panel */}
              <div className="w-1/2 flex flex-col">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-xs font-medium text-[hsl(var(--text-secondary))]">
                    🧱 积木拼装区 <span className="text-[10px] text-[hsl(var(--text-tertiary))]">（拖拽手柄排序）</span>
                  </span>
                  <span className="text-[10px] text-[hsl(var(--text-tertiary))] bg-[hsl(var(--bg-hover))] px-2 py-0.5 rounded-full">
                    {reportForm.sections.filter(s => s.enabled).length}/{reportForm.sections.length} 已启用
                  </span>
                </div>
                <div className="space-y-2 flex-1 overflow-y-auto pr-1" style={{ maxHeight: "400px" }}>
                  {reportForm.sections.map((section, i) => {
                    const meta = SECTION_META[section.type] || { description: "", icon: "📄" };
                    const isExpanded = expandedSection === section.type;
                    return (
                      <div
                        key={section.type}
                        draggable
                        onDragStart={(e) => {
                          e.dataTransfer.effectAllowed = "move";
                          e.dataTransfer.setData("text/plain", String(i));
                          setTimeout(() => {
                            (e.currentTarget as HTMLElement).style.opacity = "0.4";
                            (e.currentTarget as HTMLElement).style.transform = "scale(0.95)";
                          }, 0);
                        }}
                        onDragEnd={(e) => {
                          const el = e.currentTarget as HTMLElement;
                          el.style.opacity = "";
                          el.style.transform = "";
                        }}
                        onDragOver={(e) => {
                          e.preventDefault();
                          e.dataTransfer.dropEffect = "move";
                          const el = e.currentTarget as HTMLElement;
                          el.style.borderColor = "hsl(var(--accent))";
                          el.style.boxShadow = "0 0 0 1px hsl(var(--accent) / 0.3)";
                          el.style.transform = "scale(1.02)";
                        }}
                        onDragLeave={(e) => {
                          const el = e.currentTarget as HTMLElement;
                          el.style.borderColor = "";
                          el.style.boxShadow = "";
                          el.style.transform = "";
                        }}
                        onDrop={(e) => {
                          e.preventDefault();
                          const el = e.currentTarget as HTMLElement;
                          el.style.borderColor = "";
                          el.style.boxShadow = "";
                          el.style.transform = "";
                          const fromIdx = parseInt(e.dataTransfer.getData("text/plain"));
                          if (!isNaN(fromIdx) && fromIdx !== i) {
                            setReportForm(prev => {
                              const sections = [...prev.sections];
                              const [moved] = sections.splice(fromIdx, 1);
                              if (moved) sections.splice(i, 0, moved);
                              return { ...prev, sections };
                            });
                          }
                        }}
                        className={`group rounded-lg transition-all duration-150 select-none ${
                          section.enabled
                            ? "bg-[hsl(var(--bg-card))] border-2 border-[hsl(var(--border))] shadow-sm hover:shadow-md"
                            : "bg-[hsl(var(--bg-app))] border-2 border-dashed border-[hsl(var(--border-light))] opacity-50"
                        }`}
                      >
                        <div className="flex items-center gap-2 px-2 py-2">
                          {/* Drag handle — the grip */}
                          <div className="shrink-0 text-[hsl(var(--text-tertiary))] group-hover:text-[hsl(var(--text-primary))] transition-colors cursor-grab active:cursor-grabbing">
                            <GripVertical size={16} />
                          </div>
                          {/* Toggle */}
                          <input
                            type="checkbox"
                            checked={section.enabled}
                            onChange={() => toggleSection(i)}
                            className="accent-[hsl(var(--accent))] shrink-0 w-3.5 h-3.5"
                          />
                          <span className="text-base">{meta.icon}</span>
                          <div className="flex-1 min-w-0">
                            <div className={`text-xs font-medium ${section.enabled ? "text-[hsl(var(--text-primary))]" : "text-[hsl(var(--text-tertiary))]"}`}>
                              {section.label}
                            </div>
                            <div className="text-[10px] text-[hsl(var(--text-tertiary))] leading-tight">{meta.description}</div>
                          </div>
                          {/* Config gear */}
                          {(section.type === "basic_info" || section.type === "inspection_results") && section.enabled && (
                            <button
                              type="button"
                              onClick={(e) => { e.stopPropagation(); setExpandedSection(isExpanded ? null : section.type); }}
                              className={`shrink-0 p-1 rounded-md transition-colors ${
                                isExpanded
                                  ? "text-[hsl(var(--accent))] bg-[hsl(var(--accent)_/_0.1)]"
                                  : "text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-primary))] hover:bg-[hsl(var(--bg-hover))]"
                              }`}
                              title="配置字段"
                            ><Settings size={13} /></button>
                          )}
                        </div>
                        {/* Expanded config */}
                        {isExpanded && section.enabled && (
                          <div className="px-3 pb-2.5 border-t border-[hsl(var(--border-light))] pt-2 ml-9">
                            {section.type === "basic_info" && (
                              <div>
                                <div className="text-[10px] font-medium text-[hsl(var(--text-secondary))] mb-1.5">选择要显示的字段：</div>
                                <div className="flex flex-wrap gap-1.5">
                                  {BASIC_INFO_FIELDS.map((f) => {
                                    const fields = (section.config.fields as string[]) || [];
                                    const checked = fields.includes(f.key);
                                    return (
                                      <span
                                        key={f.key}
                                        onClick={() => toggleSectionField(i, f.key)}
                                        className={`px-2 py-1 rounded-md text-[11px] cursor-pointer border transition-all ${
                                          checked
                                            ? "bg-[hsl(var(--accent)_/_0.1)] border-[hsl(var(--accent))] text-[hsl(var(--accent))] font-medium"
                                            : "bg-[hsl(var(--bg-app))] border-[hsl(var(--border-light))] text-[hsl(var(--text-tertiary))] hover:border-[hsl(var(--border))] hover:text-[hsl(var(--text-secondary))]"
                                        }`}
                                      >{f.label}</span>
                                    );
                                  })}
                                </div>
                              </div>
                            )}
                            {section.type === "inspection_results" && (
                              <div className="flex items-center gap-4">
                                <label className="flex items-center gap-1.5 text-[11px] text-[hsl(var(--text-secondary))] cursor-pointer">
                                  <input type="checkbox" checked={section.config.show_output as boolean ?? true}
                                    onChange={(e) => updateSectionConfig(i, { show_output: e.target.checked })}
                                    className="accent-[hsl(var(--accent))] w-3 h-3" />
                                  显示原始输出
                                </label>
                                <label className="flex items-center gap-1.5 text-[11px] text-[hsl(var(--text-secondary))]">
                                  截断行数
                                  <input type="number" value={section.config.max_output_lines as number ?? 60}
                                    onChange={(e) => updateSectionConfig(i, { max_output_lines: Number(e.target.value) || 60 })}
                                    min={5} max={500}
                                    className="w-14 h-6 px-1.5 text-[11px] rounded-md border border-[hsl(var(--border))] bg-[hsl(var(--bg-app))] text-[hsl(var(--text-primary))] focus:outline-none focus:border-[hsl(var(--accent))]" />
                                </label>
                              </div>
                            )}
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>

              {/* Right: WYSIWYG preview */}
              <div className="w-1/2 border border-[hsl(var(--border))] rounded-lg bg-white flex flex-col shadow-inner">
                <div className="px-3 py-1.5 border-b border-[hsl(var(--border-light))] bg-[hsl(var(--bg-hover))] rounded-t-lg flex items-center gap-2">
                  <span className="text-[11px] font-medium text-[hsl(var(--text-secondary))]">👁 实时预览</span>
                  <span className="text-[10px] text-[hsl(var(--text-tertiary))]">（示例数据模拟最终报告效果）</span>
                </div>
                <div className="flex-1 overflow-y-auto p-3 space-y-2.5" style={{ maxHeight: "400px", fontFamily: "system-ui, -apple-system, sans-serif" }}>
                  {reportForm.sections.filter(s => s.enabled).length === 0 && (
                    <div className="text-center py-12">
                      <span className="text-3xl">🧱</span>
                      <p className="text-xs text-[hsl(var(--text-tertiary))] mt-2">从左侧拖拽积木块来组装报告</p>
                    </div>
                  )}
                  {reportForm.sections.filter(s => s.enabled).map((section, i) => (
                    <WysiwygBlock key={section.type + i} section={section} format={reportForm.format} />
                  ))}
                </div>
              </div>
            </div>
          )}

          {/* ADVANCED MODE */}
          {reportForm.mode === "advanced" && (
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">
                模板代码（{`{{变量}}`} 语法）
              </label>
              <textarea
                ref={contentTextareaRef}
                value={reportForm.content}
                onChange={(e) => setReportForm({ ...reportForm, content: e.target.value })}
                className="w-full h-72 font-mono text-xs p-3 rounded-md border border-[hsl(var(--border))] bg-[hsl(var(--bg-app))] text-[hsl(var(--text-primary))] resize-none focus:outline-none focus:border-[hsl(var(--accent))]"
                placeholder={reportForm.format === "markdown"
                  ? "# {{device_name}} 巡检报告\n\n> 生成时间: {{report_timestamp}}\n\n## 基本信息\n..."
                  : "<!DOCTYPE html>\n<html>\n..."
                }
              />
            </div>
          )}
        </div>
      </Modal>

      {/* Report Template Rendered Preview */}
      <Modal
        open={renderedPreview !== null}
        title="模板渲染预览"
        width="max-w-2xl"
        onClose={() => setRenderedPreview(null)}
        footer={
          <Button variant="secondary" onClick={() => setRenderedPreview(null)}>关闭</Button>
        }
      >
        <div className="max-h-[70vh] overflow-auto">
          {renderedPreview ? (
            renderedPreview.trim().startsWith("<") ? (
              <div dangerouslySetInnerHTML={{ __html: renderedPreview }} />
            ) : (
              <div className="prose prose-sm max-w-none text-[hsl(var(--text-primary))] [&_h1]:text-lg [&_h2]:text-base [&_h3]:text-sm [&_h1]:font-semibold [&_h2]:font-semibold [&_h3]:font-medium [&_h1]:mt-4 [&_h2]:mt-3 [&_h3]:mt-2 [&_p]:my-1 [&_ul]:my-1 [&_ol]:my-1 [&_li]:my-0.5 [&_code]:text-xs [&_code]:bg-[hsl(var(--bg-hover))] [&_code]:px-1 [&_code]:rounded [&_pre]:bg-[hsl(var(--bg-card))] [&_pre]:p-3 [&_pre]:rounded-md [&_pre]:overflow-auto [&_pre]:max-h-60 [&_pre]:text-xs [&_table]:w-full [&_table]:text-xs [&_th]:text-left [&_th]:px-2 [&_th]:py-1 [&_th]:bg-[hsl(var(--bg-hover))] [&_td]:px-2 [&_td]:py-1 [&_td]:border-b [&_td]:border-[hsl(var(--border-light))]]">
                <ReactMarkdown remarkPlugins={[remarkGfm]}>
                  {renderedPreview}
                </ReactMarkdown>
              </div>
            )
          ) : (
            <p className="text-sm text-[hsl(var(--text-tertiary))]">(空)</p>
          )}
        </div>
      </Modal>

      {/* Report Template Delete Confirm */}
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
// WYSIWYG Preview Block
// ============================================================

function chunked<T>(arr: T[], n: number): T[][] {
  const result: T[][] = [];
  for (let i = 0; i < arr.length; i += n) result.push(arr.slice(i, i + n));
  return result;
}

function WysiwygBlock({ section, format: _format }: { section: TemplateSection; format: string }) {
  const meta = SECTION_META[section.type] || { description: "", icon: "📄" };
  const fieldLabel = (f: string) => {
    const m: Record<string, string> = {
      device_name: "设备名称", device_ip: "IP 地址", vendor: "厂商", model: "型号",
      sn: "序列号", hostname: "主机名", os_release: "操作系统", kernel: "内核",
      cpu_cores: "CPU 核心数", mem_total: "内存总量", manufacturing_date: "生产日期",
    };
    return m[f] || f;
  };

  return (
    <div className="rounded-md border border-[hsl(var(--border-light))] overflow-hidden text-[11px] bg-white shadow-sm">
      {/* Section header bar */}
      <div className="px-2.5 py-1.5 bg-[hsl(var(--bg-hover))] border-b border-[hsl(var(--border-light))] flex items-center gap-1.5">
        <span className="text-xs">{meta.icon}</span>
        <span className="text-[11px] font-medium text-[hsl(var(--text-primary))]">{section.label}</span>
      </div>

      {/* Preview body */}
      <div className="px-3 py-2 space-y-1.5">
        {section.type === "title" && (
          <div className="text-center py-1">
            <div className="font-bold text-sm text-[hsl(var(--text-primary))]">示例设备 巡检报告</div>
            <div className="text-[10px] text-[hsl(var(--text-tertiary))] mt-0.5">生成时间: 2026-05-31 14:30:00</div>
          </div>
        )}

        {section.type === "basic_info" && (
          <div>
            <div className="font-medium text-[hsl(var(--text-primary))] text-xs mb-1">{section.label}</div>
            <table className="w-full text-[10px] border-collapse border border-[hsl(var(--border-light))]">
              <tbody>
                {chunked(((section.config.fields as string[]) || []), 2).map((pair, ri) => (
                  <tr key={ri} className="border-b border-[hsl(var(--border-light))] last:border-0">
                    {pair.map((f, ci) => (
                      <React.Fragment key={f}>
                        <td className="py-1 px-2 font-medium text-[hsl(var(--text-secondary))] bg-[hsl(var(--bg-hover))] w-[25%] border-r border-[hsl(var(--border-light))]">
                          {fieldLabel(f)}
                        </td>
                        <td className={`py-1 px-2 text-[hsl(var(--text-tertiary))] ${ci < pair.length - 1 ? "border-r border-[hsl(var(--border-light))]" : ""} w-[25%]`}>
                          {`{{${f}}}`}
                        </td>
                      </React.Fragment>
                    ))}
                    {pair.length === 1 && (
                      <>
                        <td className="py-1 px-2 w-[25%]"></td>
                        <td className="py-1 px-2 w-[25%]"></td>
                      </>
                    )}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {section.type === "inspection_results" && (
          <div>
            <div className="font-medium text-[hsl(var(--text-primary))] text-xs mb-1">{section.label}</div>
            <table className="w-full text-[10px] border-collapse border border-[hsl(var(--border-light))]">
              <thead>
                <tr className="bg-[hsl(var(--bg-hover))]">
                  <th className="py-1 px-1.5 text-left font-medium text-[hsl(var(--text-secondary))] border-b border-r border-[hsl(var(--border-light))] w-[30px]">序号</th>
                  <th className="py-1 px-1.5 text-left font-medium text-[hsl(var(--text-secondary))] border-b border-r border-[hsl(var(--border-light))]">巡检项目</th>
                  <th className="py-1 px-1.5 text-left font-medium text-[hsl(var(--text-secondary))] border-b border-r border-[hsl(var(--border-light))]">巡检结果</th>
                  <th className="py-1 px-1.5 text-left font-medium text-[hsl(var(--text-secondary))] border-b border-[hsl(var(--border-light))]">评判结论</th>
                </tr>
              </thead>
              <tbody>
                {[
                  { cmd: "display version", status: "正常", finding: "软件版本为推荐版本", suggestion: "" },
                  { cmd: "display cpu-usage", status: "警告", finding: "CPU 使用率偏高 (78%)", suggestion: "建议关注 CPU 负载趋势" },
                  { cmd: "display memory-usage", status: "正常", finding: "内存使用率 45%", suggestion: "" },
                ].map((row, ri) => (
                  <tr key={ri} className="border-b border-[hsl(var(--border-light))] last:border-0">
                    <td className="py-1 px-1.5 text-center text-[hsl(var(--text-tertiary))] border-r border-[hsl(var(--border-light))]">{ri + 1}</td>
                    <td className="py-1 px-1.5 text-[hsl(var(--text-primary))] border-r border-[hsl(var(--border-light))] font-medium">{row.cmd}</td>
                    <td className="py-1 px-1.5 text-[hsl(var(--text-secondary))] border-r border-[hsl(var(--border-light))] text-[10px]">{row.finding}</td>
                    <td className="py-1 px-1.5 text-[hsl(var(--text-secondary))] text-[10px]">
                      <span className={row.status === "正常" ? "text-[hsl(var(--success))]" : row.status === "警告" ? "text-[hsl(var(--warning))]" : "text-[hsl(var(--danger))]"}>
                        {row.status}
                      </span>
                      {row.suggestion && <span className="text-[hsl(var(--text-tertiary))]"> — {row.suggestion}</span>}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <div className="text-[10px] text-[hsl(var(--text-tertiary))] text-center mt-1">... 共 N 条</div>
          </div>
        )}

        {section.type === "ai_analysis" && (
          <div>
            <div className="font-medium text-[hsl(var(--text-primary))] text-xs mb-1">{section.label}</div>
            <p className="text-[hsl(var(--text-secondary))] text-[10px] leading-relaxed">
              设备整体运行状态良好。CPU 使用率偏高需要关注，建议监控趋势并在必要时进行扩容评估。内存和软件版本均处于正常范围。
            </p>
          </div>
        )}

        {section.type === "overall_assessment" && (
          <div>
            <div className="font-medium text-[hsl(var(--text-primary))] text-xs mb-1">{section.label}</div>
            <div className="text-[10px] space-y-0.5 text-[hsl(var(--text-secondary))]">
              <div className="flex gap-2"><span className="font-medium">综合判断：</span><span className="text-[hsl(var(--warning))] font-medium">⚠ 警告</span></div>
              <div className="flex gap-2"><span className="font-medium">建议：</span><span>关注 CPU 负载趋势，排查链路异常日志</span></div>
            </div>
          </div>
        )}
      </div>
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
