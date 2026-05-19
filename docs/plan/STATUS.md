# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M16 — v0.3 Paper Redesign**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-V03-PAPER-REDESIGN-A** — Paper + Archival Frontend Rebuild (foundation shipped, route sweep pending)
  - 讀先：
    `docs/design/handoff/README.md` (handoff index)
    `docs/design/handoff/paper-redesign/README.md` (cover sheet from design tool)
    `docs/design/handoff/paper-redesign/project/pk-tokens.css` (visual rule book — 3,978 lines)
    `docs/design/handoff/paper-redesign/project/PathKeep Redesign.html` (entry composition)
    `docs/design/handoff/paper-redesign/project/pk-components.jsx` (PKSidebar / PKStatusBar / PKDetailPanel / PKSearchPalette / PKHeatmap)
    `docs/design/handoff/paper-redesign/project/pk-views.jsx` (HomeView / Dashboard editorial layout — already shipped)
    `docs/design/handoff/paper-redesign/project/pk-contactsheet.jsx` (Browse: contact sheet, day sticky, sessions, domain stacks)
    `docs/design/handoff/paper-redesign/project/pk-browse-nav.jsx` (CalendarPopover / DayNavControl / YearRail / archive density)
    `docs/design/handoff/paper-redesign/project/pk-search.jsx` (3-mode search hero + day-grouped results)
    `docs/design/handoff/paper-redesign/project/pk-intelligence.jsx` (KPIs / topics / domains / sessions / refind)
    `docs/design/handoff/paper-redesign/project/pk-assistant.jsx` (chat + evidence panel)
    `docs/design/handoff/paper-redesign/project/pk-import.jsx` (method picker + wizard stepper)
    `docs/design/handoff/paper-redesign/project/pk-audit.jsx` (manifest chain + runs + storage + snapshots)
    `docs/design/design-tokens.md` (will be rewritten)
    `docs/design/screens-and-nav.md` (will be rewritten)
    `docs/design/ux-principles.md` (will be rewritten)
    `src/styles/tokens.css`
    `src/components/shell/`
  - 目標：把 v0.2 brutalist 前端徹底替換為 "Paper + Archival" 美學（cream 紙感、Newsreader serif + JetBrains Mono、3 px 圓角、paper noise、darkroom vignette），全部接入既有 Rust/Tauri 2 後端。同時新增 per-URL notes + tags 後端能力與 AI Assistant / 語意搜尋的 provider-gated 真實接入。
  - 契約：
    - 全部 user-visible copy 在 commit 時三語齊全（en / zh-CN / zh-TW），`html[lang]` 必須跟著 runtime locale 走（typography-and-font-fallback ADR）。
    - 字體預設使用 bundled Newsreader + JetBrains Mono Latin subsets，Settings 提供 "system fonts only" 切換；CJK 永遠 fall back 到系統字體。
    - 100% JS / Rust coverage 與 mutation gate 不放鬆；既有 quality-matrix.md 仍是權威。
    - 此 redesign 已獲使用者授權 override 之前 Accepted design docs（design-tokens / screens-and-nav / ux-principles / brutalist radius / typography memory）。
    - 後端只追加 url_annotations + url_tags table，現有 schema / commands 不破壞；migration 011 forward-only。
  - 進度（2026-05-19）：
    - ✅ **Foundation shipped**：Tailwind v4 + shadcn primitives + cn helper + paper tokens.css + fonts.css (bundled Newsreader / JetBrains Mono) + paper.css (noise / vignette / animations) + tailwind.css (@theme 對應 paper tokens 與 shadcn 變數)；@/ path alias 接入 tsconfig + vite。
    - ✅ **Shell shipped**：`src/components/shell/` 新增 PKBrandMark / PKGlyph / PKSidebar / PKTopbar / PKStatusBar / PKSearchPalette；`src/app/shell.tsx` 已重寫為新 shell；i18n shell namespace 新增 paper-redesign 鍵 (findAPage / archiving / sources* / palette* / epigraph1..6) 在三語齊備。
    - ✅ **Dashboard shipped**：`/` 路由已實作 paper-redesign landing page — HeroBand + greeting + 4-stat strip + On This Day card + This Week card + YearHeatmap + Active Threads + Archive card + epigraph footer；接入 `useShellData()` 與 `coreIntelligenceApi.getOnThisDay`，deep-link 進 Explorer / Intelligence。
    - ✅ **Settings Appearance section shipped**：theme / font / density / paper texture persisted prefs in place（`appearance-section.tsx` + `paper-preferences.ts`）。
    - ✅ **Design handoff preserved in-repo**：`docs/design/handoff/paper-redesign/` 收藏完整 design package（HTML / pk-tokens.css / 11 個 JSX）為 source-of-truth，搭配 `docs/design/handoff/README.md` 導讀。`/tmp/pathkeep-design/` 不再是必要依賴。
    - ⏳ **Routes 仍待完成**（按優先序，每條都是獨立 work block-sized）：
      - **`/explorer` Browse**（最大份量，視覺中心）：把現有 timeline-bar + filters + results panel + semantic / regex / runtime panels 全部重畫成 paper contact sheet — day-sticky toolbar、cs-target-banner、DayNavControl（prev / pill / next / today）、CalendarPopover（with density heatmap）、YearRail、DomainStack（>= 3 連續同 domain 折疊）、ContactFrame、ListRow、DayInsightsStrip、HourlySparkline、LoadingSkeleton、PlaceholderDay。接入既有 `useExplorerUrlState` / `useExplorerData` / `useExplorerFavicons` hooks，不要重寫 data layer。Regex / semantic / runtime / 進階篩選保留為 paper-aesthetic 抽屜或副 panel。
      - **`/search`**：literary search hero — `sv-prompt`、`sv-input` (28 px Newsreader)、3-mode toggle (keyword / regex / semantic)、filter chips、SAVED_PROMPTS / RECENT_SEARCHES empty state、day-grouped result rows、`sv-result__seein` jump-to-Browse、provider-gated semantic snippet。
      - **`/intelligence`**：4-KPI strip (`intel-kpis`)、topic timeline (`intel-topic-row` + bars)、top domains rank list、recent sessions、active threads、refind shelf、LLM-needed callouts；接 `get_intelligence_primary/secondary_overview`、`get_top_search_concepts`、`get_browsing_rhythm`、refind 演算。
      - **`/assistant`**：chat surface (`assist-wrap` + `assist-msg`)、evidence panel (`assist-evidence`)、`assist-empty-prompts`、ProviderGate fallback；接 `ask_ai_assistant` + evidence anchors。
      - **`/import`** PME：method picker (`import-methods` × 3)、wizard stepper、preview stats、file list、info callout；接 existing import preview / commit。
      - **`/audit`**：manifest chain viz (`chain-viz` + `chain-block`)、runs table、storage breakdown bars、snapshots、export panel；接 existing audit data。
      - **`/schedule` / `/security` / `/maintenance` / `/jobs` / `/integrations` / `/onboarding` / `/lock`**：每個用 paper card grid 重新編排既有資料；保留現有 hooks。
      - **`/settings`**：完整 paper Settings — General / Archive / Sources / Notifications / About / 字體切換 / Accent color / Density；風格與整體一致（即使 design 圖未涵蓋）。Appearance section 已部分完成，其他 section 仍要 paper-style 重畫。
      - **PKDetailPanel slide-over**：title + url + actions + first/last visit + visit history sparkline + provenance + title-version history + Notes textarea + Tags + "Look further" related list；textarea debounced 寫入 annotations backend。
      - **i18n**：每個重畫的 route 上線時三語齊全；現有 `lock-and-explorer-shell.test.tsx` 對舊 topbar 的兩個 failing test 在 sweep 中同步重寫。
    - ⏳ **Backend annotations**：migration 011_notes_tags.sql + `vault-core/src/annotations/` 模組 + `commands/annotations.rs`（get / set / list / search / export）；接入 backup / retention pruning；100% Rust coverage + 通過 mutation。
    - ⏳ **Docs sweep**：design-tokens.md / screens-and-nav.md / ux-principles.md / ui-review-guardrails.md / typography-and-font-fallback.md / data-model.md 全部要按新方向重寫；新增 `docs/features/annotations.md`；intelligence.md + recall.md 移除 v0.3-coming 標記。
    - ⏳ **Memory**：feedback_brutalist_radius.md / project_v0_3_redesign.md / feedback_typography_policy.md 改成記錄 brutalist → paper 轉向。
    - ⏳ **Tests / quality**：`bun run check` 仍要通過（目前在 1193/1195 unit tests pass，2 個 failure 來自 `lock-and-explorer-shell.test.tsx` 對舊 topbar Notifications / searchbox 的斷言，需於 page sweep 同步重寫）。
  - 驗收（block 結束時必須全部達標）：
    - 設計圖中每個畫面在 light + dark 下都與設計檔高度一致；Settings / Schedule / Security / Maintenance / Jobs / Integrations / Onboarding / AppLock 也用相同視覺語言補完。
    - 每個 route 接入真實後端（不再有 v0.3-coming disabled UI）；AI / semantic search 在 provider 未配置時 inline 提示 "Configure AI provider → Settings"。
    - Notes / Tags 從 detail panel 寫入後端 annotations，重新打開仍可讀；FTS 索引可以搜尋。
    - 三語 i18n parity 100%；`html[lang]` 與 locale 同步；字體切換在 Settings 真實生效。
    - `bun run check` + `bun run verify` 全綠（100% JS / Rust coverage + mutation gate + desktop bridge truth gate）。
    - design-tokens / screens-and-nav / ux-principles / ui-review-guardrails / typography-and-font-fallback / data-model / annotations feature spec / intelligence / recall / STATUS / CHANGELOG / BACKLOG / research-and-decisions 全部反映新方向。
    - 截圖：每個 route 在 light + dark 都產出，附在 release artifacts。

