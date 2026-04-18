# 數據模型與長期設計

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出，涵蓋時間格式、schema 演化和長期容量的關鍵設計決策。

---

## 1. 統一時間格式

不同瀏覽器使用完全不同的時間格式來儲存歷史紀錄：

| 瀏覽器            | 原生時間格式                                                      |
| ----------------- | ----------------------------------------------------------------- |
| Chrome / Chromium | WebKit epoch — 自 1601-01-01 00:00:00 UTC 起的微秒數              |
| Firefox           | Unix epoch 毫秒數                                                 |
| Safari            | Mac absolute time — 自 2001-01-01 00:00:00 UTC 起的秒數（浮點數） |
| Google Takeout    | ISO 8601 字串（如 `2024-03-15T10:30:00.000Z`）                    |

**我們的 archive 必須統一所有時間為單一格式。**

### 設計決策

<!-- 設計決策理由：
  INTEGER 在 B-tree 上的排序、範圍查詢、bucket 聚合都是原生操作。
  20 年、幾千萬行的資料，每次時間軸拖動都要 range scan，INTEGER vs TEXT 的差異會累積。
  Session 切分（相鄰 visit 時間差）、Burst detection、窗口統計等計算都直接對整數做算術。
  ISO 8601 保留作為人類可讀輔助，方便 debug、匯出、手動 SQL 查詢。
-->

- Archive 的**主要時間欄位**使用 **Unix epoch 毫秒數**（`INTEGER NOT NULL`）。
  - 整數在 B-tree 上的排序、範圍查詢、bucket 聚合（時間軸、趨勢分析）都是原生操作，幾千萬行資料下差異顯著。
  - Session 切分（相鄰 visit 時間差）、Burst detection、窗口統計等計算都直接對整數做算術，不需要先 parse 字串。
- 同時保留一份 **ISO 8601 UTC 字串**（如 `2024-03-15T10:30:00.000Z`）作為人類可讀的輔助欄位。
  - 方便 debug、匯出、手動 SQL 查詢時閱讀。
  - 時區一律存 UTC，前端根據用戶 locale 做顯示轉換。
- Raw capture 層保留原始瀏覽器的時間值不做轉換，並以 checkpoint-first snapshot / manifest trace 保留來源證據，而不是把每筆來源 row 的完整 JSON payload 長期熱存在 canonical archive。
- Schema 中需要明確註釋時間格式的選擇理由和轉換規則。

### 時區處理

知道用戶在當地時間的幾點瀏覽網頁是有價值的（例如分析工作時段、夜間瀏覽模式）。我們不需要去猜測時區 — 因為我們是桌面應用，可以直接讀取系統時區。

- **每次備份 run 時，記錄當時的系統時區**（如 `America/Los_Angeles`），存在 run metadata 中。
- 該 run 中所有紀錄的「當地時間」就是 UTC 時間 + run 時的時區。這對最近的數據來說足夠準確（你不太可能 3 天內換時區）。
- 對歷史數據（Takeout 導入等），無法得知當時的時區，**預設使用用戶當前系統時區**。
- 用戶可以在設定中配置一個「回退時區」（fallback timezone），用於沒有時區信息的舊數據。
- 前端顯示時，統一用用戶當前的系統時區做轉換。用戶不需要手動管理時區 — 正常使用就夠了。
- 如果用戶經常旅行或換時區，這個近似會有一些誤差，但對趨勢分析和時段統計來說已經足夠。

### 地理位置（可選）

既然每次 run 已經在記錄時區，順便記錄地理位置也很便宜，未來可以做地理相關的洞察（例如「你在東京出差那週主要在研究什麼」）。

- **完全可選，預設關閉**。需要用戶在設定中明確開啟。
- 使用 OS 的定位 API（macOS Core Location, Windows Location API, Linux GeoClue）獲取位置。
- **預設記錄完整精度的座標**。所有數據都只存在用戶本地，只有用戶自己能看到，所以完整精度沒有隱私風險。
- 用戶如果有顧慮，可以在設定中選擇降低精度（例如只保留城市級別）。
- 存在 run metadata 中，和時區一起。
- 如果獲取失敗（用戶未授權、無定位服務），靜默跳過，不影響備份。
- 未來可以用這些數據做「旅行時的瀏覽模式」、「不同城市的研究主題」等地理洞察。
- **排在 M4 或之後**。桌面端定位權限的可用性和 permission 模型比想象中複雜（尤其 Linux），需要先做原型驗證。

