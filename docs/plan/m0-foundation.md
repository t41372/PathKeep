# M0 — 重構基礎

> 從現有代碼庫過渡到新架構的必要準備工作。  
> **前置條件**：無  
> **產出**：乾淨的代碼骨架，新舊不再混雜，可以在新架構上開始 M1 工作。

---

## M0.1 — 前端清理：舊 UI 全部移除

> 現有前端是 Codex 產出的「一坨奇怪的東西」，UX 設計詭異。全部打掉，照設計師新版來。

- [ ] **M0.1.1** 刪除所有舊版前端頁面文件
  - [ ] 刪除 `src/pages/dashboard.tsx`（舊版 dashboard）
  - [ ] 刪除 `src/pages/explorer.tsx`（舊版 explorer）
  - [ ] 刪除 `src/pages/insights.tsx`（舊版 insights）
  - [ ] 刪除 `src/pages/activity-log.tsx`（舊版 activity log）
  - [ ] 刪除 `src/pages/import.tsx`（舊版 import）
  - [ ] 刪除 `src/pages/onboarding.tsx`（舊版 onboarding）
  - [ ] 刪除 `src/pages/settings/` 整個目錄（舊版 settings 套件）
- [ ] **M0.1.2** 刪除舊版組件
  - [ ] 刪除 `src/components/sidebar.tsx`（舊版 sidebar）
  - [ ] 刪除 `src/components/ui.tsx`（舊版 UI primitives）
  - [ ] 刪除 `src/components/ai-provider-editor.tsx`（舊版 AI 配置）
- [ ] **M0.1.3** 清理舊版樣式和 Shell
  - [ ] 刪除 `src/App.css`（舊版全局 CSS — 32KB 的怪物）
  - [ ] 刪除 `src/AppNew.tsx`（舊版 App shell）
  - [ ] 刪除 `src/AppNew.test.tsx`（舊版 App 測試 — 102KB）
  - [ ] 刪除 `src/App.helpers.test.tsx`
- [ ] **M0.1.4** 保留可復用的前端基礎設施
  - [ ] 保留 `src/lib/backend.ts`（Tauri IPC 調用層 — 需後續重構但暫時保留）
  - [ ] 保留 `src/lib/types.d.ts`（TypeScript 類型 — 需後續對齊但暫時保留）
  - [ ] 保留 `src/lib/i18n.ts`（國際化 — 需後續重構但暫時保留）
  - [ ] 保留 `src/lib/browser-icons.tsx`（瀏覽器圖標 — 可直接復用）
  - [ ] 保留 `src/lib/format.ts`（格式化工具 — 可直接復用）
  - [ ] 保留 `src/lib/stronghold.ts`（安全存儲 — 可直接復用）
  - [ ] 保留 `src/lib/app-context.tsx`（需大幅重構，但暫時保留作為參考）

---

## M0.2 — 建立新前端骨架

> 按照設計師的 prototype 建立全新的前端架構。

- [ ] **M0.2.1** 建立新的設計系統（Design System）
  - [ ] 建立 `src/styles/` 目錄結構
  - [ ] 建立 `src/styles/tokens.css` — 設計 tokens（顏色、字體、間距、圓角）
    - [ ] 從設計稿提取色彩系統：`--bg-base`, `--bg-elevated`, `--accent`（#FF7832 橙色系）, `--text-primary`, `--text-muted`, `--text-faint`, `--border`
    - [ ] 字體系統：Inter（UI 正文）+ JetBrains Mono（代碼/數據）
    - [ ] 間距系統：`--space-1` 到 `--space-8`
    - [ ] 暗色模式為主、淺色模式為輔
  - [ ] 建立 `src/styles/base.css` — 全局 reset 與基礎樣式
  - [ ] 建立 `src/styles/components.css` — 可復用組件樣式
    - [ ] Panel / Card 組件樣式
    - [ ] Button 系列（primary, secondary, danger, btn-tiny）
    - [ ] Table / Data table 樣式
    - [ ] Tag / Badge / Status badge 系列
    - [ ] Form elements（input, select, checkbox）
    - [ ] Toast / notification 樣式
    - [ ] Progress bar 樣式
    - [ ] Code block 樣式
  - [ ] 建立 `src/styles/layout.css` — App shell, sidebar, topbar 布局
- [ ] **M0.2.2** 建立新的 App Shell
  - [ ] 建立 `src/App.tsx` — 新的 App 入口
  - [ ] 建立 `src/components/AppShell.tsx` — App 外殼（sidebar + main content）
  - [ ] 建立 `src/components/Sidebar.tsx` — 側邊欄（按設計稿：CORE + OPERATIONS + SYSTEM 三個 section）
  - [ ] 建立 `src/components/Topbar.tsx` — 頂部欄（page title + global search + backup button）
  - [ ] 實現路由系統（使用 react-router-dom）
