# AGENTS.md — PathKeep

你是被派進來繼續工作的 AI coding agent。

PathKeep 是個跨平台 local-first 桌面應用，用於備份、聚合和分析瀏覽器的歷史記錄。
Tech: Tauri 2 + Rust + React 19 + TypeScript + Vite + Bun。

---

## 核心原則

1. **Trust & Transparency** — 所有操作走 PME（Preview → Manual → Execute），無黑盒。
2. **Data Sovereignty** — 數據不上雲；AI 用本地或用戶自配 provider。
3. **Performance is a constraint** — 兩端都要扛：後端在目標機 4 核 3GHz / 8GB RAM 上流暢支撐 1440 萬條歷史記錄與同量級一次性導入；前端任何時刻都流暢不卡、不凍結主線程。每段邏輯都要過：「1440 萬條時這會怎樣？」
4. **Intelligence is optional** — 沒 LLM/Embedding 時 PathKeep 仍完整可用。
5. **i18n is a shipping contract** — 所有 user-visible copy（含 aria-label、空/錯/載入/禁用等非顯眼狀態）開發當下就交付 `en` / `zh-CN` / `zh-TW`。
6. **選長期最優解** — 避免臨時方案，不糊弄測試，不為省開發成本妥協。代碼複雜度要控。
7. **Accepted docs 不可隨意推翻** — `docs/` 中確立的決策要推翻，需 trade-off 文檔 + 用戶明確同意。

---

## 設計哲學

**前端流暢度是硬指標**：UI 永遠不准凍結主線程——「整個 app 卡死十秒」是嚴重缺陷，不是可接受的代價。重活（解析、聚合、大量計算、大數據轉換）一律切出主線程或分片異步，畫面隨時可響應。重元素 / 頁面載入立即給 skeleton 或漸進式載入，~100ms 內必有視覺回饋，絕不留空白或凍結的 spinner。互動與轉場要有動畫、要順（目標 60fps）；生硬跳變、缺動畫視為缺陷而非預設。

**優雅高效、先量測再下結論**：代碼設計要優雅高效——避免無謂 re-render（穩定 key / ref、適度 memo）、大列表必虛擬化、render path 不放重計算、輸入用 debounce / throttle、避免 N+1 與全量掃描。性能敏感改動要 profiling（React Profiler / devtools / flamegraph）驗證後才宣稱「夠快」，不靠猜。

**構建可重現**：不依賴開發機全局 Homebrew / apt / winget。Native 依賴首選 Rust crate vendored/bundled（如 `rusqlite` 的 `bundled-sqlcipher-vendored-openssl`），次選 vcpkg manifest mode（`vcpkg.json`），都不行寫 ADR。帶 C/C++ 後端的 Rust crate 是標準做法，不需要額外審批。

**供應鏈信任**：新依賴需用戶授權，除非 GitHub stars > 6k、維護者高信譽、或由知名組織維護。不滿足門檻先寫風險評估等批准。

**模塊小、職責清**：組件單一職責，大數據必虛擬化，業務邏輯不洩漏進 UI。檔案行數硬門檻與巨型檔案清單見 `repo-baseline.md`；重構 > 1000 行的檔案先做「審查階段」（架構地圖 / 拆分方案 / 測試覆蓋確認）再動代碼。

**受保護的入口**：`src/main.tsx` 與 `src/lib/ipc/bridge.ts` 是 desktop contract / IPC 入口，受 mutation gate 保護，改動需對齊既有 contract。`src/lib/backend.ts` 是凍結的 legacy / browser-preview fixture，不擴展其 contract。

**測試是契約**：權威 gate 是 `bun run check`，具體規則見 `quality-matrix.md`。

**文檔跟著代碼走**：改功能更新 `docs/features/`，新技術決策更新 `docs/architecture/`。新模塊交付時附 doc comments——檔頭標明職責邊界（Responsibilities / Not responsible for），exported 符號說「為什麼存在」而非「做了什麼」。

**Commit 說 Why**：Conventional Commits，subject ≤ 72 字。Body 必含動機與關鍵變化。

---

## 工作循環

### 開工

1. 讀 `docs/plan/STATUS.md`，找第一個 `[ ]` work block。
2. 只讀該 block「讀先」清單裡的文檔。清單不足先補讀再修文檔。
3. 執行整個 block。改了代碼就跑 `bun run check`，過了才提交。

### 收工

1. `STATUS.md`：`[ ]` → `[x]`。
2. Append 到 `CHANGELOG.md`。
3. 同步回寫相關 source docs（features / design / architecture / research-and-decisions）。
4. `STATUS.md` 清空時，從 `BACKLOG.md` 取最多 2 個未阻塞 block。

### 分支判斷

- 有 `[ ]` → 做。全完或空 → 從 `BACKLOG.md` 補。`[!]` → 跳。計劃外大工作 → 進 `BACKLOG.md`。

---

## 文檔地圖

| 文檔                                          | 用途                             |
| --------------------------------------------- | -------------------------------- |
| `docs/plan/STATUS.md`                         | 當前 work block（每次開工必讀）  |
| `docs/plan/BACKLOG.md`                        | 後續隊列 + blocked 標記          |
| `docs/plan/CHANGELOG.md`                      | 已完成日誌（append-only）        |
| `docs/plan/README.md`                         | 里程碑總覽                       |
| `docs/plan/program/research-and-decisions.md` | 未落地技術決策                   |
| `docs/plan/program/repo-baseline.md`          | 巨型檔案與 repo 問題追蹤         |
| `docs/plan/program/quality-matrix.md`         | blocking gate 權威定義           |
| `docs/vision-and-requirements.md`             | 產品定位                         |
| `docs/features/`                              | 功能規格                         |
| `docs/architecture/`                          | 技術原則、data model、tech stack |
| `docs/design/`                                | UX 原則、設計規範、tradeoff      |

**依工作類型補讀**：UI → `docs/design/` 核心四份（`ux-principles` / `screens-and-nav` / `ui-review-guardrails` / `design-tokens`）+ 相關 tradeoff；Rust → `docs/architecture/`；功能 → `docs/features/`。

---

## 命令速查

```sh
bun run check                             # 權威 per-commit gate
bun run check:base                        # 快速 triage
bun run check:deep                        # check + full mutation sweep
bun run desktop:dev                       # Tauri 桌面 app
bun run build                             # TS + Vite bundle
bun run verify                            # check + desktop release rehearsal
bun run test:unit | test:e2e              # Vitest / Playwright
bun run coverage:js | coverage:rust       # 100% coverage gate
bun run format                            # Prettier
```

---

## 命名

- React component：`PascalCase`
- TS 工具/檔名：`kebab-case`
- Rust 模組/函數：`snake_case`

---

_Living document。發現過期或不準確就直接修 AGENTS.md 文件。_
