> **⚡ 找下一個要做的 work block？直接看 [STATUS.md](STATUS.md)**

# PathKeep — 工作計劃與進度追蹤

> **Status:** Living document · **Rebuilt:** 2026-04-05  
> 本目錄是 PathKeep 的實作層 source of truth。  
> 產品願景、需求和設計定義在 [vision-and-requirements.md](../vision-and-requirements.md) 與它的子文檔裡；這裡回答的是 **接下來怎麼做、先做什麼、哪些事情卡住了、每個里程碑拆到哪一層**。

### 工作塊追蹤系統

| 檔案                         | 用途                           | Agent 何時讀        |
| ---------------------------- | ------------------------------ | ------------------- |
| [STATUS.md](STATUS.md)       | 當前 work block（通常 1-2 個） | 每次開工            |
| [BACKLOG.md](BACKLOG.md)     | 後續 work block 佇列 + 依賴圖  | STATUS.md 清空時    |
| [CHANGELOG.md](CHANGELOG.md) | 已完成 work block 紀錄         | 不需要讀，只 append |

這一層不再追求原子 task。`STATUS.md` / `BACKLOG.md` 的單位改成**半個 milestone 左右的 work block**：

- 一個 work block 可以橫跨多個 docs / code 子任務，但要有單一可驗收成果。
- work block 可以拆成多個可 review commit；不要把「work block 變大」誤解成「commit 也要巨大」。
- 更細的 WBS 仍保留在各 milestone 文檔裡，作為 block 內部的拆解參考，而不是每日追蹤單位。

---

## 先看哪裡

| 如果你關心                            | 先看這份                                                               |
| ------------------------------------- | ---------------------------------------------------------------------- |
| 整體節奏、里程碑順序、依賴關係        | [program/README.md](program/README.md)                                 |
| 現在這個 repo 和新 vision 的距離      | [program/repo-baseline.md](program/repo-baseline.md)                   |
| 哪些技術決策還沒落地、哪些研究要先做  | [program/research-and-decisions.md](program/research-and-decisions.md) |
| 某份需求/設計文檔應該對應哪份實作計劃 | [program/traceability-map.md](program/traceability-map.md)             |
| M0 重構基礎                           | [m0-foundation/README.md](m0-foundation/README.md)                     |
| M1 Solid Archive                      | [m1-solid-archive/README.md](m1-solid-archive/README.md)               |
| M2 Recall & Trust                     | [m2-recall-and-trust/README.md](m2-recall-and-trust/README.md)         |
| M3 Intelligence                       | [m3-intelligence/README.md](m3-intelligence/README.md)                 |
| M4 Full Intelligence & Polish         | [m4-full-polish/README.md](m4-full-polish/README.md)                   |
| 產品願景、需求、畫面結構              | [../vision-and-requirements.md](../vision-and-requirements.md)         |

---

## 這一版計劃是怎麼來的

這不是把舊的 todo 清單換個排版，而是重新掃過整個 repo 和整份 `docs/` 後，做的一次真正 re-baseline。這次至少確認了幾件事：

- 新的 vision、features、architecture、design 文檔已經成形，而且方向很清楚。
- 現在的代碼庫不是「完全不能用」，而是**還在穩定驗證一套舊產品假設**。
- 舊 UI 不只是視覺上不好看，而是整個資訊架構、導航和狀態模型都還綁在舊產品上。
- Rust 端其實已經有很多功能，但大量功能長在錯的地方，巨型檔案和責任混寫很明顯。
- 有些決策其實還沒真的落地，例如 schema reset strategy、migration story、rollback visibility、AI sidecar 邊界、跨平台排程。

所以這份 plan 的目的不是「幫我們記得做哪些功能」，而是先把**正確的實作順序、決策順序和刪舊代碼的順序**講清楚。

---

## 2026-04-05 基線結論

根據這次掃描和驗證，目前可以先這樣理解 repo：

