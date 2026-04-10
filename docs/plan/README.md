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

| 如果你關心                                    | 先看這份                                                               |
| --------------------------------------------- | ---------------------------------------------------------------------- |
| 整體節奏、里程碑順序、依賴關係                | [program/README.md](program/README.md)                                 |
| 現在這個 repo 和新 vision 的距離              | [program/repo-baseline.md](program/repo-baseline.md)                   |
| 哪些技術決策還沒落地、哪些研究要先做          | [program/research-and-decisions.md](program/research-and-decisions.md) |
| 現行 quality gate、blocking path、deep checks | [program/quality-matrix.md](program/quality-matrix.md)                 |
| 某份需求/設計文檔應該對應哪份實作計劃         | [program/traceability-map.md](program/traceability-map.md)             |
| M0 重構基礎                                   | [m0-foundation/README.md](m0-foundation/README.md)                     |
| M1 Solid Archive                              | [m1-solid-archive/README.md](m1-solid-archive/README.md)               |
| M2 Recall & Trust                             | [m2-recall-and-trust/README.md](m2-recall-and-trust/README.md)         |
| M3 Intelligence                               | [m3-intelligence/README.md](m3-intelligence/README.md)                 |
| M4 Full Intelligence & Polish                 | [m4-full-polish/README.md](m4-full-polish/README.md)                   |
| 產品願景、需求、畫面結構                      | [../vision-and-requirements.md](../vision-and-requirements.md)         |

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

## 2026-04-06 基線結論

根據這次掃描和驗證，目前可以先這樣理解 repo：

- 前端入口 [`src/main.tsx`](../../src/main.tsx) 已切到 [`src/app/index.tsx`](../../src/app/index.tsx)；`AppNew` 與舊 `App.css` 已退場。
- 新 shell / route tree / sidebar / topbar / page skeleton 已建立，入口資訊架構已對齊新 prototype，而不是舊 setup-first shell。
- [`src/lib/backend.ts`](../../src/lib/backend.ts) 仍帶有 legacy / compatibility 成分，但正式 typed IPC wrapper 已移到 [`src/lib/ipc/bridge.ts`](../../src/lib/ipc/bridge.ts)，preview data 也已從主 bridge 分離。
- Rust 端的大部分複雜度仍集中在幾個巨檔裡：[`src-tauri/crates/vault-core/src/archive/mod.rs`](../../src-tauri/crates/vault-core/src/archive/mod.rs)、[`src-tauri/crates/vault-core/src/chrome.rs`](../../src-tauri/crates/vault-core/src/chrome.rs)、[`src-tauri/crates/vault-core/src/ai.rs`](../../src-tauri/crates/vault-core/src/ai.rs)、[`src-tauri/crates/vault-core/src/insights.rs`](../../src-tauri/crates/vault-core/src/insights.rs)、[`src-tauri/crates/vault-worker/src/lib.rs`](../../src-tauri/crates/vault-worker/src/lib.rs)。
- canonical archive 已有正式 migration ledger 與 schema foundation；M1 的主題不再是「先把 schema 生出來」，而是接上可信 archive engine。
- PathKeep 命名已完成 public / build metadata sweep；剩餘舊名字串只應存在於 explicit legacy alias 或 migration 註記。
- 設計師的 prototype 現在已經落成 production shell 的 token、layout 與 smoke target；prototype gap list、deep-link 與 non-prototype state coverage 也已回寫成 source docs，剩餘的全站 accessibility / release polish 留在 M4。

---

## 已做過的基線驗證

這一輪 plan 不是純主觀判斷。下面這些命令已重新執行並回寫：

- `bun run typecheck`：通過
- `bun run test:unit`：通過
- `bun run test:unit:desktop-contract`：通過
- `bun run coverage:js:desktop-contract`：通過，desktop contract slice 維持 100% coverage
- `bun run mutation:js:desktop-contract`：通過，desktop contract slice 維持 100% mutation score
- `cargo test --manifest-path src-tauri/Cargo.toml --workspace --all-targets --quiet`：通過
- `bun run test:e2e`：通過，驗證新 shell / onboarding / dashboard smoke
- `bun run check`：通過，repo-wide Markdown / Prettier debt 與驗收途中浮出的 JS ESLint、Rust Clippy 基線問題已清理
- `bun run build`：通過
- `bun run coverage:js`：通過，living M0-M3 JS quality surface 維持 100% coverage
- `bun run coverage:rust`：通過，Tauri desktop command / bridge quality surface 維持 100% coverage
- `bun run mutation:js`：通過，living M0-M3 JS quality surface 的 mutation score 恢復到 blocking threshold 之上

2026-04-07 品質 closeout：

