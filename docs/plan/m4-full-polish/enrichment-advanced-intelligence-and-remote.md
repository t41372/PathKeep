# M4-ER — Enrichment, Advanced Intelligence, And Remote

> 讀這份文檔的時機：當 archive 和 intelligence v1 都已穩定，你要開始補齊長期價值層，例如 enrichment plugins、進階洞察和異地備份。  
> 這個里程碑不再重做基礎，而是把高價值延伸能力做成熟。

---

## Source Inputs

- [../../features/intelligence.md](../../features/intelligence.md)
- [../../features/archive.md](../../features/archive.md)
- [../program/research-and-decisions.md](../program/research-and-decisions.md)
- [../m3-intelligence/providers-indexing-and-jobs.md](../m3-intelligence/providers-indexing-and-jobs.md)
- [../m3-intelligence/search-assistant-and-insights.md](../m3-intelligence/search-assistant-and-insights.md)

---

## 本工作包要交付什麼

- enrichment plugin system 和數個高價值內建插件
- advanced insights 和長期分析能力
- remote backup bundle、驗證和 restore story
- storage breakdown 和維運工具增強

**2026-04-08 closeout (`WORK-M4-A`)**：第一個可驗收 slice 已落地。這一輪正式交付了 `readable-content-refetch` plugin v1、derived-state rebuild / clear boundary、Insights storage analytics / latest growth signal，以及 Settings 中完整的 remote backup PME / bundle verify flow。未完成的多 plugin / richer intelligence / release polish 仍留在後續 M4 工作。

**2026-04-09 truth closeout (`WORK-QC-D`)**：這輪沒有把 M4 補寫成「全部 advanced intelligence 都已完成」。相反地，repo 現在明確凍結了 honest boundary：`readable-content-refetch` 仍是唯一 shipping plugin；plugin sandbox、獨立 enrichment queue family、revisit / resurfacing 類 intelligence 仍未 shipping。隱私 / data-sovereignty review 與 60-year support envelope 已補成正式文檔，見 [intelligence-60-year-envelope.md](intelligence-60-year-envelope.md)。

**2026-04-09 closeout (`WORK-M4-I`)**：deterministic intelligence 主線現已補齊可 shipping 的第一輪：Insights 正式顯示 `open-loop` / `revisit` highlights 與 Chromium query-evolution ladders；Onboarding / Dashboard 的 profile boundary 會誠實標示 browser-retention boundary；`readable-content-refetch` 也加入第一批高價值 site adapters（YouTube / Vimeo 影片 metadata parse）。這仍然**不**代表 plugin sandbox、獨立 enrichment queue family、或長期 horizon intelligence 已全部完成。

**2026-04-10 M4 closeout (`WORK-M4-K` / `WORK-M4-L`)**：為了避免把 M4 寫成「還有一堆未做完但好像快完成」的狀態，這份 WBS 中剩餘的 multi-plugin / sandbox / queue-family / longer-horizon intelligence 條目，現在已明確從 M4 移出並掛回 `WORK-M5-A` / `WORK-M5-B`。M4 真正簽收的是 enrichment / remote backup / deterministic evidence-first slice 的 v1 shipping boundary，不是完整 plugin platform。

---

## WBS

### Enrichment Plugin System

- [x] `M4-ER-PL-001` 定義 enrichment plugin contract：M4-A v1 先凍結 `id` / `enabled` / `version`、queue、derived tables、freshness、error surface 與 Settings review contract。
- [x] `M4-ER-PL-002` 決定 plugin execution sandbox 和安全邊界，避免 enrichment 任意讀寫 archive 核心資料。（2026-04-10：明確移至 `WORK-M5-A`，待 `ADR-006` 接受後再與 deterministic evidence contract 一起重開，不在 M4 內偷渡）
- [x] `M4-ER-PL-003` 實作 plugin registry、enable / disable、version、rebuild / invalidate 機制。
- [x] `M4-ER-PL-004` 將 enrichment job 和 M3 queue 整合，確保 progress、retry、cancel、artifact trace 一致。（2026-04-10：明確移至 `WORK-M5-A`，與新的 deterministic / plugin execution boundary 一起處理）
- [x] `M4-ER-PL-005` 為 enrichment-derived fields 定義 canonical vs derived 邊界，避免把不可重現資料偷偷灌進核心表。

### Core Plugins

