# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M4 — Full Polish**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-M4-J** — Explorer Recall Stabilization, Import Trust UX, And Desktop Feel
  - 讀先：
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
    `docs/features/archive.md`
    `docs/plan/m2-recall-and-trust/trust-ux-i18n-and-platforms.md`
    `docs/plan/m4-full-polish/large-archive-performance-runbook.md`
  - 目標：收斂目前直接影響信任與可用性的 Recall / shell 問題，至少包含 Explorer 搜索穩定性、第一頁 / 最後一頁 / 指定頁跳轉、翻頁保留滾動位置、desktop feel（去除全頁回彈）、collapsed sidebar 修復，以及 Import / Audit 的 trust-critical UX 重整，避免 M4 剩餘工作只停在 synthetic perf artifact 與局部 polish。
  - 驗收：`bun run verify`

- [ ] **WORK-M4-I** — Deterministic Insights, Retention Honesty, And Site Adapters
  - 讀先：
    `docs/design/screens-and-nav.md`
    `docs/design/ux-principles.md`
    `docs/features/archive.md`
    `docs/features/intelligence.md`
    `docs/plan/m4-full-polish/enrichment-advanced-intelligence-and-remote.md`
    `docs/plan/program/research-and-decisions.md`
  - 目標：補齊目前仍未 shipping 的 deterministic intelligence 主線，至少包含不依賴 embedding / LLM 的 revisit / open-loop / query-evolution 類 insight、browser-retention honesty、以及第一批高價值 site adapters（如影片站 metadata parse），同時維持現有 plugin / queue truth boundary，不把 partial support 誤寫成全部 intelligence 都已完成。
  - 驗收：`bun run verify`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
