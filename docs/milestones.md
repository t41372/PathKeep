# 里程碑

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出。

---

## M1 — Solid Archive

核心目標：把「長期保存與可恢復」做對。

- 正式 schema migration 系統（編號 SQL 檔 + migration 表）
- 增量備份（Chromium）完全可用
- 排程設定（macOS LaunchAgent）
- Run / Import / Rollback 正確的 operation model（軟刪除）
- 基本加密/不加密選擇
- 審計 manifest + hash chain
- Archive 快照 safety net（含 retention 上限）
- 歷史紀錄瀏覽和搜尋（FTS5）
- HTML/JSONL 匯出

## M2 — Recall & Trust

- Google Takeout 導入（含 dry-run, quarantine, 完整可回滾）
- 多瀏覽器支持（Firefox）
- Doctor 完整性檢查
- Run 歷史與回滾 UI
- Preview/Manual/Execute 全面落地
- i18n（en, zh-CN, zh-TW）
- Windows / Linux 排程正式驗證與支持

## M3 — Intelligence

- AI provider 配置 UI
- AI 計算任務系統（Job Queue）
- Embedding pipeline（rig.rs + LanceDB sidecar）+ 語義搜尋
- 基礎洞察：Topic timeline, On This Day, Site Analytics, 定期總結
- Ask My History（AI 問答，rig.rs 驅動 agentic RAG）
- MCP server + AI IDE Skill

## M4 — Full Intelligence & Polish

- 完整洞察套件：Thread detection, Open Loops, Contrastive Summary, Explore/Exploit 等
- Enrichment 插件系統（arXiv, GitHub, YouTube 等）
- S3 遠端備份
- 地理位置記錄（實驗性）
- 多平台完整驗證
