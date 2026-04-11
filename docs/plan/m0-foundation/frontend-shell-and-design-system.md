# M0-FE — Frontend Shell And Design System

> 讀這份文檔的時機：當你要重做整個桌面端 UI 骨架、對齊設計 prototype、清理舊 `AppNew` 結構時。  
> 這份文檔只管前端骨架和畫面契約，不處理底層 schema / parser 的實作細節。

2026-04-06 實作註記：

- 新 shell 已落在 `src/app/`、`src/components/sidebar/`、`src/components/topbar/`、`src/pages/*/index.tsx`、`src/styles/{tokens,app}.css`
- design token source of truth 已寫入 [../../design/design-tokens.md](../../design/design-tokens.md)
- 2026-04-06 審查修正：舊的 shell slice gate 已退回成 desktop contract slice（`src/main.tsx` + `src/lib/ipc/bridge.ts`）；frontend shell / route / sidebar / primitives 不再宣稱已由 coverage / mutation gate 完整收口
- 2026-04-09 closeout：prototype 未覆蓋的 production states、typed bridge / preview split、Dashboard / Explorer / Audit / Schedule / Security command-response contracts 都已由 M1 / M2 / M3 的真實頁面和測試補齊；M0 這裡不再把它們留成假開放項。

---

## Quick-Start Implementation Guide

以下是本工作包的建議執行順序。每個步驟都標註了要讀的文檔、要改的文件、和驗收方式。

> **前提**：先阅讀 `reference/PathKeep — Desktop UI Design/index.html` 和 `style.css`，直接在瀏覽器開啟 prototype 確認已有哪些畫面。再阅讀 `docs/design/screens-and-nav.md` 確認新 IA。

### Step 1: Delete old files and clear the rewrite target

**要讀的文檔**

- `docs/plan/program/repo-baseline.md` 的「前端基線」段落 — 確認哪些檔案將被淢汰
- `src/AppNew.tsx` — 先讀清楚艷子，再刪除

**要刪除的文件**

```
src/AppNew.tsx                 # 舊 shell—刪除前先備份可複用的 type/helper
src/App.css                    # 1880 行舊樣式，全部用新 design token 替換
src/AppNew.test.tsx            # 3081 行舊小型測試，按新頁面拆分後建立新測試
src/lib/i18n.ts                # 1547 行舊 i18n，M0 先暫置，M2 再正式重做
src/pages/dashboard.tsx        # 舊頁面，將被新 skeleton 取代
src/pages/explorer.tsx         # 同上
src/pages/insights.tsx         # 同上
src/pages/activity-log.tsx     # 同上
src/pages/import.tsx           # 同上
src/pages/onboarding.tsx       # 同上
src/pages/settings/           # 整個 settings 目錄，將被新 skeleton 取代
```

**要保留（可複用 / 將 refactor）的文件**

```
src/main.tsx                   # 保留，但將 import AppNew 改成 import App
src/lib/backend.ts             # 保留作 reference，但將拆出假資料和 IPC wrapper
src/lib/types/                 # 保留和擴展，新的 type 按 domain 模塊放這裡
src/lib/format.ts              # 保留
src/lib/stronghold.ts          # 保留
src/components/ui.tsx          # 保留，但用新 design token 更新
src/lib/browser-icons.tsx      # 保留
```

**驗收**

```bash
# Step 1 不應作為紅燈 checkpoint 單獨提交；刪除和替換要包在同一個 work session 內完成
bun run typecheck
```

**Commit**: `chore(fe): remove legacy AppNew shell and old page files`

---

### Step 2: Create new directory structure

**要讀的文檔**

- `docs/design/screens-and-nav.md` — 新的 IA 和導航規則
- `reference/PathKeep — Desktop UI Design/index.html` — prototype 的實際結構

**要建立的目錄和檔案**

