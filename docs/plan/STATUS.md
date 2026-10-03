# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block。
> 2026-10 之前已完成的 blocks 見 `CHANGELOG.md`，或 git 歷史裡舊版的本檔（commit `e1aba87a`）。

**當前 Milestone：M18 — Frontend redesign（Claude Design prototype → shadcn）**

---

## CURRENT FOCUS

- [ ] **WORK-FRONTEND-REDESIGN-2026-10** — 用新 prototype 整個替換前端（用戶 2026-10-02 指示）
  - 分支：`work/pathkeep`（從 `t41372/PathKeep` main 帶歷史導入，commit `e1aba87a`）。每完成一步就 commit + push，環境隨時可能被銷毀。
  - 讀先：
    `docs/design/prototype-2026-10/`（prototype HTML + 截圖；用 `python3 -m http.server` 開）
    `AGENTS.md`（新的測試規則：E2E 為主、不跑 mutation、不強制 coverage）
    `docs/architecture/desktop-command-surface.md`
    `docs/plan/program/quality-matrix.md`
  - 用戶要求：
    - 新前端用 shadcn，盡量用 pre-built components；charts 用 evilcharts（`npx shadcn add @evilcharts/...`，Recharts）；heatmap 若前者沒有，用 amicro mono-charts，視覺風格要一致。
    - prototype 缺的東西順手補上；後端 dead code 順手刪；IPC / 卡頓問題看到就修或記錄。
    - 測試規則改了：刪掉全部 mutation testing、不再強制 100% coverage、E2E 為主並且每次 E2E 產出可驗證、可重現的 artifact。
  - 步驟（做完一項就勾，並 commit）：
    - [x] 0. 建分支、導入 PathKeep（含歷史）、push；prototype 存進 `docs/design/prototype-2026-10/`
    - [x] 1. 改 gate：刪 mutation / coverage gate，AGENTS.md 加新規則，quality-matrix / TESTING 改寫，E2E artifact 設定
    - [ ] 2. 後端命令面 + IPC 性能問題盤點，寫進 `docs/architecture/`（findings 與修復記錄）
    - [x] 3. 新前端地基：design tokens（prototype 的 CSS 變數 → Tailwind theme）、窗口外殼 + 左側 nav rail、i18n（en / zh-CN / zh-TW）、主題切換、shadcn 組件、數據層
    - [ ] 4. 頁面：Home、History、Insights、Ask、Backup、Settings、Onboarding、Lock、Command palette
    - [ ] 5. 刪舊前端（舊 pages/components/preview fixtures）和沒人用的後端命令
    - [ ] 6. E2E：在 desktop bridge（真 Rust 後端）上跑中高難度場景，產出 artifact
    - [ ] 7. 文檔同步（features / design / architecture）、CHANGELOG、`bun run check` 全綠
  - 恢復提示（環境被銷毀後）：
    - Rust：`curl https://sh.rustup.rs | sh -s -- -y`，toolchain 由 `rust-toolchain.toml` 決定。
    - Linux 依賴：見 `DEVELOPMENT.md`（webkit2gtk-4.1 等）+ `xvfb`。
    - `bun install`；`npx playwright install chromium`。
  - 進度記錄：
    - 2026-10-02：步驟 0、1 完成。步驟 3 進行中：舊 UI（src/app、src/pages、src/components、src/styles、browser-preview mock、舊 unit tests）已刪；新地基在 `src/app/`（session 狀態機、backup runner、shell）、`src/lib/i18n/`（typed catalog，每個 feature 一個檔案）、`src/lib/query.ts`（TanStack Query）、`src/index.css`（prototype tokens → shadcn 變數）。各頁面目前是 placeholder（`src/features/*/`）。
    - 後端命令面與 IPC 性能盤點的結論（待寫入 docs/architecture）：每次開 archive 都對 search DB 做一次寫交易 + `COUNT(*)`（search DB 無 WAL）；每個命令都重讀 config 並查 keychain；`query_history` 每頁跑一次精確 `COUNT(*)`；舊前端每 15 秒輪詢兩個 intelligence 命令；`app_snapshot` 一次開約 5 次 DB。

- [ ] **WORK-REFACTOR-INDEXING**（用戶 2026-06-21 指示，承巨檔審計）— `ai/indexing.rs` 拆分重構。詳見舊版 STATUS（commit `e1aba87a`）。_非當前 focus_

> `WORK-V03-PAPER-REDESIGN-A`（Paper 風格前端重建）被本次 redesign 取代，不再繼續。

## 進度快照（2026-10-03）

- 已完成並推送：地基、Home、新 logo（Summit P，`docs/design/brand-icon/`，`BrandMark` 依主題換圖）、`bun run dev:demo`（真後端 + 合成 Chrome×2 / Firefox archive）。
- 新後端命令的前端契約已寫好、Rust 尚未實作：`load_source_stats`（`src/lib/backend-client/sources.ts`）、`get_url_detail`（`url-detail.ts`）、`preview_wipe_all_data` / `wipe_all_data`（`data-wipe.ts`）、`query_history` 的 `includeTotal`、digest 的 `distinctDomains` / `activeTimeMs`。
- 平行進行中（若環境被銷毀，按此重新分派）：
  - 後端：上述命令 + IPC 性能修復（每次開 archive 寫 search DB、每命令重讀 config/keychain、`app_snapshot` fan-out）+ dev bridge SSE 事件 + 刪死命令（`ask_ai_assistant`、`load_ai_assistant_job`、`download_ai_embedding_model`）。
  - History 頁（`src/features/history/`）。
  - Backup + Settings 頁（`src/features/backup/`、`src/features/settings/`）。
  - Ask + AI 設定 + Onboarding（`src/features/ask/`、`settings/ai-section.tsx`、`src/features/onboarding/`）。
  - 主 agent：Insights 頁、i18n gate（`scripts/i18n-progress.ts`）改寫成新 catalog、E2E。