- repo 現在有一份正式的 [quality matrix](program/quality-matrix.md)，把 mainline blocking path、scheduled / release deep checks，以及 desktop / preview 驗收邊界全部寫清楚。
- desktop contract slice 仍然存在，但它現在是 `bun run check` 裡的一條 targeted sub-gate，不再冒充整個產品 UI 或所有 desktop flows 都已驗收。
- 2026-04-07 closeout：`WORK-QC-B` 已把 prototype / doc parity、desktop-vs-preview 邊界、dashboard / onboarding trust copy 與 timezone-sensitive On This Day 行為重新對齊；M4 現在可從 `WORK-M4-A` 啟動。
- 2026-04-08 closeout：`WORK-M4-A` 已把 enrichment / derived-state v1、storage analytics / growth evidence、以及 remote backup 的 bundle / verify / PME 閉環正式落地；`WORK-M4-B` 隨後也已完成，正式補齊 release / support 文檔、platform validation runbook、release workflow preflight 與 Settings diagnostics。blocking path、coverage、`mutation:js`、browser-preview smoke 與 debug desktop build 都已通過；其後 `WORK-M4-D` 把 Rust mutation baseline 收斂成 parser crate + AI status/helper slice 的 signed-off contract，並把 whole-workspace `mutation:rust:full` 保留作 exploratory triage，而 `WORK-M4-C` 的安全研究也已在 ADR-005 / App Lock 實作中正式 close out。
- 2026-04-08 性能 closeout：`WORK-M4-G` 已把 Explorer day-one keyword recall 從 `LIKE` 收斂到 FTS5 `history_search` projection，manual backup 也改為透過 desktop progress event 顯示 profile-scoped phase log；同時補齊 [large-archive-performance-runbook.md](m4-full-polish/large-archive-performance-runbook.md)，讓之後的大型 archive triage 有固定 artifact bundle，而不是再靠一次性的口頭記錄。
- 2026-04-08 UI closeout：`WORK-M4-E` 已把 Dashboard / Explorer / Insights / Import / AI action 的 loading grammar 收斂成 skeleton + readable progress contract；`WORK-M4-C` 也補上 App Lock route、session guard、MCP refusal path 與 source-of-truth docs。M4 當前已切好的 work blocks 全部收口，下一輪需要從剩餘 docs/plan 開放項重新切出新的 half-milestone block。
- 2026-04-09 audit closeout：`WORK-QC-D` 與 `WORK-M1-C` 已完成，當時 closeout environment 的 `bun run verify` / `bun run check` / `bun run build` 已重新回綠；但這次審核也確認 repo **不能**聲稱「所有設計文檔需求都已完成」。M4 仍保留兩個真正的未完成主線：`WORK-M4-J`（60-year performance proof）與 `WORK-M4-I`（advanced intelligence shipping）。`WORK-M4-J` 現在已重新補回可重跑的 shell-scaling artifact script 與 checked-in bundle，但 final signoff 仍需要真實 large-profile replay，不是 synthetic bundle 即可代替。
- 2026-04-10 recoverability closeout：`WORK-M1-D` 已完成。repo 現在正式 shipping checkpoint-based `snapshot_restore` preview / execute、manual-first local retention prune，以及 run-ledger-backed rekey audit summary；M1 不再把 recoverability 留在「truth-only 文檔收尾」。仍保留 manual-first 的只剩 archive-file safety snapshot 在需要舊 key 時的恢復邊界，以及 M4 的剩餘主線 `WORK-M4-K` / `WORK-M4-L`。

這個結果很重要，因為它代表 repo 現在不只保住 desktop entry + typed IPC contract，也重新把 living M0-M3 quality surface 的 coverage、build、e2e 與 deep-check 分層拉回可兌現狀態。

2026-04-06 審查修正：M1 的 archive feature baseline 已經落地，但 milestone 本身仍有 closeout 要完成。當時非前端剩餘重點收斂到 `M1-DB` / `M1-OPS` 的 acceptance matrix、security mode taxonomy、retention / audit summary；這些 gap 現已由 `WORK-M1-D` 收口。前端 shell / route / sidebar 的驗收仍不能借用舊的 shell slice 敘事，必須由前端 owner 補上獨立驗收。

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
| `M0`   | 切斷舊 UI 和舊產品骨架，建立新的前端、後端和資料平面起點                 | `[x]` | [m0-foundation/README.md](m0-foundation/README.md)             |
| `M1`   | 把 Archive、Audit、Schedule、Security、Explorer v1 做成可信的基礎        | `[x]` | [m1-solid-archive/README.md](m1-solid-archive/README.md)       |
| `M2`   | 補齊導入、回滾、Doctor、多瀏覽器、PME、i18n 和跨平台排程                 | `[x]` | [m2-recall-and-trust/README.md](m2-recall-and-trust/README.md) |
| `M3`   | 在穩定 archive 之上加入 optional AI provider、index、assistant、insights | `[x]` | [m3-intelligence/README.md](m3-intelligence/README.md)         |
| `M4`   | 補齊 enrichment、進階洞察、remote backup、release polish 和多平台驗證    | `[/]` | [m4-full-polish/README.md](m4-full-polish/README.md)           |

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
