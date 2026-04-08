# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M4 — Full Polish**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-M4-D** — Rust Mutation Deep-Check Hardening

---

### WORK-M4-D — Rust Mutation Deep-Check Hardening

**目標**：`WORK-M4-B` 的 release closeout 已經把 blocking path、coverage、`mutation:js`、browser-preview smoke 與 debug desktop build 跑通，但 `bun run mutation:rust` 的 pre-release deep check 暴露出 `browser-history-parser` 與 `vault-core/src/ai.rs` 的存活 mutants。這個 work block 要把那批缺口收斂成可驗證、可維護、文件誠實的 Rust mutation baseline，而不是讓 repo 繼續停在「知道有洞但沒有下一步」的狀態。

**包含範圍**：

1. 盤點 `cargo mutants` 第一輪暴露的 parser / AI misses，區分真正缺測、誤導性的 helper 邊界，以及應該明確 deferred 的高成本案例
2. 補齊 `browser-history-parser` 與 `vault-core` AI status / helper 周邊的 targeted Rust tests，優先消掉 trivial boolean / equality / field-removal mutants
3. 如果 full-workspace `mutation:rust` 仍然過於昂貴或噪音過高，收斂出誠實的 scoped mutation contract、workflow 註解與 deferred rationale，避免 release docs 繼續假裝 cargo-mutants 已經 pass
4. 同步回寫 `quality-matrix.md`、`standards.md`、M4 planning docs 與相關 runbook，讓下一次 release closeout 對 deep check 狀態有單一可信 source of truth

**讀先**：

- `docs/standards.md`
- `docs/plan/program/quality-matrix.md`
- `docs/plan/m4-full-polish/platform-release-and-polish.md`

**完成訊號**：

- `bun run mutation:rust` 通過，或至少已有明確 scoped contract / deferred rationale，讓 repo 不再誤報成「整個 Rust workspace mutation 已簽收」
- `browser-history-parser` 與 `vault-core` AI 這輪暴露出的高訊號 mutants 都已有對應測試、重構、或文件化的 deferred explanation
- `docs/plan/program/quality-matrix.md`、`docs/standards.md`、`docs/plan/README.md` 與 M4 planning docs 已同步
- `bun run check`、`bun run coverage:rust`，以及任何被本 block 觸及的 targeted mutation / regression sweeps 都通過

**預期 commit 類型**：

- `test(rust): ...`
- `docs(quality): ...`
- `ci(mutation): ...`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
