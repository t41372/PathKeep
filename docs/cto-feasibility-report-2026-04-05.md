# Browser History Vault CTO 可行性研究報告

日期：2026-04-05  
對象文件：`docs/vision-and-requirements.md`

## 1. 結論摘要

### 最終判斷

這個 vision **整體技術上可行**，而且不是從零開始：目前代碼庫已經有相當強的底座，尤其是：

- 本地優先的 SQLite / SQLCipher 架構
- 分層的 Rust workspace
- staging copy 後再導入 archive 的安全備份流程
- manifest hash chain 與 Git 審計工件
- Google Takeout dry-run / quarantine 雛形
- AI provider 抽象、embedding pipeline、MCP worker 雛形
- 多平台排程 preview 設計

但這個 vision **不適合被當成「UI 重做 + 補幾個 feature」來做**。它本質上是一個產品架構升級，涉及：

- 資料模型重整
- run / import / rollback 語義重定義
- 正式 migration 系統
- 背景 job queue
- 搜尋層從 `LIKE` 升級到 FTS5
- Linux / Windows / macOS 排程一致性校正
- AI 與 enrichment 從同步請求改為離線可恢復計算

如果按工程風險來看，我的判斷是：

| 領域 | 可行性 | 判斷 |
|------|--------|------|
| Archive 核心重構 | 高 | 代碼底座已具備，主要是 schema 和 operation model 要升級 |
| Recall V1（時間軸、搜尋、篩選、匯出） | 高 | FTS5 和 timeline 聚合都可行 |
| Trust / Audit / Recoverability | 中高 | 方向正確，但 rollback 模型必須重做 |
| Cross-platform scheduler | 中 | 機制可行，但 Linux 語義目前不符合 vision，Windows/macOS 也要補實機驗證 |
| Intelligence V1 | 中 | 語義搜尋可做，洞察模塊需要 job queue 才能健康落地 |
| Full Intelligence / plugin ecosystem | 中低 | 可做，但必須放在 Archive/Recall 穩固之後，不能提早承諾交付速度 |

### CTO 建議

我建議採用以下總策略：

1. **先把 Archive 做成「可長期保存、可審計、可回滾」的資料系統。**
2. **Recall 只建立在穩定的 canonical schema 上，不直接綁死瀏覽器原生欄位。**
3. **AI / Insights / Enrichment 一律走背景 job queue，絕不阻塞備份主流程。**
4. **把幾個 vision 中的關鍵設計假設修正後再開工，否則後面會返工。**

最重要的需要修正的設計假設有四個：

- `ISO 8601 UTC string` 不應作為 archive 的主要內部時間主鍵；應改成 **UTC epoch milliseconds (`INTEGER`) 為主，ISO 8601 為輔**。
- run-level rollback 不能用物理刪除；必須改成 **軟刪除 / tombstone / reversible operation model**。
- Linux `systemd` timer 現有設計不能正確滿足「missed runs catch-up」。
- Google 帳號端歷史保留不應寫成「最多 18 個月」這種硬規則；官方行為是可配置且會變。

---

## 2. 研究方法

本報告基於三層材料：

1. 產品 vision：完整閱讀 `docs/vision-and-requirements.md`
2. 現況代碼：重點審查 `src-tauri/crates/vault-core`、`vault-platform`、`vault-worker`、`src-tauri/src/lib.rs`
3. 外部依賴與平台機制：核對官方文件，確認選型是否真的支撐 vision

這不是「需求重述」報告，而是 CTO 角度的三件事：

- 現有底座能不能支撐這個 vision
- 哪些 requirement 可以直接做，哪些要先改設計
- 哪些外部依賴現在看起來合理，哪些存在隱性風險

---

## 3. 現有代碼底座盤點

### 3.1 已經存在且可沿用的能力

