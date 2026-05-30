# HTML 巡检报告模板设计

## 目标

将巡检结果生成 HTML 格式报告，用户可在浏览器中打开并另存为 PDF，用于客户归档。替换现有纯 Markdown 报告方案。

## 方案概述

方案 C：Rust 生成 HTML → 系统浏览器打开 → 用户 Ctrl+P 另存 PDF。

## HTML 模板规格

### 纸张

- A4 竖版 (210×297mm)，四边页边距 20mm

### 字体

- 正文：宋体 11pt（`"宋体", "SimSun", serif`）
- 命令输出：Consolas 等宽 9pt
- 设备标题：14pt bold
- 报告大标题：18pt bold

### 结构（每设备一节）

```
┌─ 报告大标题 + 批次元数据
├─ [设备1] 设备标题
│   ├─ 小标题：基本信息
│   ├─ 基本信息表（3行×4列 compact 布局）
│   │   设备名称 | 值 | 设备型号 | 值
│   │   IP地址   | 值 | 设备SN   | 值
│   │   出厂日期  | 值 | 厂商    | 值
│   ├─ 小标题：巡检记录
│   └─ 巡检结果表（序号|巡检项目|巡检内容|评判结论）
│       最后一行 colspan="4" 合并 → 总体评判 + 历史趋势
├─ [设备2] ...
└─ ...
```

### 表格样式

- 边框：1pt solid #333
- 表头：浅灰底色 #f5f5f5，黑字 bold
- 总结行顶部：1.5pt 粗分割线
- 序号列：居中，40px
- 巡检项目列：居中，80px，no-wrap
- 巡检内容列：左对齐顶对齐，自适应最大宽度，命令输出等宽字体 pre-wrap
- 评判结论列：左对齐上下居中，允许折行
- 基本信息表：标签左对齐，值居中，全部 no-wrap

### 分页

- 每个设备 `page-break-after: always`
- 表头 `thead { display: table-header-group }` 跨页重复
- 设备 section `page-break-inside: avoid`

### 屏幕预览

- 灰色背景 + 白纸居中容器（`max-width: 794px`，`padding: 20mm`）
- 打印时移除容器样式

## 数据流

1. 用户选择批次，点击"导出 HTML 报告"
2. 前端 `invoke("generate_html_report", { batchId })`
3. Rust 查 DB：批次 → 设备列表 → 巡检记录 + AI 判定
4. 填充 HTML 模板占位符
5. 写入 `data/reports/batch_{id}_{timestamp}.html`
6. 返回文件路径给前端
7. 前端调用 `invoke("open_in_browser", { filePath })`
8. 系统默认浏览器打开 HTML
9. 用户 Ctrl+P → 另存为 PDF

## 实现范围

### Rust 后端

- `src-tauri/src/services/report_builder.rs` — HTML 模板常量 + `build_report_html()` 函数
  - 模板使用 `{{占位符}}` 替换，设备/行数据循环拼接
- `src-tauri/src/commands/reports.rs` 新增两个 command：
  - `generate_html_report(batch_id)` → 生成 HTML 文件返回路径
  - `open_in_browser(file_path)` → `open::that(path)` 系统浏览器打开

### 前端

- `ReportsPage.tsx` 新增"导出 HTML 报告"按钮
  - 按钮 loading 态 → 生成成功 → 提示"报告已生成，请在浏览器中 Ctrl+P 另存为 PDF"

### 依赖

- Cargo.toml 新增 `open` crate（`open = "5"`）

## 不涉及

- Markdown 报告生成保留不变，HTML 报告作为独立新增功能
- 不引入 Puppeteer/Playwright 等外部渲染引擎
- 不做服务端 PDF 生成
