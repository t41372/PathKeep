# AGENTS.md — PathKeep

你是一個被派進來繼續工作的 AI coding agent。**先讀完這頁，再動手。**

---

## 這個專案是什麼

**PathKeep** — local-first 桌面應用，備份和分析瀏覽器歷史記錄。
Tech: Tauri 2 + Rust + React 19 + TypeScript + Vite + Bun。
現狀：**正在從頭重寫**。沒有正式用戶，不需要 backward compatibility，可果斷刪舊代碼。

---

## 你的工作循環

### 開工

1. 讀 `docs/plan/STATUS.md` — 找到第一個 `[ ]` work block
2. 讀該 work block 的「讀先」列表裡的文檔（只讀列出的；如果列出的內容互相衝突、引用失效、或不足以完成工作，只補讀直接相關檔案並先修文檔）
3. 執行整個 work block 的範圍；work block 的粒度約等於半個 milestone，可包含多個子任務
4. 跑驗收命令：`bun run check && bun run build`

### 收工（每完成一個 work block）

1. 在 `STATUS.md` 把 `[ ]` 改成 `[x]`
2. 把完成的 work block 剪切（append）到 `docs/plan/CHANGELOG.md` 底部，不改舊內容
3. 同步回寫這個 work block 實際解鎖或完成的 source docs：
   - `docs/plan/program/research-and-decisions.md` 對應的 `PG-RD-*`
   - milestone `README.md` / checklist
   - `docs/plan/BACKLOG.md` 裡的 inline `[!blocked: ...]` 標記與依賴說明
   - 如果 work block 改變了功能行為或技術決策，也要同步更新 `docs/features/`、`docs/design/`、`docs/architecture/`
4. 如果 `STATUS.md` 裡沒有剩餘 `[ ]` work blocks 了：
   - 打開 `docs/plan/BACKLOG.md`
   - 從 BACKLOG 頂部取最多 2 個未被阻塞的 work blocks，剪切到 `STATUS.md` 的 CURRENT FOCUS 區
   - 更新 `BACKLOG.md` 裡的 inline `[!blocked: ...]` 標記（如果有依賴被解鎖）
5. Commit：保持可 review；`STATUS.md` 的 work block 不等於單一 commit，必要時拆成多個合理 commit

### 情況判斷

- `STATUS.md` 有 `[ ]` → 直接做第一個 work block
- `STATUS.md` 全是 `[x]` 或空的 → 去 `BACKLOG.md` 補充 work blocks
- work block 標記 `[!]` → blocked，跳過，做下一個
- 遇到計劃外的大工作 → 在 `BACKLOG.md` 加一條，不要直接做

---

## 文檔導覽

| 文檔                                          | 說明                                |
| --------------------------------------------- | ----------------------------------- |
| `docs/plan/STATUS.md`                         | 當前 work block（通常 1-2 個）      |
| `docs/plan/BACKLOG.md`                        | 後續 work block 隊列 + blocked 標記 |
| `docs/plan/CHANGELOG.md`                      | 已完成 work block 日誌（只 append） |
| `docs/vision-and-requirements.md`             | 產品定位 + 核心原則                 |
| `docs/plan/README.md`                         | 里程碑計劃總覽                      |
| `docs/plan/program/research-and-decisions.md` | 未落地技術決策                      |
| `docs/plan/program/repo-baseline.md`          | 現有 repo 問題清單                  |
| `docs/features/`                              | 各功能詳細需求規格                  |
| `docs/architecture/`                          | 技術原則 + data model + tech stack  |
| `docs/design/`                                | UX 原則 + 畫面結構規格              |
| `reference/PathKeep — Desktop UI Design/`     | 設計師 prototype（UI 對齊目標）     |

---

## 開始任務前的 checklist

### UI 相關

- 看 `reference/PathKeep — Desktop UI Design/` — 嚴格對齊設計稿
- 看 `docs/design/screens-and-nav.md` — 畫面結構和導航規格
- 看 `docs/design/ux-principles.md` — PME 模型
- `AppNew.tsx`、`App.css`、`src/pages/` 全部是要重寫的，不是修補的

### Rust/後端相關

- 看 `docs/architecture/data-model.md` — canonical schema
- 看 `docs/architecture/tech-stack.md` — 技術選型決策
- 看 `docs/plan/program/research-and-decisions.md` — 確認沒有 `[!]` blocker
- 巨型檔案要拆，不要繼續往裡加：`archive.rs`(2078)、`chrome.rs`(1229)、`ai.rs`(1916)、`insights.rs`(2481)、`vault-worker/src/lib.rs`(1577)

