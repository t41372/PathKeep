# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：PG — M0-M3 Quality Closeout**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-QC-A** — Restore Global Quality Gates Before M4

---

### WORK-QC-A — Restore Global Quality Gates Before M4

**目標**：在進入 M4 之前，把 M0-M3 遺留的 quality gate、驗收真實性與 blocking policy 全部拉回「文檔怎麼寫，repo 就怎麼擋」的狀態，停止用 preview-only 或 slice-only 驗收冒充全域完成度。

**包含範圍**：

1. 讓 repo-wide `coverage:js`、`coverage:rust`、`mutation:js`、對應的 Rust mutation / acceptance sweep 回到真正可用、可作為 blocking gate 的狀態；把暫時 carve-out、過寬 exclude、只 mutate 3 個檔的配置收斂到 living M0-M3 surface
2. 把 desktop contract 以外的高風險 surface 補上真正的 desktop / worker / Tauri 驗收，不再只靠 browser preview smoke 宣稱 Schedule / Security / Import / Intelligence 已驗收
3. 對齊 `docs/standards.md`、`docs/plan/README.md`、milestone README、CI / script 名稱與實際 blocking path，移除「checker 已恢復」但實際沒開的失真敘事
4. 產出一份清楚的 quality matrix：哪些檢查是 blocking、覆蓋哪些 surface、哪些 deep checks 仍是 on-demand；完成後才能讓 M4 work block 重新解鎖

**讀先**：

- `docs/standards.md`
- `docs/plan/README.md`
- `docs/plan/m0-foundation/README.md`
- `docs/plan/m1-solid-archive/README.md`
- `docs/plan/m3-intelligence/README.md`
- `package.json`
- `vitest.config.ts`
- `stryker.config.json`
- `tests/e2e/shell.spec.ts`

**完成訊號**：

- `bun run check && bun run build` 之外，至少 `bun run coverage:js`、`bun run coverage:rust`、`bun run mutation:js`、`bun run test:e2e` 也能穩定通過；Rust mutation / deep acceptance 也已回到明確、可執行、可追蹤的 blocking 或 release gate
- repo-wide JS mutation scope 不再只限於少數 helper；living M0-M3 surface 的 coverage / mutation / e2e 配置與標準文檔一致
- desktop / preview 的驗收邊界寫清楚，高風險流程已有真實 desktop acceptance，而不是只剩 preview fixture
- quality gate 的真實狀態、殘留例外與後續 deep checks 都已回寫文檔，M4 不再被誤開

**預期 commit 類型**：

- `test(quality): ...`
- `chore(ci): ...`
- `fix(testing): ...`
- `docs(quality): ...`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
