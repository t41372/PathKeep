# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M4 Complete / M5 Blocked**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [x] **WORK-M4-K** — Security, Privacy, And Update Boundary
  - 讀先：
    `docs/architecture/decisions/005-app-lock-session-boundary.md`
    `docs/features/archive.md`
    `docs/design/screens-and-nav.md`
    `docs/standards.md`
    `RELEASE.md`
  - 目標：先以決策文檔誠實重開 macOS biometric 與 consented analytics 兩個 accepted-contract 變更，再落地 macOS-only biometric unlock、frontend-only analytics consent boundary，以及 Settings manual update check / release availability surface。
  - 驗收：`bun run verify`

- [x] **WORK-M4-L** — Package Rename, Release Flow, Size Audit, And Code Health
  - 讀先：
    `AGENTS.md`
    `RELEASE.md`
    `docs/standards.md`
    `docs/plan/m4-full-polish/README.md`
    `docs/plan/program/repo-baseline.md`
  - 目標：把 bundle / keyring / data-root namespace 正式改成 `com.yi-ting.pathkeep`，建立真實 version bump / release runbook，並補上 artifact size attribution 與前後端 code health audit，避免發版準備只停在現有 private-release workflow。
  - 驗收：`bun run verify`

---

> 2026-04-10 closeout：`WORK-M4-K` / `WORK-M4-L` 已完成並已 append 至 [CHANGELOG.md](CHANGELOG.md)。目前 `BACKLOG.md` 只剩 blocked 的 `WORK-M5-A` / `WORK-M5-B`，因此這裡暫無新的未阻塞 `[ ]` work block。

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