---

## 2. Schema 演化與容錯

**作為**開發者 / 長期用戶，**我想要** archive 能夠適應瀏覽器未來的格式變化，**以便** 10 年後依然能正常備份新版瀏覽器的紀錄，且舊數據不受影響。

### PathKeep 2026-04-13 storage reset 方向

- PathKeep 已接受並落地 **storage-plane reset**：canonical archive、search projection、intelligence runtime、semantic / blob sidecars 已經是 repo 目前的 source-of-truth 邊界。
- hard reset 之後，source docs 不再為 legacy archive 升級、runtime compatibility patching 或 hot-path raw row storage 保留敘事空間。

### 核心策略：追蹤最新，兼容歷史

我們的策略是**永遠追蹤最新版本的瀏覽器數據格式，同時向後兼容所有舊版格式**。

- **前向追蹤**：每當瀏覽器發布新版本，主動研究其 History / Favicons DB 的 schema 變化。新增的表、欄位、關聯關係都應該被評估和捕獲。
- **向後兼容**：parser 必須能處理舊版瀏覽器的數據庫。新功能的欄位不存在時 gracefully degrade，不影響備份。
- **深度研究**：在實現或更新各瀏覽器的 parser 時，必須**深度研究該瀏覽器當前版本能提供的所有數據**，不要只提取最基本的 visits + urls。瀏覽器不斷在歷史紀錄中加入更多有價值的信息，我們應該盡可能捕獲。

### 設計要點

- 兩層處理：
- **來源 snapshot / raw capture 層**：對來源 DB 先做一致性 snapshot / checkpoint，保存 source DB snapshot、schema fingerprint、browser version、profile metadata 與 manifest trace。
- **Derived normalizer 層**：把 snapshot 中的來源資料映射到統一 schema。新增未知欄位時只降級該欄位，不丟全部數據。
- canonical archive 與 intelligence plane 都已拔掉 `profiles` / `visit_events` compatibility bridge，也不再用 archive / intelligence bootstrap 替舊 schema 補欄位。
- `profile_watermarks` 是 canonical backup pipeline 的正式一部分：每個 profile 分別記錄 visit / URL metadata / download / favicon 的增量 cursor，只在成功 ingest 後前推，並且要能回鏈最後成功的 `source_batch`。
- 真正的 canonical 寫入面是 `source_profiles`、`urls`、`visits`、`downloads`、`search_terms`、`favicons`；archive 內的舊 `profiles` / `visit_events` bridge 已移除。
- `source_profiles` 只保存穩定身份：`browser_family`、`browser_product`、`profile_key`、`profile_name`、`profile_path` 與少量 account/user hint。每次 backup/import 的來源觀測、browser version、schema version / fingerprint、parser version、capability snapshot 與 coverage stats 要落在 `archive/source-evidence.sqlite` 的 `source_batches`。
- PathKeep 的 ingest / intelligence contract 以 **capability snapshot** 為主，不以 browser/version 為主：版本資訊要保留，但主要用於 provenance、debug 與 extractor heuristics。
- 關鍵時刻保存完整原生快照（壓縮保存 History/Favicons DB 原檔）：
  - 首次備份
  - 來源 schema 變更時
  - 每季 checkpoint
- 記錄每次備份的瀏覽器版本、schema 指紋、profile metadata。

### Storage planes

- `archive/history-vault.sqlite`
  - 只保存 canonical facts 與 immutable audit facts
  - 包含 `runs`、`source_profiles`、`urls`、`visits`、`downloads`、`search_terms`、`favicons`、`profile_watermarks`、checkpoint / import / manifest / snapshot trace
- `archive/source-evidence.sqlite`
  - 保存 cold archived source-native evidence
  - 包含 `source_batches`、schema / capability observation、typed evidence tables 與 `native_entities`
  - 這是 archive contract，不是 derived-state；remote/local bundle 必須和 canonical archive 一起打包
- `derived/history-search.sqlite`
  - 只保存 lexical recall projection 與 bounded rollups
  - 包含 FTS5、Explorer keyword recall projection、Dashboard / deterministic baseline 的統計 projection
