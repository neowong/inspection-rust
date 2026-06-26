import React, { useState, useEffect, useMemo, useRef } from "react";
import { useSearchParams } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { ChevronRight, ChevronDown, Pencil, Trash2, Copy, Star, GripVertical, Lock, Plus, X } from "lucide-react";
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
  description: string;
  commands: TemplateCommandConfig[];
  report_template_id: number | null;
}

const getEmptyTemplateForm = (): TemplateForm => ({
  name: "", vendor: "H3C", model: "", description: "", commands: [], report_template_id: null,
});

interface CommandForm {
  vendor: string;
  command: string;
  description: string;
  category: string;
  expectation: string;
}

const getEmptyCommandForm = (): CommandForm => ({
  vendor: "H3C", command: "", description: "", category: "general", expectation: "",
});

type RptCategory = "network" | "linux" | "database";

interface ReportForm {
  name: string;
  category: RptCategory;
  description: string;
  config: ReportTemplateConfig;
}

const EMPTY_REPORT_FORM = (): ReportForm => ({
  name: "", category: "network", description: "",
  config: JSON.parse(JSON.stringify(DEFAULT_REPORT_CONFIG)),
});

// ============================================================
// TemplatesPage
// ============================================================

export default function TemplatesPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const initialTab = (searchParams.get("tab") as TabKey) || "templates";
  const [tab, setTab] = useState<TabKey>(initialTab);

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
  const [templateCommands, setTemplateCommands] = useState<CommandPool[]>([]);
  const [cmdSearch, setCmdSearch] = useState("");
  const [cmdVendor, setCmdVendor] = useState("");
  const [cmdModal, setCmdModal] = useState(false);
  const [editingCmd, setEditingCmd] = useState<CommandPool | null>(null);
  const [cmdForm, setCmdForm] = useState<CommandForm>(getEmptyCommandForm());
  const [confirmDeleteCmd, setConfirmDeleteCmd] = useState<number | null>(null);
  const [cmdSaving, setCmdSaving] = useState(false);
  const [cmdSaveError, setCmdSaveError] = useState<string | null>(null);

  // Dynamic vendor list: defaults + custom vendors from DB
  const [allVendors, setAllVendors] = useState<string[]>([...VENDORS] as string[]);
  // Custom vendor input toggles
  const [cmdVendorCustom, setCmdVendorCustom] = useState(false);
  const [customVendorInput, setCustomVendorInput] = useState("");
  const [tplVendorCustom, setTplVendorCustom] = useState(false);
  const [tplCustomVendorInput, setTplCustomVendorInput] = useState("");

  // Extract unique vendors from commands, custom ones sorted to top
  useEffect(() => {
    const customVendors = [...new Set(commands.map(c => c.vendor))]
      .filter(v => !(VENDORS as readonly string[]).includes(v))
      .sort();
    if (customVendors.length > 0) {
      setAllVendors([...customVendors, ...(VENDORS as unknown as string[])]);
    }
  }, [commands]);

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

  // Load commands for template editor independently (not affected by command pool tab vendor filter)
  useEffect(() => {
    if (templateModal && templateForm.vendor) {
      invoke<CommandPool[]>("list_commands", { vendor: templateForm.vendor })
        .then(setTemplateCommands)
        .catch(console.error);
    }
  }, [templateModal, templateForm.vendor]);

  const filteredTemplates = useMemo(() => templates.filter((t) =>
    !templateSearch || t.name.toLowerCase().includes(templateSearch.toLowerCase())
  ), [templates, templateSearch]);

  const filteredCommands = useMemo(() => commands.filter((c) =>
    !cmdSearch || c.command.toLowerCase().includes(cmdSearch.toLowerCase()) || (c.description && c.description.toLowerCase().includes(cmdSearch.toLowerCase()))
  ), [commands, cmdSearch]);

  const vendorFilteredCommands = useMemo(() => templateCommands.filter((c) =>
    c.vendor === templateForm.vendor
  ), [templateCommands, templateForm.vendor]);

  // ----- Template handlers -----
  const openAddTemplate = () => {
    setEditingTemplate(null);
    setTemplateForm(getEmptyTemplateForm());
    setTplVendorCustom(false);
    setTplCustomVendorInput("");
    setTemplateModal(true);
  };
  const openEditTemplate = (t: InspectionTemplate) => {
    const isCustom = !(VENDORS as readonly string[]).includes(t.vendor);
    setEditingTemplate(t);
    setTemplateForm({
      name: t.name, vendor: t.vendor, model: t.model || "",
      description: t.description || "",
      commands: t.config?.commands || [],
      report_template_id: t.report_template_id ?? null,
    });
    setTplVendorCustom(isCustom);
    setTplCustomVendorInput(isCustom ? t.vendor : "");
    setTemplateModal(true);
  };
  const duplicateTemplate = (t: InspectionTemplate) => {
    const isCustom = !(VENDORS as readonly string[]).includes(t.vendor);
    setEditingTemplate(null);   // 走创建流程
    setTemplateForm({
      name: `${t.name} (副本)`,
      vendor: t.vendor,
      model: t.model || "",
      description: t.description || "",
      commands: t.config?.commands || [],
      report_template_id: t.report_template_id ?? null,
    });
    setTplVendorCustom(isCustom);
    setTplCustomVendorInput(isCustom ? t.vendor : "");
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
  const openAddCmd = () => {
    setEditingCmd(null); setCmdForm(getEmptyCommandForm()); setCmdSaveError(null);
    setCmdVendorCustom(false); setCustomVendorInput("");
    setCmdModal(true);
  };
  const openEditCmd = (c: CommandPool) => {
    const isCustom = !(VENDORS as readonly string[]).includes(c.vendor);
    setEditingCmd(c); setCmdSaveError(null);
    setCmdForm({ vendor: c.vendor, command: c.command, description: c.description || "", category: c.category || "general", expectation: c.expectation || "" });
    setCmdVendorCustom(isCustom);
    setCustomVendorInput(isCustom ? c.vendor : "");
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
      category: vendorCategory(rt.vendor || ""),
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
      vendor: null,
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
              onClick={() => { setTab(t.key); setSearchParams({ tab: t.key }, { replace: true }); }}
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
            <Select size="sm" className="w-28" value={templateVendor} onChange={(e) => setTemplateVendor(e.target.value)}>
              <option value="">全部厂商</option>
              {allVendors.map((v) => <option key={v} value={v}>{v}</option>)}
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
              { key: "description", header: "描述", wrap: true, render: (r) => r.description || "-" },
              { key: "updated_at", header: "更新时间", render: (r) => new Date(r.updated_at).toLocaleString("zh-CN") },
              { key: "actions", header: "操作", width: "210px", noTruncate: true, render: (r) => (
                <div className="flex gap-1" onClick={(e) => e.stopPropagation()}>
                  <Button size="sm" variant="ghost" onClick={() => openEditTemplate(r)}>编辑</Button>
                  <Button size="sm" variant="ghost" onClick={() => duplicateTemplate(r)}>复制</Button>
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
            {["全部", ...allVendors].map((v) => (
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
              { key: "description", header: "描述", wrap: true, render: (r) => r.description || "-" },
              { key: "updated_at", header: "更新时间", render: (r) => new Date(r.updated_at).toLocaleString("zh-CN") },
              { key: "actions", header: "操作", width: "200px", noTruncate: true, render: (r) => (
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
          width="max-w-4xl"
          onClose={() => setTemplateModal(false)}
          footer={
            <div className="flex gap-2">
              <Button variant="secondary" onClick={() => setTemplateModal(false)}>取消</Button>
              <Button onClick={handleSaveTemplate} loading={saving}>{editingTemplate ? "保存" : "添加"}</Button>
            </div>
          }
        >
          <div className="space-y-3">
            <div className="space-y-2">
                <div className="grid grid-cols-3 gap-2">
                  <div>
                    <label className="block text-[11px] font-medium text-[hsl(var(--text-secondary))] mb-0.5">名称</label>
                    <Input value={templateForm.name} className={shakeFields.has("template_name") ? "animate-shake" : ""}
                      onChange={(e) => { setTemplateForm({ ...templateForm, name: e.target.value }); setSaveError(null); }} />
                    {saveError && <p className="mt-0.5 text-[11px] text-[hsl(var(--danger))]">{saveError}</p>}
                  </div>
                  <div>
                    <label className="block text-[11px] font-medium text-[hsl(var(--text-secondary))] mb-0.5">厂商</label>
                    <div className="flex gap-1">
                      <Select value={templateForm.vendor}
                        onChange={(e) => {
                          if (e.target.value === "__add__") return;
                          setTplVendorCustom(false);
                          setTemplateForm({ ...templateForm, vendor: e.target.value, commands: [] });
                        }}
                        className="flex-1">
                        {allVendors.map((v) => <option key={v} value={v}>{v}</option>)}
                        <option value="__add__" disabled style={{fontStyle:"italic",color:"hsl(var(--text-tertiary))"}}>── 已有厂商 ──</option>
                      </Select>
                      <button type="button" onClick={() => setTplVendorCustom(true)}
                        className="shrink-0 h-8 w-8 flex items-center justify-center rounded-md border border-[hsl(var(--border))] text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--accent))] hover:border-[hsl(var(--accent))] transition-colors"
                        title="新增自定义厂商">
                        <Plus size={15} />
                      </button>
                    </div>
                    {tplVendorCustom && (
                      <div className="flex items-center gap-1 mt-1">
                        <Input className="flex-1" placeholder="输入自定义厂商名称" value={tplCustomVendorInput}
                          onChange={(e) => setTplCustomVendorInput(e.target.value)}
                          onKeyDown={(e) => { if (e.key === "Enter" && tplCustomVendorInput.trim()) { setTemplateForm({ ...templateForm, vendor: tplCustomVendorInput.trim(), commands: [] }); setTplVendorCustom(false); }}} />
                        <Button size="sm"
                          onClick={() => { if (tplCustomVendorInput.trim()) { setTemplateForm({ ...templateForm, vendor: tplCustomVendorInput.trim(), commands: [] }); setTplVendorCustom(false); }}}>确定</Button>
                        <button type="button" onClick={() => setTplVendorCustom(false)}
                          className="h-7 w-7 flex items-center justify-center text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-primary))]">
                          <X size={14} />
                        </button>
                      </div>
                    )}
                  </div>
                  <div>
                    <label className="block text-[11px] font-medium text-[hsl(var(--text-secondary))] mb-0.5">型号</label>
                    <Input value={templateForm.model} onChange={(e) => setTemplateForm({ ...templateForm, model: e.target.value })} placeholder="可选" />
                  </div>
                </div>
                <div className="grid grid-cols-2 gap-2">
                  <div>
                    <label className="block text-[11px] font-medium text-[hsl(var(--text-secondary))] mb-0.5">描述</label>
                    <Input value={templateForm.description} onChange={(e) => setTemplateForm({ ...templateForm, description: e.target.value })} placeholder="可选" />
                  </div>
                  <div>
                    <label className="block text-[11px] font-medium text-[hsl(var(--text-secondary))] mb-0.5">报告模板</label>
                    <Select
                      value={templateForm.report_template_id ?? ""}
                      onChange={(e) => setTemplateForm({ ...templateForm, report_template_id: e.target.value ? Number(e.target.value) : null })}
                    >
                      <option value="">跟随默认</option>
                      {reportTemplates.map((rt) => (
                        <option key={rt.id} value={rt.id}>{rt.name}{rt.is_default ? " (默认)" : ""}{rt.vendor ? ` · ${rt.vendor}` : ""}</option>
                      ))}
                    </Select>
                  </div>
                </div>
              </div>

            {/* 命令选择 — 左右分栏，始终同时可见 */}
            <div className="grid grid-cols-2 gap-3" style={{ height: "390px" }}>
              {/* 左：已选命令 */}
              <div className="flex flex-col min-h-0">
                <label className="shrink-0 text-xs font-medium text-[hsl(var(--text-secondary))] mb-2">
                  已选命令 ({templateForm.commands.length})
                </label>
                <div ref={cmdListRef} className="flex-1 min-h-0 overflow-y-auto border border-[hsl(var(--border))] rounded-md p-2 space-y-1"
                  onDragOver={(e) => { e.preventDefault(); handleDragAutoScroll(e, cmdListRef.current); }}
                  onDrop={stopAutoScroll}
                  onDragEnd={stopAutoScroll}
                >
                  {templateForm.commands.length === 0 && (
                    <p className="text-xs text-[hsl(var(--text-tertiary))] text-center mt-16">从右侧勾选命令添加到此处</p>
                  )}
                  {templateForm.commands.map((spec, idx) => {
                    const cmd = templateCommands.find(c => c.id === spec.command_id);
                    if (!cmd) return null;
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
                        <div className="flex items-center gap-1.5">
                          <GripVertical size={12} className="text-[hsl(var(--text-tertiary))] shrink-0" />
                          <span className="text-[11px] text-[hsl(var(--text-tertiary))] w-4 text-right shrink-0">{idx + 1}</span>
                          <code className="text-xs bg-[hsl(var(--bg-hover))] px-1 rounded truncate">{cmd.command}</code>
                          <button type="button"
                            onClick={() => setTemplateForm({ ...templateForm, commands: templateForm.commands.filter(c => c.command_id !== spec.command_id) })}
                            className="ml-auto shrink-0 text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--danger))] text-xs leading-none">×</button>
                        </div>
                      </div>
                    );
                  })}
                </div>
                <p className="shrink-0 mt-1 text-[10px] text-[hsl(var(--text-tertiary))] text-right">列表顺序即为报告中的展示顺序</p>
              </div>

              {/* 右：可选命令（按类别分组） */}
              <div className="flex flex-col min-h-0">
                <label className="shrink-0 text-xs font-medium text-[hsl(var(--text-secondary))] mb-2">
                  可选命令
                </label>
                <div className="flex-1 min-h-0 overflow-y-auto border border-[hsl(var(--border))] rounded-md">
                  {vendorFilteredCommands.length === 0 && (
                    <p className="text-xs text-[hsl(var(--text-tertiary))] text-center mt-16">暂无 {templateForm.vendor} 命令，请先在命令库中添加</p>
                  )}
                  <AvailableCommands
                    commands={vendorFilteredCommands.filter(cmd => !templateForm.commands.some(c => c.command_id === cmd.id))}
                    onAdd={(cmd) => setTemplateForm({
                      ...templateForm,
                      commands: [...templateForm.commands, { command_id: cmd.id }]
                    })}
                  />
                </div>
              </div>
            </div>
          </div></Modal>
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
              <div className="flex gap-1">
                <Select value={cmdForm.vendor}
                  onChange={(e) => { if (e.target.value === "__add__") return; setCmdVendorCustom(false); setCmdForm({ ...cmdForm, vendor: e.target.value }); }}
                  className="flex-1">
                  {allVendors.map((v) => <option key={v} value={v}>{v}</option>)}
                </Select>
                <button type="button" onClick={() => setCmdVendorCustom(true)}
                  className="shrink-0 h-8 w-8 flex items-center justify-center rounded-md border border-[hsl(var(--border))] text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--accent))] hover:border-[hsl(var(--accent))] transition-colors"
                  title="新增自定义厂商">
                  <Plus size={15} />
                </button>
              </div>
              {cmdVendorCustom && (
                <div className="flex items-center gap-1 mt-1">
                  <Input className="flex-1" placeholder="输入自定义厂商名称" value={customVendorInput}
                    onChange={(e) => setCustomVendorInput(e.target.value)}
                    onKeyDown={(e) => { if (e.key === "Enter" && customVendorInput.trim()) { setCmdForm({ ...cmdForm, vendor: customVendorInput.trim() }); setCmdVendorCustom(false); }}} />
                  <Button size="sm"
                    onClick={() => { if (customVendorInput.trim()) { setCmdForm({ ...cmdForm, vendor: customVendorInput.trim() }); setCmdVendorCustom(false); }}}>确定</Button>
                  <button type="button" onClick={() => setCmdVendorCustom(false)}
                    className="h-7 w-7 flex items-center justify-center text-[hsl(var(--text-tertiary))] hover:text-[hsl(var(--text-primary))]">
                    <X size={14} />
                  </button>
                </div>
              )}
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
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">
                AI 评判提示词
                <span className="ml-1 text-[hsl(var(--text-tertiary))] font-normal">（可选）</span>
              </label>
              <textarea
                value={cmdForm.expectation}
                onChange={(e) => setCmdForm({ ...cmdForm, expectation: e.target.value })}
                placeholder="描述此命令的预期输出或正常阈值，供 AI 评判时参考。&#10;例如：CPU 利用率应低于 80%；各分区磁盘使用率应低于 90%"
                rows={3}
                className="w-full px-3 py-2 text-sm rounded-lg border border-[hsl(var(--border))] bg-[hsl(var(--bg-card))] text-[hsl(var(--text-primary))] placeholder:text-[hsl(var(--text-tertiary))] outline-none transition-colors duration-150 focus:border-[hsl(var(--accent))] focus:ring-2 focus:ring-[hsl(var(--accent)/0.2)] resize-none"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-[hsl(var(--text-secondary))] mb-1">分类</label>
              <Select value={cmdForm.category} onChange={(e) => setCmdForm({ ...cmdForm, category: e.target.value })}>
                {CATEGORIES.map((c) => <option key={c} value={c}>{CATEGORY_LABELS[c] || c}</option>)}
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

const SAMPLE_DEVICE: Record<string, { name: string; ip: string; vendor: string; model: string; sn: string; mfg_date: string; inspect_time: string; sysname: string; os_release: string; cpu_cores: string; memory_gb: string }> = {
  "H3C": {
    name: "SW-CORE-01", ip: "192.168.1.1", vendor: "H3C",
    model: "S6850-56HF", sn: "210235A1A1234567", mfg_date: "2024-08-12",
    inspect_time: "2026-06-13 09:30:00", sysname: "aHope-Core",
    os_release: "", cpu_cores: "", memory_gb: "",
  },
  "华为": {
    name: "HW-CORE-01", ip: "10.10.1.1", vendor: "华为",
    model: "CE12808", sn: "2102114182P0LB000001", mfg_date: "2024-12-20",
    inspect_time: "2026-06-13 14:00:00", sysname: "HW-Core",
    os_release: "", cpu_cores: "", memory_gb: "",
  },
  "思科": {
    name: "CISCO-CORE-01", ip: "172.16.1.1", vendor: "思科",
    model: "C9500-48Y4C", sn: "FCW2345A1BC", mfg_date: "2024-06-01",
    inspect_time: "2026-06-13 10:15:00", sysname: "Cisco-Core",
    os_release: "", cpu_cores: "", memory_gb: "",
  },
  "锐捷": {
    name: "RG-CORE-01", ip: "10.20.1.1", vendor: "锐捷",
    model: "RG-S7808C", sn: "G1R2345ABCDEF", mfg_date: "2024-03-15",
    inspect_time: "2026-06-13 11:00:00", sysname: "RG-Core",
    os_release: "", cpu_cores: "", memory_gb: "",
  },
  "飞塔": {
    name: "FG-EDGE-01", ip: "192.168.100.1", vendor: "飞塔",
    model: "FortiGate-80F", sn: "FGT80FTK23001234", mfg_date: "2025-01-10",
    inspect_time: "2026-06-13 08:45:00", sysname: "aHope-FW",
    os_release: "", cpu_cores: "", memory_gb: "",
  },
  "Linux": {
    name: "SRV-GENERIC", ip: "192.168.10.10", vendor: "Linux",
    model: "KVM 虚拟机", sn: "VMware-42 1a 2b 3c 4d", mfg_date: "—",
    inspect_time: "2026-06-13 09:00:00", sysname: "srv-generic",
    os_release: "Ubuntu 22.04.4 LTS", cpu_cores: "4", memory_gb: "8",
  },
  "Ubuntu": {
    name: "WEB-SRV-01", ip: "192.168.10.10", vendor: "Ubuntu",
    model: "KVM 虚拟机", sn: "VMware-42 1a 2b 3c 4d", mfg_date: "—",
    inspect_time: "2026-06-13 09:00:00", sysname: "web-srv-01",
    os_release: "Ubuntu 22.04.4 LTS", cpu_cores: "8", memory_gb: "16",
  },
  "CentOS": {
    name: "APP-SRV-01", ip: "172.16.50.40", vendor: "CentOS",
    model: "物理机 Dell R750", sn: "DEL-R750-XYZ789", mfg_date: "2024-10-15",
    inspect_time: "2026-06-13 10:00:00", sysname: "app-srv-01",
    os_release: "CentOS Linux release 7.9.2009 (Core)", cpu_cores: "16", memory_gb: "32",
  },
  "Rocky": {
    name: "K8S-NODE-01", ip: "10.30.1.10", vendor: "Rocky",
    model: "物理机 HP DL380", sn: "HP-DL380-ABC001", mfg_date: "2025-02-01",
    inspect_time: "2026-06-13 10:30:00", sysname: "k8s-node-01",
    os_release: "Rocky Linux 9.4 (Blue Onyx)", cpu_cores: "32", memory_gb: "128",
  },
  "Debian": {
    name: "DB-SRV-01", ip: "10.50.1.20", vendor: "Debian",
    model: "物理机 Supermicro", sn: "SM-SYS-001234", mfg_date: "2024-08-20",
    inspect_time: "2026-06-13 11:00:00", sysname: "db-srv-01",
    os_release: "Debian GNU/Linux 12 (bookworm)", cpu_cores: "24", memory_gb: "64",
  },
  "MySQL": {
    name: "DB-MASTER-01", ip: "192.168.50.10", vendor: "MySQL",
    model: "物理机 Dell R750", sn: "ABC123DEF456", mfg_date: "2025-03-01",
    inspect_time: "2026-06-13 16:00:00", sysname: "db-master-01",
    os_release: "Ubuntu 22.04.4 LTS", cpu_cores: "16", memory_gb: "64",
  },
  "PostgreSQL": {
    name: "DB-PG-01", ip: "192.168.50.20", vendor: "PostgreSQL",
    model: "物理机 Dell R650", sn: "XYZ789UVW012", mfg_date: "2025-06-15",
    inspect_time: "2026-06-13 16:30:00", sysname: "db-pg-01",
    os_release: "CentOS Stream 9", cpu_cores: "12", memory_gb: "48",
  },
  "Oracle": {
    name: "DB-ORA-01", ip: "192.168.50.30", vendor: "Oracle",
    model: "物理机 HP DL380", sn: "ORA123789ABC", mfg_date: "2024-11-20",
    inspect_time: "2026-06-13 17:00:00", sysname: "db-ora-01",
    os_release: "Oracle Linux 8.10", cpu_cores: "24", memory_gb: "128",
  },
};

const SAMPLE_ROWS: Record<string, { item: string; cmd: string; output: string; status: string; finding: string; suggestion: string }[]> = {
  "H3C": [
    { item: "查看设备版本", cmd: "display version",
      output: "H3C Comware Software, Version 7.1.075 R6628P12\nUptime: 60 days 4 hours",
      status: "ok", finding: "", suggestion: "" },
    { item: "查看 CPU 使用率", cmd: "display cpu-usage",
      output: "Slot 0 CPU 0 usage:\n  in last 5 seconds: 78%\n  in last 1 minute:  72%",
      status: "warning", finding: "CPU 使用率偏高 (78%)",
      suggestion: "建议持续关注 CPU 负载趋势" },
    { item: "查看接口状态", cmd: "display interface brief",
      output: "GE1/0/1   UP    1000Mbps  full\nGE1/0/2   DOWN  --        --",
      status: "info", finding: "GE1/0/2 处于 DOWN 状态", suggestion: "确认是否为预期未使用接口" },
  ],
  "华为": [
    { item: "查看设备版本", cmd: "display version",
      output: "Huawei Versatile Routing Platform Software\nVRP (R) software, Version 8.180\nUptime is 120 days, 3 hours, 15 minutes",
      status: "ok", finding: "", suggestion: "" },
    { item: "查看 CPU 使用率", cmd: "display cpu-usage",
      output: "CPU Usage Stat. Cycle: 60 (Second)\nCPU Usage            : 45% Max: 92%\nCPU Usage Stat. Time : 2026-06-13 13:55:00",
      status: "warning", finding: "CPU 历史峰值达 92%", suggestion: "检查峰值时段的任务调度" },
    { item: "查看接口状态", cmd: "display interface brief",
      output: "PHY: Physical\n*down: administratively down\n10GE1/0/1  up    up       10000Mbps  full\n10GE1/0/2  up    up       10000Mbps  full",
      status: "ok", finding: "", suggestion: "" },
  ],
  "思科": [
    { item: "查看设备版本", cmd: "show version",
      output: "Cisco IOS XE Software, Version 17.09.04a\nSystem image file is \"bootflash:packages.conf\"\nUptime: 45 days, 12 hours, 8 minutes",
      status: "ok", finding: "", suggestion: "" },
    { item: "查看 CPU 使用率", cmd: "show processes cpu",
      output: "CPU utilization for five seconds: 32%/0%; one minute: 28%; five minutes: 30%",
      status: "ok", finding: "", suggestion: "" },
    { item: "查看接口状态", cmd: "show ip interface brief",
      output: "Interface     IP-Address      Status  Protocol\nTe1/0/1       172.16.1.1      up      up\nTe1/0/2       unassigned      down    down",
      status: "info", finding: "Te1/0/2 未配置且已关闭", suggestion: "确认是否为备用端口" },
  ],
  "锐捷": [
    { item: "查看设备版本", cmd: "show version",
      output: "System description: Ruijie S7808C\nSystem start time: 2026-04-15 08:00:00\nSystem uptime: 59:3:45:12",
      status: "ok", finding: "", suggestion: "" },
    { item: "查看 CPU 使用率", cmd: "show cpu",
      output: "CPU using rate is 25%.\nCPU using rate in 5 secs is 28%.",
      status: "ok", finding: "", suggestion: "" },
    { item: "查看接口状态", cmd: "show interface status",
      output: "Interface     Status  Vlan  Duplex  Speed  Type\nTe1/0/1       up      100   Full    10G    10GBase-LR\nTe1/0/2       down    1    Auto    Auto   10GBase-LR",
      status: "info", finding: "Te1/0/2 链路 down", suggestion: "" },
  ],
  "飞塔": [
    { item: "查看系统状态", cmd: "get system status",
      output: "Version: FortiGate-80F v7.0.17,build0682,250113 (GA.M)\nSecurity Level: High\nFirmware Signature: certified\nVirus-DB: 1.00000(2018-04-09 18:07)\nExtended DB: 1.00000(2018-04-09 18:07)\nAV AI/ML Model: 0.00000(2001-01-01 00:00)",
      status: "ok", finding: "", suggestion: "" },
    { item: "查看 CPU 状态", cmd: "get system performance status",
      output: "CPU states: 8% user 4% system 0% nice 88% idle\nMemory states: 3954 MB total / 2101 MB used (53%)",
      status: "ok", finding: "", suggestion: "" },
    { item: "查看接口状态", cmd: "get system interface physical",
      output: "== [ port1 ]\n  mode: static\n  status: up  speed: 1000Mbps\n== [ port2 ]\n  status: down",
      status: "info", finding: "port2 未连接", suggestion: "确认是否需要启用" },
  ],
  "Linux": [
    { item: "系统信息", cmd: "uname -a",
      output: "Linux srv-generic 5.15.0-122-generic x86_64 GNU/Linux",
      status: "ok", finding: "", suggestion: "" },
    { item: "CPU 和内存", cmd: "lscpu && free -h",
      output: "CPU(s): 4\nMem: 7.6Gi used: 4.2Gi free: 3.4Gi",
      status: "ok", finding: "", suggestion: "" },
    { item: "磁盘使用", cmd: "df -h /",
      output: "Filesystem  Size  Used  Avail  Use%  Mounted on\n/dev/sda1   50G   35G    15G   70%  /",
      status: "warning", finding: "根分区使用率 70%", suggestion: "建议清理旧日志" },
    { item: "网络连接", cmd: "ss -tlnp",
      output: "LISTEN  0  128  0.0.0.0:22  0.0.0.0:*  (sshd)",
      status: "ok", finding: "", suggestion: "" },
  ],
  "Ubuntu": [
    { item: "系统信息", cmd: "uname -a",
      output: "Linux web-srv-01 5.15.0-122-generic x86_64 GNU/Linux",
      status: "ok", finding: "", suggestion: "" },
    { item: "CPU 和内存", cmd: "lscpu && free -h",
      output: "CPU(s): 8\nThread(s) per core: 2\nMem: 15Gi used: 8.2Gi free: 6.8Gi",
      status: "ok", finding: "", suggestion: "" },
    { item: "磁盘使用", cmd: "df -h /",
      output: "Filesystem  Size  Used  Avail  Use%  Mounted on\n/dev/sda1   100G   72G    28G   73%  /",
      status: "warning", finding: "根分区使用率 73%", suggestion: "建议清理旧日志或扩容存储" },
    { item: "网络连接", cmd: "ss -tlnp",
      output: "LISTEN  0  128  0.0.0.0:22    0.0.0.0:*  (sshd)\nLISTEN  0  511  0.0.0.0:443   0.0.0.0:*  (nginx)",
      status: "ok", finding: "", suggestion: "" },
  ],
  "CentOS": [
    { item: "系统信息", cmd: "uname -a",
      output: "Linux app-srv-01 3.10.0-1160.el7.x86_64 GNU/Linux",
      status: "ok", finding: "", suggestion: "" },
    { item: "发行版信息", cmd: "cat /etc/centos-release",
      output: "CentOS Linux release 7.9.2009 (Core)",
      status: "ok", finding: "", suggestion: "" },
    { item: "CPU 和内存", cmd: "lscpu && free -h",
      output: "CPU(s): 16\nMem: 31Gi used: 18Gi free: 13Gi",
      status: "ok", finding: "", suggestion: "" },
    { item: "磁盘使用", cmd: "df -h /",
      output: "Filesystem  Size  Used  Avail  Use%  Mounted on\n/dev/sda2   200G  165G   35G   83%  /",
      status: "warning", finding: "根分区使用率 83%", suggestion: "建议扩容或迁移日志" },
  ],
  "Rocky": [
    { item: "系统信息", cmd: "uname -a",
      output: "Linux k8s-node-01 5.14.0-427.13.1.el9_4.x86_64 GNU/Linux",
      status: "ok", finding: "", suggestion: "" },
    { item: "发行版信息", cmd: "cat /etc/rocky-release",
      output: "Rocky Linux release 9.4 (Blue Onyx)",
      status: "ok", finding: "", suggestion: "" },
    { item: "CPU 和内存", cmd: "lscpu && free -h",
      output: "CPU(s): 32\nMem: 125Gi used: 96Gi free: 29Gi",
      status: "ok", finding: "", suggestion: "" },
    { item: "磁盘使用", cmd: "df -h /",
      output: "Filesystem  Size  Used  Avail  Use%  Mounted on\n/dev/nvme0n1p2  500G  380G  120G  76%  /",
      status: "warning", finding: "根分区使用率 76%", suggestion: "" },
  ],
  "Debian": [
    { item: "系统信息", cmd: "uname -a",
      output: "Linux db-srv-01 6.1.0-21-amd64 x86_64 GNU/Linux",
      status: "ok", finding: "", suggestion: "" },
    { item: "发行版信息", cmd: "cat /etc/os-release",
      output: "PRETTY_NAME=\"Debian GNU/Linux 12 (bookworm)\"",
      status: "ok", finding: "", suggestion: "" },
    { item: "CPU 和内存", cmd: "lscpu && free -h",
      output: "CPU(s): 24\nMem: 62Gi used: 48Gi free: 14Gi",
      status: "ok", finding: "", suggestion: "" },
    { item: "磁盘使用", cmd: "df -h /",
      output: "Filesystem  Size  Used  Avail  Use%  Mounted on\n/dev/md0   1.8T  1.3T  500G   73%  /",
      status: "warning", finding: "RAID 阵列使用率 73%", suggestion: "建议扩容存储" },
  ],
  "MySQL": [
    { item: "数据库版本", cmd: "mysql --version",
      output: "mysql  Ver 8.0.36-0ubuntu0.22.04.1 for Linux on x86_64",
      status: "ok", finding: "", suggestion: "" },
    { item: "实例状态", cmd: "mysql -e 'SHOW STATUS' | head -20",
      output: "Uptime: 120 days 6 hours\\nThreads_connected: 12\\nQuestions: 89023456",
      status: "ok", finding: "", suggestion: "" },
    { item: "磁盘使用", cmd: "df -h /var/lib/mysql",
      output: "Filesystem  Size  Used  Avail  Use%  Mounted on\\n/dev/sdb1   500G  420G   80G   84%  /var/lib/mysql",
      status: "warning", finding: "数据库磁盘使用率 84%", suggestion: "建议扩容存储或清理历史数据" },
    { item: "宿主机 CPU 和内存", cmd: "lscpu && free -h",
      output: "CPU(s): 16\\nMem: 62Gi used: 48Gi free: 14Gi",
      status: "ok", finding: "", suggestion: "" },
  ],
  "PostgreSQL": [
    { item: "数据库版本", cmd: "psql --version",
      output: "psql (PostgreSQL) 16.3",
      status: "ok", finding: "", suggestion: "" },
    { item: "连接状态", cmd: "psql -c 'SELECT count(*) FROM pg_stat_activity'",
      output: " count\\n    45\\n(1 row)",
      status: "ok", finding: "", suggestion: "" },
    { item: "磁盘使用", cmd: "df -h /var/lib/postgresql",
      output: "Filesystem  Size  Used  Avail  Use%  Mounted on\\n/dev/sdc1   800G  650G  150G   82%  /var/lib/postgresql",
      status: "warning", finding: "数据目录使用率 82%", suggestion: "建议扩容" },
    { item: "宿主机 CPU 和内存", cmd: "lscpu && free -h",
      output: "CPU(s): 12\\nMem: 46Gi used: 32Gi free: 14Gi",
      status: "ok", finding: "", suggestion: "" },
  ],
  "Oracle": [
    { item: "数据库版本", cmd: "sqlplus -v",
      output: "SQL*Plus: Release 19.0.0.0.0 - Production\\nVersion 19.3.0.0.0",
      status: "ok", finding: "", suggestion: "" },
    { item: "实例状态", cmd: "echo 'SELECT INSTANCE_NAME, STATUS FROM V\\\\$INSTANCE;' | sqlplus -S / as sysdba",
      output: "INSTANCE_NAME    STATUS\\n---------------- ---------\\nORCLCDB          OPEN",
      status: "ok", finding: "", suggestion: "" },
    { item: "表空间使用", cmd: "ls -lh /oracle/oradata/ORCLCDB/",
      output: "total 15G\\n-rw-r----- 1 oracle dba 5.0G system01.dbf\\n-rw-r----- 1 oracle dba 10G undotbs01.dbf",
      status: "ok", finding: "", suggestion: "" },
    { item: "宿主机 CPU 和内存", cmd: "lscpu && free -h",
      output: "CPU(s): 24\\nMem: 125Gi used: 96Gi free: 29Gi",
      status: "ok", finding: "", suggestion: "" },
  ],
};

// ── 设备大类 → 字段集映射 ──
// 加新厂商只需改 vendorCategory()，字段集自动跟随
type DeviceCategory = "network" | "linux" | "database";

function vendorCategory(vendor: string): DeviceCategory {
  if (!vendor) return "network";
  const norm = vendor.toLowerCase();
  if (["h3c","华为","思科","锐捷","ruijie","cisco"].some(o => norm.includes(o.toLowerCase()))) return "network";
  if (["飞塔","forti"].some(o => norm.includes(o.toLowerCase()))) return "network";
  if (["linux","ubuntu","centos","rocky","debian","rhel","suse","fedora","alma"].some(o => norm.includes(o))) return "linux";
  if (["mysql","postgres","oracle","sql","达梦","mariadb","mssql","redis","mongo"].some(o => norm.includes(o))) return "database";
  return "network";
}

function categoryFields(cat: DeviceCategory): DeviceField[] {
  switch (cat) {
    case "linux":     return [...FIELD_COMMON, ...FIELD_SERVER];
    case "database":  return [...FIELD_COMMON, ...FIELD_DATABASE, ...FIELD_DB_HOST];
    default:          return [...FIELD_COMMON, ...FIELD_NETWORK];
  }
}


/** 将厂商名归一化到样本数据键 */
const FIELD_COMMON: DeviceField[] = [
  { key: "name", label: "设备名称", visible: true },
  { key: "ip", label: "管理地址", visible: true },
  { key: "vendor", label: "设备厂商", visible: true },
  { key: "inspect_time", label: "巡检时间", visible: true },
];
const FIELD_NETWORK: DeviceField[] = [
  { key: "model", label: "设备型号", visible: true },
  { key: "sn", label: "序列号", visible: true },
  { key: "mfg_date", label: "出厂日期", visible: true },
  { key: "sysname", label: "主机名", visible: true },
];
const FIELD_SERVER: DeviceField[] = [
  { key: "os_release", label: "发行版", visible: true },
  { key: "kernel_version", label: "内核版本", visible: true },
  { key: "cpu_cores", label: "CPU 核心", visible: true },
  { key: "memory_gb", label: "内存(GB)", visible: true },
  { key: "model", label: "设备型号", visible: false },
  { key: "sn", label: "序列号", visible: false },
  { key: "mfg_date", label: "出厂日期", visible: false },
  { key: "hostname", label: "主机名", visible: false },
];
const FIELD_DATABASE: DeviceField[] = [
  { key: "db_version", label: "数据库版本", visible: true },
  { key: "instance_name", label: "实例名", visible: true },
];
// 数据库宿主信息：含 OS 属性 + 物理机属性（数据库可能在物理机上）
const FIELD_DB_HOST: DeviceField[] = [
  { key: "os_release", label: "宿主机 OS", visible: true },
  { key: "cpu_cores", label: "宿主机 CPU 核心", visible: true },
  { key: "memory_gb", label: "宿主机 内存(GB)", visible: true },
  { key: "model", label: "宿主机 型号", visible: false },
  { key: "sn", label: "宿主机 序列号", visible: false },
  { key: "mfg_date", label: "宿主机 出厂日期", visible: false },
  { key: "sysname", label: "宿主机 主机名", visible: false },
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
              <label className="block text-[11px] font-medium text-[hsl(var(--text-secondary))] mb-1">设备类别</label>
              <Select value={form.category} onChange={(e) => {
                  const cat = e.target.value as RptCategory;
                  const defaults = categoryFields(cat);
                  const existingKeys = new Set(form.config.device_info.fields.map(f => f.key));
                  const merged = [
                    ...form.config.device_info.fields.filter(f => defaults.some(d => d.key === f.key)),
                    ...defaults.filter(d => !existingKeys.has(d.key)),
                  ];
                  update({ category: cat, config: { ...form.config, device_info: { ...form.config.device_info, fields: merged } } });
                }}>
                <option value="network">网络设备</option>
                <option value="linux">Linux 服务器</option>
                <option value="database">数据库</option>
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
          <p className="text-[12px] text-[hsl(var(--text-tertiary))] mb-3 bg-[hsl(var(--accent)/0.06)] px-3 py-2 rounded-md">
            封面和目录仅在<b>下载组合报告</b>（批次合并为一个文件）时出现，
            单设备报告和 ZIP 打包的逐设备报告<b>不会</b>包含封面和目录。
          </p>
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
            <Field label="包含目录">
              <label className="flex items-center gap-2 cursor-pointer">
                <input type="checkbox"
                  checked={form.config.cover.include_toc ?? false}
                  onChange={(e) => updateConfig({ cover: { ...form.config.cover, include_toc: e.target.checked } })} />
                <span className="text-[12px] text-[hsl(var(--text-secondary))]">
                  组合报告中插入目录页（需在 WPS 中手动更新域）
                </span>
              </label>
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
                {/* 添加字段：仅列出当前厂商可用且未加入的字段 */}
                {(() => {
                  const available = categoryFields(form.category);
                  const existingKeys = new Set(form.config.device_info.fields.map(f => f.key));
                  const missing = available.filter(f => !existingKeys.has(f.key));
                  if (missing.length === 0) return null;
                  return (
                    <div className="flex items-center gap-1.5 flex-wrap mt-1">
                      <span className="text-[10px] text-[hsl(var(--text-tertiary))]">添加：</span>
                      {missing.map(f => (
                        <button key={f.key} type="button"
                          onClick={() => {
                            const fields = [...form.config.device_info.fields, { ...f }];
                            updateConfig({ device_info: { ...form.config.device_info, fields } });
                          }}
                          className="text-[10px] px-1.5 py-0.5 rounded border border-dashed border-[hsl(var(--border))] hover:border-[hsl(var(--accent))] hover:text-[hsl(var(--accent))] transition-colors"
                        >+ {f.label}</button>
                      ))}
                    </div>
                  );
                })()}
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
            <DocxPreview config={form.config} category={form.category} />
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

type SampleDevice = { name: string; ip: string; vendor: string; model: string; sn: string; mfg_date: string; inspect_time: string; sysname: string; os_release: string; cpu_cores: string; memory_gb: string };
type SampleRow = { item: string; cmd: string; output: string; status: string; finding: string; suggestion: string };

function applyVars(s: string, device: SampleDevice): string {
  return s
    .replace(/\{\{vendor\}\}/g, device.vendor)
    .replace(/\{\{device_name\}\}/g, device.name)
    .replace(/\{\{page\}\}/g, "1")
    .replace(/\{\{total\}\}/g, "3");
}

function DocxPreview({ config, category }: { config: ReportTemplateConfig; category: RptCategory }) {
  const sk = category === "linux" ? "Linux" : category === "database" ? "MySQL" : "H3C";
  const dev = (SAMPLE_DEVICE[sk] || SAMPLE_DEVICE["H3C"]) as SampleDevice;
  const rows = (SAMPLE_ROWS[sk] || SAMPLE_ROWS["H3C"]) as SampleRow[];
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
      case "sysname": return dev.sysname;
      case "hostname": return dev.sysname;
      case "os_release": return dev.os_release;
      case "cpu_cores": return dev.cpu_cores;
      case "memory_gb": return `${dev.memory_gb} GB`;
      case "db_version": return "MySQL 8.0.36";
      case "instance_name": return "prod-db-01";
      case "kernel_version": return "5.15.0-122-generic";
      default: return "";
    }
  };

  // CLI 提示符：真实 sysname 来自设备配置；预览中用 aHope 模拟，不使用设备名称
  const promptOf = (): string => {
    const sysname = "aHope";
    if (category === "linux" || category === "database") return `[root@${sysname} ~]# `;
    if (category === "network") return `<${sysname}>`;
    return `<${sysname}>`;
  };

  const cellFor = (col: TableColumn, row: SampleRow, idx: number): React.ReactNode => {
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

  const problems = rows.filter((r) => r.status === "warning" || r.status === "critical");

  const containerRef = React.useRef<HTMLDivElement>(null);
  const [scale, setScale] = React.useState(1);
  React.useEffect(() => {
    const update = () => {
      if (containerRef.current) {
        const w = containerRef.current.clientWidth;
        setScale(Math.min(1, w / 794));
      }
    };
    update();
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);

  return (
    <div ref={containerRef} style={{ paddingBottom: 24 }}>
      <div style={{
        transformOrigin: "top center",
        transform: `scale(${scale})`,
        margin: "0 auto",
        display: "flex", flexDirection: "column", gap: 24, alignItems: "center",
      }}>
      {/* ──── 第 1 页：封面 ──── */}
      <div style={{
        width: "210mm", minHeight: "297mm", background: "white",
        boxShadow: "0 2px 12px rgba(0,0,0,0.08)", padding: "20mm 18mm",
        boxSizing: "border-box", color: "#222",
        fontFamily: '"FangSong", "STFangsong", "仿宋", serif', fontSize: 11,
        display: "flex", flexDirection: "column",
        alignItems: "center", textAlign: "center" as const,
      }}>
        <div style={{ fontSize: 11, color: "#999", marginBottom: 60 }}>封面（仅组合报告输出）</div>
        <div style={{ fontSize: 32, fontWeight: 700, color: accent, letterSpacing: 2 }}>{title}</div>
        <div style={{ width: 80, height: 3, background: accent, margin: "28px 0" }} />
        {config.cover.subtitle && (
          <div style={{ fontSize: 15, color: "#555", letterSpacing: 1 }}>{config.cover.subtitle}</div>
        )}
        <div style={{ flex: 1 }} />
        <div style={{ fontSize: 13, color: "#888" }}>
          {new Date().toLocaleDateString("zh-CN", { year: "numeric", month: "long", day: "numeric" })}
        </div>
      </div>

      {/* ──── 第 2 页：设备报告 ──── */}
      <div style={{
        width: "210mm", minHeight: "297mm", background: "white",
        boxShadow: "0 2px 12px rgba(0,0,0,0.08)", padding: "20mm 18mm",
        boxSizing: "border-box", color: "#222",
        fontFamily: '"FangSong", "STFangsong", "仿宋", serif', fontSize: 11,
        margin: "0 auto",
      }}>
      {/* 页眉 */}
      {headerText.trim() && (
        <div style={{ textAlign: "center", fontSize: 10, color: "#666", borderBottom: "1px solid #ddd", paddingBottom: 4, marginBottom: 12 }}>
          {headerText.replace(/\{\{[^}]+\}\}/g, "")}
        </div>
      )}

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
              {rows.map((row, i) => (
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
  version: "版本信息", clock: "系统时钟", performance: "性能",
  hardware: "硬件信息", storage: "存储", env: "运行环境",
  interface: "接口", log: "日志",
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

  const catGroup = (cat: string) => {
    if (cat === "fan" || cat === "power") return "hardware";
    if (cat === "cpu" || cat === "memory") return "performance";
    if (cat === "vlan") return "interface";
    return cat;
  };

  const grouped = useMemo(() => {
    const map = new Map<string, CommandPool[]>();
    for (const cmd of commands) {
      const c = catGroup(cmd.category || "general");
      if (!map.has(c)) map.set(c, []);
      map.get(c)!.push(cmd);
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

/** 可选命令面板——按类别分组折叠，点击添加 */
function AvailableCommands({
  commands, onAdd,
}: {
  commands: CommandPool[];
  onAdd: (cmd: CommandPool) => void;
}) {
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const toggle = (cat: string) => setCollapsed((prev) => {
    const next = new Set(prev);
    if (next.has(cat)) next.delete(cat); else next.add(cat);
    return next;
  });

  const catGroup = (cat: string) => {
    if (cat === "fan" || cat === "power") return "hardware";
    if (cat === "cpu" || cat === "memory") return "performance";
    if (cat === "vlan") return "interface";
    return cat;
  };

  const grouped = useMemo(() => {
    const map = new Map<string, CommandPool[]>();
    for (const cmd of commands) {
      const c = catGroup(cmd.category || "general");
      if (!map.has(c)) map.set(c, []);
      map.get(c)!.push(cmd);
    }
    return [...map.entries()].sort(([a], [b]) => {
      const ia = CATEGORIES.indexOf(a as typeof CATEGORIES[number]);
      const ib = CATEGORIES.indexOf(b as typeof CATEGORIES[number]);
      return (ia === -1 ? 99 : ia) - (ib === -1 ? 99 : ib);
    });
  }, [commands]);

  if (commands.length === 0) return null;

  return (
    <div className="p-1 space-y-0.5">
      {grouped.map(([cat, cmds]) => {
        const open = !collapsed.has(cat);
        return (
          <div key={cat}>
            <button onClick={() => toggle(cat)}
              className="w-full flex items-center gap-1.5 px-2 py-1 rounded hover:bg-[hsl(var(--bg-hover))] transition-colors text-left">
              {open ? <ChevronDown size={12} className="text-[hsl(var(--text-tertiary))] shrink-0" /> : <ChevronRight size={12} className="text-[hsl(var(--text-tertiary))] shrink-0" />}
              <span className="text-[11px] font-medium text-[hsl(var(--text-secondary))]">{CATEGORY_LABELS[cat] || cat}</span>
              <span className="text-[10px] text-[hsl(var(--text-tertiary))] ml-auto">{cmds.length}</span>
            </button>
            {open && (
              <div className="ml-3 space-y-0.5">
                {cmds.map((cmd) => (
                  <button key={cmd.id}
                    onClick={() => onAdd(cmd)}
                    className="w-full flex items-center gap-1.5 px-2 py-0.5 rounded text-left hover:bg-[hsl(var(--accent)_/_0.08)] transition-colors group">
                    <span className="text-[10px] text-[hsl(var(--accent))] opacity-0 group-hover:opacity-100 shrink-0 w-3">+</span>
                    <code className="text-xs bg-[hsl(var(--bg-hover))] px-1 rounded truncate">{cmd.command}</code>
                    {cmd.description && <span className="text-[11px] text-[hsl(var(--text-tertiary))] truncate">{cmd.description}</span>}
                  </button>
                ))}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
