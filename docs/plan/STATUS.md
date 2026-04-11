# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：PG / M5 — Awaiting Next Work Block**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- 目前沒有待排的 work block。`BACKLOG.md` 也已清空，等待新的 milestone 或新的使用者指派。

---

> 2026-04-10 unblock：使用者已對 `ADR-006` 明確 sign off，`WORK-M5-A` 因此從 proposal / blocked 轉為 active。M4 closeout 仍維持完成，但 2026-04-10 也補修了 onboarding archive-mode IPC 契約與 insights refresh queue regression。

> 2026-04-10 closeout：`WORK-M5-A` 已完成，deterministic foundation / taxonomy、first-party-only enrichment runtime、dual built-in plugin defaults，以及 Settings / Insights queue review / retry / cancel surface 現在都已回寫到 source docs 與實作。

> 2026-04-10 backend size closeout：使用者臨時插單的 `WORK-QC-E` 已完成。macOS release executable 透過 native keyring backend slim-down + release strip/LTO，從 `190M` 降到 `104M`；更深一層的 optional intelligence build-boundary 問題已誠實回收到 `BACKLOG.md` 的 `WORK-QC-F`。

> 2026-04-10 packaging closeout：使用者已明確 sign off 保留 default desktop build 內建 optional AI / MCP / semantic runtime；`WORK-QC-F` 因此以 [ADR-009](../architecture/decisions/009-default-desktop-optional-intelligence-shipping.md) 與 `artifacts/release/2026-04-11-size-audit/` 的 refreshed evidence 正式收口。當前 truth 是：web payload 仍低於 `1 MB`，而 unsigned macOS executable 約 `104 MiB`，這個重量現在屬 accepted trade-off，而不是 active blocker。

> 2026-04-10 platform quality closeout：使用者臨時插單的 `WORK-QC-G` 已完成。`vault-platform` 已拆成 keyring / scheduler / launcher / host capability / discovery 子模組，`bun run check` 現在固定納入 `check:platform`，會在對應 host 上跑 native keyring / scheduler / launcher / discovery / biometric smoke；updater 也已收回 typed desktop command surface，不再讓前端直接調 plugin guest API。

> 2026-04-10 testing closeout：使用者臨時插單的 `WORK-QC-H` 已完成。repo 現在有 feature-gated `desktop:dev:bridge` / `test:e2e:desktop-bridge` local dev loop，能在 macOS 上把前端跑進 Chrome 並透過 localhost 命中真實 Rust desktop command façade；`browser-preview`、`browser-desktop-bridge`、`tauri` 三種 runtime 邊界也已回寫到 quality / architecture docs。

> 2026-04-10 code-review sweep closeout：`WORK-QC-I` 與 `WORK-QC-J` 已完成。remote backup verify 現在補上 detached manifest checksum + zip entry-set drift detection、App Lock / rekey / import recoverability gaps 已回補、Insights scoped stale-state 與 Explorer drilldown 保 scope、derived rebuild / bridge updater / release size audit provenance 也都已用 regression tests 與 source docs 收口。

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