```
src/
  app/
    index.tsx              # 新 app 入口，取代 AppNew.tsx
    shell.tsx              # 主 shell layout（sidebar + topbar + content）
    router.tsx             # route tree 定義
    onboarding-shell.tsx   # onboarding 獨立 shell（不含 sidebar）
  components/
    sidebar/
      index.tsx            # Sidebar 元件
      nav-item.tsx         # 單個導航項
    topbar/
      index.tsx
    primitives/
      empty-state.tsx      # 全局共用 empty state
      loading-state.tsx    # 全局共用 loading
      error-state.tsx      # 全局共用 error
      permission-gate.tsx  # Full Disk Access / keyring 等權限請求
  pages/
    dashboard/
      index.tsx
    explorer/
      index.tsx
    insights/
      index.tsx
    assistant/
      index.tsx
    import/
      index.tsx
    audit/
      index.tsx
    schedule/
      index.tsx
    security/
      index.tsx
    settings/
      index.tsx
    onboarding/
      index.tsx
  lib/
    tokens.ts              # Design token 常數（CSS variable names）
    ipc/
      bridge.ts            # 真正的 typed IPC wrapper，不含假資料
```

**`src/main.tsx` 更新**

```tsx
// 將這一行：
import AppNew from './AppNew'
// 改成：
import App from './app'
```

**`src/app/router.tsx` 起手式**（使用現有 `react-router-dom` v7）

```tsx
export const routes = [
  { path: '/', element: <DashboardPage /> },
  { path: '/explorer', element: <ExplorerPage /> },
  { path: '/insights', element: <InsightsPage /> },
  { path: '/assistant', element: <AssistantPage /> },
  { path: '/import', element: <ImportPage /> },
  { path: '/audit', element: <AuditPage /> },
  { path: '/schedule', element: <SchedulePage /> },
  { path: '/security', element: <SecurityPage /> },
  { path: '/settings', element: <SettingsPage /> },
  { path: '/onboarding', element: <OnboardingPage /> },
]
```

**驗收**

```bash
bun run typecheck
bun run test:unit
# 確認 src/AppNew.tsx 已不存在， src/app/index.tsx 已存在
```

**Commit**: `feat(fe): create new src/app/ directory structure and route tree`

---

### Step 3: Extract design tokens from prototype style.css

**要讀的文檔**

- `reference/PathKeep — Desktop UI Design/style.css` — 讀取全文 `:root` 的所有 CSS variable
- `docs/plan/program/research-and-decisions.md` 的 `PG-RD-UX-003` — token 轉換明確要求

**要建立的文件**

```
src/lib/tokens.ts         # Token 常數對照表（TS）
src/styles/tokens.css     # CSS variable 定義（從 prototype 直接搬過來並整理）
```

**Prototype 的 `:root` 平直搬到 `tokens.css`**

```css
/* src/styles/tokens.css */
:root {
  /* Core Palette */
  --bg: #0a0a0a;
  --bg-elevated: #111111;
  --bg-surface: #161616;
  --bg-hover: #1a1a1a;
  --border: #2a2a2a;
  --border-active: #3a3a3a;

  /* Text */
  --text: #c8c8c8;
  --text-muted: #6a6a6a;
  --text-faint: #3e3e3e;
  --text-bright: #e8e8e8;

  /* Accent */
  --accent: #ff7832;
  --accent-dim: rgba(255, 120, 50, 0.15);
  --accent-hover: #ff944d;
  --accent-glow: rgba(255, 120, 50, 0.08);

  /* Semantic */
  --success: #4ade80;
  --success-dim: rgba(74, 222, 128, 0.15);
  --warning: #fbbf24;
  --warning-dim: rgba(251, 191, 36, 0.15);
  --error: #f87171;
  --error-dim: rgba(248, 113, 113, 0.15);
  --info: #60a5fa;

  /* Typography */
  --font-ui:
    ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont,
    'Segoe UI Variable Text', 'Segoe UI', Roboto, 'Noto Sans', Ubuntu,
    Cantarell, 'Helvetica Neue', Arial, sans-serif;
  --font-body: var(--font-ui);
  --font-code:
    ui-monospace, 'SFMono-Regular', 'SF Mono', 'Cascadia Mono', 'Cascadia Code',
    Consolas, 'Liberation Mono', Menlo, monospace;
  --font-mono: var(--font-ui); /* legacy alias only */

  /* Spacing (4px grid) */
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-5: 20px;
  --space-6: 24px;
  --space-8: 32px;
  --space-10: 40px;
  --space-12: 48px;

  /* Radius — Brutalism: no rounding */
  --radius: 0px;

  /* Transition */
  --transition: 120ms ease;
}
```

