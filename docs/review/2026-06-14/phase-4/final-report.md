# PathKeep 全面審查報告 — 2026-06-14

## 執行摘要

PathKeep 的代碼庫健康度整體**良好**。在 ~292k 行代碼（183k TS + 109k Rust）的全面審查中，13 個審查子代理產出 144 項發現與 43 項表彰；5 個對抗性驗證子代理獨立驗證後，存活率為 **50%**（13 項確認 + 8 項 trade-off / 42 項驗證）。11 項被駁回為幻影問題，10 項被降級。

產品完成度高：Archive 和 Recall 兩大功能域完整可用，確定性 Intelligence 已上線，v0.3 deferred 的 AI 功能在 UI 上有清晰的誠實標示。Paper + Archival 美學在核心路由上執行到位，三語 i18n 覆蓋完整。

**最重要的 5 項確認發現：**

1. 🔴 **CRITICAL** — regex recall 路徑在 14.4M 記錄時會 OOM（全表載入記憶體後才過濾）
2. 🟠 **MAJOR** — 36 個 Core Intelligence 同步命令跑在 Tauri 主線程，會凍結 UI
3. 🟠 **MAJOR** — `load_visible_visits` 與 `visit_ids` Vec 無上限增長（intelligence 重建時）
4. 🟠 **MAJOR** — design-tokens.md 與 tokens.css 的顏色值/命名大面積不一致
5. 🟠 **MAJOR** — 「This Week」卡片用全量數據冒充週數據，違反 Trust & Transparency

---

## 審查與驗證統計

### 覆蓋範圍

| 維度 | 數量 |
|------|------|
| Focus Areas | 10 code + 3 cross-cutting |
| 審查子代理 | 13 |
| 驗證子代理 | 5 |
| 前端文件（審查） | ~380 / 685 |
| Rust 文件（審查） | ~200 / 255 |
| 子代理 token 消耗 | ~2.54M (Phase 1: 2.18M + Phase 3: 0.37M) |
| 總 tool 呼叫 | 888 (Phase 1: 654 + Phase 3: 234) |
| 總耗時 | ~12 min (Phase 1: 7.2 min + Phase 3: 4.6 min) |

### 發現存活率

| 階段 | 數量 |
|------|------|
| Phase 1 原始發現 | 144 |
| 送驗數量（critical + major + GAP） | 42 |
| ✅ 確認 | 13 (31%) |
| ⚖️ 確認但屬 trade-off | 8 (19%) |
| 🔄 降級 | 10 (24%) |
| ❌ 駁回 | 11 (26%) |
| **存活率** (確認 + trade-off) | **50%** |

### 駁回原因分布

| 誤判類型 | 數量 | 說明 |
|----------|------|------|
| PHANTOM（幻影問題） | 7 | 代碼實際正確，審查者誤讀邏輯或遺漏上下文 |
| FEATURE-FICTION（功能虛構） | 2 | 聲稱缺少的功能已知已追蹤，或有意 defer |
| TRADEOFF-BLIND + FEATURE-FICTION | 1 | 有意的平台策略被當缺陷 |
| PHANTOM (partial) | 1 | Safari 查詢實際使用了索引，非相關子查詢 |

---

## 產品完整度評估

### 使命對齊

| 功能域 | 完成度 | 評語 |
|--------|--------|------|
| **Archive** | ⭐⭐⭐⭐⭐ | 備份、加密、審計鏈、hash chain、rollback/revert 完整可用 |
| **Recall** | ⭐⭐⭐⭐ | Browse + Search + 高級語法 + FTS5 完備；detail panel 的 visit history sparkline 和 title versions 未接入（已驗證為 downgraded suggestion） |
| **Intelligence** | ⭐⭐⭐⭐ | 確定性洞察完整上線；AI/語義搜尋誠實標示為 v0.3 deferred |

### 核心原則落地

