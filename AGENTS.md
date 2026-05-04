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
4. 如果改了代碼，跑驗收命令：`bun run check`
   - `bun run check` 是權威 per-commit gate：base checks、100% JS/Rust coverage、browser build、browser-preview e2e、desktop-bridge truth gate、以及 desktop-contract JS mutation 必須在同一條 checker 裡通過。

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
6. 只要完成了**可以獨立提交**的工作量，就要立即整理成原子 commit；不要等整個 work block 全部收尾才一起提交
7. commit message 一律遵守 Conventional Commits，且內容要精準描述這一個原子改動

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

- 看 `docs/design/screens-and-nav.md` — 畫面結構和導航規格
- 看 `docs/design/ui-review-guardrails.md` — 長期 UI review / implementation 紅線（全寬白名單、限高內滾動、真機 truth gate、Explorer pagination / detail rail）
- 看 `docs/design/ux-principles.md` — PME 模型
- 看 `docs/design/design-tokens.md` — token 與 theme contract 的 source of truth
- i18n 是 UI 的硬性契約：所有新的 user-visible copy、placeholder、aria-label、loading / skeleton label、empty / error / disabled state、以及 browser preview honesty copy 都必須先想好 `en` / `zh-CN` / `zh-TW`
- `src/app/`、`src/styles/`、`src/pages/` 是目前活的 shell surface；不要把新工作塞回已刪除的 `AppNew` 思路

### Rust/後端相關

- 看 `docs/architecture/data-model.md` — canonical schema
- 看 `docs/architecture/tech-stack.md` — 技術選型決策
- 看 `docs/plan/program/research-and-decisions.md` — 確認沒有 `[!]` blocker
- 巨型檔案要拆，不要繼續往裡加：`archive/mod.rs`、`chrome.rs`、`ai.rs`、`insights.rs`、`vault-worker/src/lib.rs`

### 任何工作

- 先讀對應的 `docs/features/` 子文檔
- 如果「讀先」文檔之間打架：先修文檔，讓 source of truth 恢復一致，再寫代碼
- 新功能沒有 i18n 就不算完成：至少同步補齊翻譯 key、locale / pseudo-locale smoke、以及相關 literal guard / coverage
- 代碼改動，完成後跑 `bun run check`，通過才提交

---

## 代碼結構