**`src/lib/tokens.ts` — TS 層對照**

```ts
export const tokens = {
  color: {
    bg: 'var(--bg)',
    bgElevated: 'var(--bg-elevated)',
    bgSurface: 'var(--bg-surface)',
    accent: 'var(--accent)',
    text: 'var(--text)',
    textMuted: 'var(--text-muted)',
    success: 'var(--success)',
    warning: 'var(--warning)',
    error: 'var(--error)',
  },
  font: {
    ui: 'var(--font-ui)',
    body: 'var(--font-body)',
    code: 'var(--font-code)',
  },
  space: {
    1: 'var(--space-1)',
    2: 'var(--space-2)',
    4: 'var(--space-4)',
    6: 'var(--space-6)',
    8: 'var(--space-8)',
  },
} as const
```

**在 `src/index.css` 引入 tokens.css**

```css
/* src/index.css */
@import './styles/tokens.css';
/* ... 其他 global reset ... */
```

**驗收**

```bash
bun run typecheck          # tokens.ts 沒有 type error
bun run build              # 確認 tokens.css 已被打包
# 手動檢查：沒有頁面直接寫無對照的顏色字串（如 #FF7832 或 #0A0A0A）
```

**Commit**: `feat(design): extract design tokens from prototype`

---

### Step 4: Build shell layout

**要讀的文檔**

- `reference/PathKeep — Desktop UI Design/index.html` — prototype 的實際 HTML 結構
- `reference/PathKeep — Desktop UI Design/style.css` 的 `.app-shell`、`.sidebar`、`.main-content` 段落
- `docs/design/screens-and-nav.md` — sidebar hierarchy 和 nav item 規格

**要建立的文件**

```
src/app/shell.tsx
src/components/sidebar/index.tsx
src/components/sidebar/nav-item.tsx
src/components/topbar/index.tsx
```

**Prototype 的 HTML 結構對照**—從 prototype 可看到 app shell 的模式：

```html
<div class="app-shell">
  <div class="dot-grid"></div>
  <!-- 背景點陣列 -->
  <nav class="sidebar">...</nav>
  <main class="main-content">...</main>
</div>
```

**`src/app/shell.tsx` 起手式**

```tsx
import { Outlet, NavLink } from 'react-router-dom'
import { Sidebar } from '../components/sidebar'
import { Topbar } from '../components/topbar'

export function MainShell() {
  return (
    <div className="app-shell">
      <div className="dot-grid" aria-hidden />
      <Sidebar />
      <div className="shell-right">
        <Topbar />
        <main className="main-content">
          <Outlet />
        </main>
      </div>
    </div>
  )
}
```

**Sidebar nav items**—順序和圖示依照 prototype （直接從 `reference/PathKeep — Desktop UI Design/app.js` 查看）

```tsx
const NAV_ITEMS = [
  { path: '/', label: 'Dashboard', icon: 'dashboard' },
  { path: '/explorer', label: 'Explorer', icon: 'search' },
  { path: '/insights', label: 'Insights', icon: 'insights' },
  { path: '/assistant', label: 'Assistant', icon: 'chat' },
  { path: '/import', label: 'Import', icon: 'upload' },
  { path: '/audit', label: 'Audit', icon: 'history' },
]
// settings / schedule / security 在 sidebar 底部
```

**驗斖**

```bash
bun run dev   # app 可啟動，能看到 sidebar + main content 區域
# 視覚檢查：與 prototype 的寬度 / 間距 / 差異小於視覺誤差範圍
bun run typecheck
```

**Commit**: `feat(fe): build main shell layout with sidebar and topbar`

---

### Step 5: Create route tree with react-router-dom

**要讀的文檔**

- `docs/design/screens-and-nav.md` — 這是 route tree 的 source of truth
- Step 2 中建立的 `src/app/router.tsx`

**要更新的文件**

```
src/app/index.tsx     # 使用 createBrowserRouter 時需要在這裡組裝
```

**Route 結構**（注意 onboarding 独立 shell，不含 sidebar）

