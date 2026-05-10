# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M15 — v0.2.0 Release Closeout**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [x] **WORK-RELEASE-020-A** — v0.2.0 Planning Repair, Security Refresh, And Publication
  - 讀先：
    `README.md`
    `RELEASE.md`
    `docs/plan/BACKLOG.md`
    `docs/plan/CHANGELOG.md`
    `docs/plan/program/quality-matrix.md`
    `docs/features/intelligence.md`
    `docs/features/intelligence-current-state.md`
    `docs/features/archive.md`
    `docs/architecture/tech-stack.md`
    `docs/design/screens-and-nav.md`
    `.github/workflows/release.yml`
  - 目標：把 v0.2.0 發佈 truth 收斂到已完成內容，先處理 Dependabot alerts，再修復 milestone / backlog / status / source docs 的 v0.2 / v0.3 out-of-sync，最後 bump、驗證、tag、發佈 v0.2.0。
  - v0.2.0 發佈範圍：Lexical Recall V2、advanced keyword syntax、Windows unsigned installer / scheduler preview、release/security hardening、既有 archive / deterministic Core Intelligence。
  - 移出 v0.2.0 的 blocker：AI Assistant、embedding、semantic / hybrid search、MCP / skill artifacts、vector sidecar、readable webpage body fetch。這些全部搬到 `BACKLOG.md` 的 `WORK-AI-V03-A` / `WORK-READABLE-CONTENT-V03-A`，作為 v0.3.0 blocker 管理。
  - 契約：不可假裝 AI / readable-content 已可用；user-visible copy 必須同步 `en` / `zh-CN` / `zh-TW`；release 前必須處理 Dependabot alerts、跑 `bun run check` 與 `bun run verify`；release notes 必須包含本次 release 相關的真實 app 截圖。
  - 驗收：
    - GitHub Dependabot alerts #13 / #15 (`openssl`) 與 #14 (`tauri`) 已更新到 patched dependency versions；GitHub alert state 以 dependency graph rescan 為準。
    - `README.md`、feature / architecture / design docs、`BACKLOG.md`、`STATUS.md`、`CHANGELOG.md` 對 v0.2.0 / v0.3.0 scope 一致。
    - app 內 disabled AI / readable-content copy 改為 v0.3 roadmap，且三語 i18n parity 維持 100%。
    - `bun run check`、`bun run verify` 通過；release screenshot assets 由當前 app 產生並嵌入 GitHub release note。
  - 2026-05-09 closeout：v0.2.0 發佈 scope 收斂到已完成的 local-first archive、Lexical Recall V2 / advanced keyword syntax、deterministic Core Intelligence、Windows unsigned installer / scheduler preview 與 release/security hardening；未完成的 AI Assistant、embedding、semantic / hybrid search、MCP / skill artifacts、vector sidecar、readable webpage body fetch 全部移入 `BACKLOG.md` 的 v0.3.0 blocker blocks。
  - 發佈準備：版本已 bump 到 `0.2.0`；preview fixtures、backend deferred notes、Jobs / Assistant / Settings / Integrations / Explorer copy 與三語 i18n 已同步 v0.2.0 / v0.3 truth；release notes 與真實 app 截圖已產生於 `artifacts/release/v0.2.0/`。
  - 驗證結果：`bun run check` 與 `bun run verify` 通過，包含 100% JS/Rust coverage、browser-preview E2E、desktop-bridge truth gate、desktop-contract mutation gate、Rust supply-chain audit、release config guard 與 debug desktop build rehearsal。

> `BACKLOG.md` 目前的前兩個 blocked blocks 是 v0.3.0 AI / readable-content scope；maintenance / deep mutation hardening 不屬於 v0.2.0 release blocker，除非使用者另外排 dedicated window。
