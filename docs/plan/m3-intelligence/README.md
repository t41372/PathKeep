# M3 — Intelligence

> 目標：在不破壞 Archive 可信基礎的前提下，引入 optional intelligence。  
> 這個階段的原則是：AI 是可關閉、可重建、可降級的增值層。

---

## M3 的完成定義

- AI provider 管理、secret storage、connection test 和 model selection 完成。
- Semantic index 和 query pipeline 以 sidecar / derived-state 的方式落地。
- Job queue、embedding build、semantic search、assistant、insights v1 可用。
- 無 AI 配置時，UI 和 backend 仍能正常退化。
- MCP server 和 IDE skill 可以把個人 history 安全暴露給外部 AI 工具。

> 2026-04-07 狀態註記：M3 這裡的勾選代表 intelligence v1 的功能 slice 已經落地，並且目前 `bun run check`、`bun run build`、`bun run test:e2e` 皆通過。這**不等於** repo 已完成最終 release-readiness signoff；repo-wide JS/Rust coverage 與 full mutation sweep 仍低於最終標準，屬於後續 quality closeout。

---

## 本里程碑文檔

- [providers-indexing-and-jobs.md](providers-indexing-and-jobs.md)
- [search-assistant-and-insights.md](search-assistant-and-insights.md)

---

## 里程碑檢查表

- [x] `M3-001` provider 和 index build 流程已驗證可重跑、可清空、可重建。（2026-04-07，WORK-M3-A）
- [x] `M3-002` semantic search 有 evidence、篩選和降級路徑。（2026-04-07，WORK-M3-B）
- [x] `M3-003` AI assistant 只能在有明確 evidence 時回答，沒有 evidence 時必須誠實退化。（2026-04-07，WORK-M3-B）
- [x] `M3-004` On This Day、Site Analytics、Periodic Summary、Topic Timeline 至少形成第一版可驗收體驗。（2026-04-07，WORK-M3-B）
