# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M4 — Full Polish**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-M4-B** — Release Readiness And Platform Polish

---

### WORK-M4-B — Release Readiness And Platform Polish

**目標**：在 `WORK-M4-A` 已經落地 enrichment / advanced intelligence / remote backup v1 之後，完成 release engineering、多平台真機驗收與最後一輪 platform / accessibility / docs polish，讓 M4 真正具備對外發版條件。

**包含範圍**：

1. 完成 macOS / Windows / Linux 的發版前 runbook、真機驗收矩陣與 known limitations，特別補齊 encrypted archive、remote restore、scheduler、keyring fallback、upgrade / reinstall story
2. 收斂 signing / notarization / installer / packaging / release workflow，使 artifact matrix、rollback plan、versioning 與 CI secrets 能支撐內部到外部發版
3. 完成 README / CONTRIBUTING / DEVELOPMENT / TESTING / troubleshooting / support 診斷資訊的最終對齊，讓 docs 與真實 UI / capability 不再脫節
4. 做完整 performance / accessibility / observability / final QA 收尾；若途中發現新的 post-GA 大工作，先回寫 `BACKLOG.md`，不要在 `WORK-M4-B` 內無限擴張

**讀先**：

- `docs/standards.md`
- `docs/plan/m4-full-polish/platform-release-and-polish.md`

**完成訊號**：

- 多平台驗收 / installer / signing / release runbook、README / troubleshooting、performance / accessibility / observability 收尾都已落地，且文檔與產品行為一致
- M4 遺留的 release-critical risk 都已被明確收斂成「已解決」或「明確 deferred with rationale」，不再停留在模糊 TODO
- `docs/plan/m4-full-polish/`、`docs/plan/README.md`、`docs/standards.md`、必要的 feature / architecture / design docs、以及 release workflow / support docs 都已同步
- `bun run check && bun run build`，以及 `platform-release-and-polish.md` 中列出的 final QA / debug build / e2e / traceability sweep 都通過

**預期 commit 類型**：

- `build(release): ...`
- `docs(release): ...`
- `feat(platform): ...`
- `test(release): ...`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