| 能力 | 現況 | 判斷 |
|------|------|------|
| 多瀏覽器資料擷取 | `vault-core/src/chrome.rs` 已支援 Chromium / Firefox / Safari profile 探測與 staging copy | 可沿用 |
| 增量備份主流程 | `vault-core/src/archive.rs` 已有 `run_backup`、watermark、manifest 生成 | 可沿用，但要升級 schema |
| 審計 manifest hash chain | `vault-core/src/archive.rs:181-249` | 已符合正確方向 |
| Raw capture | `vault-core/src/archive-schema.sql:50-63` 的 `raw_row_versions` | 是長期 schema 演化的關鍵資產 |
| 原生快照 checkpoint | `vault-core/src/archive.rs:634-649`、`1100+` | 已有雛形，應擴展成正式 safety net |
| SQLCipher rekey / export | `vault-core/src/archive.rs:468-488` | 與 SQLCipher 能力匹配 |
| Takeout dry-run / quarantine | `vault-core/src/takeout.rs` | 已有產品雛形 |
| AI provider abstraction | `vault-core/src/ai.rs` | 選型合理，可延展 |
| MCP worker | `vault-worker/src/lib.rs:707-804` | 足夠支持 V1 本地 MCP 搜尋 |
| Remote backup bundle | `vault-core/src/remote.rs` | 已有明確雛形 |

### 3.2 已存在但需要重構的能力

| 能力 | 現況 | 問題 |
|------|------|------|
| Schema 演化 | `create_schema()` 依賴 `ensure_column()` 動態補欄位 | 不是正式 migration ledger，長期不可控 |
| 搜尋 | `list_history()` 仍是 SQL `LIKE` 查詢 | 無法支撐大規模 Recall |
| 回滾 | Takeout revert 仍對 `visit_events` 做 `DELETE` | 違反 append-only 與可逆 rollback vision |
| Insights | `run_insights()` 內含同步 refetch / 計算 / 持久化 | 缺少 job queue，會阻塞且不可恢復 |
| AI indexing | `build_ai_index()` 逐筆同步 embedding | 缺少排隊、重試、限速、可觀測性 |
| 排程 apply | `vault-platform/src/lib.rs:80-123` 只有 macOS 真正 apply | Windows / Linux 仍是 manual-first |

### 3.3 明顯缺口

- 沒有正式 `schema_migrations` 表與版本號管理
- 沒有 run-level reversible data model
- 沒有 FTS5 index 與 recall-oriented query planner
- 沒有 background job queue / retry / pause / resume
- 沒有 enrichment plugin framework
- 沒有 canonical run context（timezone / location / device context）
- 沒有跨平台排程的一致保證與驗證矩陣

---

## 4. 需求逐項可行性判斷

## 4.1 Archive

### 增量備份

**結論：高可行。**

理由：

- 現有主流程已經是正確架構：先複製 browser DB 到 staging，再導入 archive。
- raw row versioning 已存在，這是 archive longevity 的核心。
- manifest 與 source hash 已存在，審計方向正確。

要補的不是「能不能做」，而是：

- 正式 migration
- run metadata 擴展
- canonical schema 重整
- rollback 模型

### 排程

**結論：可行，但需平台語義修正。**

現況與風險：

- macOS 方案合理，`LaunchAgent + StartInterval` 與 vision 一致。
- Windows 用 Task Scheduler XML + `StartWhenAvailable=true` 是正確方向。
- Linux 現在生成的是 `OnBootSec=2m + OnUnitActiveSec={}h + Persistent=true`，但這個組合**不能靠 `Persistent=true` 保證 missed-run catch-up**。

因此：

- macOS：可列為第一平台正式支持
- Windows：可做，但要補 apply、registration、uninstall、status 檢查
- Linux：設計需要改成 calendar timer，不能沿用現在的 monotonic timer 假設

### Google Takeout 導入

**結論：高可行，但 rollback 模型必須重做。**

dry-run、recognized/quarantine、preview 方向都正確；真正不符合 vision 的是 revert 實作。現在 `revert_import_batch()` 仍然直接刪資料，這會造成：

- 不可逆
- 不可審計地改變主 archive 視圖
- 無法支撐「取消回滾」

### 遠端備份

**結論：可行。**

現有 `remote.rs` 已有：

- bundle 建立
- archive copy
- manifest 打包
- S3 / S3-compatible upload URL 推導
- preview command

但長期來看，建議把 `curl --aws-sigv4` 漸進收斂成內建 Rust client，而不是一直依賴外部 `curl`。

### 安全與加密

**結論：高可行。**

方向與 current stack 非常匹配：

- Tauri Stronghold 可承接 wrapped database secret
- system keyring 可作為 convenience unlock
- SQLCipher 的 `PRAGMA key` / `sqlcipher_export()` / `PRAGMA rekey` 都支撐 vision 的明文↔加密、改密碼流程

這一塊我認為是本專案 vision 裡最穩的一部分。

## 4.2 Recall

### Full-text search / timeline / filters

**結論：高可行。**