- [x] `M4-ER-CP-001` 完成 favicon / title normalization plugin，補強 recall 顯示品質。（2026-04-10：移至 `WORK-M5-A`，等新的 plugin / taxonomy foundation 凍結後再排）
- [x] `M4-ER-CP-002` 完成 page excerpt / readable text fetch plugin v1，具 timeout、fetch / decode / unsupported-content failure surface，robots / explicit 429 backoff policy 先收斂成 non-blocking deferred work。
- [x] `M4-ER-CP-003` 完成 topic / entity extraction plugin，要求 evidence 回鏈和 confidence 標示。（2026-04-10：移至 `WORK-M5-A`，避免在現有 session / dwell-centric contract 上繼續擴寫）
- [x] `M4-ER-CP-004` 完成 periodic summarization plugin，支援以 window 或 saved search 為範圍重建。（2026-04-10：移至 `WORK-M5-A`，待新的 deterministic baseline / module registry 成形後再做）
- [x] `M4-ER-CP-005` 為目前已交付的 plugin 定義 storage impact、manual trigger 和 clear / rebuild story；更細的 cost guardrail 仍待多 plugin 階段補齊。

### Advanced Intelligence

- [x] `M4-ER-AI-001` 擴展 topic timeline 和 periodic summary，支援更長時間窗口和 richer evidence。（2026-04-10：移至 `WORK-M5-B`，作為 groups / threads / reference pages 的後續 surface）
- [x] `M4-ER-AI-002` 實作 storage analytics 和 archive growth insight，顯示來源、成長趨勢、可清理空間。
- [x] `M4-ER-AI-003` 實作 revisit / resurfacing 類功能，例如 forgotten pages、returning topics、session patterns。（2026-04-09，`WORK-M4-I`：先正式 shipping deterministic `open-loop` / `revisit` cards 與 query ladder surface；更長 horizon 的 returning topics / session-pattern families 仍可後續加法補強）
- [x] `M4-ER-AI-004` 評估並實作至少一項高價值個人 intelligence 功能，但必須符合 optional、evidence-first 原則。（2026-04-09，`WORK-M4-I`：query-evolution ladder 以 Chromium search-term evidence 正式落地，保持 non-LLM、evidence-first 與 Explorer deep-link contract）
- [x] `M4-ER-AI-005` 對已交付的 advanced insight / enrichment slice 補上 disable / rebuild / delete controls 和資料來源說明。

### Remote Backup

- [x] `M4-ER-RB-001` 定義 remote backup bundle format，包含 manifest、checksums、archive mode、version info。
- [x] `M4-ER-RB-002` 決定 remote backup provider scope：day-one 以 S3-compatible upload + mockable backend adapter 為正式 boundary。
- [x] `M4-ER-RB-003` 實作 remote push preview、manual instructions、execute，對齊 PME 語法。
- [x] `M4-ER-RB-004` 實作 remote restore validation，至少能驗證 bundle 完整性、版本相容性、可恢復性。
- [x] `M4-ER-RB-005` 為 remote backup 建立 retention / prune / retry guidance，先以 manual-first boundary 落地。

### Testing And Acceptance

- [x] `M4-ER-QA-001` 建立 plugin contract tests，覆蓋 enable / disable、failure、rebuild、clear derived data。
- [x] `M4-ER-QA-002` 建立 core plugins acceptance，驗證 evidence、cost guardrail、storage impact 可被追蹤。（2026-04-10：移至 `WORK-M5-A` / `WORK-M5-B`，待新的 plugin families 真正存在後再做 acceptance）
- [x] `M4-ER-QA-003` 建立 remote backup acceptance：bundle create、upload、tamper detect、restore preview / verify。
- [x] `M4-ER-QA-004` 建立 M4-A advanced insights acceptance，驗證 storage / growth evidence 仍可回到原始 archive / audit evidence。
- [x] `M4-ER-QA-005` 補一輪隱私和資料主權 review，確保 enrichment / remote 不違反核心原則。（2026-04-09，`WORK-QC-D`：見 [intelligence-60-year-envelope.md](intelligence-60-year-envelope.md) 的 `Privacy And Data Sovereignty Review`。）

---

## Exit Artifacts

- enrichment plugin framework
- 一組高價值內建 plugins
- advanced intelligence modules
- remote backup bundle 能力

## M5 Rehome Map

- `WORK-M5-A`：`M4-ER-PL-002`、`M4-ER-PL-004`、`M4-ER-CP-001`、`M4-ER-CP-003`、`M4-ER-CP-004`、`M4-ER-QA-002`
- `WORK-M5-B`：`M4-ER-AI-001`，以及更長 horizon 的 returning-topics / session-pattern / reference-page 家族