```
src/                          React 19 + TypeScript 前端
  main.tsx                    入口（載入 src/app）
  app/                        shell、router、onboarding shell
  styles/                     token layer + app shell styles
  lib/backend.ts              legacy helper / reference / browser-preview fixture surface，避免新增 shell contract，也不要往裡加新的 raw English UI copy
  lib/ipc/bridge.ts           typed IPC wrapper
  lib/app-context.tsx         舊全域狀態，reference only，不再擴寫
  pages/                      route-scoped page skeletons 與後續正式頁面
  components/                 shared components / primitives

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

**前端架構紅線（Frontend Architecture Guardrails）**：

- 組件只負責渲染和局部交互；業務邏輯、數據轉換、副作用管理不允許洩漏進 UI 層
- 禁止「上帝組件」（God Component）：單個組件承擔超過一個清晰職責時，必須拆分
- 大數據量渲染必須使用虛擬化（virtualization）；禁止在列表/表格組件中全量渲染無上限的數據集
- Props drilling 超過兩層時必須引入 context 或狀態管理，不允許繼續透傳
- 新的共享狀態不允許塞進 `lib/app-context.tsx`（已標記為 reference only）；需要新增狀態管理時先在 `BACKLOG.md` 提出設計方案

---

## 核心原則

1. **Trust & Transparency** — 所有操作走 PME 流程（Preview → Manual → Execute），沒有黑盒執行
2. **Data Sovereignty** — 數據不上傳雲端，AI 也用本地或用戶自配的 provider
3. **Performance Is A First-Class Constraint** — 目標機器為 4 核 3GHz CPU / 8GB RAM，需流暢支撐 1440 萬條+ 歷史記錄（中度用戶 60 年積累），並支持一次性批量導入同等量級的數據。任何代碼改動都需通過心理測試：_「1440 萬條數據時，這段邏輯會怎樣？」_ 不能製造不必要的全量加載、主線程阻塞、或無上限的內存增長。UI 響應延遲是核心關注點。UI 不能在任何情況下凍結，對於潛在的耗時任務和性能熱點，要專門做 UI 優化。
4. **Intelligence Is Optional** — 沒有 LLM 或 Embedding provider 時 PathKeep 仍完整可用，AI 是增值層
5. **Internationalization Is A Shipping Contract** — user-visible copy、honesty note、loading label、preview fixture text 都不是最後才補的 polish，而是開發當下就要交付的產品契約

---

## 工作規範

- **品質紅線**：永遠遵守最佳實踐，總是選擇長期最優解，避免臨時方案，絕對不能糊弄測試，或是為了降低開發成本做出妥協。開發的時間和精力成本不在決策考慮範圍內，但要注意降低代碼複雜度。
- **新增依賴供應鏈紅線**：未經用戶明確授權，不允許引入新的 Cargo / npm / Bun / Tauri / build-time / dev-time 依賴，除非該依賴至少滿足以下任一條件：
  - GitHub 星標大於 **6k**。
  - 維護者名聲極大、社會地位極高，且項目有嚴格代碼審查。
  - 項目由高知名度可信開源組織或科技巨頭維護。
    若官方方案不可用，且替代依賴無法明確滿足上述信任門檻，必須先輸出供應鏈風險評估與替代方案，等待用戶明確批准；不能為了實作便利引入低信任度依賴。
- **C / C++ native 依賴管理紅線**：產品碼不得靠 Homebrew / apt / winget / 全局 `pkg-config` / 全局 dylib 來找到 native library。新的 C / C++ 產品依賴必須先進 `vcpkg.json`，由 `vcpkg-configuration.json` pin 住 registry baseline，並透過 `scripts/native-deps.mjs` 安裝到 repo-local `var/native-deps/vcpkg_installed`；若 vcpkg 方案不可行，必須先寫 ADR 說明替代方案、CI proof、release packaging、rollback path，不能直接在 `build.rs` 裡下載或編譯任意 C/C++ source。Tauri / OS SDK 所需平台 framework 仍可作為明確 host prerequisite，但不得把 OpenCC、marisa、SQLite extension、或其他產品 native library 混進全局前置條件。
- **Commit**：`feat(ui): ...` / `fix(archive): ...` / `chore(deps): ...`，保持 commit 可 review；不要因為 work block 變大就做單一巨型 commit。只要手上的變更已經形成一個可驗證、可回顧的原子單位，就要直接提交，不要把多個無關修復或文檔更新長時間混在 working tree
- **Tests**：JS/TS 用 Vitest，Rust 用 `cargo test`。現行 blocking / release gate 以 `docs/plan/program/quality-matrix.md` 為準：`bun run check` 必須跑 base checks、`coverage:js`、`coverage:rust`、`build`、`test:e2e`、`test:e2e:desktop-bridge:truth`、以及 desktop-contract JS mutation。`coverage:js` 覆蓋所有 active `src/**/*.{ts,tsx}` runtime source；只允許排除 tests、fixtures、assets、generated/type-only files、以及已證明不是 runtime surface 的 reference-only files。`coverage:rust` 覆蓋 full `src-tauri/**/src/*.rs` workspace source。全量 frontend Stryker 與 whole-workspace `cargo mutants` 保留為 `check:deep` / scheduled mutation workflow，不作 per-commit hard gate；surviving mutant 只能用補測、修產品碼、或 narrow equivalent/inapplicable exclusion + doc note 處理，不能用 broad exclusion 偽裝成通過。
- **Focused helpers**：`check:base`、Rust quality slice、full mutation sweeps 等只作 triage/deep helpers；不能替代 signed-off `bun run check`。desktop contract mutation 是 `bun run check` 內的 lightweight mutation gate，保護 `src/main.tsx` 與 `src/lib/ipc/bridge.ts` 的入口/IPC contract。
- **Accepted docs 決策不可隨意推翻**：如果 `docs/` 中已經有 `Accepted` 狀態、或其他明文確立的 source-of-truth 決策，**不允許** agent 因為直覺、實作偏好、或一句「降低複雜度」就直接改寫。這不代表不能推翻；但要推翻時，必須先產出**詳細的 trade-off 決策文檔**，包含問題定義、約束、候選方案、優缺點、多方案比較、風險、回滾策略與推薦理由。
- **推翻 accepted docs 的前置要求**：必須先做深度調研與必要的外部研究 / benchmark / packaging / upgrade / operational evidence，不能只靠本地猜測；必須明確說明為什麼既有決策不再成立、替代方案為什麼更好、代價是什麼；整理完後**先徵求用戶意見**，得到明確確認後，才能修改 accepted docs 與相關計劃 / 架構文檔。
- **Backlog / research item 不是推翻授權**：`research-and-decisions.md` 裡的未決項、`[!]` blocker、或 work block 需求，不代表 agent 可以自行覆寫既有 accepted docs。若發現 backlog 與 accepted docs 看起來衝突，先停下來補研究或提問，不能直接把 pending research 當成重開決策的授權。
- **Test 位置**：`foo.ts` → `foo.test.ts`（放旁邊），E2E 放 `tests/e2e/`
- **注釋**：代碼注釋即開發者文檔，重要技術決策、trade-off 在代碼處寫注釋
- **文檔更新**：改功能行為 → 更新 `docs/features/`；新技術決策 → 更新 `docs/architecture/`
- **提交前**：`bun run check` 全過才提交；`bun run check` 已包含 browser build。
- **大文件重構的兩階段原則**：任何超過 1000 行的文件，必須先完成審查階段（輸出架構地圖、職責清單、拆分方案），詳細審查影響，並確認或補充自動化測試，確保拆壞了也能發現後，才能進入執行階段。審查階段不改一行代碼。這條規則同樣適用於 AI agent 主動發現的大文件，不只是被明確指派的重構任務。

- **文件行數硬限制**：
  - 單文件超過 **1200 行**：必須在當前 work block 的收工步驟裡，在 `BACKLOG.md` 新增一條可維護性分析任務，評估是否拆分。
  - 單文件超過 **1400 行**：當前 work block **不允許**再往該文件新增業務邏輯；只允許 bug fix 或臨時性的最小改動，並在 `BACKLOG.md` 標記為高優先級
  - 新創建的文件不得在初始版本就超過 800 行；超過說明職責邊界沒有想清楚，先拆模塊再寫代碼
  - 注意: 不要為了拆而拆。如果很明顯不拆是更好的架構設計，且評估過拆與不拆的架構設計，可以不拆並忽略文件行數的硬限制。

- **Doc Comments 標準**：所有新建或整段重寫的模塊，必須在交付時附帶完整 doc comments，不允許事後補充：
  - **文件頂部**：一句話職責說明 + `## Responsibilities`（列舉） + `## Not responsible for`（明確排除，防止邊界蔓延） + `## Dependencies` + `## Performance notes`（如有大數據量注意事項）
  - **所有 exported function / hook / class / type**：說明「為什麼需要這個」而不是「它做了什麼」（後者代碼已經說了）；標注參數語義、返回值語義、邊界條件；性能敏感路徑必須標注
  - 注釋說人話，不寫 `This function does X` 式廢話
  - Design document 的決策理由融入 doc comments，我們不單獨維護文檔站

---

## 常用命令

```bash
bun run desktop:dev      # 完整 Tauri 桌面 app
bun run build            # TypeScript + Vite bundle
bun run check            # 權威 per-commit checker（base + 100% coverage + build + e2e + desktop-contract mutation）
bun run check:base       # fast triage helper（static/unit/native checks）
bun run verify           # check + debug desktop build release rehearsal
bun run test:unit        # Vitest unit tests
bun run test:unit:desktop-contract # desktop contract slice 的 targeted unit tests
bun run check:desktop-contract # desktop contract slice 的 targeted unit + coverage sub-gate
bun run test:e2e         # Playwright e2e
bun run test:e2e:desktop-bridge:truth # Chrome + real Rust desktop command bridge truth gate
bun run coverage:js      # active src runtime TS/TSX 的 100% coverage gate
bun run coverage:js:desktop-contract # desktop contract slice 的 targeted JS 覆蓋率
bun run coverage:rust    # full src-tauri/**/src/*.rs 的 100% coverage gate
bun run mutation:js      # desktop contract slice 的 targeted mutation tests
bun run mutation:js:full # active src runtime TS/TSX 的 full Stryker sweep（manual/deep）
bun run mutation:rust    # whole-workspace cargo-mutants sweep（manual/deep）
bun run mutation:rust:full # mutation:rust 的明確別名 / deep entrypoint
bun run mutation         # full JS + full Rust mutation sweep（manual/deep）
bun run check:deep       # check + full JS/Rust mutation sweep
bun run native-deps:doctor # 檢查 project-scoped vcpkg native dependency 設定
bun run native-deps:install:opencc # 安裝 OpenCC native proof lane 到 var/native-deps
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

_這份文檔是 living document。發現過期或不準確，直接修正它 (AGENTS.md)。_