```tsx
import { createBrowserRouter, RouterProvider } from 'react-router-dom'
import { MainShell } from './shell'
import { OnboardingShell } from './onboarding-shell'

const router = createBrowserRouter([
  {
    path: '/onboarding',
    element: <OnboardingShell />,
    children: [{ index: true, element: <OnboardingPage /> }],
  },
  {
    path: '/',
    element: <MainShell />, // 包含 sidebar + topbar
    children: [
      { index: true, element: <DashboardPage /> },
      { path: 'explorer', element: <ExplorerPage /> },
      { path: 'insights', element: <InsightsPage /> },
      { path: 'assistant', element: <AssistantPage /> },
      { path: 'import', element: <ImportPage /> },
      {
        path: 'audit',
        element: <AuditPage />,
        children: [{ path: ':runId', element: <AuditDetailPage /> }],
      },
      { path: 'schedule', element: <SchedulePage /> },
      { path: 'security', element: <SecurityPage /> },
      { path: 'settings', element: <SettingsPage /> },
    ],
  },
])

export default function App() {
  return <RouterProvider router={router} />
}
```

**Onboarding 第一次啟動轉向邏輯**

```tsx
// src/app/index.tsx 或 main.tsx 的一次性轉向
// 如果 backend.ts 的 archive_initialized === false ，則 navigate('/onboarding')
// 如果已初始化，则直接到主 shell
```

**驗收**

```bash
bun run typecheck
bun run test:unit        # 路由相關 tests pass
# 手動驗成：切換各個 route，確認 sidebar active 狀態正確、back button 正常
```

**Commit**: `feat(fe): implement react-router route tree`

---

### Step 6: Create page skeleton components

**要讀的文檔**

- `reference/PathKeep — Desktop UI Design/index.html` — 每個歓迎畫面的 HTML 結構
- `docs/design/screens-and-nav.md` — 每層欳構的內容規格

**Skeleton 建立觳則**

- 每個 skeleton 只做 layout 和 slot 定義，不含真實 IPC 呼叫
- 用注解標注每個 slot 將連到哪個 IPC command（M1 實作時再對接）
- 每個頁面都要有 `EmptyState` 、 `LoadingState` 、 `ErrorState` 視圖

**要建立的文件（一次建全）**

```
src/pages/dashboard/index.tsx
  └── slots: recent-runs, archive-health, next-schedule, storage-summary, trust-callouts

src/pages/explorer/index.tsx
  └── slots: search-bar, facet-bar, timeline-list-switch, result-list, detail-pane, export-action

src/pages/insights/index.tsx
  └── slots: chart-modules, time-range-picker, zero-state, ai-unavailable-fallback

src/pages/assistant/index.tsx
  └── slots: thread-list, composer, evidence-panel, provider-not-configured-fallback

src/pages/import/index.tsx
  └── slots: file-picker, dry-run-summary, preview-artifacts, quarantine-status, execute-action

src/pages/audit/index.tsx
  └── slots: run-ledger-table, run-detail-panel, artifact-viewer, copy-command, rollback-entry

src/pages/onboarding/index.tsx
  └── slots: product-intro, storage-choice, browser-detection, schedule-preview, privacy-promise

src/pages/schedule/index.tsx
  └── slots: current-schedule, pme-flow, next-run-preview

src/pages/security/index.tsx
  └── slots: encryption-status, rekey-flow, key-backup-hint

src/pages/settings/index.tsx
  └── slots: general-tab, providers-tab, storage-tab
```

**Dashboard skeleton 範例**

```tsx
export function DashboardPage() {
  // TODO M1: const { data, isLoading } = useQuery('dashboard_summary', ...)
  return (
    <div className="page dashboard">
      <header className="page-header">
        <h1>Dashboard</h1>
      </header>
      <section className="dashboard-grid">
        {/* 近期備份 run 列表 — M1-DB */}
        <div className="widget" data-slot="recent-runs">
          <LoadingState label="Loading recent runs…" />
        </div>
        {/* Archive 健康狀態 — M1-DB */}
        <div className="widget" data-slot="archive-health">
          <LoadingState label="Loading archive health…" />
        </div>
        {/* 下次蒐排 — M1-OPS */}
        <div className="widget" data-slot="next-schedule">
          <EmptyState label="Schedule not configured" />
        </div>
      </section>
    </div>
  )
}
```

