# Core Intelligence — Ultimate Design

> **狀態：Accepted (2026-04-15 Core Intelligence reset baseline)**
> **日期：2026-04-15**
> **取代：** `deterministic-intelligence.md`（M5 baseline）、`intelligence.md` 中殘留的 deterministic/insights baseline 敘述、Codex 現有 insights 實現
> **前提：** 本方案不使用 LLM 和 Embedding。所有功能只基於可觀測的瀏覽器數據。

> **2026-04-15 accepted note:** 這份文檔現在是 PathKeep Core Intelligence 的正式 source of truth。舊的 deterministic intelligence / insights 文檔保留為歷史背景與 optional-AI 邊界說明，不再定義 deterministic product contract。
>
> **2026-04-19 accepted override:** `Browsing Rhythm` 主圖不再沿用本文早期版本裡的「週 × 小時」首屏契約。使用者已明確確認改成 **真實日期日曆熱力圖**，並把小時分布退到「選中某一天後的 detail 區」。完整 trade-off 與回滾邊界見 [`../design/intelligence-rhythm-calendar-heatmap-tradeoff.md`](../design/intelligence-rhythm-calendar-heatmap-tradeoff.md)。
>
> **2026-04-19 accepted entity note:** `day` 與 `domain` 現在都已升格成 first-class shared entity surface。`/intelligence/day/:date` 是 exact local day 的完整 insights route；`/intelligence/domain/:domain` 正式作為 `Domain Insights` module。day/domain-facing surface 預設採 `Insights first`，Explorer evidence 降為 secondary CTA；目前唯一的 UI 特例是 `/intelligence` overview 與 Dashboard 的 `Browsing Rhythm` 卡片，點日格時會先打開 inline preview，再由明確 CTA 進入 shared day route。完整 trade-off 見 [`../design/intelligence-entity-route-tradeoff.md`](../design/intelligence-entity-route-tradeoff.md)。
>
> **2026-04-19 accepted generic-entity note:** M7 已把 generic insight-entity navigation 收斂成正式 contract：`query family`、`refind page`、`session`、`trail` 也已升格成 first-class shared insights route，其餘 active entity 則必須解析到既有 shared destination，而不是各 surface 各自決定 deep-link。Explorer 的 `session` / `trail` grouped view 仍是 browse-first canonical surface；route promotion 只承接 reusable detail / explainability / evidence CTA。完整 trade-off 見 [`../design/intelligence-generic-entity-navigation-tradeoff.md`](../design/intelligence-generic-entity-navigation-tradeoff.md)。
>
> **2026-04-20 accepted search-browser note:** `Search Activity` 的第四個 tab 現在正式收斂成 `Search Keywords` browser，不再叫 `Recent Queries`。它仍然讀 `get_search_queries`、仍維持 distinct `(search_engine, normalized_query)` row 與 shared `query-family` / `trail` / evidence CTA grammar，但 UI 已升格成 bounded paged browser，而不是 additive load-more card list。`Top Search Concepts` 也不再用詞雲；accepted 規格改為 horizontal bar chart，且 concept/keyword-facing surface 只允許吃 keyword-eligible search rows，不再把 URL-like / navigational noise 混進排名。完整 trade-off 見 [`../design/search-activity-keyword-browser-tradeoff.md`](../design/search-activity-keyword-browser-tradeoff.md)。
>
> **2026-04-19 accepted M8 note:** M8 已把 aggregate entity identity / context reuse 收口成正式 contract：`compare set` 升格成 `/intelligence/compare-set/:compareSetId` first-class route；shared non-overview insights routes additive 支援受限的 `focusType` / `focusId`；`path flow` 改成 stable `flowId` + typed `steps`；trusted external outputs 也改帶 structured entity targets，而 `public snapshot` 維持 redacted。完整 trade-off 見 [`../design/intelligence-aggregate-entity-focus-tradeoff.md`](../design/intelligence-aggregate-entity-focus-tradeoff.md)。
>
> **2026-04-19 accepted M9 note:** M9 已正式把 shared route composition 收斂成 accepted contract：route-level metric strip、`query-family-card`、compare-set page list、structured target label，以及 section heading + evidence/freshness badge 現在都屬 single-source front-end primitive；這一輪刻意不把 scope 擴成 backend transport refactor。完整 trade-off 見 [`../design/intelligence-shared-route-composition-tradeoff.md`](../design/intelligence-shared-route-composition-tradeoff.md)。
>
> **2026-04-20 accepted performance note:** `/intelligence` overview 的 request-path contract 進一步收緊：同一批 primary / secondary overview 讀取，backend 只允許重用一條 intelligence connection / attached archive 與一份 runtime snapshot；frontend same-scope revisit 必須優先走 warm cache + in-flight dedupe + background revalidate，而不是重新把整頁打回 cold skeleton。`Search Activity` 的 hidden tabs 也必須在首屏穩定後自動 prewarm，而不是等第一次點 tab 才開始載入。

---

## 0. 讀這裡先

項目內所有涉及數據分析的 intelligence 功能分成兩類:

1. Core Intelligence (核心智能): 之前叫 `deterministic intelligence`，是不啟用 LLM 和 embedding 模型下提供的基礎 intelligence 能力。
2. Advanced Intelligence (進階智能, 簡稱 AI): 啟用了 LLM 和/或 embedding 之後，基於這兩個技術做的功能。

由於數據分析功能對於我們的性能 baseline 來說，計算量較大，我們需要一個任務隊列來慢慢處理大量的計算任務，並且可以處理中斷和程序關閉的情況。

### 0.1 這份文件是什麼

整合了五套方案——原始草稿、Claude Sonnet 4.6、Opus 4.6、Gemini 2.5/3.1、ChatGPT Pro——的所有有價值想法，經過 feasibility 分析、去重合併、與現有代碼庫對齊後，形成的終極設計。

### 0.2 所有方案都同意的三件事

1. **後台任務隊列是底線。** Intelligence 計算不能阻塞 UI。
2. **物化視圖策略是效能命脈。** UI 永遠只讀取預計算表，不掃描千萬級原始 visits。
3. **Canonical 數據模型必須在第一天建立。** 所有瀏覽器的數據在導入時就標準化。

### 0.3 本方案明確迴避的陷阱

- **不推算停留時間**：除非瀏覽器直接報告（Chromium `total_foreground_duration`、Firefox `total_view_time`），否則不計算、不顯示。
- **不做無錨點 topic clustering**：沒有 embedding 的全局 topic merge 又貴又假準。
- **不用抽象黑盒命名**：UI 文案說人話，不出現 `Query Ladder`、`Thread`、`Open Loop` 等術語。
- **不為任意時間窗口物化完整結果**：event-level derived facts + daily rollup + on-demand window compose。

### 0.4 性能 Baseline 確認

| 指標                   | 值                                                 |
| ---------------------- | -------------------------------------------------- |
| 目標硬件               | 4 核 3GHz CPU / 8GB RAM                            |
| 60 年數據量            | ≥ 1440 萬條 visits                                 |
| 每月常規增長           | ~2 萬條（中度用戶）                                |
| 首次導入峰值           | Firefox 10 年 / Chrome 18 月 / 1440 萬條一次性導入 |
| `visits` 表估算        | ~3GB（含索引）                                     |
| `urls` 表估算          | ~200MB                                             |
| `insights_*` rollup 表 | ~500MB                                             |
| **總磁碟上限**         | **~5–8GB**                                         |
| UI 查詢目標            | < 50ms                                             |

### 0.5 與現有代碼庫的關係

現有代碼庫中 `derived/history-intelligence.sqlite` 的 insights 表（`insight_bursts`、`insight_query_groups`、`insight_threads` 等）和 `insights.rs` 的實現**將被重新設計**。

**保留不動的部分：**

- `archive/history-vault.sqlite` 的 canonical schema（`runs`、`source_profiles`、`urls`、`visits`、`downloads`、`search_terms`、`favicons`）
- `archive/source-evidence.sqlite` 的 typed evidence tables（`visit_search_evidence`、`visit_navigation_evidence`、`visit_engagement_evidence`、`visit_context_evidence`、`native_entities`）
- `derived/history-search.sqlite` 的 FTS projection
- `intelligence_jobs` 任務隊列基礎設施
- `deterministic_module_runtime` 模塊註冊機制

**將被替換的部分：**

- `visit_insight_features` 的 schema 和計算邏輯
- `insight_bursts` → 重命名為 `sessions`
- `insight_query_groups` → 重新設計為 `search_trails`
- `insight_threads` → 重新設計為 `reopened_investigations`
- `insight_reference_pages` → 重新設計為 `refind_pages`
- `insight_source_effectiveness` → 保留但精簡
- `insight_cards` / `insight_snapshot_payloads` → 重新設計 rollup 策略

---

## 1. 架構基礎

### 1.A 與現有 Storage Planes 的對齊

本方案的所有 derived tables 全部落在 `derived/history-intelligence.sqlite`，符合現有架構的 storage-plane 劃分：