SQLite FTS5 完全足夠承擔 V1 的全文搜尋與 query/filter 組合。時間軸也不需要額外搜尋引擎，SQLite aggregation 足夠。

需要做的不是引入外部 search service，而是：

- 建立 canonical recall document
- 把 `visit_events`、title、URL、search terms、enrichment text 投影到 FTS5
- 為 timeline 建 bucketized statistics tables 或 materialized views

### Export

**結論：已基本可行。**

目前已有 HTML / Markdown / Text / JSONL 匯出雛形，可沿用。

## 4.3 Intelligence

### Semantic search

**結論：可行，但不能用現狀同步流程直接放大。**

embedding provider abstraction 已存在，Ollama / LM Studio / OpenAI-compatible provider 的路是通的。問題在於執行模型：

- 現在是同步逐筆處理
- 沒有 retries / backoff / concurrency budget
- 沒有 provider rate-limit policy
- 沒有 queue visibility

所以語義搜尋本身不是問題，**運營模型**才是問題。

### Ask My History / Agentic RAG

**結論：可行，但應明確收斂 V1 範圍。**

V1 建議只做：

- 搜尋
- evidence-backed retrieval
- 限定上下文窗口內的問答

不要一開始就承諾高度 agentic 的 multi-step planner，否則會過早把系統複雜度拉高。

### Insights

**結論：部分可行，應採模塊化漸進落地。**

下列洞察很適合先做：

- On This Day
- Site Analytics
- 基礎週/月摘要
- 簡單 topic trend

下列洞察應延後：

- Thread reopen
- Open Loops
- Learning trajectory
- Narrative arc

原因不是演算法做不到，而是它們對資料清洗、embedding 品質、job queue、enrichment coverage 的依賴太高。

---

## 5. 需要先修正的關鍵架構決策

## 5.1 時間主格式不應以 ISO 字串為主

vision 目前寫的是「Archive 內部統一使用 ISO 8601 UTC 字串作為主要時間格式，另外保留 epoch millis」。

我不建議這樣做。

### 建議改成

- **主欄位：`visited_at_ms INTEGER NOT NULL`**
- **輔助欄位：`visited_at_iso TEXT NOT NULL` 或 view/generated projection**
- raw payload 保留瀏覽器原始時間值

### 原因

- 排序、範圍查詢、bucket aggregation、窗口分析，全部都更適合 `INTEGER`
- 未來做 session、thread、trend、burst detection，`INTEGER` 更穩定
- SQLite 雖支援 ISO 8601 text，但官方也明確支援 Unix timestamp time-values；這是顯示與查詢的問題，不是主存格式必須選 text
- 20 年後的可讀性靠 schema 註釋、view、export format 就夠了，不需要讓主欄位為了「人眼可讀」犧牲運算特性

### CTO 判斷

這不是實作細節，而是會影響整個 archive schema 的底層決策。應在重構前修正。

## 5.2 回滾必須是 tombstone / reversible，而不是 delete

vision 寫得很清楚：回滾是軟刪除、可逆、保留審計。

目前實作不符合。

### 建議資料模型

- 所有 write operation 都有 `run_id`
- 所有可見記錄都帶 `ingested_by_run_id`
- 新增 `reverted_by_run_id NULL`
- 新增 `revert_reason / hidden_reason / reverted_at`
- 視圖層一律過濾 `reverted_by_run_id IS NULL`
- 取消回滾時，把 `reverted_by_run_id` 清空，並留下新的 audit operation

### 這樣的好處

- O(records in run) 回滾
- 可逆
- 可審計
- 不破壞 append-only 敘事

## 5.3 schema migration 必須正式化

目前 `create_schema()` 用 `execute_batch + ensure_column()` 補欄位，短期方便，長期危險。

### 必做項目

- 新增 `schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT, checksum TEXT, backup_path TEXT)`
- migration 採編號 SQL 檔
- 每次 migration 前自動做 archive snapshot
- migration report 寫入 audit artifacts

## 5.4 Linux scheduler 語義必須改

這是我在本次研究中最確定的一個工程性 bug。

vision 要求的是：

- 固定週期檢查
- 沒開機時不漏跑
- 開機/登入後能補跑

但 systemd 官方文件明確說明：`Persistent=` **只對 `OnCalendar=` 生效**，不對純 monotonic timer（如 `OnUnitActiveSec=`）生效。

因此現有 Linux 方案不應直接沿用。

