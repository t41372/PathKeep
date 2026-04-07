# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M1 — Solid Archive**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-M1-B** — Archive UX And Operations

---

### WORK-M1-B — Archive UX And Operations

**目標**：把 M1-A 打好的 archive engine foundation 接成真正可驗收的使用流程，讓 Onboarding、Dashboard、Explorer、Audit、Export 與 Security day-one UX 全部接上真實 read model 和 PME 邊界。

**包含範圍**：

1. 重寫 onboarding 流程，接通 storage / browser detection / security / schedule / first backup 的第一版體驗
2. 實作 Dashboard v1，展示 archive health、recent runs、coverage、storage summary 與 zero / unhealthy states
3. 實作 Explorer v1 的搜尋、篩選、結果列表 / detail pane、evidence source 與 locked / empty / loading states
4. 實作 Audit / run detail / Export v1，把 artifacts、warning、copy path、匯出入口接上真實資料
5. 補齊 smoke / e2e / interaction acceptance，並對照 prototype 收斂 trust copy 與視覺層級

**讀先**：

- `docs/design/screens-and-nav.md`
- `docs/features/archive.md`
- `docs/plan/m1-solid-archive/explorer-export-and-onboarding.md`

**完成訊號**：

- onboarding 可走到 first backup ready / dashboard entry，且不偷做高風險操作
- dashboard / explorer / audit / export 讀取真實 archive read model，而不是只靠 preview data
- 至少一輪 onboarding / dashboard / explorer / audit smoke 或 interaction 驗收可重跑
- `bun run check && bun run build`

**預期 commit 類型**：

- `feat(onboarding): ...`
- `feat(dashboard): ...`
- `feat(explorer): ...`
- `feat(audit): ...`
- `test(e2e): ...`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