```
archive/history-vault.sqlite        ← canonical facts（不可改）
archive/source-evidence.sqlite      ← cold source-native evidence（不可改）
derived/history-search.sqlite       ← FTS projection（可重建）
derived/history-intelligence.sqlite ← Core Intelligence 所有產出（可重建）
```

### 1.B 增量 Rollup 策略

```
                                  ┌──────────────────────────┐
                                  │   canonical visits/urls   │
                                  │   (history-vault.sqlite)  │
                                  └──────────┬───────────────┘
                                             │
               ┌────── 可讀取 ───────────────┤
               │                             │
               ▼                             ▼
 ┌─────────────────────────┐   ┌──────────────────────────────┐
 │  source-evidence.sqlite │   │  intelligence worker         │
 │  (cold, on-demand read) │   │  (background queue)          │
 └─────────────────────────┘   └──────────┬───────────────────┘
                                          │
                        ┌─────────────────┼─────────────────┐
                        ▼                 ▼                 ▼
              ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
              │ visit-level  │  │ daily rollups │  │  structural  │
              │ derived facts│  │              │  │  entities    │
              │ (is_new_dom, │  │ (site, cat,  │  │ (sessions,   │
              │  session_id, │  │  engine,     │  │  trails,     │
              │  trail_id)   │  │  search)     │  │  refind...)  │
              └──────────────┘  └──────────────┘  └──────────────┘
                        │                 │                 │
                        └─────────────────┼─────────────────┘
                                          ▼
                                  ┌──────────────┐
                                  │   UI 讀取層   │
                                  │ (合成任意窗口) │
                                  └──────────────┘
```

**只有兩種東西會被物化：**

1. **Visit-level derived facts**（如 `is_new_domain`、`session_id`、`trail_id`）— 在 visit 導入時一次性計算
2. **Daily rollups** — 後台任務掃描當天新增紀錄，計算各維度日統計

週/月/季/年/自定義窗口**全部由 daily rollups 即時合成**。

> **2026-04-17 implementation note:** repo 現在已落地 `core_intelligence_stage_checkpoints` per `(profile_id, stage)` checkpoint ledger，保存 `stage_version`、visibility-aware source watermark、`last_processed_visit_id`、`dirty_from_visit_ms` / `dirty_date_key`、`last_run_id`、`fallback_reason`、`updated_at`。append-only `visit_derive`、`daily_rollup`、`structural_rebuild` 現在會優先走 checkpoint-backed incremental path；若遇到 visibility regression、rule/version drift、缺失 checkpoint 或 manual full rebuild，則誠實回退成 scoped `fallback-full`。

### 1.C 任務隊列與計算調度

繼續使用現有的 `intelligence_jobs` 表和 lease/heartbeat 機制。任務類型擴展為：

| 任務類型             | 觸發時機                      | 優先級 |
| -------------------- | ----------------------------- | ------ |
| `visit_derive`       | 新 visit 導入                 | 中     |
| `daily_rollup`       | 每日 / 新 visit 觸發          | 中     |
| `structural_rebuild` | session/trail/refind 需要重建 | 低     |
| `full_rebuild`       | schema 升級 / 用戶手動        | 低     |

> **2026-04-18 implementation note:** runtime / Jobs artifact 現在會補充 `executionMode`、`affectedProfiles`、`dirtyVisitCount`、`dirtyDateKeys`、`fallbackReason`。這些欄位的定位是 queue/review truth；current primary macOS host 的 `14.4M / 60y` full replay 與 expired-lease recovery signoff 已落在 `artifacts/benchmarks/2026-04-18-intelligence-long-horizon-signoff/`。第二台主機 benchmark parity 目前不在當前 scope 內，如要補證據必須重新立項。

### 1.D Site Dictionary / 規則系統

規則優先級（固定不可改）：

```
1. user_override         ← 用戶在設定中手動指定
2. builtin_rule_pack     ← 對應 taxonomy pack：global-core, cn-core, us-core...
3. url_heuristic         ← URL 結構推斷
4. domain_heuristic      ← domain/TLD 推斷
5. unknown
```

**Site Dictionary 管理的內容：**

| 管理項              | 說明                           | 範例                            |
| ------------------- | ------------------------------ | ------------------------------- |
| 網站別名            | domain → 顯示名                | `bilibili.com → BiliBili`       |
| 搜索引擎            | domain + query param rule      | `google.com: q=`                |
| 網站分類            | domain → `domain_category`     | `github.com → developer`        |
| Noisy domain        | 隱藏 / 降權                    | `doubleclick.net`               |
| Tracking params     | URL 規範化時去除               | `utm_*`, `fbclid`, `gclid`      |
| Page category rules | path pattern → `page_category` | `github.com/*/issues/* → issue` |

### 1.E 模塊化架構

每個 Core Intelligence 功能都是獨立模塊，繼續使用現有的 `deterministic_module_runtime` 表：

```rust
trait IntelligenceModule {
    fn id(&self) -> &str;
    fn version(&self) -> &str;
    fn dependencies(&self) -> &[&str];       // 依賴的其他模塊 ID
    fn required_capabilities(&self) -> &[&str]; // 需要的 capability families
    fn derived_tables(&self) -> &[&str];     // 產出的表名
    fn settings_schema(&self) -> SettingsSchema; // 模塊自己的設定項
    fn rebuild_mode(&self) -> RebuildMode;   // incremental / full / window
    fn collect(&self, inputs: &[Visit]) -> Vec<DerivedRow>;
    fn explain(&self, entity_id: &str) -> Explanation;
    fn clear(&self);
}
```

---

## 2. 功能總覽與 UI 佈局

### 2.1 UI 結構概覽

Core Intelligence 的功能分布在三個位置：

```
┌──────────────────────────────────────────────────────────────────┐
│  PathKeep                                                        │
├──────────┬───────────────────────────────────────────────────────┤
│          │                                                       │
│  側邊欄   │  主區域                                                │
│          │                                                       │
│  ■ 總覽   │  ┌─ Dashboard ──────────────────────────────────────┐ │
│  ■ 探索   │  │  智能功能的入口和摘要卡片                          │ │
│  ■ 智能   │  └──────────────────────────────────────────────────┘ │
│  ■ 設定   │                                                       │
│          │  ┌─ Explorer ────────────────────────────────────────┐ │
│          │  │  歷史紀錄瀏覽，可按會話/旅程分組                    │ │
│          │  └──────────────────────────────────────────────────┘ │
│          │                                                       │
│          │  ┌─ Intelligence ───────────────────────────────────┐ │
│          │  │  所有深度分析功能的專屬頁面                         │ │
│          │  └──────────────────────────────────────────────────┘ │
│          │                                                       │
└──────────┴───────────────────────────────────────────────────────┘
```

### 2.2 Intelligence 主頁 ASCII 佈局