**驗斖**

```bash
bun run typecheck
bun run test:unit   # 新 skeleton 的 smoke render tests pass
bun run dev         # 所有 route 會渲染出頁面，不出現空白頁
```

**Commit**: `feat(fe): add page skeleton components for all routes`

---

### Step 7: Update legacy backend.ts and create typed IPC bridge

**要讀的文檔**

- `src/lib/backend.ts` — 先全文學讀一遍，列出其中假資料和真實 IPC 呼叫的關鍵字

**要建立的文件**

```
src/lib/ipc/bridge.ts   # 只有真實 invoke() 呼叫和返回型別，不含 mock data
```

**拆分原則**

```
src/lib/backend.ts 中要移除的部分：
  - 模擬資料（browser preview fixture）
  - 舊產品文案字串（"Browser History Backup" 等）
  - 舊導航狀態管理（activePage 等）

src/lib/backend.ts 中要保留的部分：
  - 真實的 invoke() 包裝（archive_status、run_backup 等）
  - 返回型別定義
  → 搬到 src/lib/ipc/bridge.ts
```

**驗斖**

```bash
bun run typecheck   # 沒有宣告假資料的 export 被其他檔案依賴
```

**Commit**: `refactor(fe): split backend.ts into typed IPC bridge and remove mock data`

---

## Source Inputs

- [../../vision-and-requirements.md](../../vision-and-requirements.md)
- [../../design/ux-principles.md](../../design/ux-principles.md)
- [../../design/screens-and-nav.md](../../design/screens-and-nav.md)
- [../../reference-review.md](../../reference-review.md)
- `reference/PathKeep — Desktop UI Design/`
- [../program/repo-baseline.md](../program/repo-baseline.md)
- [backend-and-data-rearchitecture.md](backend-and-data-rearchitecture.md)

---

## 本工作包要交付什麼

- 新的 app shell、route tree、sidebar IA、page title / breadcrumb 規範
- 從 prototype 抽取出的正式 design tokens、component primitives、theme contract
- 新的 screen skeleton：Onboarding、Dashboard、Explorer、Insights、Assistant、Import、Audit、Schedule、Security、Settings
- 舊 `AppNew` / `App.css` / 單檔巨大測試的淘汰策略
- 新前端對後端的資料契約與 loading / error / empty state 規範

---

## WBS

### IA And Route Tree

- [x] `M0-FE-IA-001` 盤點目前 [`src/AppNew.tsx`](../../../src/AppNew.tsx) 的 page model、context model、navigation model，標記哪些概念要保留、哪些直接淘汰。
- [x] `M0-FE-IA-002` 依照 [../../design/screens-and-nav.md](../../design/screens-and-nav.md) 凍結新的 top-level route tree，包含每個 route 的 path、title、sidebar group、secondary action。
- [x] `M0-FE-IA-003` 定義 Dashboard / Explorer / Insights / Assistant / Import / Audit / Schedule / Security / Settings 的 sidebar hierarchy，不再沿用舊 setup-first shell。
- [x] `M0-FE-IA-004` 定義 onboarding 何時獨立於主 shell 顯示、何時可被再次打開、完成後如何回到 dashboard。
- [ ] `M0-FE-IA-005` 定義全域 command bar / search entry 的位置、快捷鍵、與 Explorer / Assistant 的關係。
- [ ] `M0-FE-IA-006` 為每個 route 定義 URL state contract，例如 Explorer filters、Assistant thread id、Audit run id、Settings sub-tab。
- [x] `M0-FE-IA-007` 定義 breadcrumb、page header、status rail、context panel 的共用規範，避免每頁自行發明資訊階層。
- [x] `M0-FE-IA-008` 明確列出 prototype 未覆蓋的 route state：empty state、error state、permission denied、loading、zero data、first-run。

### Design Tokens And Primitive Layer

