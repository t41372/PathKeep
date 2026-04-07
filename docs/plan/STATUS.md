# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M2 — Recall & Trust**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-M2-A** — Imports, Rollback, And Multi-Browser

---

### WORK-M2-A — Imports, Rollback, And Multi-Browser

**目標**：把 canonical archive 從「可備份 Chromium」提升到「能吸收多來源歷史、能安全回滾、能自我診斷」的 Recall & Trust 基線。

**包含範圍**：

1. 完成 Google Takeout dry-run / preview / quarantine / execute / audit artifact
2. 打通 Firefox backup pipeline，並補 Safari path detection / permission guidance / baseline ingest
3. 落地 rollback preview / execute / un-revert / visibility-aware query filtering
4. 擴展 doctor 與 repair run，至少涵蓋 broken visibility / missing artifact / stale derived state
5. 補齊 fixtures、smoke / acceptance / e2e，確保 import / rollback / multi-browser flows 可重跑

**讀先**：

- `docs/features/archive.md`
- `docs/features/recall.md`
- `docs/plan/m2-recall-and-trust/imports-browsers-and-rollback.md`

**完成訊號**：

- Google Takeout 可 dry-run、execute、rollback、un-revert
- Firefox 正式支持，Safari 至少有清楚 guidance + baseline ingest / unsupported contract
- Explorer / Export / Dashboard / Audit 全部遵守 visibility-aware filtering
- doctor 能報出並修復至少一類 damaged import / rollback finding
- `bun run check && bun run build`

**預期 commit 類型**：

- `feat(import): ...`
- `feat(rollback): ...`
- `feat(browser): ...`
- `test(acceptance): ...`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