```
┌─ Intelligence ───────────────────────────────────────────────────────────────┐
│                                                                              │
│  [日] [週] [月] [季] [年] [自訂 ▾]     ← 全局時間範圍選擇器                  │
│  ─────────────────────────────────────────────────────────────────────────── │
│                                                                              │
│  ┌─ 本期摘要 ────────────────────────────────────────────────────────────┐   │
│  │ ╭──────────╮ ╭──────────╮ ╭──────────╮ ╭──────────╮ ╭──────────╮     │   │
│  │ │ 📊 訪問  │ │ 🔍 搜索  │ │ 🌐 新站  │ │ 📖 深讀  │ │ 🔄 重找  │     │   │
│  │ │  12,847  │ │   423    │ │   67     │ │   31     │ │   15     │     │   │
│  │ │  +12% ↑  │ │  -5% ↓  │ │ +23% ↑  │ │  +8% ↑  │ │  =       │     │   │
│  │ ╰──────────╯ ╰──────────╯ ╰──────────╯ ╰──────────╯ ╰──────────╯     │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ┌─ Top 網站 ───────────────────────┐  ┌─ 搜索活動 ────────────────────┐     │
│  │ 1. GitHub ·········· 847 次     │  │ [搜索入口] [常查概念] [搜索演化]│     │
│  │ 2. YouTube ········· 612 次     │  │ Google     ████████ 312       │     │
│  │ 3. Stack Overflow ·· 389 次     │  │ YouTube    ██████  189       │     │
│  │ 4. BiliBili ········ 234 次     │  │ GitHub     ███     67        │     │
│  │ 5. Google ·········· 201 次     │  │ [選中某組 → 搜索演化 / drilldown]│   │
│  │ [搜索特定網站...] [查看全部 →]   │  └──────────────────────────────┘     │
│  └──────────────────────────────────┘                                       │
│                                                                              │
│  ┌─ 搜索活動 ────────────────────────────────────────────────────────────┐   │
│  │ [搜索入口] [常查概念] [搜索演化]                                      │   │
│  │ ╭──────────────────────────────────────────────────────────────────╮  │   │
│  │ │ ┌───────────────────────────────────────┐ Google     ████████ 312│  │   │
│  │ │ │        sqlite       rust              │ YouTube    ██████  189│  │   │
│  │ │ │    wal     tauri   async              │ BiliBili   ████    98│  │   │
│  │ │ │  checkpoint   v2     tokio            │ GitHub     ███     67│  │   │
│  │ │ │      performance                      │ DuckDuckGo ██      34│  │   │
│  │ │ └───────────────────────────────────────┘ 淘寶       █       12│  │   │
│  │ │         ↑ 詞雲 / Packed Bubble                ↑ 引擎使用量     │  │   │
│  │ ╰──────────────────────────────────────────────────────────────────╯  │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ┌─ 常重找的頁面 ────────────────────────────────────┐                       │
│  │ 📄 SQLite WAL Checkpoint 文檔                     │                       │
│  │    在 8 個不同日期被重新打開，5 次從搜索進入       │                       │
│  │ 📄 Tauri v2 Migration Guide                       │                       │
│  │    出現在 6 個搜索旅程中，跨 12 天重訪             │                       │
│  │ [查看全部 →]                                      │                       │
│  └───────────────────────────────────────────────────┘                       │
│                                                                              │
│  ┌─ 使用構成 ──────────────────────┐  ┌─ 瀏覽節奏 ──────────────────┐       │
│  │ ■ 開發  ████████████  42%      │  │ Apr      May                │       │
│  │ ■ 影音  ███████       28%      │  │ 日 ▪ ▪ ▪ ▪ · ▪ ▪ ▪ · ·     │       │
│  │ ■ 社群  ████          15%      │  │ 一 · ▪ · ▪ · ▪ · ▪ ▪ ·     │       │
│  │ ■ 購物  ██             8%      │  │ 二 · ▪ ▪ ▪ ▪ ▪ · · ▪ ·     │       │
│  │ ■ 其他  ██             7%      │  │ 三 · · ▪ ▪ · ▪ ▪ · ▪ ▪     │       │
│  │ vs 上期: 開發 +5%, 影音 -3%    │  │ 四 ▪ · ▪ · · ▪ ▪ · ▪ ·     │       │
│  │ [詳細趨勢 →]                   │  │ 五 ▪ ▪ · · ▪ · ▪ · · ▪     │       │
│  └─────────────────────────────────┘  │ 六 · ▪ · ▪ · ▪ · ▪ · ▪     │       │
│                                       │ [選中某天 → 當天 digest + 24h]│     │
│                                       └──────────────────────────────┘       │
│                                                                              │
│  ┌─ 更多洞察 ────────────────────────────────────────────────────────────┐   │
│  │ [穩定答案來源] [搜索效率] [探索率] [習慣] [高摩擦來源] [多瀏覽器對比] │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 2.3 Explorer 頁面的 Intelligence 增強

Explorer（歷史瀏覽）頁面新增「View by」分組選項：

```
┌─ Explorer ───────────────────────────────────────────────────────────────────┐
│                                                                              │
│  排列方式: [時間] [會話 ▾] [搜索旅程]     搜索: [_______________]            │
│  ─────────────────────────────────────────────────────────────────────────── │
│                                                                              │
│  ┌─ 會話 · 4月15日 14:23 – 15:47 · 研究 SQLite WAL ─────────────────────┐   │
│  │  ▶ 47 個頁面 · 3 次搜索 · 🏷 深度研究                                │   │
│  │  ┌──────────────────────────────────────────────────────────────────┐  │   │
│  │  │ 🔍 Google: "sqlite wal checkpoint"                     14:23   │  │   │
│  │  │   📄 SQLite Write-Ahead Logging — sqlite.org            14:24   │  │   │
│  │  │   📄 WAL mode checkpoint — Stack Overflow               14:26   │  │   │
│  │  │ 🔍 Google: "sqlite wal too large fix"                   14:31   │  │   │
│  │  │   📄 WAL file size management — GitHub Issue            14:32   │  │   │
│  │  │   📄 PRAGMA wal_checkpoint docs — sqlite.org    📖      14:35   │  │   │
│  │  │ 🔍 Google: "sqlite wal_checkpoint passive"              14:41   │  │   │
│  │  │   📄 SQLite Checkpoint Semantics — sqlite.org   📖      14:42   │  │   │
│  │  └──────────────────────────────────────────────────────────────────┘  │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ┌─ 會話 · 4月15日 16:10 – 16:45 · 在 YouTube 觀看影片 ─────────────────┐   │
│  │  ▶ 12 個頁面 · 0 次搜索                                              │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ── 單條歷史展開時的導航溯源面板 ──────────────────────────────────────────  │
│                                                                              │
│  📄 PRAGMA wal_checkpoint docs — sqlite.org                                 │
│  ┌─ 你是怎麼找到這裡的 ─────────────────────────────────────────────────┐   │
│  │  🔍 Google: "sqlite wal_checkpoint passive"                           │   │
│  │    → 📄 SQLite Checkpoint Semantics — sqlite.org                     │   │
│  │      → 📄 PRAGMA wal_checkpoint docs ← 你在這裡                      │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 2.4 網站深度分析頁 (Domain Insights / Domain Deep Dive)

```
┌─ 網站深度分析 · GitHub ──────────────────────────────────────────────────────┐
│                                                                              │
│  ╭───────────╮ ╭───────────╮ ╭───────────╮ ╭───────────╮                    │
│  │ 總訪問次數 │ │ 活躍天數   │ │ 搜索旅程  │ │ 到達方式   │                    │
│  │   3,847   │ │   182     │ │   45      │ │ 搜索 47%  │                    │
│  ╰───────────╯ ╰───────────╯ ╰───────────╯ │ 連結 31%  │                    │
│                                             │ 輸入 22%  │                    │
│  ┌─ 訪問趨勢 ────────────────────────┐      ╰───────────╯                    │
│  │        ╱╲    ╱╲                   │                                       │
│  │  ╱╲  ╱  ╲  ╱  ╲   ╱╲            │  ┌─ Top 頁面 ──────────────────────┐  │
│  │ ╱  ╲╱    ╲╱    ╲ ╱  ╲           │  │ 1. /issues — 412 次            │  │
│  │╱                 ╲    ╲──        │  │ 2. /pulls — 287 次             │  │
│  │ Jan  Feb  Mar  Apr               │  │ 3. /blob — 203 次              │  │
│  └───────────────────────────────────┘  │ 4. /search — 156 次           │  │
│                                         └─────────────────────────────────┘  │
│  ┌─ 常見入口 ────────────────────┐  ┌─ 離開後去哪 ────────────────────────┐  │
│  │ Google → GitHub    312 次     │  │ GitHub → Stack Overflow   89 次    │  │
│  │ 直接輸入            287 次     │  │ GitHub → 官方文檔         67 次    │  │
│  │ Stack Overflow →    134 次     │  │ GitHub → Google          45 次    │  │
│  └────────────────────────────────┘  └────────────────────────────────────┘  │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. 功能清單

功能按四大區塊組織：**儀表板與摘要**、**排行榜與統計**、**結構化瀏覽路徑**、**深度分析與洞察**。

---

### 一、儀表板與摘要

#### 1.1 時間段總結 (Digest Cards)

| 項目         | 內容                                                   |
| ------------ | ------------------------------------------------------ |
| **為什麼**   | 快速回答「我這段時間在幹嘛？」，是 Intelligence 的入口 |
| **是什麼**   | 指定時間段的 top-line 指標匯總卡片，點擊跳轉詳情       |
| **怎麼算**   | 從各 `daily_rollups` 按時間範圍查詢 TOP N 並匯總       |
| **怎麼展示** | 儀表板頂部的數字卡片集合                               |
| **性能**     | 純讀取預計算表，毫秒級                                 |
| **數據依賴** | 各 rollup 表                                           |

卡片包含：總訪問次數、搜索次數、新發現域名數、深度閱讀頁面數、常重找頁面數。每項標注與上一等長時段的環比變化（↑/↓/=）。

---

#### 1.2 歷史上的今天 (On This Day)

> **2026-04-19 IA note:** 這張卡現在是 **Dashboard-only** surface。deterministic behavior 不變，但它不再佔用 `/intelligence` 的主頁版面，因為它不受 route time scope 影響。

| 項目         | 內容                                          |
| ------------ | --------------------------------------------- |
| **為什麼**   | 瀏覽歷史是記憶的一部分，回顧有情感和認知價值  |
| **是什麼**   | 過去同一天（月-日匹配）的摘要卡片，按年份倒序 |
| **怎麼算**   | `daily_rollups` 按 `month-day` 匹配查詢       |
| **性能**     | 索引查詢，毫秒級                              |
| **跨瀏覽器** | ✅ 全覆蓋（只需 `visit_time`）                |

配合其他功能的聯動：「兩年前的今天你進行了一次關於 X 的深度研究」、「一年前你開始使用 GitHub」。

---

### 二、排行榜與統計

#### 2.1 Top 網站統計 (Top Visited Sites)

| 項目         | 內容                                                                      |
| ------------ | ------------------------------------------------------------------------- |
| **為什麼**   | 最基礎的使用習慣鏡子                                                      |
| **是什麼**   | 按 `registrable_domain` 聚合：總訪問次數、獨立訪問天數、平均每日訪問次數  |
| **怎麼算**   | `GROUP BY registrable_domain` 預計算到 `domain_daily_rollups`             |
| **怎麼展示** | 可排序、可搜索的表格或條形圖。別名系統在顯示層應用。點擊→ Domain Insights |
| **性能**     | 讀取 rollup 聚合，毫秒級                                                  |
| **跨瀏覽器** | ✅ 全覆蓋                                                                 |

**`registrable_domain` 用 `publicsuffix` crate 提取**，而不是簡單按 `.` 切割。

---

#### 2.2 搜索活動 (Search Activity)

**取代原版「Top 搜索關鍵詞」**——raw query 幾乎不重複，統計沒意義。

目前 shipping 為四個子視圖，其中第四個已正式命名為 `Search Keywords` browser，不重開新的 Explorer URL grammar：

**A. 搜索入口排行**

- 各搜索引擎/平台的搜索次數排行
- `GROUP BY search_engine`，daily rollup
- 內建 Google、Bing、YouTube、BiliBili、GitHub、DuckDuckGo、百度、淘寶等
- 用戶可在 Maintenance derived-state panel 的 search-engine rule editor 中自定義追加

**B. 高頻搜索概念 (Top Search Concepts)**

| 項目         | 內容                                                                                                                                                                                                                                        |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **為什麼**   | 大部分搜索詞只出現一次，raw top keyword 沒意義。真正有價值的是「你持續關心什麼概念」                                                                                                                                                        |
| **怎麼算**   | 只對 keyword-eligible `search_events` 拆 token 並統計頻率。`"sqlite wal checkpoint"` 和 `"sqlite wal too large"` 都為 `sqlite` 和 `wal` 貢獻計數；看起來像 pasted URL / hostname / navigational input 的 row 先排除，不再進 concept surface |
| **分詞策略** | Latin → 空格分詞 + 去停用詞。CJK → n-gram + 可選 `jieba-rs`（Rust 原生，幾十 KB 字典）。Unicode normalization → 轉小寫、去標點；URL-like stop tokens（如 `http` / `https` / `www` / `com`）也不得主導概念排行                               |
| **怎麼展示** | ranked horizontal bar chart，並附 chart description 說明目前顯示的是當前視窗內最常出現的搜索概念                                                                                                                                            |

**C. Query Family（反覆搜索的問題）**

在同一搜索旅程或短時間窗口內，將同一問題的多次搜索合併：

合併條件（**局部鄰域**內，不做全庫兩兩比較）：

- 同一搜索引擎
- Token Jaccard similarity > 0.5
- 或 one query contains another（增加了限制詞、版本號、error code、`site:`）
- 時間窗口：同一 trail 內 或 24 小時內的同 engine query

```
Query Family 範例:
  ├─ "sqlite wal"
  ├─ "sqlite wal too large"
  └─ "sqlite wal checkpoint not working"