### 建議

- 改為 `OnCalendar=` 方案
- 保留 worker 內部的 `due_only` gating
- timer 只負責喚醒，不負責真正判斷該不該備份

## 5.5 地理位置功能不應列為 M1/M2 關鍵路徑

這個需求本身技術上可做，但我不建議早期納入核心範圍。

理由：

- 桌面端定位權限與可用性比 timezone 複雜很多
- Tauri geolocation 官方文件雖列出 desktop platform，但安裝與 permission 範例明顯偏 mobile-first；不適合在沒有原型驗證前當作桌面基礎依賴
- location 是高敏感資料，即使本地保存，也應慎選預設精度

### 建議

- M1/M2 只記錄 timezone
- geolocation 延後到 M4 或實驗 feature
- 若未來啟用，預設使用 coarse precision，而不是 full precision


---

## 6. 外部依賴驗證

| 依賴 / 機制 | 判斷 | CTO 結論 |
|------------|------|----------|
| Tauri 2 | 適合 | 與 current repo 完全對齊，適合做本地優先桌面殼層 |
| `@tauri-apps/plugin-autostart` | 適合但用途有限 | 官方定位是 app login/startup autostart，不可取代備份 scheduler；vision 將兩者分離是正確的 |
| Tauri geolocation plugin | 不建議作為 desktop 核心依賴 | 官方文檔 setup/configuration 明顯偏 mobile；desktop 行為要先原型驗證 |
| SQLite | 非常適合 | 穩定文件格式、單檔本地存儲、可長期保存 |
| SQLite FTS5 | 適合 | V1 Recall 的正確方案，不需要外置搜尋引擎 |
| SQLCipher | 適合 | 與加密 / rekey / export 需求匹配 |
| macOS launchd / LaunchAgent | 適合 | 完全能支撐 scheduler vision |
| Windows Task Scheduler | 適合 | `StartWhenAvailable` 能支撐 missed-run 補跑語義 |
| Linux systemd user timer | 適合但要改設計 | 用 `OnCalendar + Persistent`，不能依賴現在的 `OnUnitActiveSec` |
| Ollama | 適合 | 官方 OpenAI compatibility 足夠支撐 provider abstraction |
| LM Studio | 適合 | 官方提供 OpenAI-compatible endpoints 與 headless deployment 路徑 |
| MCP | 適合做本地搜尋接口 | local stdio / localhost 模式可行，但 V1 不應綁定未來 async MCP 特性 |
| S3-compatible object storage | 適合 | remote backup 成立，但建議長期改成內建 client 而非外部 curl |

### 特別說明

#### SQLite / SQLCipher

我認為這個專案最好的決策之一就是堅持 SQLite / SQLCipher，而不是過早引入：

- Postgres
- 外部向量資料庫
- 外部搜尋服務

這些東西只會損害 local-first、可攜性與 20 年可讀性。

#### AI provider abstraction

現在的 provider 抽象方向是好的：

- OpenAI-compatible
- Anthropic
- Google
- Ollama / LM Studio 作為本地推理入口

但要注意一點：**provider abstraction 不是 job orchestration**。前者已經有了，後者現在還沒有。

---

## 7. 建議目標架構

## 7.1 五層架構

### 第一層：Capture Plane

負責：

- profile discovery
- source DB staging copy
- quick_check / source fingerprint
- raw row capture
- browser-specific normalizer

這層的目標是「盡可能抓全，盡可能不猜」。

### 第二層：Archive Plane

負責：

- canonical facts tables
- run ledger
- tombstone / rollback state
- schema migrations
- archive snapshots

這層是產品的核心資產，優先級高於所有 AI 功能。

### 第三層：Recall Plane

負責：

- FTS5 index
- timeline aggregates
- filters
- detail projections
- export projections

這層不應直接查 raw tables，而應查 canonical views / indexed projections。

### 第四層：Intelligence Plane

負責：

- embeddings
- session / thread / topic derivation
- enrichment outputs
- insights outputs
- LLM-generated summaries

這層所有結果都應該是**可重建的 derived state**。

### 第五層：Control Plane

負責：

- settings
- scheduler
- audit artifacts
- key management
- job queue
- status / health / doctor

## 7.2 建議資料分區

我建議把資料概念分成四類：

1. **Raw immutable capture**
2. **Canonical normalized facts**
3. **Derived analytics**
4. **Operational metadata**

對應上：

