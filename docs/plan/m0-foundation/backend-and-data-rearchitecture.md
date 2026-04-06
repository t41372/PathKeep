# M0-BE — Backend And Data Rearchitecture

> 讀這份文檔的時機：當你要把現有巨型 Rust 模組拆回健康邊界、建立新 canonical data plane、為 M1 的 archive engine 打地基時。  
> 這份文檔不追求把所有 Archive 功能做完；它追求的是把功能放回正確的位置。

---

## Source Inputs

- [../../vision-and-requirements.md](../../vision-and-requirements.md)
- [../../architecture/data-model.md](../../architecture/data-model.md)
- [../../architecture/tech-stack.md](../../architecture/tech-stack.md)
- [../../database-selection-decision-2026-04-05.md](../../database-selection-decision-2026-04-05.md)
- [../../features/archive.md](../../features/archive.md)
- [../program/research-and-decisions.md](../program/research-and-decisions.md)
- [../program/repo-baseline.md](../program/repo-baseline.md)

---

## 本工作包要交付什麼

- `browser-history-parser` 的 crate 邊界、API 和 fixture strategy
- `vault-core` / `vault-platform` / `vault-worker` / Tauri command 的責任重切
- canonical archive schema、migration ledger、run model、timestamp contract 的正式設計
- 新 command surface 和 job orchestration foundation
- 可支撐 M1 的測試基線和 fixture 結構

---

## Open Blockers

- [!] `M0-BE-BLK-001` 先完成 [../program/research-and-decisions.md](../program/research-and-decisions.md) 中 `PG-RD-ARCH-001` 到 `PG-RD-ARCH-006` 的核心決策，否則 schema 重構會一直返工。
- [!] `M0-BE-BLK-002` 先決定 archive reset strategy，否則無法安排舊 DB 和新 DB 的切換與 migration story。

---

## WBS

### Module Boundary Reset

- [ ] `M0-BE-MD-001` 盤點 [`src-tauri/crates/vault-core/src/archive.rs`](../../../src-tauri/crates/vault-core/src/archive.rs) 內所有責任，拆成 schema、backup pipeline、query、export、doctor、rollback、security、browser ingestion 類別。
- [ ] `M0-BE-MD-002` 盤點 [`src-tauri/crates/vault-core/src/chrome.rs`](../../../src-tauri/crates/vault-core/src/chrome.rs) 內的 browser discovery、path heuristics、staging copy、profile metadata，分出 parser-layer 和 platform-layer 邊界。
- [ ] `M0-BE-MD-003` 盤點 [`src-tauri/crates/vault-core/src/ai.rs`](../../../src-tauri/crates/vault-core/src/ai.rs) 和 [`src-tauri/crates/vault-core/src/insights.rs`](../../../src-tauri/crates/vault-core/src/insights.rs) 中哪些屬於 M3 derived state，哪些暫時保留，哪些應搬走。
- [ ] `M0-BE-MD-004` 建立新的 module map，明確定義 parser、archive core、platform adapters、worker orchestration、desktop command facade 的責任。
- [ ] `M0-BE-MD-005` 決定 Tauri commands 是否繼續全部留在 [`src-tauri/src/lib.rs`](../../../src-tauri/src/lib.rs) facade 下，或拆出 command modules；要求命名和 use case 對齊而不是對齊舊實作。
- [ ] `M0-BE-MD-006` 為每個 crate 定義可接受依賴方向，禁止 parser 依賴 Tauri、禁止 archive core 依賴 UI 命名與桌面框架。
- [ ] `M0-BE-MD-007` 為 worker 和 core 的共用型別建立最小共享層，避免跨 crate copy-paste 或反向依賴。

### Browser History Parser Extraction

- [ ] `M0-BE-PR-001` 建立 `browser-history-parser` crate 草案結構，至少分出 Chromium、Firefox、Safari、Takeout 的 provider module。
- [ ] `M0-BE-PR-002` 定義 parser crate 的輸入輸出：原始檔案路徑 / 目錄、staging source、parsed rows、metadata、warning surface。
- [ ] `M0-BE-PR-003` 凍結 parser crate 不碰 canonical schema、不碰 Tauri command、不碰 keyring 和 scheduler。
- [ ] `M0-BE-PR-004` 抽出 Chromium visit / url / download / search-term 解析邏輯到 parser crate，建立最小 compile/test pass。
- [ ] `M0-BE-PR-005` 抽出 Firefox 解析邏輯到 parser crate，保留 profile discovery 和 staging 決策在 platform / core。
- [ ] `M0-BE-PR-006` 抽出 Safari 解析和 macOS 特有 path knowledge 的邊界，避免平台檢測和資料解析混寫。
- [ ] `M0-BE-PR-007` 抽出 Google Takeout parsing 和 validation 基礎，至少形成 fixture 可跑的 importer parser。
- [ ] `M0-BE-PR-008` 為 parser crate 建立 edge-case fixture：鎖檔、缺欄位、歷史 schema、空資料、異常時間戳、損毀列。
- [ ] `M0-BE-PR-009` 決定 parser crate 的版本管理、公開 API 和 internal helper 的穩定性等級。