```

搜索詞提取優先級：

1. Chromium `search_terms` / Firefox `moz_places_metadata_search_queries`
2. canonical `search_terms` 表（已存在於 archive）
3. Maintenance derived-state panel 中維護的 search-engine rules（`q=`、`query=`、`search_query=` 與 host/path matching）
4. Generic query param fallback

**性能：** 分詞在導入時一次性完成，結果存入 `search_event_terms` 表。Query family merge 只在局部窗口內做，O(k²) where k = 局部 query 數量（通常 < 20）。

**跨瀏覽器：** Chromium ✅（`search_terms`） / Firefox ✅（`search_queries`） / Safari ⚠️（URL 規則 fallback） / Takeout ✅

**D. Search Keywords（目前時間窗裡真的查了什麼）**

| 項目         | 內容                                                                                                                                                                                                              |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **為什麼**   | `Top Search Concepts` 與 `Query Family` 都是 aggregate view，但使用者有時仍需要回看當前時間窗內真的查了哪些 keyword，且要保留 reusable identity                                                                   |
| **怎麼算**   | 讀 keyword-eligible `search_events`，在請求視窗內先 dedupe 最新一筆 `(search_engine, normalized_query)`，再回填 `query_families` / `search_trails` context                                                        |
| **怎麼展示** | `Search Activity` 內的 bounded browser，支援 text filter、engine filter、nested date subrange、sort、pagination、page size；有 `familyId` 時 primary CTA 走 query-family insights，次要 CTA 才是 trail / evidence |
| **邊界**     | 不新增 Explorer `queries` view、不新增新的 route grammar，也不把 M7 已接受的 promoted entity CTA 拉回 page-local deep-link；row 粒度仍是 distinct keyword，而不是每次 search event                                |

---

#### 2.3 常重找的頁面 (Refind Pages)

| 項目       | 內容                                                           |
| ---------- | -------------------------------------------------------------- |
| **為什麼** | 很多頁面的價值不在「去了多少次」，而在「每次都需要重新找一遍」 |
| **是什麼** | 識別被行為證明有長期價值的頁面                                 |

**Refind Score 由以下信號組成：**

| 信號                      | 權重 | 說明                               |
| ------------------------- | ---- | ---------------------------------- |
| `cross_day_revisit_count` | 高   | 至少在 2 個不同日期訪問            |
| `distinct_trail_count`    | 高   | 出現在多少個不同的 Search Trail 中 |
| `search_arrival_count`    | 中   | 從搜索結果被帶回來幾次             |
| `typed_revisit_count`     | 低   | 直接在地址列輸入訪問（已記住）     |

**排除噪音：** 首頁、搜索結果頁、redirect 中間頁、tracking 頁。

**URL 規範化（必須先做）：**

- 去 tracking params（`utm_*`、`fbclid`、`gclid` 等）
- 保留語義 params（video id、issue id、商品 id、搜索詞）
- 規範化路徑（去尾部 `/`、規範大小寫）

**展示：** 帶解釋的列表卡片：

> 「這頁在過去 90 天內被你在 8 個不同日期重新打開，其中 5 次是從搜索結果再次進入。」

**跨瀏覽器：** ✅ 全覆蓋

---

#### 2.4 定期訪問偵測 (Habitual Visit Detector)

| 項目       | 內容                                                           |
| ---------- | -------------------------------------------------------------- |
| **為什麼** | 區分「集中訪問了幾次」和「每週固定來一次」。後者才是真正的習慣 |
| **怎麼算** | 對每個 domain，計算訪問間隔的變異係數 CV = σ/μ                 |

**分類規則：**

| 類型                 | 平均間隔 μ    | CV 閾值  | 最低要求           |
| -------------------- | ------------- | -------- | ------------------ |
| `daily_habit`        | μ < 2 天      | CV < 0.5 | ≥ 5 次，跨 ≥ 14 天 |
| `weekly_habit`       | 5 ≤ μ ≤ 10 天 | CV < 0.6 | ≥ 5 次，跨 ≥ 14 天 |
| `periodic_reference` | μ > 10 天     | CV < 0.8 | ≥ 5 次，跨 ≥ 14 天 |

**習慣中斷偵測：** `last_visited` 距今 > `μ × 2` 且有 habit 標記 → 「你已有 3 週沒有訪問 X 了」

**性能：** Per-domain 序列計算，O(N)，可增量更新。

**跨瀏覽器：** ✅ 全覆蓋（只需 `visit_time` + `registrable_domain`）

---

### 三、結構化瀏覽路徑

**這是 Core Intelligence 的精華**——將扁平歷史重建成有結構的使用者故事。

#### 3.1 瀏覽會話 (Browsing Sessions)

| 項目       | 內容                                   |
| ---------- | -------------------------------------- |
| **為什麼** | 人的瀏覽行為是一段段的任務，不是連續流 |
| **是什麼** | 時間上連續的瀏覽活動塊                 |

**新會話開始條件（任一）：**

1. 距上一次 visit 超過 N 分鐘（可配置，預設 30 分鐘）
2. `transition_type` 是入口型（`TYPED`、`BOOKMARK`、`AUTO_TOPLEVEL`）

**算法：** 按 `visit_time_ms` 排序一次線性掃描 O(N)。`session_id` 寫入 `visit_derived_facts` 表。可增量處理。

**每個 session 計算：**

- 起止時間、visit 數量、涉及 domain 列表
- 是否含搜索事件
- 是否為深度研究 session（見 4.10）
- 自動標題：session 內最高頻 domain + 搜索關鍵詞

**展示：** Explorer 的「按會話分組」view by 選項。每個 session 是可折疊區塊。

**跨瀏覽器：** ✅ 全覆蓋

---

#### 3.2 搜索旅程 (Search Trails)

**這是最像 Chrome「按組查看」但更強大的功能。**

| 項目                 | 內容                                                                     |
| -------------------- | ------------------------------------------------------------------------ |
| **為什麼**           | 還原「從提出問題到找到答案」的完整路徑。回答「我是怎麼找到這個頁面的？」 |
| **是什麼**           | 從一次搜索開始，追蹤所有由此衍生的點擊鏈                                 |
| **不需要 embedding** | 基於 `from_visit` 的有向圖遍歷                                           |

**起點識別：**

- `search_terms` 不為空的 visit（最可靠）
- URL 符合搜索引擎 query 規則的 visit（fallback）

**吸附規則：**

- `from_visit_id` / `referrer_url` 指向 trail 內已有 visit
- 同一 session 內、時間窗口（15 分鐘）內的後續點擊

**終止條件（任一）：**

- 時間間隔超過閾值（可配置，預設 15 分鐘）
- 出現新的無關搜索事件
- 遇到 `TYPED` 或 `BOOKMARK` transition
- 多筆連續 visit 與當前 trail 無關

**內嵌子視圖：搜索演化 (Query Reformulation)**
同一 trail 內的 query 序列展示為演化鏈：

```
"sqlite wal"
  → "sqlite wal too large"
    → "sqlite wal checkpoint not working"
       └── 落點：sqlite.org/wal.html
