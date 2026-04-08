# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：PG — M0-M3 Quality Closeout**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-QC-B** — Close Remaining M0-M3 Product And Doc Debt

---

### WORK-QC-B — Close Remaining M0-M3 Product And Doc Debt

**目標**：在 `WORK-QC-A` 已恢復 quality gate truthfulness 之後，把 M0-M3 剩下的產品與文檔債收乾淨，讓 prototype / 設計 / 需求 / 驗收邊界與實際實作重新對齊，之後才開 M4。

**包含範圍**：

1. 逐頁比對 `docs/design/screens-and-nav.md`、`docs/design/ux-principles.md`、`reference/PathKeep — Desktop UI Design/` 與目前活的 `src/app/` / `src/pages/` surface，補齊或收斂 prototype / IA / copy / state gap
2. 收掉 M0-M3 還沒真正簽收的 trust-critical product debt，包含 desktop-vs-preview 驗收邊界、On This Day / evidence / timezone / i18n / PME / reduced-motion / keyboard-only walkthrough 等與需求或設計不一致的行為
3. 對齊 `docs/features/`、`docs/design/`、milestone README、`docs/plan/README.md` 與相關 acceptance 記述，避免舊的 preview fixture 或局部測試被誤寫成完整產品簽收
4. 若發現超出 QC-B 可控範圍的大工作，先回寫 source docs 並追加到 `BACKLOG.md`，不要直接偷跑進 M4

**讀先**：

- `docs/vision-and-requirements.md`
- `docs/design/ux-principles.md`
- `docs/design/screens-and-nav.md`
- `docs/plan/m0-foundation/README.md`
- `docs/plan/m1-solid-archive/README.md`
- `docs/plan/m3-intelligence/README.md`

**完成訊號**：

- 目前活著的 M0-M3 頁面、trust copy、PME steps、evidence / timezone / i18n 行為，已和需求 / 設計 / prototype 對齊；若仍有缺口，也已被清楚回寫且不再冒充已完成
- desktop / preview 的驗收邊界已寫清楚，高風險流程不再靠 browser preview smoke 假裝已經完成 desktop signoff
- 相關 `docs/features/`、`docs/design/`、`docs/plan/README.md`、milestone README 與 `BACKLOG.md` 已同步；`WORK-M4-A` 只會在 QC-B 真正收乾淨後解鎖
- `bun run check && bun run build`，以及與本次變更直接相關的 targeted tests / e2e / Rust tests 都通過

**預期 commit 類型**：

- `feat(ui): ...`
- `fix(product): ...`
- `test(acceptance): ...`
- `docs(product): ...`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
