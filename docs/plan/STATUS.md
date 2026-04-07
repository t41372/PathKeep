# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M2 — Recall & Trust**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-M2-B** — Trust UX, I18n, And Platforms

---

### WORK-M2-B — Trust UX, I18n, And Platforms

**目標**：把已落地的 import / rollback / doctor / multi-browser backend 能力，收斂成真正可長期信任使用的產品體驗，補齊 PME、跨平台 guidance 與核心語系。

**包含範圍**：

1. 為 import、rollback / restore、rekey、doctor repair 補齊 PME UI 與共用 interaction grammar
2. 補齊 Dashboard / Audit / Settings 的 trust-first warning、permission guidance 與修復入口
3. 建立 `en` / `zh-CN` / `zh-TW` 的 namespace-based i18n 架構，完成 trust-critical 文案
4. 把 Windows / Linux scheduler、Safari Full Disk Access、keyring unavailable 等平台限制做成正式 capability / troubleshooting UX
5. 補齊 PME / i18n / platform acceptance、accessibility walkthrough 與相關文檔同步

**讀先**：

- `docs/design/ux-principles.md`
- `docs/standards.md`
- `docs/plan/m2-recall-and-trust/trust-ux-i18n-and-platforms.md`

**完成訊號**：

- import、rollback / restore、rekey、doctor repair 都有一致的 PME 流程與可驗證結果
- Full Disk Access、keyring unavailable、scheduler mismatch / manual install 都有清楚且可重用的 guidance UX
- `en` / `zh-CN` / `zh-TW` 的核心 trust flows 可用，且沒有核心流程英語 fallback
- Windows / Linux 的排程 capability、manual / apply story 與 troubleshooting 文案可驗收
- `bun run check && bun run build`

**預期 commit 類型**：

- `feat(trust-ui): ...`
- `feat(i18n): ...`
- `feat(platform): ...`
- `test(acceptance): ...`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