```

**每個 trail 記錄：**

- 初始 query + reformulation 鏈
- 訪問頁面列表（帶層級結構）
- Landing pages（末端穩定落點）
- 改寫次數、總深度

**展示：** Explorer 的「按搜索旅程分組」view by。每個 trail 卡片顯示搜索引擎 icon、初始 query、reformulation 次數 badge、頁面列表、最終落點。

**路徑溯源 SQL：**

```sql
WITH RECURSIVE path(visit_id, url, title, visited_at_ms, depth) AS (
  SELECT v.id, u.url, u.title, v.visit_time_ms, 0
  FROM visits v JOIN urls u ON v.url_id = u.id
  WHERE v.id = ?
  UNION ALL
  SELECT v2.id, u2.url, u2.title, v2.visit_time_ms, p.depth + 1
  FROM path p
  JOIN visits v2 ON p.visit_id = v2.from_visit
  JOIN urls u2 ON v2.url_id = u2.id
  WHERE p.depth < 10
)
SELECT * FROM path ORDER BY depth DESC;
```

**性能：** Trail 構建在導入時增量完成 O(N)。單條路徑溯源 on-demand O(depth)，depth 通常 < 5。

**跨瀏覽器：**

- Chromium: `from_visit` + `opener_visit` + `transition` → 完整 ✅
- Firefox: `from_visit` + `triggeringPlaceId` + `visit_type` → 完整 ✅
- Safari: `redirect_source` / `redirect_destination` → 部分 ⚠️
- Takeout Session: `navigation[]` + `referrer` → 完整 ✅

---

#### 3.3 導航溯源 (Navigation Path Tracer)

| 項目         | 內容                                                         |
| ------------ | ------------------------------------------------------------ |
| **為什麼**   | 「我是怎麼找到這個頁面的？」是用戶最常有但瀏覽器不回答的問題 |
| **是什麼**   | 對任意歷史紀錄，向上追溯 `from_visit` 鏈，最多 10 跳         |
| **展示位置** | Explorer 展開歷史紀錄時的面板                                |
| **性能**     | O(depth)，on-demand 查詢                                     |

**附加：Hub 頁面分析（批次預計算）**
統計每個 URL：有多少其他 trail 的溯源路徑包含這個 URL。高分的是「探索起點 Hub」。

---

### 四、深度分析與洞察

#### 4.1 網站深度分析 (Domain Insights / Domain Deep Dive)

| 項目         | 內容                                                                                                                                                                                                         |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **為什麼**   | 回答「我跟某個特定網站的關係是怎樣的？」                                                                                                                                                                     |
| **包含**     | Top Pages、Top Referrers、Top Exits、Activity Over Time、搜索入口、到達方式分佈；若該 domain 是已知 search engine 且目前視窗內有 keyword-eligible rows，還要額外顯示 domain-scoped `Search Keywords` browser |
| **觸發方式** | 從 Top 網站列表點擊域名進入                                                                                                                                                                                  |
| **性能**     | 基於 rollup 和索引查詢                                                                                                                                                                                       |

---

#### 4.2 穩定答案來源 (Stable Answer Sources)

| 項目       | 內容                                                                                                                     |
| ---------- | ------------------------------------------------------------------------------------------------------------------------ |
| **為什麼** | 看哪些站對你真正有用。不是「最常訪問」，而是「哪裡成為旅程終點」                                                         |
| **怎麼算** | Trail outcome 分析：某 domain 出現在 trail 後半段的頻率、出現後 reformulation 是否下降、是否被跨天 reopen 後再次成為落點 |
| **展示**   | 兩個排行榜：「常作為入口的來源」「常作為最終落點的來源」                                                                 |

---

#### 4.3 搜索效率分析 (Search Effectiveness)

| 項目       | 內容                                                                                                           |
| ---------- | -------------------------------------------------------------------------------------------------------------- |
| **為什麼** | 不只告訴你搜了什麼，還告訴你搜得有沒有效率                                                                     |
| **指標**   | 改寫次數 (reformulation count)、落地頁類型 (resolution category)、深度 (visit depth)、再搜間隔 (re-search lag) |
| **聚合**   | 到搜索引擎層級：「在 Google 搜技術問題平均改寫 2.8 次，BiliBili 搜索平均改寫 1.2 次」                          |
| **展示**   | 各引擎平均改寫次數 bar chart、Top 解答來源、最難找的主題                                                       |

---

#### 4.4 碰壁與高摩擦偵測 (Friction Detection)

| 項目       | 內容                 |
| ---------- | -------------------- |
| **為什麼** | 告訴你哪裡在浪費精力 |

**強證據（有就用）：**

- Chromium `response_code` = 4xx/5xx
- Safari `load_successful = false`
- Redirect chain 過長

**弱證據（universal fallback）：**

- A (搜索) → B (某頁) → A (搜索) 短時間反覆 ≥ 2 次
- 同一 Query Family 短時間 ≥ 3 次 reformulation
- 點進去後很快又回搜索（bounce pattern）

**展示文案（說人話）：**

- 「常讓你回去繼續搜索的網站」
- 「經常失敗的頁面」

---

#### 4.5 反覆回來查的問題 (Reopened Investigations)

| 項目       | 內容                                                                                                                                  |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| **為什麼** | Thread/Open Loop 的務實版本。不宣稱「未完成任務」，只說「你多次回來看這件事」                                                         |
| **怎麼算** | 兩種錨點做 deterministic merge：**Query Family 錨點**（同類 family 跨天重現）和 **Reference Page 錨點**（同批 Refind Pages 跨天重現） |
| **關鍵**   | 不做全庫 topic merge。只在有強錨點時才合併，寧可碎不假準                                                                              |

**展示文案：**

- 「你在 3 個不同日期重新搜索了關於 Tauri 的問題」
- 「你反覆回來查這 2 個頁面，跨越了 5 個不同日期」

---

#### 4.6 瀏覽節奏熱圖 (Browsing Rhythm Heatmap)

> **2026-04-20 dashboard note:** Dashboard 現在也會共用這套真實日期日曆熱力圖，但固定以 calendar year 呈現；若 archive 內跨多個年份，卡片必須顯示當前查看年份，並以 bounded pager 在「最早有資料的年份」到 `max(當前年份, 最晚有資料的年份)` 的**連續年份帶**之間前後翻頁。中間空白年份仍要顯示空熱力圖；當前年份永遠要存在，且只要使用者目前不在當前年份，就必須提供明確的「回到當前年份」捷徑，而不是用 hourly detail API 或任意跳到不在這條連續年份帶裡的年份。

| 項目       | 內容                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **為什麼** | 使用者先需要知道「哪些真實日期值得看」，再決定要不要往同一天的具體時段下鑽。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| **是什麼** | **主圖：** GitHub 式真實日期日曆熱力圖（1 格 = 1 天，hover 要顯示 exact date + visits/new sites tooltip）<br />**摘要：** 卡片頂部固定顯示 visit-based summary line，直接用目前畫面內的 `totalVisits` 加總；Dashboard 用 calendar-year wording，`/intelligence` overview 則必須對應目前實際選定的單日 / 整月 / 整年 / date span，而不是把 rolling preset 假裝成自然月或自然年；若 start/end 在同一年內，exact-range wording 不應重複年份。<br />**主互動：** 點某一天後，先在卡片內顯示 compact day preview（當天 digest / 全寬 flat 24 小時分布 / 全寬重點網站列 / proportion bar 活動構成）；使用者再透過明確的 `查看詳情` CTA 進 `/intelligence/day/:date` 的完整 day insights route。 |

> **2026-04-20 detail layout note:** `/intelligence/day/:date` 現在也共用同一套 flat 24 小時分布與 proportion bar 活動構成；只有 detail route 的 `Standout Sites` 保留較豐富的原本呈現，不跟 overview preview 一起縮成 chip row。

**主圖資料來源：**

```sql
SELECT
  date_key,
  SUM(new_domains) AS new_domain_count,
  SUM(total_visits) AS total_visits