| 原則 | 評分 | 佐證 |
|------|------|------|
| **Trust & Transparency** | ⭐⭐⭐⭐ | PME 流程完整；「This Week」卡片是唯一被確認的誠實性問題 |
| **Data Sovereignty** | ⭐⭐⭐⭐⭐ | 零外部網路呼叫（除用戶主動的 og:image fetch）；全本地 |
| **Longevity** | ⭐⭐⭐⭐⭐ | SQLite + forward-only migration + snapshot + hash chain |
| **Intelligence Optional** | ⭐⭐⭐⭐⭐ | AI 完全可選，核心功能不受影響 |
| **Recoverability** | ⭐⭐⭐⭐ | Rollback 完整；ErrorBoundary 覆蓋不全是唯一缺口 |

---

## 已確認的關鍵發現

### 🔴 CRITICAL — 性能風險

#### R-PERF-001: Regex recall 路徑全表載入記憶體
- **檔案**: `vault-core/src/archive/history.rs:461-525`
- **問題**: regex 搜尋設定 `pageLimit = -1`，先 `.collect::<Vec<_>>()` 全部載入記憶體，再用 regex 過濾，最後取一頁。14.4M 記錄時會 OOM。
- **驗證**: 獨立確認 line 486 設定 `:pageLimit: -1i64`，line 492 collect 全表，lines 494-498 才過濾。
- **修復方向**: 分批串流 + regex 在 SQL 層（`REGEXP` 或 batch-scan with cap）。**已追蹤於 BACKLOG line 577**。

---

### 🟠 MAJOR — 性能風險

#### R-PERF-002: 36 個 Core Intelligence 同步命令跑在 Tauri 主線程
- **檔案**: `src-tauri/src/commands/intelligence/core.rs`
- **問題**: 所有 37 個命令定義為 `fn`（非 `async fn`），在 Tauri 2 中跑在主線程。同模組的 `runtime.rs` 已正確使用 `async fn` + `run_blocking_command`。
- **影響**: 每次 intelligence 讀取都會凍結 UI 直到查詢完成。

#### R-PERF-003: `visit_ids` Vec 無上限增長
- **檔案**: `vault-core/src/intelligence/intelligence_structural_aggregates.rs:51, 412`
- **問題**: `RefindAccumulatorEntry.visit_ids` 無限 push，熱門頁面可能累積數十萬 ID。Line 479 序列化為 JSON。
- **修復方向**: 上限 50 筆 + totalVisitCount。**已追蹤於 BACKLOG**。

#### R-PERF-004: `load_visible_visits` 全量載入後才截斷
- **檔案**: `vault-core/src/intelligence/intelligence_visit_records.rs:38-76`
- **問題**: SQL 無 LIMIT，line 63 collect 全表到 Vec，之後 `split_off` 只留末尾 N 條。
- **修復方向**: SQL 加 `ORDER BY ... DESC LIMIT ?2`，Rust 側 reverse。

---

### 🟠 MAJOR — 設計偏離

#### R-DESIGN-001: design-tokens.md 與 tokens.css 大面積不一致
- **檔案**: `src/styles/tokens.css:16-70` vs `docs/design/design-tokens.md`
- **問題**: 12+ 個 token 值不一致，命名方案也不同（`--bg-paper` / `--bg-card-paper` / `--bg-page`）。文檔明確聲明自己是 contract。
- **修復方向**: 逐一審計，統一到 tokens.css 或文檔（擇一為真）。

---

### 🟠 MAJOR — UX 品質

