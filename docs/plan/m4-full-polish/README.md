# M4 — Full Intelligence & Polish

> 目標：補齊 enrichment、advanced insights、remote backup、release polish 和多平台驗證，讓產品從可用走向可發布。
>
> **2026-04-07 解鎖註記**：`WORK-QC-A` 與 `WORK-QC-B` 都已完成，quality gate / CI / docs truthfulness matrix、prototype / doc parity、desktop-vs-preview 邊界與剩餘 M0-M3 trust debt 都已收斂。M4 現在從 `WORK-M4-A` 正式啟動。
>
> **2026-04-08 closeout 註記**：`WORK-M4-A` 已完成，正式交付 enrichment / derived-state v1、storage analytics / growth insight，以及 remote backup 的 Preview / Manual / Execute / Verify 閉環。M4 剩餘主線收斂到 `WORK-M4-B` 的 release readiness 和 platform polish。
>
> **2026-04-08 closeout 註記（晚）**：`WORK-M4-B` 已完成，正式交付 release / support 文檔、platform validation runbook、release workflow preflight、以及 Settings 的 build / path diagnostics。release-ready 主線驗收已收斂，但 pre-release `mutation:rust` 深檢另外暴露出 parser / AI hardening follow-up，已提升成 `WORK-M4-D`；稍後 `WORK-M4-C` 也已透過 ADR-005 + App Lock 實作正式 close out。
>
> **2026-04-08 性能 triage 註記**：用真實大型 Chromium profile（Yi-Ting 的 Chrome profile）跑 manual backup 時，匯入完成後 app 仍有明顯卡頓，說明 M4 的剩餘風險不只在 mutation hardening，也在 large-archive usability。第一輪止血已完成：manual backup 不再同步卡住 insights rebuild、busy overlay 已補 phase/detail、ingest path 減少部分重複序列化與 per-visit URL bounds update、dashboard totals 改成優先讀 cached run stats；但 Explorer 的 `LIKE` read path、parser materialization、以及 whole-app profiling artifact 仍未完成，因此額外切出 `WORK-M4-G` 作為大型 archive 性能主線 follow-up。
>
> **2026-04-08 性能 closeout 註記**：`WORK-M4-G` 已完成，Explorer keyword recall 已切回 FTS5 `history_search` projection，backup overlay 也改為接收 profile-scoped phase progress event；同時新增 [large-archive-performance-runbook.md](large-archive-performance-runbook.md) 讓 webview trace、Rust sample 與 SQLite query plan 有固定 artifact bundle。當時剩餘未完成的 M4 主線回到 `WORK-M4-D`、`WORK-M4-E`、`WORK-M4-F`，以及 blocked 的 `WORK-M4-C`；其後 `WORK-M4-D` / `WORK-M4-F` 已另行 close out。
>
> **2026-04-08 mutation closeout 註記**：`WORK-M4-D` 已把 Rust mutation baseline 誠實收斂成兩塊：`browser-history-parser` crate，以及 `vault-core/src/ai.rs` 的 AI status/helper slice。`bun run mutation:rust` 和 GitHub `rust-mutation` workflow 現在只對這個 contract 背書；`bun run mutation:rust:full` 則保留作 exploratory whole-workspace triage。parser `open_readonly` 的 `|` / `^` 變異也已明確標記為等價 mutant，而不是再冒充真缺測。
>
> **2026-04-08 UX closeout 註記**：使用者改派後，原本暫掛 `ANTIGRAVITY-FE` 的 `WORK-M4-H` 已和 `WORK-M4-F` 一起收斂完成。`Insights` 現在正式對齊 shared profile scope honesty；`Assistant` / `Audit` / `Schedule` 的 IA 與 disabled / verify surface 也已補齊；raw internal route reload 與 i18n / app-shell / trust-flow / intelligence-surface checker drift 已全部收斂回 `bun run check && bun run build`。之後 `WORK-M4-E` 也已把 loading grammar 收斂為 skeleton + readable progress contract，而 `WORK-M4-C` 則正式補上 App Lock 的 session boundary、lock route、desktop / MCP refusal path 與 platform degradation copy。

---

## M4 的完成定義

- Enrichment plugin system 和至少數個核心插件可用。
- Advanced insights 和長期分析能力補齊。
- Remote backup、storage breakdown、release docs、多平台 validation 完成。
- Release workflow、README、CONTRIBUTING、驗收清單可支撐公開發版。

---

## 本里程碑文檔

- [enrichment-advanced-intelligence-and-remote.md](enrichment-advanced-intelligence-and-remote.md)
- [platform-release-and-polish.md](platform-release-and-polish.md)
- [large-archive-performance-runbook.md](large-archive-performance-runbook.md)

---

## 里程碑檢查表

- [x] `M4-001` enrichment 和 remote backup 完成基本可用版本。
- [x] `M4-002` advanced insights 和 storage analytics 完成。
- [x] `M4-003` macOS / Windows / Linux 都完成至少一輪真正的發版前驗證。
- [x] `M4-004` README、CONTRIBUTING、release pipeline 和 docs 皆對齊最終產品。
