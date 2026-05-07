# AGENTS.md — PathKeep

你是被派進來繼續工作的 AI coding agent。**先讀完這頁，再動手。**

PathKeep 是個跨平台 local-first 桌面應用，用於備份，聚合和分析瀏覽器的歷史記錄。
Tech: Tauri 2 + Rust + React 19 + TypeScript + Vite + Bun。

---

## 核心原則（不可妥協）

1. **Trust & Transparency** — 所有操作走 PME（Preview → Manual → Execute），無黑盒。
2. **Data Sovereignty** — 數據不上雲；AI 用本地或用戶自配 provider。
3. **Performance is a constraint, not a goal** — 目標機 4 核 3GHz / 8GB RAM，需流暢支撐 1440 萬條歷史記錄（中度用戶 60 年積累）與同量級一次性導入。每段邏輯都要過心理測試：「1440 萬條時這會怎樣？」禁止全量加載、主線程阻塞、無上限內存增長；UI 不可凍結。
4. **Intelligence is optional** — 沒 LLM/Embedding 時 PathKeep 仍完整可用。
5. **i18n is a shipping contract** — 所有 user-visible copy（含 placeholder、aria-label、loading/empty/error/disabled、preview honesty note）開發當下就要交付 `en` / `zh-CN` / `zh-TW`，不是事後 polish。

**品質紅線**：選長期最優解，避免臨時方案，不糊弄測試。開發時間成本不在決策考量內，但代碼複雜度要控。

**Accepted docs 不可隨意推翻**：`docs/` 中 `Accepted` 狀態或明文確立的決策，不允許因直覺或「降低複雜度」就改寫。要推翻必須先做外部研究/benchmark，產出含問題定義、候選方案、優缺點、風險、回滾策略的 trade-off 文檔，**徵得用戶明確同意後**才修改。`research-and-decisions.md` 的 backlog/`[!]` 不等於推翻授權。

---

## 工作循環

### 開工

1. 讀 `docs/plan/STATUS.md`，找第一個 `[ ]` work block。
2. 只讀該 block「讀先」清單裡的文檔。若清單衝突/失效/不足，先補讀直接相關檔案並**修文檔**，再寫代碼。
3. 執行整個 block 範圍（粒度約半個 milestone）。
4. 改了代碼就跑 `bun run check`，過了才提交。

### 收工

1. `STATUS.md`：`[ ]` → `[x]`。
2. 把完成的 block append 到 `docs/plan/CHANGELOG.md` 底部（不改舊內容）。
3. 同步回寫 source docs：`research-and-decisions.md` 的 `PG-RD-*`、milestone README/checklist、`BACKLOG.md` 的 `[!blocked: ...]`。功能或技術決策有變動，連同 `docs/features/` `docs/design/` `docs/architecture/` 一併更新。
4. `STATUS.md` 清空時，從 `BACKLOG.md` 頂部取最多 2 個未阻塞 block 剪到 CURRENT FOCUS，並更新阻塞標記。

### 分支判斷

- `STATUS.md` 有 `[ ]` → 做第一個。
- 全 `[x]` 或空 → 從 `BACKLOG.md` 補。
- 標 `[!]` → blocked，跳過。
- 計劃外大工作 → 進 `BACKLOG.md`，不直接做。

---

## 文檔地圖

| 文檔                                          | 用途                                                          |
| --------------------------------------------- | ------------------------------------------------------------- |
| `docs/plan/STATUS.md`                         | 當前 work block（每次開工必讀）                               |
| `docs/plan/BACKLOG.md`                        | 後續隊列 + inline blocked 標記                                |
| `docs/plan/CHANGELOG.md`                      | 已完成日誌（append-only）                                     |
| `docs/plan/README.md`                         | 里程碑總覽                                                    |
| `docs/plan/program/research-and-decisions.md` | 未落地技術決策                                                |
| `docs/plan/program/repo-baseline.md`          | 現有 repo 問題清單（含大檔行數）                              |
| `docs/plan/program/quality-matrix.md`         | blocking gate 權威定義                                        |
| `docs/vision-and-requirements.md`             | 產品定位                                                      |
| `docs/features/`                              | 功能規格                                                      |
| `docs/architecture/`                          | 技術原則、data model、tech stack                              |
| `docs/design/`                                | UX 原則、screens-and-nav、ui-review-guardrails、design-tokens |

**開工前依工作類型補讀**：

- UI 工作 → `docs/design/` 全部四份 + 規劃好三語 i18n key。
- Rust/後端 → `docs/architecture/data-model.md`、`tech-stack.md`，確認 `research-and-decisions.md` 無 `[!]` blocker。
- 任何功能 → 對應 `docs/features/` 子文檔。

「讀先」清單之間打架時：先修文檔讓 source of truth 一致，再寫代碼。

---

## 代碼結構約束

只列**靠看樹狀圖看不出來**的約束，目錄結構自己 `ls`：

- `src/main.tsx` 是入口；`src/app/` `src/styles/` `src/pages/` 是活的 shell surface。**禁止**回到已刪除的 `AppNew` 思路。
- `src/lib/backend.ts` 是 legacy / browser-preview fixture surface，**不新增 shell contract，也不加 raw English UI copy**。
- `src/lib/app-context.tsx` 標為 reference only，**不准擴寫**；新共享狀態先在 `BACKLOG.md` 提設計。
- `src/lib/ipc/bridge.ts` 與 `src/main.tsx` 是 desktop contract 入口，受 mutation gate 保護。
- 巨型檔案不准再加業務邏輯（具體清單與行數見 `repo-baseline.md`）。

