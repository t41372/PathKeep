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

**目標**：在已穩定的 archive + intelligence v1 之上，補齊 enrichment plugin system、advanced intelligence 與 remote backup 的第一個可驗收 slice，同時維持 optional、evidence-first、recoverable 原則。

**包含範圍**：

1. 定義 enrichment plugin contract、queue integration、derived-state boundary 和至少一組高價值 core plugins
2. 擴展 intelligence 到更長時間窗口、storage / revisit 類 insight，且每個 advanced insight 都保留 evidence 與 disable / rebuild controls
3. 完成 remote backup bundle 的 preview / execute / validation / restore story，對齊 PME grammar
4. 把 storage / operations / remote docs、驗收樣本與 M4 source docs 同步回寫

**讀先**：

- `docs/features/intelligence.md`
- `docs/features/archive.md`
- `docs/plan/m4-full-polish/enrichment-advanced-intelligence-and-remote.md`

**完成訊號**：

- enrichment plugin framework、advanced intelligence、remote backup bundle 都有第一版可驗收實作，且不破壞 archive / intelligence v1 的可信邊界
- remote backup / enrichment / advanced insight 的 evidence、cost / storage guardrail、clear / rebuild / restore 路徑都可驗證
- M4 enrichment / remote / advanced intelligence docs、驗收樣本與 research backlog 已同步
- `bun run check && bun run build`

**預期 commit 類型**：

- `feat(enrichment): ...`
- `feat(remote): ...`
- `feat(insights): ...`
- `test(remote): ...`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
