# UI 全面重设计 — 设计规格

**日期**: 2026-05-29
**方向**: 专业工具风 · 暗色模式 · Inspect

## 一、设计方向

深色专业仪表板。纯暗色模式（无亮色切换），深 navy 底色 + 蓝色强调。Inter 系系统字体，lucide-react 图标库。对标 VS Code / Linear 暗色美学。

## 二、布局结构

保留三段式骨架，不改路由/页面结构：

| 区域 | 规格 |
|------|------|
| Sidebar | 收起 56px / 展开 220px，背景 `--bg-app` (#0b1120)，激活态左侧色条指示 |
| Content | 背景 `--bg-content` (#111827)，内边距 24px，页面标题 18px/600 |
| StatusBar | 高 28px，背景 `--bg-app`，左侧状态点，右侧版本号 |

## 三、配色系统

全部使用 CSS 自定义属性（HSL），组件中禁止裸 hex。

```
背景层级:
  --bg-app:       215 28% 7%     侧栏/底栏
  --bg-content:    217 19% 11%    内容区
  --bg-card:       217 19% 14%    卡片/表格
  --bg-hover:      217 19% 18%    悬浮态
  --bg-active:     217 19% 22%    激活/选中

文字层级:
  --text-primary:   210 20% 95%   标题/正文
  --text-secondary: 215 12% 65%   辅助文字
  --text-tertiary:  215 12% 45%   禁用/占位

强调色:
  --accent:         217 91% 60%   主按钮/链接
  --accent-subtle:  217 91% 60% / .12

语义色:
  --success: 142 71% 45%   --danger: 0 72% 51%
  --warning: 38 92% 50%    --info: 217 91% 60%

边框:
  --border:        217 15% 20%    分割线
  --border-light:  217 15% 15%    弱分割
```

WCAG AA 级对比度保证。

## 四、字体系统

**字体栈**（系统原生，不加载外部字体）：
```
ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont,
"Segoe UI", Roboto, "Helvetica Neue", Arial,
"Noto Sans", "Microsoft YaHei", "PingFang SC", sans-serif
```

**字号体系**：

| Token | 大小 | 用途 |
|-------|------|------|
| text-xs | 11px | 状态栏、badge |
| text-sm | 13px | 表格数据、表单标签 |
| text-base | 14px | 正文、输入框、按钮 |
| text-lg | 16px | 卡片标题 |
| text-xl | 18px | 页面标题 |
| text-2xl | 22px | 弹窗标题 |

Body 默认 14px，行高 1.5。等宽字体用于 IP 地址/命令输出。

## 五、组件规格

技术方案：React 组件 + TailwindCSS + `class-variance-authority` (cva) 管理变体。所有组件写为独立文件在 `src/components/ui/` 下。

### 5.1 Button

```
文件: src/components/ui/Button.tsx
变体: primary | secondary | ghost | danger
大小: sm(h=28) | md(h=32) | icon(32×32)
状态: default / hover / active / disabled / loading(spinner)
```
- primary: 蓝底白字，hover 变亮
- secondary: 透明底+边框，hover 浅蓝底
- ghost: 纯透明，hover 浅灰底
- danger: 红底白字
- 使用 lucide `Loader2` 图标 + animate-spin 做 loading 态

### 5.2 Input / Select

```
文件: src/components/ui/Input.tsx
```
- 暗底 `--bg-card`，1px 边框 `--border`
- focus: 边框蓝 + ring 2px
- placeholder: `--text-tertiary`
- 高度 32px(sm) / 36px(md)
- disabled: opacity-50 + cursor-not-allowed

### 5.3 Card

```
文件: src/components/ui/Card.tsx
```
- 背景 `--bg-card`，1px 边框，圆角 8px
- props: padding (boolean, 默认 true → p-4)

### 5.4 Modal

```
文件: src/components/ui/Modal.tsx  (重构现有)
```
- 遮罩: bg-black/60 + backdrop-blur-sm
- 面板: `--bg-card`，圆角 10px
- 入场: scale(0.96→1) + opacity，150ms ease-out
- ESC/点击遮罩关闭

### 5.5 DataTable

```
文件: src/components/ui/DataTable.tsx  (重构现有)
```
- 表头: `--bg-hover` 底，text-xs，w500
- 行 hover: `--bg-hover` 过渡
- 选中行: `--accent-subtle` 底
- 单元格: py-2 px-3，`--border-light` 水平分割

### 5.6 StatusBadge

```
文件: src/components/ui/StatusBadge.tsx  (重构现有)
```
- 半透明底色 + 同色系文字，h-5，text-[11px]，rounded-[4px]
- 小圆点+文字布局

### 5.7 SearchInput

```
文件: src/components/ui/SearchInput.tsx  (重构现有)
```
- lucide Search 图标左侧，暗底输入框
- 有内容时显示 X 清除按钮

## 六、图标迁移

- 删除 AppShell 中的手写 SVG `NAV_ICONS`
- 全部改用 `lucide-react`：
  - 设备管理: `Server`, 模板: `FolderTree`, 巡检: `Play`, 报告: `FileText`
  - AI 配置: `Bot`, 设置: `Settings`
- 导航项 `size={18}`，按钮内 `size={14}`，页面装饰 `size={16}`

## 七、动效规范

| 类型 | 时长 | 缓动 | 用途 |
|------|------|------|------|
| 微交互 | 150ms | ease-out | hover、focus、active 状态切换 |
| 入场 | 200ms | ease-out | 页面切换、弹窗打开 |
| 出场 | 120ms | ease-in | 弹窗关闭 |
| 加载 | 持续 | linear | spinner 旋转 |

- 使用 Tailwind `transition-all duration-150` 等工具类
- 页面切换保留现有 `animate-in` 但调整时长到 200ms
- 不引入 framer-motion 等额外依赖

## 八、页面改造范围

| 页面 | 改造内容 |
|------|----------|
| AppShell | Sidebar 重写，图标 lucide，配色更新 |
| DevicesPage | Button/Input/Card/Table/Modal/Badge 组件替换 |
| TemplatesPage | 同上组件替换 |
| InspectionPage | 同上组件替换 |
| ReportsPage | 同上组件替换 |
| AiConfigPage | 同上组件替换 |
| SettingsPage | 同上组件替换 |

各页面业务逻辑不变，仅替换 UI 组件和样式类名。

## 九、文件清单

### 新建
- `src/components/ui/Button.tsx`
- `src/components/ui/Input.tsx`
- `src/components/ui/Card.tsx`

### 重构
- `src/components/ui/Modal.tsx`
- `src/components/ui/DataTable.tsx`
- `src/components/ui/StatusBadge.tsx`
- `src/components/ui/SearchInput.tsx`
- `src/index.css` — 配色变量、基础重置、移除旧 .btn/.card CSS 类
- `src/layouts/AppShell.tsx` — Sidebar 样式、图标替换
- `src/pages/*.tsx` — 6 个页面组件替换

### 不变
- `tailwind.config.js` — 现有结构足够，不动
- `src/types/` — 类型定义不变
- `src/App.tsx` — 路由不变
- Rust 后端所有代码

## 十、不做

- 不支持亮色模式切换
- 不引入 shadcn/ui 或其他 UI 库
- 不引入 framer-motion
- 不改动路由和页面结构
- 不改动业务逻辑
- 不加载 Google Fonts（保持离线可用）