- `derived/history-intelligence.sqlite`
  - 只保存 queue、assistant trace、enrichment metadata、deterministic read model 與 runtime state
- `sidecars/semantic-index/`
  - LanceDB vectors 與 ANN index
- `sidecars/intelligence-blobs/`
  - content-addressed、可清除、可重建的正文 blob

### Run ledger 與 rollback visibility

- canonical schema 採 **unified run ledger**：`backup`、`import`、`revert`、`doctor`、`snapshot_restore` 共用 `runs` 表，以 `run_type` 和 `trigger` 區分語義。
- `manifests`、`snapshots`、checkpoint trace 視為 immutable audit facts，不做 soft-delete。
- `visits`、`downloads`、`search_terms` 等 user-visible facts 用 `reverted_at` / `reverted_by_run_id` 表示 rollback visibility。
- `urls` 和 `source_profiles` 作為 canonical anchors，不直接以 rollback 做 destructive delete；read model 以關聯 facts 的可見性來判斷是否顯示。

### FTS projection 範圍

- lexical recall 不再與 canonical archive 共住同一個 SQLite。FTS 只存在 `derived/history-search.sqlite`，作為 canonical URL-document 的 projection。
- FTS 只索引 **URL、title、search term**，以及後續明確挑選的 bounded enrichment projection 欄位。
- 一筆 canonical `urls.id` 對應一份 URL-document search projection，Explorer keyword recall 透過 `MATCH` 命中 projection 後再 join 回可見 `visits`，避免 rollback hidden rows 洩漏回主查詢。
- 完整 refetch 文本、readable content、AI 生成摘要不直接塞進 FTS 主索引。
- 若某個 enrichment 需要被全文搜尋，先經 projection / truncation / field whitelist 進入獨立 projection table，再由 FTS 索引。

### Aggregation strategy

- canonical v1 不把 `daily_visit_counts`、`domain_daily_counts`、heatmap density、timeline buckets 當作 source of truth。
- M1 先以 on-demand query + bounded bucket strategy 支撐 UI；如果之後需要 materialized tables，它們也只能是 **可重建的 derived state**。
- 任何 materialized aggregation table 都必須能從 canonical facts 重新產生，rollback 後靠 rebuild / invalidate 修正，不在 aggregation table 內複製 rollback metadata。

### AI derived-state ledger / queue

- AI queue、assistant trace、semantic metadata、deterministic runtime state 都不再由 canonical archive bootstrap 建表；它們透過 `derived/history-intelligence.sqlite` 持久化。
- `ai_index_ledger` 以 `(provider_id, model)` 為 key，記錄 `sidecar_table`、`index_version`、`state`、`source_watermark`、`last_run_id`、build started / finished、clear time 與 failure reason。
- semantic metadata、assistant trace、AI queue 與 deterministic read model 都落在 `derived/history-intelligence.sqlite`。SQLite 僅保留 compact metadata / runtime accounting；向量 payload 不再進 SQLite。
- `ai_assistant_runs` 保存 run-linked assistant trace：`run_id`、question / answer、LLM provider、retrieval provider、citations JSON 與 notes JSON。queued assistant job 完成後要能回到同一筆 trace，而不是只剩暫時性的 UI state。
- sidecar 可以整個刪除後再依 `ai_index_ledger` / canonical archive facts 重建；刪 sidecar 不應修改任何 canonical facts。

### Source-native evidence boundary

- `archive/source-evidence.sqlite` 的 typed evidence tables 目前至少預留：
  - `visit_search_evidence`
  - `visit_navigation_evidence`
  - `visit_engagement_evidence`
  - `visit_context_evidence`
- `native_entities` 用來保存非 visit 粒度、尚未 promotion、或 browser-family-specific 的來源資料，例如 Firefox `moz_inputhistory` / metadata、Chromium clusters / annotations / task graph、Safari tombstones / tags、Takeout Session / tab navigation。
- extractor 遇到新欄位時，預設應保留到 typed/native evidence；只有明確證明低價值且能從 retained raw artifact 無損重建的資料，才可不進 source-evidence archive。
- source-evidence 讀路徑預設是 cold path：rebuild、explainability、debug attach、field promotion / re-extract 可以讀；Explorer / Dashboard / default shell query path 不得直接退化成掃 native payload。

