# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M4 — Full Polish**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-QC-C** — Program Traceability And Legacy Gap Closeout
  - 讀先：
    `docs/plan/program/README.md`
    `docs/plan/program/traceability-map.md`
    `docs/plan/program/repo-baseline.md`
    `docs/plan/program/research-and-decisions.md`
    `docs/plan/m0-foundation/backend-and-data-rearchitecture.md`
    `docs/plan/m0-foundation/frontend-shell-and-design-system.md`
    `docs/plan/m1-solid-archive/schema-backup-and-ledger.md`
    `docs/plan/m1-solid-archive/schedule-security-and-storage.md`
  - 目標：把目前已落地但尚未回寫的 PG / M0 / M1 source-of-truth 條目一次補齊，包含 traceability、repo baseline gap table、module / command boundary 對照、doctor / snapshot / retention / schedule / security acceptance matrix，以及必要的 truthful defer / accepted rationale。
  - 驗收：`bun run check && bun run build`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
