# M5-DI — Deterministic Insights And Evidence Controls

> 讀這份文檔的時機：當 insights v1 已可用，但你發現高價值能力仍過度依賴 LLM 或同步 heuristics，缺少 evidence control、rebuild story 和更強的 deterministic layers。  
> 這份文檔的重點是「不用模型也能成立的 intelligence」。

---

## Source Inputs

- [../../features/intelligence.md](../../features/intelligence.md)
- [../../design/screens-and-nav.md](../../design/screens-and-nav.md)
- [../../design/ux-principles.md](../../design/ux-principles.md)
- [../m3-intelligence/search-assistant-and-insights.md](../m3-intelligence/search-assistant-and-insights.md)
- [../m4-full-polish/enrichment-advanced-intelligence-and-remote.md](../m4-full-polish/enrichment-advanced-intelligence-and-remote.md)

---

## 本工作包要交付什麼

- richer deterministic insights：不依賴 LLM 也能提供高信號洞察
- evidence controls：每個洞察都能回到來源、生成窗口、使用的 enrichment / plugin
- insight rebuild / clear / invalidate story

---

## WBS

### Deterministic Insight Modules

- [ ] `M5-DI-MD-001` 補強 On This Day、revisit / resurfacing、returning topics、session pattern 類洞察，要求完全 deterministic 也可成立。
- [ ] `M5-DI-MD-002` 補強 site analytics / source role map / explore vs exploit 的統計層，讓結果可跨 profile 和時間窗口穩定比較。
- [ ] `M5-DI-MD-003` 對 thread / task detection 補上 reopen、cooldown、closure signal 的更明確 heuristics 和 evidence link。
- [ ] `M5-DI-MD-004` 為每個 deterministic insight 定義 rebuild cost、依賴資料、freshness 和 invalidation hook。

### Evidence Controls

- [ ] `M5-DI-EV-001` 為 insight card、topic、thread、profile facet 顯示 generated at、window、source tables、是否包含 enrichment。
- [ ] `M5-DI-EV-002` 建立共用 explanation / evidence drawer grammar，讓引用、摘錄、reason label 和 score band 一致。
- [ ] `M5-DI-EV-003` 對所有 insight 補齊 disabled / stale / degraded state，而不是只在 happy path 顯示結果。
- [ ] `M5-DI-EV-004` 補 clear derived data、full rebuild、single-module rerun 的操作語義和 UX 文案。

### Testing And Acceptance

- [ ] `M5-DI-QA-001` 建立 deterministic insight acceptance，覆蓋 zero-state、single-profile、multi-profile、rollback 後 invalidation。
- [ ] `M5-DI-QA-002` 建立 evidence correctness tests，驗證每個洞察都能回鏈到實際 history rows 或 enrichment rows。
- [ ] `M5-DI-QA-003` 建立 degrade-state UX tests，確保沒有 enrichment、queue 未跑、archive locked 時仍誠實可用。

---

## Exit Artifacts

- 一批更強的 deterministic insights
- evidence control 和 rebuild UX
- deterministic insight acceptance coverage