### Enrichment / insight derived-state boundary

- `AppConfig.enrichment.plugins[*]` 是 enrichment plugin 的設定 surface，至少保存 `id`、`enabled`、`version`。缺漏設定必須能從 built-in defaults 回補，避免舊 config 因為新增 plugin 而失真。
- `AppConfig.deterministic.modules[*]` 是 Core Intelligence module 的設定 surface，至少保存 `id`、`enabled`、`version`。2026-04-15 hard reset 後，正式 shipping 的 built-ins 改為 `visit-derived-facts`、`daily-rollups`、`sessions`、`search-trails`、`refind-pages`、`activity-mix`、`search-effectiveness`、`domain-deep-dive`；缺漏設定同樣必須能從 built-in defaults 回補。
- M5-A 起內建 enrichment plugin 固定有兩個：`title-normalization`（`m5-v1`、local-only）與 `readable-content-refetch`（`m4-v1`、network-backed）。它們都預設啟用，且都必須能從 built-in defaults 自動回補到 config。
- enrichment queue contract 掛在 `derived/history-intelligence.sqlite` 的 `intelligence_jobs`，並以 `job_type = 'enrichment-plugin'`、`plugin_id`、`run_id`、payload / artifact trace、`lease_owner`、`lease_expires_at`、`heartbeat_at`、`stop_requested` 區分；額外 trigger 關聯則落在 `intelligence_job_triggers`。這是 derived-state runtime policy，不是 canonical ingest schema 的一部分。
- built-in enrichment runtime 目前是 first-party only；Settings / Insights 可以 review、retry、cancel 內建 job，但 third-party plugin execution 仍保持 deferred，不進 shipping runtime。
- `visit_content_enrichments` 仍保留在 `derived/history-intelligence.sqlite`，因為 readable-text evidence 仍服務 optional AI / assistant / enrichment flows；但它不再被視為 legacy insights snapshot 的一部分。
- 2026-04-15 hard reset 後，deterministic/Core Intelligence 的可重建 derived tables 改為：`visit_derived_facts`、`domain_daily_rollups`、`category_daily_rollups`、`engine_daily_rollups`、`daily_summary_rollups`、`sessions`、`search_trails`、`search_trail_members`、`search_events`、`search_event_terms`、`query_families`、`refind_pages`、`source_effectiveness`、`habit_patterns`、`reopened_investigations`、`path_flows`。`clear_derived_intelligence_state` 只會清掉這些 Core Intelligence rows 與對應 runtime trace，不會動 canonical archive facts。
- `visit_derived_facts` 是新的 visit-level explanation state，持久化 `session_id`、`trail_id`、`registrable_domain`、`canonical_url`、`domain_category`、`page_category`、`search_engine`、`search_query`、`is_new_domain`、`is_search_event`、`evidence_tier`、`taxonomy_source`、`taxonomy_pack`、`taxonomy_version`。這些都屬 derived explanation state，而不是 canonical archive fact。
- desktop / worker app snapshot 的 canonical runtime readiness 欄位現在是 `intelligenceStatus`，對應型別是 `IntelligenceStatus`；任何殘留的 `insightStatus` / `InsightStatus` 只可視為 legacy alias，而不是新的 accepted contract。
- Core Intelligence read path 不再依賴 `insight_snapshot_payloads` 這種整包 JSON snapshot。前台應以實體 / rollup query（sessions、trails、rollups、refind、domain deep dive、digest）為主路徑；若保留舊 snapshot payload，只能視為 legacy inert data，不再是 accepted contract。
- Core Intelligence 的 persisted read model 以 `profile_id`、時間範圍、和對應實體 id（例如 `session_id`、`trail_id`、`family_id`、`canonical_url`）分區；single-profile / all-profile rebuild 不得共享同一批實體 row 或互相清空。
- `get_intelligence_embed_cards`、`get_intelligence_widget_snapshot`、`get_intelligence_public_snapshot` 現在屬於 read-only backend payload providers。這些是已型別化的 data contract，但不等於 embed/widget/public host integration 已完成。
- `deterministic_module_runtime` 是 module-registry trace table，不是 canonical truth。它只保存 module version、status、dependencies、derived tables、last run / built / invalidated time、stale reason 與 notes，供 Settings / Insights 誠實顯示 rebuild-required state。
- derived clear / rebuild 絕不能修改 canonical `visits`、`downloads`、`search_terms`、`runs`、`manifests` 或 rollback visibility 欄位。任何 derived maintenance 都只能留下 trace，不可改寫 source facts。2026-04-12 起，deterministic rebuild 的 live snapshot 也不得先清空再等待後續 commit；同 scope 的 derived rows、snapshot payload 與 module runtime 必須在同一個 intelligence transaction 內替換完成，避免留下半清空狀態。
- refetch freshness / fetch status / snippet / readable text 都屬 derived evidence，而不是 source of truth。這些資料可因 plugin disable、full rebuild、clear derived state 或 pipeline version 升級而被重新計算或刪除。