### 任何工作

- 先讀對應的 `docs/features/` 子文檔
- 如果「讀先」文檔之間打架：先修文檔，讓 source of truth 恢復一致，再寫代碼
- 完成後跑 `bun run check`，通過才提交

---

## 代碼結構

```
src/                          React 19 + TypeScript 前端
  main.tsx                    入口（掛著舊 AppNew，M0 要換掉）
  AppNew.tsx                  舊 shell，待刪除
  App.css                     1880 行舊 CSS，待刪除
  lib/backend.ts              IPC wrapper（混著假資料，M0 拆清）
  lib/app-context.tsx         全域狀態（太肥，M0 重設計）
  pages/                      舊頁面（全部重寫）
  components/                 部分 components 可能有可復用邏輯

src-tauri/
  src/lib.rs                  Tauri command facade
  src/session.rs              session 狀態
  src/worker_bridge.rs        worker bridge
  crates/vault-core/          核心 archive 邏輯（要拆）
    src/archive.rs            2078 行，職責混雜，待拆
    src/chrome.rs             1229 行，browser discovery + 解析，待拆
    src/ai.rs                 1916 行，AI 邏輯，M3 才動
    src/insights.rs           2481 行，insights，M3 才動
  crates/vault-platform/      平台適配
  crates/vault-worker/
    src/lib.rs                1577 行，orchestration + MCP + CLI，待拆

docs/plan/
  STATUS.md                   當前 work block（每次開工必讀）
  BACKLOG.md                  後續 work block 隊列 + inline blocked 標記
  CHANGELOG.md                已完成 work block 日誌
  README.md                   里程碑總覽
  program/                    決策 backlog、repo baseline
  m0-foundation/              M0 詳細 WBS
  m1-solid-archive/           M1 詳細 WBS

reference/
  PathKeep — Desktop UI Design/   設計師 prototype
```

---

## 核心原則

1. **Trust & Transparency** — 所有操作走 PME 流程（Preview → Manual → Execute），沒有黑盒執行
2. **Data Sovereignty** — 數據不上傳雲端，AI 也用本地或用戶自配的 provider
3. **Longevity** — 設計壽命 20 年，選 SQLite/plaintext，不鎖死用戶
4. **Intelligence Is Optional** — 沒有 AI provider 時 PathKeep 仍完整可用，AI 是增值層
5. **Recoverability** — 所有操作可回滾，用戶不因我們的 bug 永久丟失數據

---

## 工作規範

- **Commit**：`feat(ui): ...` / `fix(archive): ...` / `chore(deps): ...`，保持 commit 可 review；不要因為 work block 變大就做單一巨型 commit
- **Tests**：JS/TS 用 Vitest，Rust 用 `cargo test`。M0 重寫期間，舊碼不再靠 repo-wide coverage / mutation gate 阻塞；但**所有新建或整段重寫的模組**都必須有測試，且該 slice 要做到 100% coverage + mutation verification，否則不算完成
- **Test 位置**：`foo.ts` → `foo.test.ts`（放旁邊），E2E 放 `tests/e2e/`
- **注釋**：代碼注釋即開發者文檔，重要技術決策、trade-off 在代碼處寫注釋
- **文檔更新**：改功能行為 → 更新 `docs/features/`；新技術決策 → 更新 `docs/architecture/`
- **提交前**：`bun run check` + `bun run build` 全過才提交

---

## 常用命令

```bash
bun run dev              # browser-only Vite preview (127.0.0.1:1420)
bun run desktop:dev      # 完整 Tauri 桌面 app
bun run build            # TypeScript + Vite bundle
bun run check            # 所有 quality gate
bun run verify           # 本地 CI 全掃（coverage + e2e + debug build）
bun run test:unit        # Vitest unit tests
bun run test:e2e         # Playwright e2e
bun run coverage:js      # JS 覆蓋率（要求 100%）
bun run coverage:rust    # Rust 覆蓋率
bun run mutation         # Mutation tests
bun run format           # Prettier 格式化
```

---

## 命名規範

- React components：`PascalCase`
- 工具模組/文件名：`kebab-case`（e.g. `browser-icons.tsx`）
- Rust 模組/函數：`snake_case`
- 縮排：TS/TSX 2 空格，Rust 4 空格
- Prettier：no semicolons, single quotes, trailing commas

---

_這份文檔是 living document。發現過期或不準確，直接修正它。_