### Canonical Schema And Migration Foundation

- [ ] `M0-BE-SC-001` 依照 [../../architecture/data-model.md](../../architecture/data-model.md) 產出新 canonical schema v1 草案，不直接沿用現有 `archive-schema.sql`。
- [ ] `M0-BE-SC-002` 凍結 timestamp contract：毫秒整數欄位、ISO 顯示輔助欄位、timezone metadata、unknown timezone fallback 規則。
- [ ] `M0-BE-SC-003` 凍結 run ledger 模型，決定 `backup`、`import`、`revert`、`doctor`、`snapshot restore` 是否共用同一張 run table。
- [ ] `M0-BE-SC-004` 凍結 rollback visibility model，區分 immutable raw facts、logical visibility、derived state rebuild policy。
- [ ] `M0-BE-SC-005` 設計 `schema_migrations` 和 migration execution flow，取代 ad-hoc 升級方式。
- [ ] `M0-BE-SC-006` 決定 migration 檔案格式、命名、checksum、idempotency 和測試策略。
- [ ] `M0-BE-SC-007` 設計 manifest、snapshot、watermark、source profile、run artifact 的表關係和外鍵策略。
- [ ] `M0-BE-SC-008` 明確標記哪些 intelligence 表是 canonical、哪些是 derived、哪些應搬到 sidecar。
- [ ] `M0-BE-SC-009` 產出現有 schema 到新 schema 的 gap table，標明 `drop`、`rename`、`migrate`、`defer to M3`。

### Core Service And Command Surface

- [ ] `M0-BE-CM-001` 盤點現有 Tauri command 清單，對應到新畫面 use case，刪掉只有舊 UI 會呼叫的接口。
- [ ] `M0-BE-CM-002` 為 onboarding、dashboard、explorer、audit、schedule、security、settings 定義新的 command / read-model 分層。
- [ ] `M0-BE-CM-003` 區分 query API、mutating command、long-running job trigger、artifact fetch 四種接口類型。
- [ ] `M0-BE-CM-004` 為每種 mutating command 定義 preview / manual / execute 所需的 request / response envelope。
- [ ] `M0-BE-CM-005` 為 run detail / audit artifact 建立一致的 serialization contract，避免每個命令各自拼字串。
- [ ] `M0-BE-CM-006` 定義錯誤模型：user-facing error code、action hint、retry hint、support / debug payload 分層。

### Worker And Orchestration Foundation

- [ ] `M0-BE-WK-001` 盤點 [`src-tauri/crates/vault-worker/src/lib.rs`](../../../src-tauri/crates/vault-worker/src/lib.rs) 的角色，拆出 desktop run orchestration、future queue worker、MCP bridge 的邊界。
- [ ] `M0-BE-WK-002` 定義 long-running job 最小生命週期：queued、previewed、running、succeeded、failed、rolled_back、cancelled。
- [ ] `M0-BE-WK-003` 決定桌面端前景執行和背景 worker 的責任分配，避免 schedule 和 manual run 走兩套不同邏輯。
- [ ] `M0-BE-WK-004` 定義 artifact 落盤策略：preview artifact、run log、manifest、snapshot metadata、doctor report 放在哪裡。
- [ ] `M0-BE-WK-005` 決定 worker 如何回報進度、warning、partial result、final artifact index 給前端。
- [ ] `M0-BE-WK-006` 為未來 M3 job queue 預留最小抽象，但不提前把 AI 複雜度混入 M0 archive orchestration。

### Test Fixtures And Verification

- [ ] `M0-BE-QA-001` 建立 parser fixtures 目錄結構，區分 Chromium、Firefox、Safari、Takeout、damaged samples。
- [ ] `M0-BE-QA-002` 為 migration system 建立 from-scratch、upgrade-from-legacy、upgrade-twice、checksum mismatch 測試。
- [ ] `M0-BE-QA-003` 為 run ledger / timestamp contract 建立 deterministic tests，避免 timezone 和 clock 引入 flake。
- [ ] `M0-BE-QA-004` 為 Tauri command facade 建立 contract tests，確保前端型別和後端回傳一致。
- [ ] `M0-BE-QA-005` 在 M0 結束前把核心巨檔拆分到足以讓 coverage、mutation 和 review 具可讀性。
- [ ] `M0-BE-QA-006` 產出正式的重構分支順序和合併策略，避免 parser / core / schema / commands 同時改造成難以 review 的 mega PR。

---

## Exit Artifacts

- crate / module boundary map
- parser crate skeleton 和 fixture strategy
- canonical schema v1 草案與 migration ledger 設計
- 新 command surface 草案
- worker / orchestration 基礎設計