### Remote backup bundle contract

- M4-A 的 remote backup artifact 是 `pathkeep.remote-backup.v1` zip bundle，不是直接把 live archive path 指向 object storage。bundle 至少包含 `archive/history-vault.sqlite`、`archive/source-evidence.sqlite`、`config/config.json`、`metadata/bundle-manifest.json`、`metadata/bundle-manifest.sha256`，並在存在時附帶 `audit/manifests/` 與 scheduler artifacts；derived search / intelligence DB 與 sidecars 都屬 rebuildable state，不進 canonical remote bundle。
- `bundle-manifest.json` 是 bundle 內的 restore contract：必須記錄 `bundleVersion`、`appVersion`、`createdAt`、`archiveMode`、`bucket`、`objectKey` 與每個 entry 的 `relativePath` / `sha256` / `sizeBytes`。
- Verify 不只檢查 zip 能不能打開；它還必須驗證 bundle version、required entries、manifest 宣告的 entry set 是否與實際 zip entry set 一致、detached manifest checksum 是否吻合、每個 manifest file 的 checksum / size，並嘗試用本機 restore path 打開打包後的 SQLite archive。encrypted bundle 驗證需要 session key；plaintext bundle 要留下明確 warning。
- `bundle-manifest.sha256` 與 entry-set 檢查在 v1 的定位是 corruption / drift detection，不是 detached signing 或 remote authenticity attestation；PathKeep 目前仍不宣稱 bundle 已具備 cryptographic publisher proof。
- remote object lifecycle 在 v1 仍是 manual-first。PathKeep 可以記錄 `lastUploadedAt`、`lastUploadedObjectKey`、`lastError` 與 verify report，但不在未完成 restore rehearsal 前自動 prune bucket 內容。

---

## 3. 長期容量設計原則

重度瀏覽器使用者（每天 ~2,500 visits）在 20 年間可能累積近 2000 萬筆記錄。核心 archive 本身不會是瓶頸（預估僅 40-80 GB），但 AI 相關資產如果設計不當會急速膨脹。

- **AI 資產（embedding、向量索引、enrichment 文本、insight 衍生表）是可重建的衍生狀態**，不是核心數據。清空重跑不會丟失任何原始歷史紀錄。
- **向量 payload 不進 SQLite**。semantic retrieval 只走 LanceDB sidecar；SQLite 只保留 compact semantic metadata 與 rebuild accounting。
- **語義搜尋不能做全表掃描**。幾千萬行 embedding 全量 cosine 計算不可能保持互動性，需要 ANN（近似最近鄰）索引。LanceDB 提供 disk-based IVF-PQ 索引。
- **AI 資產不能拖慢核心 archive**。FTS、intelligence runtime、embedding / 向量索引、正文 blob 都和 canonical archive 隔離。
- **設定頁面應顯示各類資產的磁碟佔用**：core archive、search projection、intelligence projection、semantic / blob sidecars、快照等，讓用戶清楚知道空間花在哪裡，以及最近的增長趨勢。
- M4-A 的 storage analytics v1 以 `core`、`audit`、`exports`、`rebuildable` 四個 slice 呈現現況，並把 `exports + staging + quarantine` 視為目前可回收空間的近似值；更細的 per-plugin / per-model accounting 可在後續里程碑再補。
- **快照必須有保留上限**。一個 100 GB 的 archive 保留 8 份快照就是 800 GB。預設保留最近 4-8 個 archive 快照，用戶可調。
