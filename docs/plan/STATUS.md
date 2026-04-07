# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M0 — 重構基礎**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [x] **WORK-M0-A** — Data Plane Reset
- [ ] **WORK-M0-B** — Product Shell Reset

---

### WORK-M0-B — Product Shell Reset

**目標**：把 M0 前端與產品表面前半直接做完，讓新 shell、design token、命名清理與重寫期 quality policy 一次到位，而不是再開一串臨時中繼 task。

**包含範圍**：

1. 抽出 prototype design tokens，建立 token docs / CSS token layer
2. 刪除舊 UI 入口並建立新 shell / router / page skeletons
3. 重寫 Playwright shell smoke target
4. 完成 PathKeep rename sweep（package / Tauri / README / workflow / public strings）
5. 把 rewrite-phase quality policy 寫進 docs / AGENTS / CI

**讀先**：

- `reference/PathKeep — Desktop UI Design/`
- `docs/design/ux-principles.md`
- `docs/design/screens-and-nav.md`
- `docs/standards.md`
- `docs/plan/program/repo-baseline.md`
- `docs/plan/m0-foundation/frontend-shell-and-design-system.md`
- `docs/plan/m0-foundation/rename-quality-and-rewrite-discipline.md`
- `src/main.tsx`
- `src/AppNew.tsx`
- `src/App.css`
- `tests/e2e/shell.spec.ts`

**完成訊號**：

- `docs/design/design-tokens.md` 與前端 token layer 已建立
- 新 shell 成為主入口，舊 `AppNew` 不再是主流程
- Playwright smoke 對齊新 shell / onboarding / dashboard
- PathKeep 對外命名在 package / Tauri / README / workflow 層完成清理
- CI / standards / AGENTS 對同一套 rewrite quality policy 沒有互相打架
- 新建或整段重寫的 frontend slice 已有測試，且該 slice 的 100% coverage + mutation verification 已完成或明確記錄
- `bun run check && bun run build`

**預期 commit 類型**：

- `docs(design): ...`
- `feat(shell): ...`
- `test(e2e): ...`
- `chore(rename): ...`
- `docs(quality): ...`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