FROM daily_summary_rollups
WHERE date_key BETWEEN ? AND ?
GROUP BY date_key
ORDER BY date_key ASC
```

**單日 detail 的 24 小時分布：**

```sql
SELECT
  CAST(strftime('%H', datetime(visit_time_ms/1000, 'unixepoch', 'localtime')) AS INTEGER) AS hour,
  COUNT(*) AS visit_count
FROM visits
WHERE visit_time_ms BETWEEN ? AND ?
  AND reverted_at IS NULL
GROUP BY hour
```

**時區處理：** backup 時自動記錄設備時區（`runs.timezone`），用於 localtime 轉換。歷史導入數據使用當前時區（可能有誤差，UI 標註）。

**展示：** 主圖固定是按真實日期的日曆熱力圖；長時間窗允許卡片內橫向滾動。小時分布只出現在選中某一天後的 detail 區，不再回頭冒充主圖。

**跨瀏覽器：** ✅ 全覆蓋

---

#### 4.7 探索率趨勢 (Discovery vs. Familiarity Ratio)

| 項目       | 內容                                                                                                |
| ---------- | --------------------------------------------------------------------------------------------------- |
| **為什麼** | 「你最近是在開拓新領域還是重複消費」是有意思的 meta-insight                                         |
| **怎麼算** | 每條 visit 的 `registrable_domain` 是否在此之前首次出現。`is_new_domain` 在導入時 O(N) 計算並持久化 |

```
discovery_rate(window) = COUNT(new_domain visits) / COUNT(total visits)
```

**展示：** 折線圖（探索率 %）+ 柱狀圖（新域名數量）。「探索里程碑」：每個首次訪問的 domain 按時間排列。

---

#### 4.8 使用構成 (Activity Mix)

| 項目       | 內容                                                                       |
| ---------- | -------------------------------------------------------------------------- |
| **為什麼** | 看這段時間各類別的佔比和變化趨勢                                           |
| **怎麼算** | 基於 `domain_category` 的訪問構成分析。日粒度存入 `category_daily_rollups` |

**進階：** 計算相鄰月份的 category 向量 L1 距離，找「主題組成變化最大的時間點」。

**展示：** 堆疊面積圖 (Stacked Area Chart) + 構成條 (Composition Bar) + 主題突變點標注。

---

#### 4.9 集中度分析 (Breadth vs. Depth Index)

| 項目       | 內容                                                                              |
| ---------- | --------------------------------------------------------------------------------- |
| **為什麼** | 識別信息繭房信號                                                                  |
| **怎麼算** | HHI (Herfindahl Index) = Σ (visit_share_i)²。接近 1 = 高度集中，接近 0 = 高度分散 |
| **展示**   | Breadth Score (0–100)、「你 50% 的瀏覽集中在 X 個 domain」、可選 Lorenz curve     |

---

#### 4.10 深度研究 Session 偵測 (Deep Dive Session Detection)

| 項目       | 內容                                                                                                         |
| ---------- | ------------------------------------------------------------------------------------------------------------ |
| **為什麼** | 區分淺層刷刷和真正的深度探索                                                                                 |
| **怎麼算** | 對每個 session 計算：navigation chain depth、unique domain count、new domain ratio、query count、visit count |

**Deep dive 判定：**

```
is_deep_dive =
  navigation_chain_depth >= 4 AND
  unique_domain_count >= 5 AND
  visit_count >= 8
```

**展示：** 在 session 上加 badge（🔬 深度研究），配合 On This Day。

---

#### 4.11 常見路線 (Path Flows)

| 項目       | 內容                                                               |
| ---------- | ------------------------------------------------------------------ |
| **為什麼** | 讓用戶知道自己常走哪些路                                           |
| **怎麼算** | 在 session/trail 內統計 2-step/3-step domain n-gram 頻率           |
| **展示**   | 路線 chip 列表：`Google → Stack Overflow (847 次)`，點開看最近實例 |

---

#### 4.12 比較中的頁面組 (Compare Sets)

| 項目       | 內容                                                                                 |
| ---------- | ------------------------------------------------------------------------------------ |
| **為什麼** | 很多瀏覽行為是多頁比較：商品、框架、教程                                             |
| **怎麼算** | 同一 trail 內：同 `page_category` 的多頁 + 短時間反覆切換 + 多 domain 或 sibling URL |
| **可行性** | ⚠️ 中。啟發式精準度可能不高，但只在強信號時觸發。建議第二批                          |
| **展示**   | 卡片：搜索 query + 比較頁面列表 + 每頁被看幾次 + 是否形成落點                        |

---

#### 4.13 多瀏覽器/Profile 對比 (Multi-Browser Behavioral Diff)

| 項目       | 內容                                                          |
| ---------- | ------------------------------------------------------------- |
| **為什麼** | PathKeep 支持多瀏覽器導入，不應浪費這個獨特優勢               |
| **怎麼算** | Exclusive domains、Shared domains、Category distribution diff |
| **展示**   | Domain overlap 清單、Category 分佈並排 bar chart              |

---

#### 4.14 瀏覽器直接報告的互動數據 (Observed Interaction Module)

| 項目       | 內容                                           |
| ---------- | ---------------------------------------------- |
| **為什麼** | 不做弱推算，但瀏覽器直接給的強信號可以誠實使用 |
| **設計**   | Capability-gated：沒有數據就不顯示此模塊       |

**數據來源：**

| 瀏覽器   | 可用信號                                                                                | 存儲位置                    |
| -------- | --------------------------------------------------------------------------------------- | --------------------------- |
| Chromium | `total_foreground_duration`、`page_end_reason`                                          | `visit_engagement_evidence` |
| Firefox  | `total_view_time`、`scrolling_time`、`scrolling_distance`、`key_presses`、`typing_time` | `visit_engagement_evidence` |
| Safari   | `load_successful` / failure indicators                                                  | `visit_engagement_evidence` |

**文案必須極其誠實：**

- ✅ 「瀏覽器直接報告的前景停留時間」
- ❌ 「你花了多少時間」

---

## 4. 貫穿性能力

### 4.A 可解釋性面板 (Explainability Panel)

每個 derived entity（refind page、trail、habit、compare set...）都能回答「為什麼會算成這樣」：

```
┌─ 為什麼這頁出現在「常重找的頁面」？ ──────────────────────────────┐
│                                                                    │
│  觸發規則：Refind Score ≥ 0.7                                      │
│                                                                    │
│  得分因子：                                                        │
│  ├─ 跨天回訪：8 個不同日期         ×0.4  → 3.2                    │
│  ├─ 搜索帶回：5 次從搜索結果進入   ×0.3  → 1.5                    │
│  ├─ 不同旅程：出現在 6 個 trail    ×0.2  → 1.2                    │
│  └─ 地址列輸入：2 次              ×0.1  → 0.2                    │
│                                                                    │
│  參與的 visit IDs：#1234, #1567, #1890, #2345... [在 Explorer 查看] │
└────────────────────────────────────────────────────────────────────┘
```

### 4.B 全局時間範圍選擇器

所有功能共用：日/週/月/季度/年/自定義。是一個全局 UI 組件。

### 4.C 外部服務（低優先級）

- 第一個 shipping external-output surfaces 已落在 Integrations：manual review / copy-export panel 仍是 canonical baseline，而第一個可重用宿主則是 `browser-snippet-v1` trusted local artifact。使用者現在可 preview `embed cards`、`widget snapshot`、`public snapshot`，也可 review `index.html` / `bundle.json` 並建立 `app_root/integrations/core-intelligence/browser-snippet-v1/`；但這仍只限受信任本地宿主，不等於 OS widget、localhost API 或 public API 已完成
- trusted external-output payload 現在可帶 structured `primaryTarget` / `secondaryTargets`，供 Integrations 與 trusted local host 產生 reusable app links；`public snapshot` 仍維持 redacted，不下放 internal reusable IDs
- Intelligence 卡片做成 web snippet，允許嵌入
- Mac 小工具
- 不敏感 intelligence 結果的 API

---

## 5. Derived Tables Schema

所有表都在 `derived/history-intelligence.sqlite`。

### 5.1 Visit-Level Derived Facts

```sql
CREATE TABLE visit_derived_facts (
  visit_id           INTEGER PRIMARY KEY,  -- FK → archive.visits.id
  session_id         TEXT,
  trail_id           TEXT,
  registrable_domain TEXT NOT NULL,
  canonical_url      TEXT NOT NULL,
  domain_category    TEXT NOT NULL DEFAULT 'unknown',
  page_category      TEXT NOT NULL DEFAULT 'unknown',
  search_engine      TEXT,                 -- 如果這是搜索事件
  search_query       TEXT,                 -- 提取的搜索詞
  is_new_domain      INTEGER NOT NULL DEFAULT 0,
  is_search_event    INTEGER NOT NULL DEFAULT 0,
  evidence_tier      TEXT NOT NULL DEFAULT 'tier-c',
  taxonomy_source    TEXT NOT NULL DEFAULT 'unknown',
  taxonomy_pack      TEXT,
  taxonomy_version   TEXT,
  computed_at        TEXT NOT NULL
);

