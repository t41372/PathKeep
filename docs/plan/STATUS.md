# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：PG / M5 — Awaiting Next Work Block**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [x] **WORK-QC-L** — Intelligence Recovery And Desktop Truth Gate
  - 讀先：
    `docs/plan/e2e-workflow-tests.md`
    `docs/features/intelligence.md`
    `docs/features/deterministic-intelligence.md`
    `docs/design/screens-and-nav.md`
    `docs/architecture/desktop-command-surface.md`
  - 目標：把 deterministic insights、Settings / Insights copy、desktop-bridge e2e 與 CI 驗收重新收斂成真的可用 surface，而不是 preview fixture / placeholder completion。
  - 契約：backup / import 後 deterministic rebuild 必須自動排入並留下可 review 的 runtime trace；`On This Day` 只能回看過去年份；主產品 UI 不得外露 `m4-v1` / `m5b-v1` 這類內部里程碑版本字串；desktop bridge 必須驗到 live Rust flow，而不是只停在 health / build-info smoke。
  - 驗收：`bun run build`、targeted Rust / Vitest regression tests、`test:e2e:desktop-bridge:truth` 能在有權限的 host 上穩定跑完；source docs 與 plan tracking 同步回寫真實邊界。

- [ ] **WORK-QC-P** — Browser Support Truth Pass And Adapter Playbook
  - 讀先：
    `README.md`
    `docs/features/archive.md`
    `TESTING.md`
    `RELEASE.md`
    `docs/plan/m4-full-polish/release-readiness-runbook.md`
    `docs/reference-review.md`
  - 目標：收斂 PathKeep 對外瀏覽器承諾，只公開承諾目前有本機驗證 evidence 的 Google Chrome 與 macOS Safari baseline，並補一份其他瀏覽器 adapter playbook 給後續開發者。
  - 契約：Firefox 與其他 Chromium / Firefox-family adapter 仍保留實作，但 README / onboarding / root docs 不得再把它們寫成已 shipping promise；browser adapter 要有 discovery / parser / ingest / caveat / tests / local validation evidence 才能升級成 public support。
  - 驗收：README / root docs / source docs / onboarding i18n 與 regression tests 同步更新；Safari 有獨立 baseline acceptance 或 unreadable regression；`bun run check && bun run build`

- [ ] **WORK-QC-N** — Backend Rustdoc Sweep And Module Decomposition
  - 讀先：
    `docs/architecture/data-model.md`
    `docs/architecture/module-boundary-map.md`
    `docs/architecture/desktop-command-surface.md`
    `docs/architecture/tech-stack.md`
    `docs/features/archive.md`
    `docs/features/intelligence.md`
    `docs/features/deterministic-intelligence.md`
  - 目標：把 Rust backend 補成 self-explanatory map。所有 runtime Rust 檔案都要有清楚檔頭與符號級 doc comments，並在補文檔時同步拆掉 `vault-worker`、`vault-core::archive`、`vault-core::{chrome, ai, insights}` 等現有 hotspot 的責任混寫。
  - 契約：維持現有 Tauri command、CLI command、serde payload 與 top-level re-export 穩定；任何行為修正都必須附對應測試與 source-doc 更新。
  - 驗收：`bun run check && bun run build`

---

> 2026-04-10 unblock：使用者已對 `ADR-006` 明確 sign off，`WORK-M5-A` 因此從 proposal / blocked 轉為 active。M4 closeout 仍維持完成，但 2026-04-10 也補修了 onboarding archive-mode IPC 契約與 insights refresh queue regression。

> 2026-04-10 closeout：`WORK-M5-A` 已完成，deterministic foundation / taxonomy、first-party-only enrichment runtime、dual built-in plugin defaults，以及 Settings / Insights queue review / retry / cancel surface 現在都已回寫到 source docs 與實作。

> 2026-04-10 backend size closeout：使用者臨時插單的 `WORK-QC-E` 已完成。macOS release executable 透過 native keyring backend slim-down + release strip/LTO，從 `190M` 降到 `104M`；更深一層的 optional intelligence build-boundary 問題已誠實回收到 `BACKLOG.md` 的 `WORK-QC-F`。

> 2026-04-10 packaging closeout：使用者已明確 sign off 保留 default desktop build 內建 optional AI / MCP / semantic runtime；`WORK-QC-F` 因此以 [ADR-009](../architecture/decisions/009-default-desktop-optional-intelligence-shipping.md) 與 `artifacts/release/2026-04-11-size-audit/` 的 refreshed evidence 正式收口。當前 truth 是：web payload 仍低於 `1 MB`，而 unsigned macOS executable 約 `104 MiB`，這個重量現在屬 accepted trade-off，而不是 active blocker。

> 2026-04-10 platform quality closeout：使用者臨時插單的 `WORK-QC-G` 已完成。`vault-platform` 已拆成 keyring / scheduler / launcher / host capability / discovery 子模組，`bun run check` 現在固定納入 `check:platform`，會在對應 host 上跑 native keyring / scheduler / launcher / discovery / biometric smoke；updater 也已收回 typed desktop command surface，不再讓前端直接調 plugin guest API。

> 2026-04-10 testing closeout：使用者臨時插單的 `WORK-QC-H` 已完成。repo 現在有 feature-gated `desktop:dev:bridge` / `test:e2e:desktop-bridge` local dev loop，能在 macOS 上把前端跑進 Chrome 並透過 localhost 命中真實 Rust desktop command façade；`browser-preview`、`browser-desktop-bridge`、`tauri` 三種 runtime 邊界也已回寫到 quality / architecture docs。

> 2026-04-10 code-review sweep closeout：`WORK-QC-I` 與 `WORK-QC-J` 已完成。remote backup verify 現在補上 detached manifest checksum + zip entry-set drift detection、App Lock / rekey / import recoverability gaps 已回補、Insights scoped stale-state 與 Explorer drilldown 保 scope、derived rebuild / bridge updater / release size audit provenance 也都已用 regression tests 與 source docs 收口。

> 2026-04-11 frontend maintainability closeout：`WORK-QC-K` 已完成。活躍前端 `src/` surface 現在補上 file header 與 declaration-level doc comments，把 shell IA、PME / trust grammar、i18n contract、shared profile scope、design token / typography policy 直接寫回代碼；同時也抽出 `src/pages/settings/helpers.ts`、補齊對應 tests、刪除 stale `src/lib/i18n/messages.ts` duplicate，並補記新的 transitive `RUSTSEC-2026-0097` allowlist rationale 讓 `bun run check` 重新回綠。

> 2026-04-12 intelligence recovery closeout：`WORK-QC-L` 已完成。Jobs / Insights 現在會用真實 queue / enrichment / deterministic runtime 誠實呈現 backlog、needs-review、content-fetch 失敗原因與 analysis snapshot，不再把 deferred work 誤報成整條功能失敗；browser-desktop-bridge truth gate 也已修補 multi-process fixture drift、cold-start cache 與 stale port 問題，`bun run test:e2e:desktop-bridge:truth` 在這台主機上已連續兩次跑綠，後續 hosted-runner platform-native truth 只保留在 manual workflow，不再燒每次 push / PR 的 mainline CI 分鐘。

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
