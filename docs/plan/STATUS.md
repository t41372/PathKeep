# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M5 — Deterministic Intelligence**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-M5-B** — Query Groups, Threads, Reference Pages, And Module Registry
  - 讀先：
    `docs/features/deterministic-intelligence.md`
    `docs/plan/m5-deterministic-intelligence/README.md`
    `docs/plan/m5-deterministic-intelligence/groups-threads-and-surfaces.md`
    `docs/plan/m4-full-polish/intelligence-60-year-envelope.md`
    `docs/design/screens-and-nav.md`
  - 目標：把 query groups、query ladders、cross-day thread merge、open loops、source effectiveness、reference pages 與 deterministic module registry 做成 explainable、可重建、可 profile-scope、可 invalidate 的正式 shipping surface。
  - 併入的 M4 deferred work：longer-horizon topic timeline / periodic summary、returning topics / session-pattern families，以及其對應 reference-page / thread surfaces。
  - 驗收：`bun run check && bun run build`

---

> 2026-04-10 unblock：使用者已對 `ADR-006` 明確 sign off，`WORK-M5-A` 因此從 proposal / blocked 轉為 active。M4 closeout 仍維持完成，但 2026-04-10 也補修了 onboarding archive-mode IPC 契約與 insights refresh queue regression。

> 2026-04-10 closeout：`WORK-M5-A` 已完成，deterministic foundation / taxonomy、first-party-only enrichment runtime、dual built-in plugin defaults，以及 Settings / Insights queue review / retry / cancel surface 現在都已回寫到 source docs 與實作。

> 2026-04-10 backend size closeout：使用者臨時插單的 `WORK-QC-E` 已完成。macOS release executable 透過 native keyring backend slim-down + release strip/LTO，從 `190M` 降到 `104M`；更深一層的 optional intelligence build-boundary 問題已誠實回收到 `BACKLOG.md` 的 `WORK-QC-F`。

> 2026-04-10 platform quality closeout：使用者臨時插單的 `WORK-QC-G` 已完成。`vault-platform` 已拆成 keyring / scheduler / launcher / host capability / discovery 子模組，`bun run check` 現在固定納入 `check:platform`，會在對應 host 上跑 native keyring / scheduler / launcher / discovery / biometric smoke；updater 也已收回 typed desktop command surface，不再讓前端直接調 plugin guest API。

> 2026-04-10 testing closeout：使用者臨時插單的 `WORK-QC-H` 已完成。repo 現在有 feature-gated `desktop:dev:bridge` / `test:e2e:desktop-bridge` local dev loop，能在 macOS 上把前端跑進 Chrome 並透過 localhost 命中真實 Rust desktop command façade；`browser-preview`、`browser-desktop-bridge`、`tauri` 三種 runtime 邊界也已回寫到 quality / architecture docs。

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