- 前端入口 [`src/main.tsx`](../../src/main.tsx) 仍然直接載入 [`src/AppNew.tsx`](../../src/AppNew.tsx)，舊 shell 還是主入口。
- 舊 UI 不是少量修補就能對齊新設計的狀態。至少 [`src/App.css`](../../src/App.css)、[`src/AppNew.test.tsx`](../../src/AppNew.test.tsx)、[`src/lib/i18n.ts`](../../src/lib/i18n.ts) 都已經到重寫優先的程度。
- [`src/lib/backend.ts`](../../src/lib/backend.ts) 不只是 IPC 包裝，還混著 browser preview fixture、舊產品文案、舊 app 路徑和假資料模型。
- Rust 端的大部分複雜度集中在幾個巨檔裡：[`src-tauri/crates/vault-core/src/archive.rs`](../../src-tauri/crates/vault-core/src/archive.rs)、[`src-tauri/crates/vault-core/src/chrome.rs`](../../src-tauri/crates/vault-core/src/chrome.rs)、[`src-tauri/crates/vault-core/src/ai.rs`](../../src-tauri/crates/vault-core/src/ai.rs)、[`src-tauri/crates/vault-core/src/insights.rs`](../../src-tauri/crates/vault-core/src/insights.rs)、[`src-tauri/crates/vault-worker/src/lib.rs`](../../src-tauri/crates/vault-worker/src/lib.rs)。
- canonical archive 目前仍然是 `archive-schema.sql` 加上啟動時補欄位的做法，還沒有正式 migration ledger。
- 命名遷移沒有完成。`Browser History Backup`、`Chrome History Backup`、`Chrome History Vault` 仍殘留在 `package.json`、Tauri config、README、workflow、前端 mock、keyring / schedule 文案、export 文案、AI/MCP 文案等多處。
- 設計師的 prototype 很清楚，但當前代碼庫並不是朝著那套 IA 在長，而是另一條舊路徑。

---

## 已做過的基線驗證

這一輪 plan 不是純主觀判斷。下面這些命令已在 2026-04-05 重新執行：

- `bun run typecheck`：通過
- `bun run test:unit`：通過，8 個 test files / 142 tests
- `cargo test --manifest-path src-tauri/Cargo.toml --workspace --all-targets --quiet`：通過
- `bun run test:e2e`：失敗，失敗點是 [`tests/e2e/shell.spec.ts`](../../tests/e2e/shell.spec.ts) 仍然要求舊的 `Setup` heading 和舊 setup shell 文案

2026-04-06 補充：

- `bun run check`：通過，repo-wide Markdown / Prettier debt 與驗收途中浮出的 JS ESLint、Rust Clippy 基線問題已清理
- `bun run build`：通過

這個結果很重要，因為它說明 repo 現在不是「壞掉」，而是「還在穩定地保護舊目標」。所以第一性工作不是零碎修 bug，而是**先重設產品骨架、驗收目標和測試基線**。

---

## 進度符號

- `[ ]` 未開始
- `[/]` 進行中
- `[x]` 已完成
- `[~]` 已有部分實作，但不符合新 vision，需要重做或重構
- `[!]` 阻塞中，必須先做研究或決策

---

## WBS 根節點

```
PG  Program / Baseline / Research
M0  Foundation reset
M1  Solid Archive
M2  Recall & Trust
M3  Intelligence
M4  Full Intelligence & Polish
```

每個里程碑目錄都有：

- `README.md`：這個里程碑在做什麼、何時算完成、有哪些工作包
- 2 到 4 份工作包文檔：把待辦拆到更細的功能域和實作層
- `STATUS.md` / `BACKLOG.md`：以 half-milestone work block 追蹤目前真正要做的範圍

---

## 里程碑入口

| 里程碑 | 目標                                                                     | 狀態  | 入口                                                           |
| ------ | ------------------------------------------------------------------------ | ----- | -------------------------------------------------------------- |
| `PG`   | 盤清 repo 現況、建立決策 backlog、維護文檔導覽和依賴關係                 | `[/]` | [program/README.md](program/README.md)                         |
| `M0`   | 切斷舊 UI 和舊產品骨架，建立新的前端、後端和資料平面起點                 | `[ ]` | [m0-foundation/README.md](m0-foundation/README.md)             |
| `M1`   | 把 Archive、Audit、Schedule、Security、Explorer v1 做成可信的基礎        | `[ ]` | [m1-solid-archive/README.md](m1-solid-archive/README.md)       |
| `M2`   | 補齊導入、回滾、Doctor、多瀏覽器、PME、i18n 和跨平台排程                 | `[ ]` | [m2-recall-and-trust/README.md](m2-recall-and-trust/README.md) |
| `M3`   | 在穩定 archive 之上加入 optional AI provider、index、assistant、insights | `[ ]` | [m3-intelligence/README.md](m3-intelligence/README.md)         |
| `M4`   | 補齊 enrichment、進階洞察、remote backup、release polish 和多平台驗證    | `[ ]` | [m4-full-polish/README.md](m4-full-polish/README.md)           |

---

## 與其他文檔的關係

```
docs/vision-and-requirements.md   WHY + WHAT
  ├── docs/architecture/          技術原則與資料長期設計
  ├── docs/features/              功能需求詳細規格
  ├── docs/design/                UX 原則與畫面結構
  ├── docs/milestones.md          里程碑概覽
  ├── docs/standards.md           品質標準
  └── docs/plan/                  HOW + WHEN + WBS
       ├── README.md
       ├── program/
       ├── m0-foundation/
       ├── m1-solid-archive/
       ├── m2-recall-and-trust/
       ├── m3-intelligence/
       └── m4-full-polish/
```