- [x] **WORK-RELEASE-020-A** — v0.2.0 Planning Repair, Security Refresh, And Publication
  - 讀先：
    `README.md`
    `RELEASE.md`
    `docs/plan/BACKLOG.md`
    `docs/plan/CHANGELOG.md`
    `docs/plan/program/quality-matrix.md`
    `docs/features/intelligence.md`
    `docs/features/intelligence-current-state.md`
    `docs/features/archive.md`
    `docs/architecture/tech-stack.md`
    `docs/design/screens-and-nav.md`
    `.github/workflows/release.yml`
  - 目標：把 v0.2.0 發佈 truth 收斂到已完成內容，先處理 Dependabot alerts，再修復 milestone / backlog / status / source docs 的 v0.2 / v0.3 out-of-sync，最後 bump、驗證、tag、發佈 v0.2.0。
  - v0.2.0 發佈範圍：Lexical Recall V2、advanced keyword syntax、Windows unsigned installer / scheduler preview、release/security hardening、既有 archive / deterministic Core Intelligence。
  - 移出 v0.2.0 的 blocker：AI Assistant、embedding、semantic / hybrid search、MCP / skill artifacts、vector sidecar、readable webpage body fetch。這些全部搬到 `BACKLOG.md` 的 `WORK-AI-V03-A` / `WORK-READABLE-CONTENT-V03-A`，作為 v0.3.0 blocker 管理。
  - 契約：不可假裝 AI / readable-content 已可用；user-visible copy 必須同步 `en` / `zh-CN` / `zh-TW`；release 前必須處理 Dependabot alerts、跑 `bun run check` 與 `bun run verify`；release notes 必須包含本次 release 相關的真實 app 截圖。
  - 驗收：
    - GitHub Dependabot alerts #13 / #15 (`openssl`) 與 #14 (`tauri`) 已更新到 patched dependency versions；GitHub alert state 以 dependency graph rescan 為準。
    - `README.md`、feature / architecture / design docs、`BACKLOG.md`、`STATUS.md`、`CHANGELOG.md` 對 v0.2.0 / v0.3.0 scope 一致。
    - app 內 disabled AI / readable-content copy 改為 v0.3 roadmap，且三語 i18n parity 維持 100%。
    - `bun run check`、`bun run verify` 通過；release screenshot assets 由當前 app 產生並嵌入 GitHub release note。
  - 2026-05-09 closeout：v0.2.0 發佈 scope 收斂到已完成的 local-first archive、Lexical Recall V2 / advanced keyword syntax、deterministic Core Intelligence、Windows unsigned installer / scheduler preview 與 release/security hardening；未完成的 AI Assistant、embedding、semantic / hybrid search、MCP / skill artifacts、vector sidecar、readable webpage body fetch 全部移入 `BACKLOG.md` 的 v0.3.0 blocker blocks。
  - 發佈準備：版本已 bump 到 `0.2.0`；preview fixtures、backend deferred notes、Jobs / Assistant / Settings / Integrations / Explorer copy 與三語 i18n 已同步 v0.2.0 / v0.3 truth；release notes 與真實 app 截圖已產生於 `artifacts/release/v0.2.0/`。
  - 驗證結果：`bun run check` 與 `bun run verify` 通過，包含 100% JS/Rust coverage、browser-preview E2E、desktop-bridge truth gate、desktop-contract mutation gate、Rust supply-chain audit、release config guard 與 debug desktop build rehearsal。

