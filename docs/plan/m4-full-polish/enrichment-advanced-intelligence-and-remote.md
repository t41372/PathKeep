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

---

## WBS

### Enrichment Plugin System

- [ ] `M4-ER-PL-001` 定義 enrichment plugin contract：輸入資料、輸出欄位、evidence、成本、rate limit、error surface。
- [ ] `M4-ER-PL-002` 決定 plugin execution sandbox 和安全邊界，避免 enrichment 任意讀寫 archive 核心資料。
- [ ] `M4-ER-PL-003` 實作 plugin registry、enable / disable、version、rebuild / invalidate 機制。
- [ ] `M4-ER-PL-004` 將 enrichment job 和 M3 queue 整合，確保 progress、retry、cancel、artifact trace 一致。
- [ ] `M4-ER-PL-005` 為 enrichment-derived fields 定義 canonical vs derived 邊界，避免把不可重現資料偷偷灌進核心表。

### Core Plugins

- [ ] `M4-ER-CP-001` 完成 favicon / title normalization plugin，補強 recall 顯示品質。
- [ ] `M4-ER-CP-002` 完成 page excerpt / readable text fetch plugin，具 robots / 429 / timeout handling。
- [ ] `M4-ER-CP-003` 完成 topic / entity extraction plugin，要求 evidence 回鏈和 confidence 標示。
- [ ] `M4-ER-CP-004` 完成 periodic summarization plugin，支援以 window 或 saved search 為範圍重建。
- [ ] `M4-ER-CP-005` 為每個 plugin 定義 cost guardrail、storage impact、manual trigger 和 clear / rebuild story。

### Advanced Intelligence

- [ ] `M4-ER-AI-001` 擴展 topic timeline 和 periodic summary，支援更長時間窗口和 richer evidence。
- [ ] `M4-ER-AI-002` 實作 storage analytics 和 archive growth insight，顯示來源、成長趨勢、可清理空間。
- [ ] `M4-ER-AI-003` 實作 revisit / resurfacing 類功能，例如 forgotten pages、returning topics、session patterns。
- [ ] `M4-ER-AI-004` 評估並實作至少一項高價值個人 intelligence 功能，但必須符合 optional、evidence-first 原則。
- [ ] `M4-ER-AI-005` 對每個 advanced insight 補上 disable / rebuild / delete controls 和資料來源說明。

### Remote Backup

- [ ] `M4-ER-RB-001` 定義 remote backup bundle format，包含 manifest、checksums、encryption metadata、version info。
- [ ] `M4-ER-RB-002` 決定 remote backup provider scope，至少先支援抽象介面和一個真實 provider 或明確 mockable adapter。
- [ ] `M4-ER-RB-003` 實作 remote push preview、manual instructions、execute，對齊 PME 語法。
- [ ] `M4-ER-RB-004` 實作 remote restore validation，至少能驗證 bundle 完整性、版本相容性、可恢復性。
- [ ] `M4-ER-RB-005` 為 remote backup 建立 retention、prune、retry、bandwidth / error guidance。

### Testing And Acceptance

- [ ] `M4-ER-QA-001` 建立 plugin contract tests，覆蓋 enable / disable、failure、rebuild、clear derived data。
- [ ] `M4-ER-QA-002` 建立 core plugins acceptance，驗證 evidence、cost guardrail、storage impact 可被追蹤。
- [ ] `M4-ER-QA-003` 建立 remote backup acceptance：bundle create、upload、tamper detect、restore preview。
- [ ] `M4-ER-QA-004` 建立 advanced insights acceptance，驗證每個 insight 都可回到原始 archive evidence。
- [ ] `M4-ER-QA-005` 補一輪隱私和資料主權 review，確保 enrichment / remote 不違反核心原則。

---

## Exit Artifacts

- enrichment plugin framework
- 一組高價值內建 plugins
- advanced intelligence modules
- remote backup bundle 能力