- [ ] **M0.2.3** 建立 placeholder 頁面
  - [ ] `src/pages/DashboardPage.tsx` — Dashboard 頁面骨架
  - [ ] `src/pages/ExplorerPage.tsx` — Explorer 頁面骨架
  - [ ] `src/pages/InsightsPage.tsx` — Insights 頁面骨架
  - [ ] `src/pages/AssistantPage.tsx` — AI Assistant 頁面骨架
  - [ ] `src/pages/ImportPage.tsx` — Import 頁面骨架
  - [ ] `src/pages/AuditPage.tsx` — Audit Ledger 頁面骨架
  - [ ] `src/pages/SchedulePage.tsx` — Schedule 頁面骨架
  - [ ] `src/pages/SecurityPage.tsx` — Security 頁面骨架
  - [ ] `src/pages/SettingsPage.tsx` — Settings 頁面骨架
  - [ ] `src/pages/OnboardingPage.tsx` — Onboarding 頁面骨架
- [ ] **M0.2.4** 建立測試基礎
  - [ ] 為 App Shell 寫測試
  - [ ] 為 Sidebar 寫測試
  - [ ] 為 Topbar 寫測試
  - [ ] 驗證路由切換
  - [ ] 100% test coverage

---

## M0.3 — Rust 後端重組

> 重組 Rust workspace 邊界，為新需求做準備。

- [ ] **M0.3.1** 建立 `browser-history-parser` 獨立 crate
  - [ ] 在 `src-tauri/crates/` 下建立 `browser-history-parser/` crate
  - [ ] 把 `vault-core/src/chrome.rs` 中的 **profile discovery** 邏輯移入
  - [ ] 把 **Chromium History DB 解析**邏輯移入
  - [ ] 定義 public API trait：`BrowserDiscovery`, `HistoryParser`
  - [ ] 定義解析輸出的 data types（不依賴 archive schema）
  - [ ] 保留 vault-core 作為 consumer，依賴 `browser-history-parser`
  - [ ] 確保所有既有測試在重組後通過
  - [ ] 100% test coverage + mutation test
- [ ] **M0.3.2** 審查和清理 vault-core 模塊
  - [ ] 審查 `archive.rs`（93KB — 太大了，需要拆分）
    - [ ] 拆分為 `archive/mod.rs`, `archive/backup.rs`, `archive/query.rs`, `archive/export.rs`, `archive/snapshot.rs`
  - [ ] 審查 `ai.rs`（72KB — 太大了，需要拆分）
    - [ ] 拆分為 `ai/mod.rs`, `ai/provider.rs`, `ai/embedding.rs`, `ai/search.rs`, `ai/assistant.rs`
  - [ ] 審查 `insights.rs`（90KB — 太大了，需要拆分）
    - [ ] 拆分為 `insights/mod.rs`, `insights/engine.rs`, `insights/cards.rs`, `insights/topics.rs`, `insights/threads.rs`
  - [ ] 審查 `takeout.rs`（52KB — 需要到 M2 時重構）
  - [ ] 審查 `chrome.rs`（47KB — 大部分應該移到 browser-history-parser）
  - [ ] 每次拆分都要確保測試通過且覆蓋率不下降
- [ ] **M0.3.3** Schema 審查與規劃
  - [ ] 對比現有 `archive-schema.sql` 與新需求，列出 gap：
    - [ ] 缺少 `schema_migrations` 表
    - [ ] 缺少 FTS5 虛擬表
    - [ ] `backup_runs` 缺少 run type 區分（backup vs import vs revert）
    - [ ] 缺少軟刪除相關欄位（`reverted_at`, `reverted_by_run_id`）
    - [ ] 缺少 enrichment 相關表
    - [ ] 缺少聚合統計表（`daily_visit_counts` 等）
    - [ ] 時間欄位需要統一為 Unix epoch 毫秒（目前混用 TEXT 和 INTEGER）
    - [ ] 缺少時區記錄（run metadata 中的 timezone）
    - [ ] `visit_events` 缺少 ISO 8601 輔助時間欄位
  - [ ] 設計 migration 系統方案
  - [ ] 撰寫 migration 0001 的 SQL

---

## M0.4 — 更新產品名稱和配置

> 統一把所有「Browser History Backup」改為「PathKeep」。

- [ ] **M0.4.1** Rust 側更新
  - [ ] 更新 `src-tauri/Cargo.toml` 中的 package name 和 description
  - [ ] 更新 `src-tauri/src/lib.rs` 中的 `PRODUCT_DISPLAY_NAME`
  - [ ] 更新 `src-tauri/tauri.conf.json` 中的 app 名稱
- [ ] **M0.4.2** 前端側更新
  - [ ] 更新 `package.json` 中的 name
  - [ ] 更新 `index.html` 中的 title
  - [ ] 更新 `README.md`
- [ ] **M0.4.3** CI/CD 更新
  - [ ] 更新 GitHub Actions workflow 中的相關名稱
  - [ ] 更新 `.pre-commit-config.yaml` 如有需要

---

## M0.5 — 確認基礎設施可用

- [ ] **M0.5.1** 驗證構建
  - [ ] `bun run check` 通過
  - [ ] `bun run build` 成功
  - [ ] `bun run desktop:build:debug` 成功
  - [ ] `bun run coverage:js` 達標（100%）
  - [ ] `bun run coverage:rust` 達標
- [ ] **M0.5.2** 提交原子化 commits
  - [ ] 每個 M0.x 段落對應 1-3 個 conventional commit
  - [ ] 確保 commit history clean