- `raw_row_versions`：保留
- `visit_events` / `url_versions` / `download_versions`：升級為 canonical facts
- `ai_embeddings` / insights tables / enrichments：視為 derived
- `backup_runs` / `import_batches` / `jobs` / `schema_migrations` / `run_context`：operational metadata

---

## 8. 建議的 schema 與 operation model

以下不是最終 DDL，而是我建議的資料語義。

### 8.1 必增表

- `schema_migrations`
- `run_context`
- `write_operations`
- `archive_snapshots`
- `jobs`
- `job_attempts`
- `enrichment_plugins`
- `fts_documents`

### 8.2 需要補欄位的核心表

`backup_runs`

- `run_kind`：manual_backup / scheduled_backup / takeout_import / revert / restore / migration
- `tool_version`
- `app_commit`
- `timezone_id`
- `location_payload_json`
- `preview_payload_json`

`visit_events`

- `ingested_by_run_id`
- `import_batch_id`
- `reverted_by_run_id`
- `reverted_at`
- `visibility_state`
- `visited_at_ms`
- `visited_at_iso`

`raw_row_versions`

- `import_batch_id`
- `source_time_payload`
- `source_version_payload`

### 8.3 視圖策略

對 UI / Recall / export 暴露的，不應該是裸表，而應該是：

- `live_visit_events`
- `live_documents`
- `live_urls_latest`

規則統一放在視圖或 query layer 裡，避免 rollback / dedupe / visibility 邏輯散落。

---

## 9. Job Queue 設計建議

這是 Intelligence 能不能健康落地的分水嶺。

### 9.1 任務類型

- `embedding`
- `refetch`
- `insight`
- `llm_summary`
- `fts_rebuild`
- `snapshot`

### 9.2 任務欄位

- `job_type`
- `state`：queued / running / succeeded / failed / paused / canceled
- `priority`
- `scope_json`
- `provider_id`
- `attempt_count`
- `max_attempts`
- `next_attempt_at`
- `lease_owner`
- `lease_expires_at`
- `error_json`

### 9.3 執行原則

- 備份完成後只 enqueue，不同步做 AI
- 每種 job 有併發上限
- provider-aware rate limiting
- crash-safe lease recovery
- manual pause / resume
- 可重算 derived state

如果不先做這個層，後面的 insight / enrichment / semantic search 都會變成一堆 UI 按鈕去觸發長任務，最終不可維護。

---

## 10. 風險清單

| 風險 | 等級 | 說明 | 緩解建議 |
|------|------|------|----------|
| rollback 模型錯誤 | 高 | 一旦繼續用 delete，後面所有 trust 敘事都站不住 | 先重做 operation model |
| schema migration 無版本治理 | 高 | 長期資料庫會累積不可預測分支 | 立刻引入 numbered migrations |
| Linux scheduler 行為與 vision 不一致 | 中高 | 用戶會以為有 missed-run catch-up，實際沒有 | 改 timer 語義並加實機驗證 |
| AI 計算阻塞主流程 | 中高 | 大資料量時體驗會迅速惡化 | 引入 job queue |
| 過早承諾高階 insights | 中高 | 演算法和產品價值尚需大量真實資料驗證 | 先做低風險 insights |
| geolocation 隱私與平台複雜度 | 中 | 功能可做，但收益不一定高於風險 | 延後、預設關閉 |
| 過度依賴外部 `curl` | 中 | remote backup 的可攜性與錯誤控制有限 | 中期改 Rust native client |
| FTS / canonical schema 重整成本 | 中 | 需要一次性 migration 成本 | 在 M1 結束前完成，避免後面再搬 |

---

## 11. 建議里程碑重排

我建議保留原本 M1-M4 的精神，但調整工程順序。

## M1：Archive Foundation

目標：把「長期保存與可恢復」做對。

- 正式 migration 系統
- canonical time model
- run / import / rollback operation model
- snapshot safety net
- manifest / audit 強化
- Chromium 主流程穩定化

## M2：Recall Foundation

目標：讓 archive 真正可找回。

- FTS5
- timeline 聚合
- filters / detail projections
- export 強化
- Firefox 補齊
- Takeout import 完整可回滾

## M3：Platform & Trust Completion

目標：把平台與操作透明性做完整。

- Preview / Manual / Execute 全面落地
- Windows / Linux scheduler 正式支持
- Doctor / integrity report
- i18n
- 安全與 rekey UX 完整化

