# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M1 — Solid Archive**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-M1-A** — Archive Engine Foundation

---

### WORK-M1-A — Archive Engine Foundation

**目標**：把 M0 打好的 schema / shell foundation 接成第一批真正可信的 archive engine，讓 Chromium manual backup、run ledger、manifest、snapshot safety net 與 M1 必需的 schedule / security / storage foundation 一次成形。

**包含範圍**：

1. 把 canonical schema / migration executor 接成 archive init 與 upgrade path 的正式入口
2. 做出 Chromium profile discovery、staging copy、parse-to-canonical ingest、dedupe、watermark 的第一版 backup pipeline
3. 接通 run ledger、manifest chain、snapshot artifact、doctor baseline 與第一批 Audit / Dashboard read model
4. 建立 Explorer / Audit / Dashboard 可直接消費的 query/read model foundation
5. 落下 macOS schedule PME、security mode、storage layout 的 domain contract，讓高風險操作開始有 Preview / Manual / Execute 邊界

**讀先**：

- `docs/features/archive.md`
- `docs/architecture/data-model.md`
- `docs/plan/m1-solid-archive/schema-backup-and-ledger.md`
- `docs/plan/m1-solid-archive/schedule-security-and-storage.md`

**完成訊號**：

- 至少一個 Chromium profile 的 manual backup path 可重跑驗收
- run ledger / manifest / snapshot artifact 能被 Audit / Dashboard read model 使用
- canonical migration system 成為 archive init 的正式入口，不再靠 ad-hoc schema bootstrapping
- schedule / security / storage foundation 已建立對應 docs、domain contract 與最小 acceptance
- `bun run check && bun run build`

**預期 commit 類型**：

- `feat(archive): ...`
- `feat(audit): ...`
- `feat(schedule): ...`
- `feat(security): ...`
- `test(archive): ...`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
