# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M5 — Deterministic Intelligence**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [/] **WORK-M5-A** — Deterministic Evidence Contract, Foundation, And Taxonomy
  - 讀先：
    `docs/architecture/decisions/006-deterministic-intelligence-boundary.md`
    `docs/features/deterministic-intelligence.md`
    `docs/features/intelligence.md`
    `docs/architecture/data-model.md`
    `docs/architecture/module-boundary-map.md`
    `docs/architecture/tech-stack.md`
  - 目標：凍結 honest evidence contract、URL normalization / registrable-domain / search-parser baseline、多維 taxonomy precedence、region rule packs、script-aware tokenization 與 unknown / override governance，避免 deterministic intelligence 再建立在 estimated dwell 或 session-duration 假設上。
  - 進度：2026-04-10 已接受 `ADR-006`、解除 M5 blocker，並落地第一版 `vault-core::deterministic` normalization / search-parser foundation；其餘 taxonomy / rule-pack / override / evidence-tier contract 仍待本 block 收尾。
  - 併入的 M4 deferred work：plugin execution sandbox、dedicated enrichment queue family、favicon / title normalization、topic / entity extraction、periodic summarization plugin 與對應 acceptance contract。
  - 驗收：`bun run check && bun run build`

---

> 2026-04-10 unblock：使用者已對 `ADR-006` 明確 sign off，`WORK-M5-A` 因此從 proposal / blocked 轉為 active。M4 closeout 仍維持完成，但 2026-04-10 也補修了 onboarding archive-mode IPC 契約與 insights refresh queue regression。

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