## M4：Intelligence V1

目標：只上成熟且可證明價值的智能功能。

- job queue
- semantic search
- Ask My History
- On This Day
- Site Analytics
- 基礎 summary

## M5：Advanced Intelligence / Ecosystem

- thread / open loops / contrastive summary
- enrichment plugins
- MCP 深化
- geolocation experiments
- remote backup polish

換句話說，我會把原 vision 中的一部分 M3 / M4 內容延後，先把 Archive/Recall 的資料地基做穩。

---

## 12. 最終 Go / No-Go 建議

### Go

這個 vision 值得做，而且以目前代碼庫狀態來看，**不是概念驗證，而是可以進入正式產品重構階段**。

### 但有三個前提

1. **先接受這不是小型重構，而是資料架構升級。**
2. **先修正時間格式、rollback、Linux scheduler 這三個關鍵設計點。**
3. **把 Intelligence 明確放在 Archive/Recall 之後，而不是並行把所有野心一起拉滿。**

### 如果這三個前提成立

我對這個專案的 CTO 級判斷是：

- **技術上可行**
- **架構方向正確**
- **現有代碼有足夠可延續性**
- **但需要一次有紀律的 schema / operation model 重整，否則後期成本會指數上升**

---

## 13. 官方參考資料

### 平台與桌面機制

- Tauri Autostart: [https://v2.tauri.app/plugin/autostart/](https://v2.tauri.app/plugin/autostart/)
- Tauri Geolocation: [https://v2.tauri.app/plugin/geolocation/](https://v2.tauri.app/plugin/geolocation/)
- Apple launchd / LaunchAgents: [https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html)
- Windows Task Scheduler `StartWhenAvailable`: [https://learn.microsoft.com/en-us/windows/win32/taskschd/taskschedulerschema-startwhenavailable-settingstype-element](https://learn.microsoft.com/en-us/windows/win32/taskschd/taskschedulerschema-startwhenavailable-settingstype-element)
- systemd timer: [https://www.freedesktop.org/software/systemd/man/devel/systemd.timer.html](https://www.freedesktop.org/software/systemd/man/devel/systemd.timer.html)

### 儲存、搜尋、加密

- SQLite file format: [https://sqlite.org/fileformat2.html](https://sqlite.org/fileformat2.html)
- SQLite FTS5: [https://sqlite.org/fts5.html](https://sqlite.org/fts5.html)
- SQLite Backup API: [https://sqlite.org/backup.html](https://sqlite.org/backup.html)
- SQLite date/time functions: [https://www.sqlite.org/lang_datefunc.html](https://www.sqlite.org/lang_datefunc.html)
- SQLCipher API: [https://www.zetetic.net/sqlcipher/sqlcipher-api/](https://www.zetetic.net/sqlcipher/sqlcipher-api/)

### AI / Provider / Protocol

- LM Studio developer docs: [https://lmstudio.ai/docs/developer](https://lmstudio.ai/docs/developer)
- Ollama OpenAI compatibility: [https://docs.ollama.com/api/openai-compatibility](https://docs.ollama.com/api/openai-compatibility)
- MCP transports: [https://modelcontextprotocol.io/specification/2024-11-05/basic/transports](https://modelcontextprotocol.io/specification/2024-11-05/basic/transports)

### Google / Browser retention

- Google Search / Web & App Activity auto-delete options: [https://support.google.com/websearch/answer/6096136](https://support.google.com/websearch/answer/6096136)
- Google activity deletion help: [https://support.google.com/accounts/answer/465](https://support.google.com/accounts/answer/465)
- Chromium history expiration source reference: [https://chromium.googlesource.com/chromium/src/+/4153a6a25785476fe84837c1b56c27c42d38f479/components/history/core/browser/expire_history_backend.h](https://chromium.googlesource.com/chromium/src/+/4153a6a25785476fe84837c1b56c27c42d38f479/components/history/core/browser/expire_history_backend.h)

### Remote backup

- Amazon S3 `PutObject`: [https://docs.aws.amazon.com/AmazonS3/latest/API/API_PutObject.html](https://docs.aws.amazon.com/AmazonS3/latest/API/API_PutObject.html)
- Amazon S3 SigV4: [https://docs.aws.amazon.com/AmazonS3/latest/API/sig-v4-authenticating-requests.html](https://docs.aws.amazon.com/AmazonS3/latest/API/sig-v4-authenticating-requests.html)
