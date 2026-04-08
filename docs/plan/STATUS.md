# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block；不要把 `STATUS.md` 再拆回原子 task。

**當前 Milestone：M4 — Full Polish**

---

## CURRENT FOCUS

> 這裡的單位是 **work block**，每個 block 的份量大約是半個 milestone。
> work block 內可以包含多個子任務、ADR、代碼變更與文檔同步，但只有整塊達成可驗收成果時才改成 `[x]`。
> `STATUS.md` 通常只維持 1-2 個 work blocks。commit 仍保持可 review，不要求「一個 work block = 一個 commit」。

- [ ] **WORK-M4-E** — Loading States & Skeleton Screens
- [ ] **WORK-M4-F** — Profile-Scoped Insights

---

### WORK-M4-E — Loading States & Skeleton Screens

**目標**：M4 的 shell / route surface 雖然功能已基本到位，但 loading story 仍不一致，部分頁面還停在 spinner、空白區塊或缺少可讀進度說明。這個 work block 要把 Dashboard / Explorer / Insights / Import / AI 操作的 loading state 收斂成符合 PME 與 design token 的 skeleton / progress grammar，避免 large archive 或 AI 背景工作時看起來像「卡住」。

**包含範圍**：

1. 依 `docs/design/ux-principles.md` 的規格，補齊 Dashboard / Explorer / Insights 的 skeleton screen，讓 placeholder 佈局與最終內容尺寸一致
2. 把 Import / backup / AI 操作的 progress 與 pulsing status indicator 收斂成可讀的數字與階段說明，不允許只有 spinner
3. 補上 `var(--border)` pulse animation 與 `prefers-reduced-motion` fallback，並確認 loading UI 不造成明顯 layout shift

**讀先**：

- `docs/design/ux-principles.md`
- `docs/design/screens-and-nav.md`
- `reference/PathKeep — Desktop UI Design/`

**完成訊號**：

- Dashboard / Explorer / Insights / Import / AI 操作都有符合規格的 loading UI，而不是 generic spinner 或空白
- skeleton / progress overlay 會顯示可讀狀態說明與數字，並有 reduced-motion fallback
- 對應頁面 tests 與 `bun run check`、`bun run build` 通過

**預期 commit 類型**：

- `feat(ui): ...`
- `test(ui): ...`

---

### WORK-M4-F — Profile-Scoped Insights

**目標**：Explorer / Assistant 已有 shared profile scope 語法，但 Insights 仍偏向 archive-wide。這個 work block 要把 Insights 正式接到 shared profile scope，讓 scoped view 與 archive-wide 的界線清楚、文案誠實，並且與 Explorer / Assistant 的篩選語法保持一致。

**包含範圍**：

1. Insights 頁面接入 shell chrome 的 shared profile scope，讓 insight cards、topic timeline、threads、query ladders、periodic summaries 切到 scoped view
2. 補上 scoped vs archive-wide 的 callout / badge，清楚標示哪些統計仍維持 archive-wide
3. 驗證 query string / shared scope 的語法與 Explorer / Assistant 一致，不引入新的 route 分叉

**讀先**：

- `docs/features/intelligence.md`
- `docs/design/screens-and-nav.md`
- `docs/architecture/data-model.md`

**完成訊號**：

- Insights 在選定 profile 時會切換到 scoped data surface，且 archive-wide KPI / storage 類訊息仍有明確說明
- scope 切換不新增 route，並與 Explorer / Assistant 的 scope 語法一致
- 對應 UI / contract tests、`bun run check`、`bun run build` 通過

**預期 commit 類型**：

- `feat(insights): ...`
- `test(insights): ...`

---

> 做完了？→ 把完成的 work block append 到 [CHANGELOG.md](CHANGELOG.md)，同步 source docs，然後再從 [BACKLOG.md](BACKLOG.md) 補下一個 block。
