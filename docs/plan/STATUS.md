# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M4 — Full Polish**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-M4-J** — 60-Year Performance Proof And Shell Scaling
  - 讀先：
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
    `docs/plan/m4-full-polish/large-archive-performance-runbook.md`
    `docs/plan/m4-full-polish/intelligence-60-year-envelope.md`
    `docs/features/intelligence.md`
    `docs/features/archive.md`
  - 目標：為「60 年資料量、所有 insights / AI 開啟、8 GB / 4-core 仍可流暢使用」建立可重跑的真實 evidence：產出 artifact bundle、收斂 whole-shell refresh / route payload / main bundle 風險，並在必要時直接做 code-splitting、query / rerender reduction 或其他實作優化，而不是只補說明文。
  - 驗收：`bun run verify`，且產出至少一份 `artifacts/perf/<date>-large-archive-...>/` artifact bundle

- [ ] **WORK-M4-I** — Advanced Intelligence Plugins And Revisit Surfaces
  - 讀先：
    `docs/features/intelligence.md`
    `docs/plan/m3-intelligence/providers-indexing-and-jobs.md`
    `docs/plan/m4-full-polish/enrichment-advanced-intelligence-and-remote.md`
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
    `docs/plan/program/research-and-decisions.md`
  - 目標：補齊目前仍未 shipping 的 advanced intelligence 主線，至少包含 plugin sandbox / queue family truth、revisit / resurfacing 類功能，或另一項符合 optional / evidence-first 原則的高價值 intelligence surface，避免 M4 README 再把 partial support 誤寫成完成。
  - 驗收：`bun run verify`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