#### R-UX-001: ink-faint 和 ink-ghost token 不符合 WCAG AA
- **檔案**: `src/styles/tokens.css`
- **問題**: light mode ink-faint (#beb3a2) on bg-paper (#f6f3ed) 對比度 ~1.87:1，遠低於 AA 的 3:1（大字）/ 4.5:1（正文）。ink-ghost 更低。
- **影響**: metadata、placeholder、caption 等文字對比不足。
- **修復方向**: 調深 ink-faint / ink-ghost 值，確保大字至少 3:1。審計所有 ink-ghost 使用場景。

#### R-UX-002: 「This Week」卡片用全量數據冒充週數據
- **檔案**: `src/pages/dashboard/this-week-card.tsx:17-25`
- **問題**: 接收的 `totalPages` 和 `totalUrls` 是全量 archive 計數，但 UI 標題是「This week」。14.4M 全量記錄顯示在「本週」標題下，嚴重誤導。
- **修復方向**: (a) 最小誠實修復：改標題為「Archive overview」 (b) 正確做法：接入真正的週查詢。

---

### 🟠 MAJOR — Bug

#### R-BUG-001: AI job retry/cancel 靜默吞錯
- **檔案**: `src/pages/jobs/index.tsx:206-226`
- **問題**: `handleReplayAiJob` 和 `handleCancelAiJob` 有 `try/finally` 但無 `catch`。同檔的 runtime job handlers 有正確的 `catch`。
- **修復方向**: 加 catch，調用 `setPageError(describeError(e))`。

#### R-BUG-002: Takeout `stable_key_i64` hash 碰撞風險
- **檔案**: `browser-history-parser/src/takeout/browser_history.rs:471-480`
- **問題**: Java-style polynomial hash (乘 31)，先 hex-encode（加倍長度無意義），碰撞率在大量 takeout URL 下偏高。
- **修復方向**: 替換為 SipHash 或 FNV-1a。**已追蹤於 BACKLOG WORK-IMPORT-SCALE-TEST-A**。

---

### 🟠 MAJOR — GAP

#### R-GAP-001: 9 個路由缺少 ErrorBoundary
- **檔案**: `src/app/router.tsx:277-457`
- **缺失路由**: dashboard, assistant, import, audit, schedule, integrations, security, maintenance, settings
- **影響**: 任何未保護路由的 render error 會摧毀整個 shell。
- **修復方向**: 加 `ErrorBoundary: ShellRouteErrorBoundary`（已存在，低風險）。

---

### 🟡 MINOR — 已確認

| ID | 類別 | 發現 | 檔案 |
|----|------|------|------|
| R-GAP-002 | GAP | Detail panel firstVisitAt/lastVisitAt 永遠相同 | `paper-detail-panel-mount.tsx:75-76` |
| R-GAP-003 | GAP | Active Threads 卡片連結全指向 /intelligence 總覽 | `dashboard/index.tsx:191` |
| R-BUG-003 | BUG | security-step.tsx import 了 legacy backend 模組 | `onboarding/security-step.tsx:8` |
| R-DESIGN-002 | DESIGN-DRIFT | Assistant/Audit/Security 仍用 v0.2 legacy CSS class | `assistant/index.tsx`, `audit/index.tsx` |

---

## Trade-off 複審清單

以下 8 項確認為真實問題，但屬有意的工程權衡。建議在下一里程碑重新評估。

| 原始嚴重度 → 最終 | 發現 | 原始權衡理由 |
|---|---|---|
| major → minor | Topbar 缺通知佇列按鈕 | Paper 重設計有意精簡 topbar；docs/CHANGELOG 記錄了 v0.2→v0.3 的 topbar 簡化 |
| major → minor | Profile scope 在 status bar | Paper redesign 有意將 ambient telemetry 移到底欄 |
| major → minor | Intelligence 快取無 eviction | 實際使用中 scope 切換不頻繁；加 LRU 是改進但非必要 |
| minor → suggestion | This Week 卡片無 drill-down | 設計意圖是提示性摘要，非深度分析入口 |
| major → minor | schedule/index.tsx 1308 行 | 已追蹤於 BACKLOG WORK-SCHEDULE-PAGE-MAINT-A |
| major → minor | visit_event_fingerprint Chrome epoch | 已追蹤，fingerprint 只用於去重不影響 UI 時間顯示 |
| minor → minor | url_last_visit_marker mixed units | 已文檔化於 import-dedup-audit.md |
| major → suggestion | archive_flows.rs 1845 行 | 已追蹤於 BACKLOG WORK-BACKEND-ARCH-FLOWS-SPLIT-A |

---

## 已駁回發現摘要

| 誤判類型 | 發現 | 駁回理由 |
|----------|------|----------|
| PHANTOM | Regex 驗證誤拒 named groups | 測試明確 assert 此行為；是有意的保守策略 |
| PHANTOM | Status bar popover 缺 ARIA | 實際有完整 `role="listbox"` / `aria-label` / `aria-selected` / 鍵盤導航 |
| PHANTOM | Explorer O(N) re-aggregation | 審查者計算的 25k 項是不可能的；cap 是 500 頁 × ~50 項；hydration 是增量的 |
| PHANTOM | Intelligence stale cache | 實際有 `stale-while-revalidate` + `force=true` 背景刷新；scope key 包含 profileId |
| PHANTOM | overviewCache 無 eviction（XA-PERF） | 與 VB-2 中的同一發現重複；驗證者確認實際記憶體影響有限 |
| PHANTOM | Import 確認按鈕未防並發 | shell-tasks.ts 的 `beginArchiveTask` 已在 shell 層強制 single-writer |
| FEATURE-FICTION | Schedule 頁 1308 行（XA-PRODUCT） | 與 FA-SETTINGS 重複且已追蹤於 BACKLOG |
| PHANTOM | Safari URL 查詢使用相關子查詢 | 實際用的是 `MAX(hv.id)` aggregate，SQLite 走 index scan 而非 full scan |
| PHANTOM | Firefox `SELECT *` 加 `format!` table name | table name 來自硬編碼 enum，非用戶輸入；`SELECT *` 用於 schema 探測 |
| FEAT-FICTION + TRADE | Linux scheduler 無狀態檢測 | 有意的 MVP 策略，已記錄於 repo-baseline.md |
| PHANTOM | worker_args[1..] panic | 呼叫者 `scheduler::install` 保證至少 2 個元素 |

---

## 系統性模式與項目級建議

### 1. Rust 命令線程模式不一致
**模式**: intelligence/core.rs 的 36 個命令是 sync，同模組的 runtime.rs 是 async + spawn_blocking。archive.rs 是 async。
**建議**: 統一所有非瑣碎查詢命令為 `async fn` + `run_blocking_command`。

### 2. 文檔與代碼的 token 合約偏移
**模式**: design-tokens.md 聲明自己是 contract，但 tokens.css 已進化。screens-and-nav.md 的 topbar/notification 描述也已過時。
**建議**: 做一次文檔掃描，把 paper redesign 後的實際狀態回寫到設計文檔。

### 3. 前端 i18n 字串匹配反模式
**模式**: app-lock-section.tsx、retention-section.tsx、import/index.tsx、onboarding/index.tsx 都用 `=== '完整英文句子'` 比對後端訊息來決定是否本地化。
**建議**: (a) 後端改發 typed error/warning code (b) 前端統一用 `localizeBackendMessage()` helper（security/helpers.ts 已有正確範本）。

### 4. 無上限記憶體增長（多處）
**模式**: `visit_ids` Vec、`load_visible_visits` 全量載入、regex recall 全表 collect、intelligence caches 無 eviction。
**建議**: 在 BACKLOG 中建立 `WORK-PERF-BOUNDED-MEMORY-SWEEP-A`，統一掃描所有無上限集合。

### 5. Legacy CSS class 殘留
**模式**: Assistant、Audit、Security 頁面仍用 v0.2 brutalist CSS class（page-shell、panel、btn-secondary）。
**建議**: 已追蹤於 BACKLOG WORK-V03-PAPER-REMAINING-ROUTES 系列。

---

## UX 一致性地圖

| 路由 | Paper 美學 | 狀態完整度 | 備註 |
|------|-----------|-----------|------|
| `/` Dashboard | ✅ 完整 | ✅ | This Week 卡片數據誤導需修 |
| `/explorer` Browse | ✅ 完整 | ✅ | viewport 虛擬化、sticky header 均正常 |
| `/search` | ✅ 完整 | ✅ | 3-mode toggle + day-grouped results |
| `/intelligence` | ✅ 完整 | ✅ | staged loading 優秀 |
| `/intelligence/*` entity routes | ✅ 完整 | ✅ | |
| `/assistant` | ⚠️ v0.2 CSS | ✅ | v0.3 deferred 標示清晰 |
| `/import` | ⚠️ 部分 | ✅ | paper panel 在 `?layout=paper` 後面未啟用 |
| `/jobs` | ✅ 基本 | ✅ | |
| `/audit` | ⚠️ v0.2 CSS | ✅ | |
| `/schedule` | ✅ 基本 | ✅ | 1308 行需拆分 |
| `/security` | ⚠️ v0.2 CSS | ⚠️ | busy button 反饋不足；keyring 按鈕在 plaintext 時多餘 |
| `/settings` | ✅ 部分 | ✅ | paper header 已套用 |
| `/onboarding` | ⚠️ v0.2 shell | ✅ | 步驟內容已重做，外殼未遷移 |

---

## 值得肯定的設計

### 架構

1. **Shell 分解與職責分離** — `shell.tsx` / `shell-data.tsx` / `shell-data-actions.ts` / `shell-tasks.ts` 的四檔分解，每個檔案都有 doc header 標示 Responsibilities / Not Responsible For。
2. **Explorer 分層架構** — URL state → data fetching → infinite scroll → favicon/og-image hydration → presentation，每層獨立可測。
3. **Staged intelligence overview** — 三階段載入（warm cache → primary batch → idle secondary），直接解決了 IPC fan-out 的性能問題。

### 性能

4. **Cursor-based pagination** — `HistoryCursor` enum 支援 Chronological 和 Relevance 兩種 keyset 分頁，O(1) 頁面存取。
5. **Viewport-driven day recycling** — IntersectionObserver 驅動的日區塊回收，解決了 71k DOM 節點問題。
6. **Migration 串流** — 64 KiB buffer + per-file SHA-256 串流驗證，記憶體使用恆定。

### 產品

7. **v0.3 deferred 的誠實處理** — AI/語義搜尋在 UI 上有清晰的三語「Coming in v0.3」標示，非空洞 disabled。
8. **App Lock 路由邊界** — RequireLockScreen 和 RequireUnlockedShell 互斥，安全邊界清晰。
9. **PME 流程完整** — Security rekey、Data Migration import 的 Preview → Confirm → Execute 流程忠實執行。

### 品質

10. **三字體系統 + CJK 降級** — Newsreader / system sans / JetBrains Mono 三套字體，`:root:lang()` 為 zh-CN / zh-TW 配置了正確的 CJK fallback。
11. **Paper 材質層** — SVG fractalNoise 2.8% 透明度紙感 + CSS radial-gradient 暗房 vignette，`prefers-reduced-motion` 全面處理。
12. **信任流程測試** — import-flows.test.tsx 的 1324 行深度測試，覆蓋 zh-CN 翻譯語境下的完整導入旅程。

---

## 文檔反饋

| 文檔 | 問題 | 建議 |
|------|------|------|
| `design-tokens.md` | 12+ token 值與 tokens.css 不一致 | 全面審計後統一 |
| `screens-and-nav.md` | topbar 通知按鈕和 profile switcher 位置已過時 | 反映 paper redesign 後的實際 layout |
| `recall.md` | detail panel 的 visit sparkline 和 title versions 描述了未接入的功能 | 標記為「future enhancement」或接入 |

---

## 附錄：未驗證發現統計

Phase 1 的 72 項 minor 和 38 項 suggestion 中，未送入 Phase 3 驗證的共 102 項。按類別分布：

| 類別 | 數量 |
|------|------|
| PERF-RISK | 20 |
| UX-QUALITY | 15 |
| TECH-DEBT | 13 |
| ARCH-SMELL | 12 |
| UX-INCOMPLETE | 11 |
| DESIGN-DRIFT | 8 |
| EVOLUTION | 7 |
| BUG | 6 |
| I18N | 4 |
| SECURITY | 2 |
| GAP | 4 |

完整的未驗證發現列表保存在 `docs/review/2026-06-14/phase-1/all-reports.json`。

---

_Report generated by the PathKeep Full Review Pipeline (docs/plan/program/review-pipeline.md)_
_Phase 1: 13 agents × 7.2 min | Phase 2: inline triage | Phase 3: 5 agents × 4.6 min | Phase 4: synthesis_
