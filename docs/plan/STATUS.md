# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M3 — Intelligence**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-M3-A** — Providers, Queue, And Indexing

---

### WORK-M3-A — Providers, Queue, And Indexing

**目標**：在不破壞 Archive 可信基礎的前提下，完成 AI provider 設定、job queue、embedding / semantic index foundation，讓 optional intelligence 開始可重跑、可清空、可重建。

**包含範圍**：

1. 補齊 AI provider 管理、secret storage / clear、connection test 和 model selection 的 command + UI contract
2. 建立 job queue persistence、worker orchestration 與 retry / replay 邊界
3. 完成 embedding / semantic index 的 build、clear、rebuild foundation，並保留無 AI 配置時的安全降級
4. 把 provider / queue / index 的驗收、文檔與決策同步回寫到 M3 source docs

**讀先**：

- `docs/features/intelligence.md`
- `docs/plan/m3-intelligence/providers-indexing-and-jobs.md`

**完成訊號**：

- provider 管理、secret storage / clear、connection test 與 model selection 都有可驗證結果
- semantic index build / clear / rebuild 與 job queue persistence 已落地，且沒有 AI 配置時仍能正常降級
- M3 provider / queue / indexing docs 與 research backlog 已同步
- `bun run check && bun run build`

**預期 commit 類型**：

- `feat(ai): ...`
- `feat(queue): ...`
- `feat(index): ...`
- `test(ai): ...`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