CREATE INDEX idx_vdf_session ON visit_derived_facts(session_id);
CREATE INDEX idx_vdf_trail ON visit_derived_facts(trail_id);
CREATE INDEX idx_vdf_domain ON visit_derived_facts(registrable_domain);
CREATE INDEX idx_vdf_search ON visit_derived_facts(is_search_event, search_engine);
```

### 5.2 Daily Rollups

```sql
CREATE TABLE domain_daily_rollups (
  date_key           TEXT NOT NULL,         -- 'YYYY-MM-DD'
  registrable_domain TEXT NOT NULL,
  domain_category    TEXT NOT NULL,
  visit_count        INTEGER NOT NULL,
  search_count       INTEGER NOT NULL,
  new_domain_visits  INTEGER NOT NULL,
  unique_urls        INTEGER NOT NULL,
  PRIMARY KEY(date_key, registrable_domain)
);

CREATE TABLE category_daily_rollups (
  date_key           TEXT NOT NULL,
  domain_category    TEXT NOT NULL,
  visit_count        INTEGER NOT NULL,
  unique_domains     INTEGER NOT NULL,
  PRIMARY KEY(date_key, domain_category)
);

CREATE TABLE engine_daily_rollups (
  date_key           TEXT NOT NULL,
  search_engine      TEXT NOT NULL,
  search_count       INTEGER NOT NULL,
  PRIMARY KEY(date_key, search_engine)
);

CREATE TABLE daily_summary_rollups (
  date_key           TEXT PRIMARY KEY,
  total_visits       INTEGER NOT NULL,
  total_searches     INTEGER NOT NULL,
  new_domains        INTEGER NOT NULL,
  unique_domains     INTEGER NOT NULL,
  hhi_score          REAL,                  -- Herfindahl index
  discovery_rate     REAL                   -- new domain visits / total
);
```

### 5.3 Structural Entities

```sql
CREATE TABLE sessions (
  session_id         TEXT PRIMARY KEY,
  first_visit_ms     INTEGER NOT NULL,
  last_visit_ms      INTEGER NOT NULL,
  visit_count        INTEGER NOT NULL,
  search_count       INTEGER NOT NULL,
  domain_count       INTEGER NOT NULL,
  is_deep_dive       INTEGER NOT NULL DEFAULT 0,
  auto_title         TEXT,
  computed_at        TEXT NOT NULL
);

CREATE TABLE search_trails (
  trail_id           TEXT PRIMARY KEY,
  session_id         TEXT,
  initial_query      TEXT NOT NULL,
  search_engine      TEXT NOT NULL,
  reformulation_count INTEGER NOT NULL DEFAULT 0,
  visit_count        INTEGER NOT NULL,
  landing_url        TEXT,
  landing_domain     TEXT,
  first_visit_ms     INTEGER NOT NULL,
  last_visit_ms      INTEGER NOT NULL,
  max_depth          INTEGER NOT NULL DEFAULT 0,
  queries_json       TEXT NOT NULL,          -- ordered query list
  computed_at        TEXT NOT NULL
);

CREATE TABLE search_trail_members (
  trail_id           TEXT NOT NULL,
  visit_id           INTEGER NOT NULL,
  ordinal            INTEGER NOT NULL,
  role               TEXT NOT NULL,          -- 'search_event' / 'click' / 'landing'
  PRIMARY KEY(trail_id, visit_id)
);

CREATE TABLE search_events (
  visit_id           INTEGER PRIMARY KEY,
  search_engine      TEXT NOT NULL,
  raw_query          TEXT NOT NULL,
  normalized_query   TEXT NOT NULL,
  trail_id           TEXT,
  computed_at        TEXT NOT NULL
);

CREATE TABLE search_event_terms (
  visit_id           INTEGER NOT NULL,
  term               TEXT NOT NULL,
  PRIMARY KEY(visit_id, term)
);

CREATE TABLE query_families (
  family_id          TEXT PRIMARY KEY,
  anchor_query       TEXT NOT NULL,
  member_count       INTEGER NOT NULL,
  search_engine      TEXT NOT NULL,
  first_seen_ms      INTEGER NOT NULL,
  last_seen_ms       INTEGER NOT NULL,
  queries_json       TEXT NOT NULL,
  computed_at        TEXT NOT NULL
);

CREATE TABLE search_engine_rules (
  rule_id            TEXT PRIMARY KEY,
  engine_id          TEXT NOT NULL,
  display_name       TEXT NOT NULL,
  host_pattern       TEXT NOT NULL,
  path_prefix        TEXT,
  query_param_key    TEXT NOT NULL,
  example_url        TEXT,
  note               TEXT,
  enabled            INTEGER NOT NULL DEFAULT 1,
  created_at         TEXT NOT NULL,
  updated_at         TEXT NOT NULL
);

CREATE TABLE refind_pages (
  canonical_url      TEXT PRIMARY KEY,
  url                TEXT NOT NULL,
  title              TEXT,
  registrable_domain TEXT NOT NULL,
  cross_day_count    INTEGER NOT NULL,
  trail_count        INTEGER NOT NULL,
  search_arrival_count INTEGER NOT NULL,
  typed_revisit_count INTEGER NOT NULL,
  refind_score       REAL NOT NULL,
  evidence_json      TEXT NOT NULL,
  first_seen_ms      INTEGER NOT NULL,
  last_seen_ms       INTEGER NOT NULL,
  computed_at        TEXT NOT NULL
);

CREATE TABLE source_effectiveness (
  registrable_domain TEXT PRIMARY KEY,
  source_role        TEXT NOT NULL,          -- 'entry' / 'landing' / 'reference'
  trail_count        INTEGER NOT NULL,
  stable_landing_count INTEGER NOT NULL,
  effectiveness_score REAL NOT NULL,
  evidence_json      TEXT NOT NULL,
  computed_at        TEXT NOT NULL
);

CREATE TABLE habit_patterns (
  registrable_domain TEXT PRIMARY KEY,
  habit_type         TEXT NOT NULL,          -- 'daily' / 'weekly' / 'periodic'
  mean_interval_days REAL NOT NULL,
  cv                 REAL NOT NULL,
  visit_count        INTEGER NOT NULL,
  last_visited_ms    INTEGER NOT NULL,
  is_interrupted     INTEGER NOT NULL DEFAULT 0,
  computed_at        TEXT NOT NULL
);

CREATE TABLE reopened_investigations (
  investigation_id   TEXT PRIMARY KEY,
  anchor_type        TEXT NOT NULL,          -- 'query_family' / 'reference_page'
  anchor_id          TEXT NOT NULL,
  occurrence_count   INTEGER NOT NULL,
  distinct_days      INTEGER NOT NULL,
  first_seen_ms      INTEGER NOT NULL,
  last_seen_ms       INTEGER NOT NULL,
  evidence_json      TEXT NOT NULL,
  computed_at        TEXT NOT NULL
);