- [x] `M0-FE-DS-001` 從 prototype `style.css` 抽取顏色、字級、spacing、radius、shadow、surface、border、motion token。
- [x] `M0-FE-DS-002` 形成正式 token 文檔和程式碼落點，避免 token 繼續散落在單頁 CSS 裡。
- [x] `M0-FE-DS-003` 決定 token 的實作方式：CSS variables、theme helper 或等價方案；要求和現有 Vite / React 結構相容。
- [x] `M0-FE-DS-004` 凍結 light / dark theme contract，確認 day one 是否 dark-first，但不讓 light theme 完全消失。
- [x] `M0-FE-DS-005` 為 success / warning / danger / info / muted / selected / focus 定義語義色，不允許頁面自己挑色。
- [x] `M0-FE-DS-006` 定義資料密度階梯：sidebar、table row、timeline item、chip、code snippet、artifact card 的 spacing 規則。
- [ ] `M0-FE-DS-007` 為數據可視化建立最小 token 集：chart palette、heatmap density、empty chart fallback、trend up / down 狀態。
- [ ] `M0-FE-DS-008` 決定字體資產策略，確認 prototype 字體在桌面端的授權、打包和跨平台 fallback。
- [x] `M0-FE-DS-009` 為動畫建立 guardrail：page reveal、panel transition、loading shimmer、背景動效允許哪些效果，哪些不要做過頭。

### Shell And Shared Layout

- [x] `M0-FE-SH-001` 建立新的 `src/app/` 或等價目錄作為 shell 入口，停止讓 `AppNew` 承載全部畫面與狀態。
- [x] `M0-FE-SH-002` 建立 route-aware shell 結構，分清 onboarding shell、main shell、settings shell 是否共用 chrome。
- [x] `M0-FE-SH-003` 建立共用 sidebar、top bar、command entry、status summary、content container primitives。
- [x] `M0-FE-SH-004` 定義 shell 層的 responsive 規則，至少覆蓋桌面窄寬度和小窗模式，不追求 mobile app，但也不能一縮就崩。
- [x] `M0-FE-SH-005` 建立 page metadata system，讓 route 可以聲明 title、subtitle、danger level、required capability、loading policy。
- [x] `M0-FE-SH-006` 建立 global empty / loading / error component contract，讓後續頁面可以共用而不是複製文案和布局。
- [x] `M0-FE-SH-007` 為 permission-required flows 建立專用 panel，例如 Full Disk Access、directory permission、keyring unavailable、AI provider not configured。
- [x] `M0-FE-SH-008` 決定全域狀態管理策略，只保留最小 shared state；不要再把整個 app 塞回一個肥大 context。

### Screen Skeletons

- [x] `M0-FE-PG-001` 建立 Onboarding skeleton，至少含產品定位、storage choice、browser detection、schedule preview、privacy promise。
- [x] `M0-FE-PG-002` 建立 Dashboard skeleton，預留 recent runs、archive health、next schedule、storage summary、trust callouts 插槽。
- [x] `M0-FE-PG-003` 建立 Explorer skeleton，預留搜索框、facet bar、timeline / list switch、detail pane、export action、saved search slot。
- [x] `M0-FE-PG-004` 建立 Insights skeleton，預留 chart modules、time range、zero-state、AI unavailable fallback。
- [x] `M0-FE-PG-005` 建立 Assistant skeleton，預留 thread list、composer、evidence panel、capability gating、provider not configured fallback。
- [x] `M0-FE-PG-006` 建立 Import skeleton，預留 file picker、dry-run summary、preview artifacts、quarantine status、execute action。
- [x] `M0-FE-PG-007` 建立 Audit skeleton，預留 run ledger table、run detail、artifact viewer、copy command、rollback entrypoint。
- [x] `M0-FE-PG-008` 建立 Schedule、Security、Settings skeleton，為 PME、encryption、providers、language、storage location 提前留位。
- [x] `M0-FE-PG-009` 逐頁補上 prototype 沒畫但 production 必需的 empty / error / loading / offline / permission-denied states。（2026-04-09，`WORK-QC-C`：見 `screens-and-nav.md` 的 `Non-Prototype State Coverage`，以及 `trust-flows` / `intelligence-surfaces` tests）

### Frontend Data Contract

