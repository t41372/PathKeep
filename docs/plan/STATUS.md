# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M4 — Full Polish**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-M4-A** — Enrichment And Remote Backup

---

### WORK-M4-A — Enrichment And Remote Backup

**目標**：在已完成的 quality / product closeout 之上，補齊 enrichment plugin system、advanced intelligence 與 remote backup 的第一個可驗收 slice，讓 M4 正式從「可用」走向「可發布的延伸能力」。

**包含範圍**：

1. 定義 enrichment plugin contract、derived-state 邊界、queue / rebuild / clear story，避免 enrichment 任意滲進 canonical archive source of truth
2. 補出至少一條高價值 advanced intelligence slice，要求 evidence-first、可回鏈、可關閉、可重建
3. 把 remote backup 從現有基礎能力收斂成可驗收的 Preview / Manual / Execute / Verify story，包含 bundle / checksum / restore validation 的最小閉環
4. 若 M4-A 途中發現 release / platform polish 類工作，回寫到 `WORK-M4-B` 對應 docs，不要提前偷跑

**讀先**：

- `docs/features/intelligence.md`
- `docs/features/archive.md`
- `docs/plan/m4-full-polish/enrichment-advanced-intelligence-and-remote.md`

**完成訊號**：

- enrichment plugin / remote bundle / advanced insight 的 source docs、Rust / TS contract、UI / command surface 與 acceptance tests 已至少形成一個 honest、可重跑、可回滾的 v1 slice
- evidence、cost / storage impact、disable / rebuild / clear story 與 remote restore validation 都已可見，不會只剩 happy-path demo
- `docs/features/`、`docs/architecture/`、`docs/plan/m4-full-polish/`、`docs/plan/README.md`、`BACKLOG.md` 與 milestone README 已同步
- `bun run check && bun run build`，以及與本次變更直接相關的 acceptance / Rust tests / e2e 都通過

**預期 commit 類型**：

- `feat(enrichment): ...`
- `feat(remote): ...`
- `test(m4): ...`
- `docs(m4): ...`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
