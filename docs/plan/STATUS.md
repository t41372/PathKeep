# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：Mixed — M1 Recoverability / M4 Boundary**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-M1-D** — Snapshot Restore, Retention, And Rekey Audit Shipping
  - 讀先：
    `docs/features/archive.md`
    `docs/architecture/desktop-command-surface.md`
    `docs/plan/m1-solid-archive/schema-backup-and-ledger.md`
    `docs/plan/m1-solid-archive/schedule-security-and-storage.md`
    `docs/plan/program/research-and-decisions.md`
  - 目標：把目前只停在 deferred / partial support 的 snapshot restore preview / execute、retention / prune、以及 richer rekey audit summary 拉回真正可 shipping 的 recoverability contract，而不是永遠停在 truth-closeout 文檔上。
  - 驗收：`bun run verify`

- [ ] **WORK-M4-K** — Security, Privacy, And Update Boundary
  - 讀先：
    `docs/architecture/decisions/005-app-lock-session-boundary.md`
    `docs/features/archive.md`
    `docs/design/screens-and-nav.md`
    `docs/standards.md`
    `RELEASE.md`
  - 目標：先以決策文檔誠實重開 macOS biometric 與 consented analytics 兩個 accepted-contract 變更，再落地 macOS-only biometric unlock、frontend-only analytics consent boundary，以及 Settings manual update check / release availability surface。
  - 驗收：`bun run verify`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