- [x] `M0-FE-DC-001` 為 Dashboard 定義 day-one IPC data contract，區分必需資料、lazy-loaded 資料、可選 intelligence 資料。（2026-04-09，`WORK-QC-C`：`AppSnapshot` / `DashboardSnapshot` / shell data provider 已正式接線）
- [x] `M0-FE-DC-002` 為 Explorer 定義 query result contract，包含 filters、cursor / pagination、sort、highlight、evidence placeholder。（2026-04-09，`WORK-QC-C`：`HistoryQuery` / `HistoryEntry` / semantic recall contract 已落地）
- [x] `M0-FE-DC-003` 為 Audit / Run detail 定義 artifact contract，包含 manifest、snapshot、warnings、copyable command、log excerpt。（2026-04-09，`WORK-QC-C`：`AuditRunDetail` / artifact viewer / reveal path 已落地）
- [x] `M0-FE-DC-004` 為 Onboarding / Schedule / Security / Settings 定義 command-response contract，區分 preview mode 和 execute mode。（2026-04-09，`WORK-QC-C`：對應 worker / Tauri surface 與 trust-flow tests 已接通）
- [x] `M0-FE-DC-005` 把 [`src/lib/backend.ts`](../../../src/lib/backend.ts) 裡的假資料和 IPC wrapper 拆開，建立真正的 typed bridge layer。（2026-04-09，`WORK-QC-C`：typed IPC wrapper 以 [`src/lib/ipc/bridge.ts`](../../../src/lib/ipc/bridge.ts) 為正式入口）
- [x] `M0-FE-DC-006` 決定前端如何表示 capability gating，例如 `archive_ready`、`scheduler_supported`、`keyring_available`、`ai_configured`。（2026-04-09，`WORK-QC-C`：`AppSnapshot` / `SecurityStatus` / `ScheduleStatus` / `AiIndexStatus` / App Lock refusal path 已形成正式 contract）

### Legacy Removal

- [x] `M0-FE-LG-001` 盤點 [`src/AppNew.tsx`](../../../src/AppNew.tsx)、[`src/App.css`](../../../src/App.css)、[`src/AppNew.test.tsx`](../../../src/AppNew.test.tsx)、[`src/lib/i18n.ts`](../../../src/lib/i18n.ts) 的可重用片段和應淘汰片段。
- [x] `M0-FE-LG-002` 把舊 shell 中仍有價值的文案、型別、輔助函式搬到新結構或正式刪除，不留「先放著以後再看」。
- [x] `M0-FE-LG-003` 將 `AppNew` 從主入口直接移除，不讓舊 shell 留在主流程旁邊。
- [x] `M0-FE-LG-004` 重寫或刪除舊 setup-first 相關 CSS 和測試斷言，避免新 shell 被舊快照和舊文案拖住。
- [x] `M0-FE-LG-005` 盤點 `src/pages/` 舊頁面，標記 `rewrite in place`、`replace with new file`、`delete immediately after replacement`。

### Testing And Design Verification

- [x] `M0-FE-QA-001` 為新 shell 建立最小 smoke test：app 啟動、route 切換、sidebar 可見、onboarding gating 正常。
- [x] `M0-FE-QA-002` 把巨型 [`src/AppNew.test.tsx`](../../../src/AppNew.test.tsx) 拆成 route-scoped test files，對應新頁面和 shared primitives。
- [ ] `M0-FE-QA-003` 建立 visual review checklist，逐頁比對 prototype 和實作，記錄可接受偏差和不可接受偏差。
- [ ] `M0-FE-QA-004` 定義 accessibility baseline：keyboard nav、focus ring、contrast、reduced motion、screen reader landmarks。
- [x] `M0-FE-QA-005` 為 design token 建立 snapshot 或 contract test，防止顏色 / 間距 / 語義狀態被無意改壞。
- [x] `M0-FE-QA-006` 在 M0 結束前，更新 e2e smoke 目標，讓 Playwright 不再驗證舊 setup shell。

---

## Exit Artifacts

- 新 shell 實作與 route map
- design tokens 與 primitive component layer
- prototype gap list 和補稿需求清單
- 前端 IPC contract 草案
- 舊 UI 刪除 / 保留清單
