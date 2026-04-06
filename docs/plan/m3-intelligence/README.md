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

---

## 本里程碑文檔

- [providers-indexing-and-jobs.md](providers-indexing-and-jobs.md)
- [search-assistant-and-insights.md](search-assistant-and-insights.md)

---

## 里程碑檢查表

- [ ] `M3-001` provider 和 index build 流程已驗證可重跑、可清空、可重建。
- [ ] `M3-002` semantic search 有 evidence、篩選和降級路徑。
- [ ] `M3-003` AI assistant 只能在有明確 evidence 時回答，沒有 evidence 時必須誠實退化。
- [ ] `M3-004` On This Day、Site Analytics、Periodic Summary、Topic Timeline 至少形成第一版可驗收體驗。