> `BACKLOG.md` 目前的前兩個 blocked blocks 是 v0.3.0 AI / readable-content scope；maintenance / deep mutation hardening 不屬於 v0.2.0 release blocker，除非使用者另外排 dedicated window。

- [x] **WORK-PREVIEW-SHOWCASE-A** — Vercel Browser Preview Synthetic Dataset
  - 讀先：
    `docs/plan/STATUS.md`
    `docs/plan/BACKLOG.md`
    `docs/plan/CHANGELOG.md`
    `docs/plan/program/quality-matrix.md`
    `docs/features/archive.md`
    `docs/features/intelligence.md`
    `docs/features/intelligence-current-state.md`
    `docs/design/ux-principles.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ui-review-guardrails.md`
    `docs/design/design-tokens.md`
    `src/lib/backend-preview-fixtures.ts`
    `src/lib/backend-preview-state.ts`
    `src/lib/backend-preview-shell-commands.ts`
    `src/lib/backend-preview-intelligence-commands.ts`
    `src/lib/backend-preview-search.ts`
  - 目標：讓 Vercel 靜態 browser preview 預設使用 synthetic showcase data，讓訪客能看到有資料時的 Dashboard / Explorer / deterministic Intelligence 形態。
  - 契約：不得把真實 archive、raw browser history、URL、title、profile name 或 secret 寫進 repo / bundle；本地真實資料只允許用 read-only aggregate shape 作參考。Tauri / desktop runtime 不得接入 showcase fixture；browser preview 必須繼續誠實標示 fixture boundary，不得冒充 desktop truth。
  - 驗收：Vercel build path 可明確啟用 showcase dataset；local default browser-preview tests 不被迫改走 showcase；targeted preview tests、`bun run build` 與 `bun run check` 通過。
  - 2026-05-10 closeout：新增 browser-preview showcase dataset，以 synthetic public-domain rows 和 modeled aggregate totals 呈現 dataful Dashboard / Explorer / deterministic Core Intelligence；Vercel 透過 `vercel.json` build command 明確使用 `PATHKEEP_BROWSER_PREVIEW_DATASET=showcase`，local default 仍是 setup fixture。
  - 隔離邊界：showcase fixtures 只在 browser preview bundle 使用；Tauri / desktop `isTauri()` path 不讀取或接入 showcase data。本地真實 archive 只透過 read-only aggregate shape script 參考總量、活躍時段、來源族群與月份分佈，未寫入 raw URLs、titles、search terms、profile paths 或 secrets。
  - 驗證結果：targeted preview / showcase tests、`PATHKEEP_BROWSER_PREVIEW_DATASET=showcase bun run build`、Playwright static preview smoke（Dashboard / Explorer / Intelligence）與完整 `bun run check` 通過；`bun run check` 包含 100% JS/Rust coverage、browser-preview E2E、desktop-bridge truth gate 與 desktop-contract mutation gate。