CREATE TABLE path_flows (
  flow_pattern       TEXT NOT NULL,          -- 'google.com → github.com'
  step_count         INTEGER NOT NULL,       -- 2 or 3
  occurrence_count   INTEGER NOT NULL,
  last_seen_ms       INTEGER NOT NULL,
  PRIMARY KEY(flow_pattern, step_count)
);
```

---

## 6. 跨瀏覽器覆蓋矩陣

| 功能              | Chromium           | Firefox             | Safari               | Takeout    |
| ----------------- | ------------------ | ------------------- | -------------------- | ---------- |
| 1.1 時間段摘要    | ✅                 | ✅                  | ✅                   | ✅         |
| 1.2 歷史上的今天  | ✅                 | ✅                  | ✅                   | ✅         |
| 2.1 Top 網站      | ✅                 | ✅                  | ✅                   | ✅         |
| 2.2 搜索活動      | ✅ `search_terms`  | ✅ `search_queries` | ⚠️ URL 規則          | ✅         |
| 2.3 常重找頁面    | ✅                 | ✅                  | ✅                   | ✅         |
| 2.4 習慣識別      | ✅                 | ✅                  | ✅                   | ✅         |
| 3.1 瀏覽會話      | ✅                 | ✅                  | ✅                   | ✅         |
| 3.2 搜索旅程      | ✅ `from_visit`    | ✅ `from_visit`     | ⚠️ redirect only     | ✅ Session |
| 3.3 導航溯源      | ✅                 | ✅                  | ⚠️ partial           | ✅         |
| 4.1 網站深度分析  | ✅                 | ✅                  | ✅                   | ✅         |
| 4.2 穩定答案來源  | ✅                 | ✅                  | ⚠️                   | ✅         |
| 4.3 搜索效率      | ✅                 | ✅                  | ⚠️                   | ✅         |
| 4.4 碰壁偵測      | ✅ `response_code` | ✅ pattern          | ✅ `load_successful` | ⚠️ pattern |
| 4.5 反覆查的問題  | ✅                 | ✅                  | ⚠️                   | ✅         |
| 4.6 瀏覽節奏      | ✅                 | ✅                  | ✅                   | ✅         |
| 4.7 探索率        | ✅                 | ✅                  | ✅                   | ✅         |
| 4.8 使用構成      | ✅                 | ✅                  | ✅                   | ✅         |
| 4.9 集中度        | ✅                 | ✅                  | ✅                   | ✅         |
| 4.10 深度 Session | ✅                 | ✅                  | ⚠️                   | ✅         |
| 4.11 常見路線     | ✅                 | ✅                  | ⚠️                   | ✅         |
| 4.12 比較頁面組   | ✅                 | ✅                  | ⚠️                   | ✅         |
| 4.13 多瀏覽器對比 | ✅                 | ✅                  | ✅                   | ✅         |
| 4.14 觀測互動     | ✅ foreground      | ✅ scroll/key       | ⚠️ load only         | ❌         |

✅ = 完整覆蓋 | ⚠️ = 部分覆蓋 / 降級可用 | ❌ = 不可用

---

## 7. 建議落地順序

### Phase 1 — 核心架構 + 最高 ROI（必做）

這批建立整個 intelligence 的數據基礎，且直接提供最高用戶價值：

| #   | 功能                 | 理由                       |
| --- | -------------------- | -------------------------- |
| 1   | 架構基礎 1.A-E       | 所有功能的前提             |
| 2   | 3.1 瀏覽會話         | 最基礎的分段，很多功能依賴 |
| 3   | 3.2 搜索旅程         | 最核心的結構化能力         |
| 4   | 2.2 搜索活動 (A+B+C) | 搜索旅程的呈現層           |
| 5   | 2.1 Top 網站統計     | 最基礎排行榜               |
| 6   | 2.3 常重找的頁面     | 高價值功能                 |
| 7   | 4.8 使用構成         | Digest 卡片的核心材料      |
| 8   | 1.1 時間段摘要       | 依賴上面所有功能的匯總入口 |
| 9   | 3.3 導航溯源         | Explorer 的核心增強        |
| 10  | 可解釋性面板         | 信任基礎                   |

### Phase 2 — 深度洞察（強烈建議做）

| #   | 功能                 |
| --- | -------------------- |
| 11  | 4.2 穩定答案來源     |
| 12  | 4.3 搜索效率分析     |
| 13  | 4.4 碰壁與高摩擦偵測 |
| 14  | 4.5 反覆回來查的問題 |
| 15  | 4.1 網站深度分析     |
| 16  | 4.6 瀏覽節奏熱圖     |
| 17  | 4.7 探索率趨勢       |
| 18  | 1.2 歷史上的今天     |

### Phase 3 — 進階分析（錦上添花）

| #   | 功能                  |
| --- | --------------------- |
| 19  | 4.9 集中度分析        |
| 20  | 2.4 習慣識別          |
| 21  | 4.10 深度研究 Session |
| 22  | 4.11 常見路線         |
| 23  | 4.14 觀測互動模塊     |

### Phase 4 — 進階功能

| #   | 功能                       |
| --- | -------------------------- |
| 24  | 4.12 比較頁面組            |
| 25  | 4.13 多瀏覽器/Profile 對比 |
| 26  | 外部服務/嵌入              |

---

## 8. 明確不做的東西

| 不做                                    | 為什麼                                                              |
| --------------------------------------- | ------------------------------------------------------------------- |
| Raw Top 搜索詞排行                      | 噪音太高。用搜索活動（token 頻率 + query family）完全取代           |
| Thread / Open Loop 作為一級 UI 概念     | 太抽象，沒人懂。用「反覆回來查的問題」取代——只說事實                |
| 大範圍 Topic Clustering（無 embedding） | 又貴又假準。只在有強錨點時才合併                                    |
| 弱推算 Dwell Time                       | 明確遵守——沒有強數據就不顯示                                        |
| 任意窗口全量物化                        | Storage 和 invalidation 會拖死。用 daily rollup + on-demand compose |

---

## 9. 各方案貢獻追溯

| 功能                                          | 主要來源                                                |
| --------------------------------------------- | ------------------------------------------------------- |
| 架構基礎（任務隊列、rollup、canonical model） | Gemini 2.5 + Gemini 3.1 + GPT Pro（三方共識）           |
| Site Dictionary / Rules Editor                | GPT Pro（最完整）+ 原始草稿                             |
| 瀏覽會話                                      | Gemini 2.5（session 定義最完整）                        |
| 搜索旅程                                      | Gemini 2.5 + 3.1 + GPT Pro（三方都提出，Gemini 最清晰） |
| 搜索活動（取代 Top 搜索詞）                   | GPT Pro（重構最清楚）+ Claude + Gemini 3.1              |
| 常重找的頁面                                  | GPT Pro（多維度信號 + explainability）                  |
| 穩定答案來源                                  | GPT Pro                                                 |
| 搜索效率分析                                  | Claude Sonnet 4.6                                       |
| 碰壁偵測                                      | Gemini 3.1 + GPT Pro（弱證據 fallback）                 |
| 反覆查的問題                                  | GPT Pro（Reopened Investigations）                      |
| 瀏覽節奏熱圖                                  | Claude Sonnet 4.6                                       |
| 探索率趨勢                                    | Claude Sonnet 4.6                                       |
| 使用構成                                      | GPT Pro + Claude                                        |
| 集中度分析                                    | Claude Sonnet 4.6（HHI/Gini）                           |
| 習慣識別                                      | Claude Sonnet 4.6（CV 分析 + 中斷偵測）                 |
| 導航溯源 + Hub 頁面                           | Claude Sonnet 4.6                                       |
| 深度 Session                                  | Claude Sonnet 4.6                                       |
| 常見路線                                      | GPT Pro                                                 |
| 比較頁面組                                    | GPT Pro                                                 |
| 多瀏覽器對比                                  | Claude Sonnet 4.6                                       |
| 觀測互動模塊                                  | Gemini 3.1 + GPT Pro（capability-gated 設計）           |
| 可解釋性面板                                  | GPT Pro                                                 |
| 網站深度分析                                  | Gemini 2.5                                              |
| 模塊化架構                                    | 原始草稿                                                |

---

## 10. 一句話定義

> **Core Intelligence 不是在猜你在想什麼，而是在大規模歷史資料中，用可解釋、可回溯、可增量重建的方式，把「搜索、路徑、重找、落點、習慣」這些高價值結構提取出來，讓你的瀏覽歷史從一條條 URL 變成一個個有上下文的故事。**

---

## 附錄 A：搜索引擎識別規則範例

```json
{
  "google.com": { "params": ["q"], "path_prefix": "/search" },
  "bing.com": { "params": ["q"], "path_prefix": "/search" },
  "duckduckgo.com": { "params": ["q"] },
  "youtube.com": { "params": ["search_query"], "path_prefix": "/results" },
  "bilibili.com": { "params": ["keyword"], "path_prefix": "/search" },
  "github.com": { "params": ["q"], "path_prefix": "/search" },
  "baidu.com": { "params": ["wd", "word"] },
  "taobao.com": { "params": ["q"], "path_prefix": "/search" },
  "amazon.com": { "params": ["k"], "path_prefix": "/s" },
  "zhihu.com": { "params": ["q"], "path_prefix": "/search" },
  "stackoverflow.com": { "params": ["q"], "path_prefix": "/search" }
}
```

## 附錄 B：Tracking Params 黑名單範例

```
utm_source, utm_medium, utm_campaign, utm_term, utm_content,
fbclid, gclid, gclsrc, dclid, msclkid,
mc_eid, mc_cid, _ga, _gl, _hsenc, _hsmi,
ref, source, ref_src, ref_url,
campaign_id, ad_id, adset_id
```

## 附錄 C：Domain → Category 內建規則範例

```
# developer
github.com, gitlab.com, stackoverflow.com, developer.mozilla.org,
docs.rs, crates.io, npmjs.com, pypi.org

# video
youtube.com, bilibili.com, vimeo.com, twitch.tv

# social
twitter.com, x.com, reddit.com, facebook.com, instagram.com,
weibo.com, zhihu.com, v2ex.com

# shopping
amazon.com, taobao.com, jd.com, ebay.com, shopee.com

# search
google.com, bing.com, duckduckgo.com, baidu.com

# docs
notion.so, confluence.atlassian.com, readthedocs.io

# ai
chat.openai.com, claude.ai, gemini.google.com, poe.com

# news
bbc.com, cnn.com, reuters.com, hackernews.com

# finance
yahoo.com/finance, bloomberg.com, investing.com

# entertainment
netflix.com, spotify.com, disneyplus.com
```

## 附錄 D：顯示別名內建範例

```
bilibili.com    → BiliBili
youtube.com     → YouTube
github.com      → GitHub
stackoverflow.com → Stack Overflow
spotify.com     → Spotify
google.com      → Google
twitter.com     → X (Twitter)
reddit.com      → Reddit
amazon.com      → Amazon
notion.so       → Notion
```
