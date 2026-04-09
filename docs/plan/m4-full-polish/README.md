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
>
> **2026-04-09 intelligence truth closeout 註記**：`WORK-QC-D` 沒有把 M4 粉飾成「advanced intelligence 全做完」。這輪實際完成的是 semantic stale / cost read model、MCP consent / scope / audit preview、model-scoped index readiness 與 run-type truth；同時新增 [intelligence-60-year-envelope.md](intelligence-60-year-envelope.md)，明確標出 repo 目前**不能**聲稱已通過「60 年資料量、所有 AI 開啟、8 GB / 4-core 仍流暢」的最終性能背書。revisit / resurfacing、plugin sandbox、獨立 enrichment queue family 仍屬 deferred。
>
> **2026-04-09 shell-scaling 註記**：shell 已切成 route-level chunks，checked-in artifact bundle [`artifacts/perf/2026-04-09-large-archive-shell-scaling/`](../../../artifacts/perf/2026-04-09-large-archive-shell-scaling/) 也已重新補回可重跑的 `bun run perf:artifact:shell` 入口。當前 bundle 會從最新 production build 生成 `context.md`、`shell-payload-summary.json`、`route-chunk-breakdown.md`、synthetic `sqlite-query-plan.txt`，以及誠實標記的 placeholder `webview-trace.json` / `rust-sample.txt`；目前可重跑的 shell-scale 證據是 base shell 約 `580261` bytes、heaviest first route 約 `629465` bytes（`settings`）。這仍**不等於**已完成真實 large-profile replay。
>
> **2026-04-09 審核結論**：M4 仍未整體完成。release / support / remote backup / truthful intelligence v1 已簽收；尚未簽收的是兩塊真正會決定「設計文檔是否全完成」的剩餘工作：`WORK-M4-J` 的 60-year performance proof，以及 `WORK-M4-I` 的 advanced intelligence shipping（plugin sandbox / queue family / revisit surfaces）。`bun run verify` 現在已可在這台機器上重新跑到全綠；`WORK-M4-J` 仍不能 close out 的原因不再是 CI / build gate，而是 checked-in bundle 仍屬 synthetic shell-scaling 證據，尚未補到一次真實 large-profile replay。

---

## M4 的完成定義

> 2026-04-09 註記：下列條目描述的是 **full M4 signoff**，不是目前已完成的 truthful partial support。

- Enrichment plugin system 和至少數個核心插件可用。
- Advanced insights 和長期分析能力補齊。
- Remote backup、storage breakdown、release docs、多平台 validation 完成。
- Release workflow、README、CONTRIBUTING、驗收清單可支撐公開發版。

---

## 本里程碑文檔

- [enrichment-advanced-intelligence-and-remote.md](enrichment-advanced-intelligence-and-remote.md)
- [platform-release-and-polish.md](platform-release-and-polish.md)
- [large-archive-performance-runbook.md](large-archive-performance-runbook.md)
- [intelligence-60-year-envelope.md](intelligence-60-year-envelope.md)

---

## 里程碑檢查表

- [~] `M4-001` enrichment 和 remote backup 完成基本可用版本。
  - remote backup PME、derived-state v1 與 `readable-content-refetch` 已交付；plugin sandbox、獨立 queue family 與更多 core plugins 仍未 shipping。
- [~] `M4-002` advanced insights 和 storage analytics 完成。
  - storage analytics、scoped insights、semantic / assistant truth boundary 已交付；revisit / resurfacing 與 long-horizon advanced intelligence 仍未 shipping，也尚未完成 60-year perf signoff。
- [x] `M4-003` macOS / Windows / Linux 都完成至少一輪真正的發版前驗證。
- [x] `M4-004` README、CONTRIBUTING、release pipeline 和 docs 皆對齊最終產品。