### 前端架構紅線

- 組件只負責渲染與局部交互；業務邏輯/數據轉換/副作用不洩漏進 UI。
- 禁止上帝組件；單一組件超過一個清晰職責就拆。
- 列表/表格大數據必須虛擬化，不允許全量渲染無上限數據集。
- Props drilling 超過兩層 → 引 context/state；禁止繼續透傳。

### 文件行數

- 新建檔案初版不得超過 **800 行**（超過代表職責邊界沒想清楚）。
- 既有檔案 > **1200 行** → 收工時在 `BACKLOG.md` 加可維護性分析任務。
- 既有檔案 > **1400 行** → 只允許 bug fix 或最小改動，禁止新業務邏輯，並在 `BACKLOG.md` 標高優先級。
- 例外：經過評估、不拆明顯更好的，可豁免（在代碼或 doc comment 留下理由）。

### 大文件重構兩階段

任何 > 1000 行的檔案，重構前先完成**審查階段**（架構地圖、職責清單、拆分方案、測試覆蓋確認），審查階段不改一行代碼，再進入**執行階段**。AI agent 主動發現也適用。

---

## 依賴紅線

**新增依賴需用戶授權**，除非至少滿足一條：

- GitHub stars > 6k；
- 維護者高知名度且嚴格 code review；
- 由可信開源組織或科技巨頭維護。

不滿足時必須先寫供應鏈風險評估與替代方案，等批准。

**C/C++ native 依賴**：產品碼**不得**依賴 Homebrew/apt/winget/全局 `pkg-config`/全局 dylib。新依賴必入 `vcpkg.json`，由 `vcpkg-configuration.json` pin baseline，透過 `scripts/native-deps.mjs` 安到 `var/native-deps/vcpkg_installed`。vcpkg 不可行 → 先寫 ADR（替代方案、CI proof、release packaging、rollback）。Tauri/OS SDK 平台 framework 可作 host prerequisite，但不准混入 OpenCC、marisa、SQLite extension 等產品 native lib。**禁止**在 `build.rs` 下載或編譯任意 C/C++ source。

---

## Commit & 測試

### Commit

- Conventional Commits：`feat(ui): ...` / `fix(archive): ...` / `chore(deps): ...`，subject ≤ 72 字。
- 一個 work block 可拆多個 atomic commit；只要形成可驗證的原子單位就立即提交，不要在 working tree 久留。
- Body 必含 **Why**（動機/不改的後果，最重要）與 **What**（改了哪些模塊與關鍵變化）；有重要 trade-off 寫 **How**；屬於更大目標寫 **Context**。禁止「fix bug」「update code」「minor changes」式描述。

### 測試

- JS/TS 用 Vitest（`foo.ts` → `foo.test.ts` 同目錄），Rust 用 `cargo test`，E2E 在 `tests/e2e/`。
- **權威 gate 是 `bun run check`**：base checks + 100% `coverage:js` + 100% `coverage:rust` + build + e2e + `test:e2e:desktop-bridge:truth` + desktop-contract JS mutation。具體規則以 `docs/plan/program/quality-matrix.md` 為準。
- `coverage:js` 覆蓋 active `src/**/*.{ts,tsx}` runtime；只允許排除 tests / fixtures / assets / 純型別 / 已證非 runtime 的 reference-only files。
- `coverage:rust` 覆蓋 full `src-tauri/**/src/*.rs`。
- 全量 Stryker / `cargo mutants` 在 `check:deep` 跑，非 per-commit gate。surviving mutant 必須補測、修產品碼或加 narrow exclusion + doc note，**不允許**用 broad exclusion 偽裝過關。
- `check:base` 等 helper 是 triage 工具，**不能**取代 signed-off `bun run check`。

### 文檔與註解

- 改功能行為 → 更新 `docs/features/`；新技術決策 → 更新 `docs/architecture/`。
- 新建/整段重寫的模塊**交付時**附 doc comments（不允許事後補）：
  - 檔頭：一句話職責 + `## Responsibilities` + `## Not responsible for`（防邊界蔓延）+ `## Dependencies` + 必要時 `## Performance notes`。
  - exported 符號：解釋「為什麼存在」而非「做了什麼」；標參數/回傳語義、邊界條件；性能敏感路徑必標注。
- 註解說人話，不寫 `This function does X` 廢話。

---

## 命令速查（權威 gate 是 `bun run check`）

```sh
bun run desktop:dev                       # 完整 Tauri 桌面 app
bun run build                             # TS + Vite bundle
bun run check                             # 權威 per-commit gate
bun run check:deep                        # check + full JS/Rust mutation sweep
bun run verify                            # check + debug desktop release rehearsal
bun run check:base                        # 快速 triage（不可取代 check）
bun run test:unit | test:e2e              # Vitest / Playwright
bun run coverage:js | coverage:rust       # 100% coverage gate
bun run mutation:js | mutation:rust       # 完整 mutation sweep（manual/deep）
bun run native-deps:doctor                # 檢查 project-scoped vcpkg 設定
bun run format                            # Prettier
```

---

## 命名

- React component：`PascalCase`
- TS 工具/檔名：`kebab-case`（e.g. `browser-icons.tsx`）
- Rust 模組/函數：`snake_case`

格式由 Prettier / editorconfig 管，不在這裡重複。

---

_Living document。發現過期或不準確就直接修。_
