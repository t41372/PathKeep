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
- `favicons` 保存來源觀測到的 page-level favicon facts，包含 `page_url`、normalized `page_host`、`page_registrable_domain`、`last_updated_ms` 與去重後的 `image_blob_hash`；實際 icon bytes 由 `favicon_blobs` content-addressed 儲存去重。Explorer 只在 lazy hydration path 讀 favicon：不晚於 visit time 的 exact page icon 優先，其次才用 indexed host / registrable-domain fallback；fallback 必須是 miss-only staged lookup，每級用 indexed `LIMIT 1` 先縮到單一候選後才讀 blob，不得合成會排序大量候選或提前 join blob 的 monolithic SQL。這個 read-time fallback 不改寫 canonical visit 或舊 favicon fact，也不得在 archive open / schema bootstrap 時同步回填舊 favicon metadata。
- `og_images` 保存每張卡片模式預覽圖的 fetch 結果（migration 012），包含 `page_url`、診斷用的 `page_host`、解析到的 `source_og_url`、`fetch_status` / `http_status`、`fetched_at`、`last_shown_at`（LRU 訊號）與 `refetch_after`（負緩存退避）。實際 image bytes 由 `og_image_blobs` 以 `sha256_hex(bytes)` content-addressed 去重儲存；不同 page 共用同一張預覽時只佔一份。**讀路徑明確不做 host fallback** — GitHub 與 Medium 同 host 不同 page 的社交卡截然不同，所以 lookup 一律是 exact-page-url。負緩存（`fetch_status='missing'` / `'http_error'` 等）也是正當 row，避免 retry storm。og:image 快取是 derived，**不進 backup export**，restore 後 lazy 重建；詳見 `docs/features/og-images.md`。

### Storage planes

Turso 的新庫 schema 見 [store-schema.md](store-schema.md)：四個 durable `.db`
各有一份 consolidated v1，scratch 為空；archive 包含跨 profile 的 `pages`、
每頁搜尋文件與 transactional outbox，全文索引由 Tantivy 持有。各表用途、keys、
索引、writer 與刪除清單以該文檔為準。archive 寫入、備份、Takeout / Browser Direct
匯入、批次撤銷與恢復、筆記、標籤、星號和預覽圖已經透過 `Vault` 寫入 Turso。
History 時間軸、日期統計、archive read models、來源統計、URL 詳情、圖片讀取、Doctor 與一般歷史匯出使用同一個 `Vault`。搜尋使用 owner 的 vault-search 索引與 Turso；intelligence、AI queue/indexing、enrichment、Agent 對話/journal 與整庫維護（快照、匯出包、改密碼、加密切換、還原）也使用原生 Store。下列 `.sqlite` 路徑只在轉換舊 archive 時讀取；它們不是
[ADR-012](decisions/012-turso-storage-engine.md) 的目標設計。

### Migration in progress

Agent storage now uses `derived/agent.db`, including native encryption when the
Vault has a key. Public chat and journal operations receive `&Vault`; SQL helpers
receive `&vault_store::Session`. Reads use the Agent read pool and changes use its
interactive writer. The old per-operation agent SQLite opener and migration runner
are removed. Schema changes are in the unshipped consolidated v1.

Conversation lists order by `(updated_at DESC, id DESC)`. A limit is a page size
(default 50, maximum 200), with a continuation cursor. Messages have a unique
`(conversation_id, seq)` and page in sequence order. Each page reconstructs usage
and citations from the latest answering run using bounded bulk reads. Complete
transcript loading retains one consistent read snapshot for existing clients that
save the entire transcript. Such clients must not replace a transcript with only
one message page. Failed saves roll back both the transcript and its summary;
re-saves retain conversation and message creation times.

Agent transport additions (camelCase):

| Command                 | Request                                                                    | Response                                                                    |
| ----------------------- | -------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `list_ai_conversations` | `{ request: { limit?: number, cursor?: string } }`                         | `{ conversations: AgentConversationSummary[], nextCursor: string \| null }` |
| `list_ai_messages`      | `{ request: { conversationId: string, limit?: number, cursor?: number } }` | `{ messages: AgentMessage[], nextCursor: number \| null }`                  |

Pass `nextCursor` unchanged to the same command. `load_ai_conversation` continues
to return the complete transcript; save, rename and delete payloads are unchanged.
All six desktop/worker entry points use the session's Vault and retain the existing
App Lock gate. The agent journal uses that same Vault, including after keychain
unlock without a legacy SQLCipher password. Process-death tests cover unfinished
journals, completed runs, citations and idempotent tool-result replay in both modes.

This branch is not usable as a complete archive application until migration area 6.
Archive writes use Turso `.db` files. Password unlock retains the exact typed archive
password for unported SQLCipher readers, separately from the zeroized data key. Backend
keychain auto-unlock has no typed password. The snapshot still reports `unlocked: true`,
with native archive reads available, including history, run/import lists, totals, export,
search and intelligence. Native intelligence and Agent operations use the Vault data key,
not a retained SQLCipher password. Wipe, security status, schedule status and chat cancellation
do not need that password. Only attempts to open a
SQLCipher connection return `storage-migration-incomplete` until the user supplies the
archive password. A locked encrypted archive returns a successful snapshot with
`encrypted: true, unlocked: false`; status reads never retrieve a keychain secret.
Keyword, regex, grouped, AI and export search read the native archive through the owner search engine. Insights, AI queues and indexing, enrichment, Agent and maintenance use the native Store as well. The old SQLite files are read only by the converter (`vault-convert-sqlcipher`); nothing reads them as a live archive.
The owner IPC prevents another process from opening databases.

Maintenance commands now use the native Vault: password rewrap, encryption-mode
switching, portable export/import, full safety-snapshot restore and encryption
reconciliation no longer return `storage-migration-incomplete`. Rewrap changes only
`archive/keys.envelope`; remembered data keys remain valid. Mode changes copy all
four databases through encrypted ATTACH, compare table counts and typed content
digests, rebuild search under the destination mode, save a verified safety snapshot
in that same mode, then install the verified set. Retention uses the native snapshot
ledger and preserves the last openable safety snapshot.

The worker bundle tests `export_app_data_writes_a_bundle_at_the_requested_target`,
`preview_import_reads_a_bundle_written_by_export_app_data`, and
`apply_import_restores_an_exported_bundle_onto_a_fresh_project_root` exercise native
bundles again. The desktop migration round-trip assertions are restored. Native
maintenance tests cover both modes, committed backup prefixes, password/keychain
continuity, interruption recovery and wipe. Legacy SQLCipher fixtures remain for the converter's tests and until
area 6 removes the old openers.

`vault-convert-sqlcipher` reads retained `.sqlite` files and writes staged native
files through Vault. It recomputes URL, favorite, annotation and enrichment keys,
uses the normal document/outbox writer, and rebuilds search. A durable page cursor
supports retry after cancellation or process exit. Counts and logical content are
verified before installation. The old SQLCipher files are never removed by conversion.
Canonical collisions in enrichment jobs and refind merge deterministically: newest
timestamp content, summed counters, unioned sets and remapped job triggers. Scratch follows the target encryption mode and holds the expected aggregates; a restarted attempt cannot add counters twice.
Startup/session and desktop assessment recover installation journals before deciding
whether retained SQLCipher files still require conversion.

### Turso archive read contract

Public archive readers receive `&Vault`; SQL helpers receive `&vault_store::Session`.
Archive reads use `vault.read(Db::Archive, ...)`, never the writer queue or a per-call
SQLCipher connection. A reader during a multi-batch backup sees its committed prefix,
including `running` runs, while the next transaction is still open. Reads never initialize
schemas or rebuild search projections. The old source-evidence opener is isolated with
unported consumers as `open_legacy_source_evidence_connection`; native access belongs to Store.

Timeline order is `(visit_time_ms, id)`, ascending for oldest and descending for newest.
Every page reads one extra match to determine continuation. Turso 0.8.1 leaves tuple
inequalities as residual filters and can select a weaker duplicate bound, so continuation
uses a seek within the last timestamp followed by a strict remaining time range. Tests
inspect the production query plans and the seek opcode. Date insights accumulate 4,096-row
read pages, retaining counters for distinct URLs/domains/queries rather than all visits.
The top-card counts remain summaries of the complete requested day.

Archive and per-source totals come from `archive_totals`; source counters use `sourceVisits:<profile_id>` keys, updated in the same fact transaction. Exact-page lifetime visit counts come from the page/profile summaries. First/last visits
are index seeks. Date/domain match counts and weekly histograms are indexed range
aggregates, not substitutes for cached archive totals. Unknown URLs and empty sources
remain valid empty read models. A zero `profilesProcessed` is preserved, not replaced
by the number of requested profiles.

Ordinary HTML, Markdown, text and JSONL exports read 1,000-row pages from the encrypted
or plaintext Store, with a captured upper visit ID and a 256 KiB write buffer. The screen's
page/cursor does not truncate an export. Each page gets a committed snapshot; later visit
IDs wait for the next export. User visibility changes can be observed between pages.
Publication, progress and cancellation keep their existing contract. Turso has no public
running-statement interrupt; cancellation is checked between pages, rows and 64 KiB write
chunks. Search exports delegate to the existing search function owned by area 3.

Doctor uses Turso `integrity_check` for the four durable files and Store's schema-version
inspection. Turso 0.8.1 also offers `quick_check`, but has no `foreign_key_check` pragma;
Doctor checks canonical references with explicit indexed parent lookups. Full integrity
checking is not interruptible. The MVCC/PASSIVE live-integrity timeout in the probes does
not justify claiming a generic interrupt or using MVCC; this Store uses single-owner WAL.
Doctor also checks every snapshot/manifest/import artifact, visibility provenance, latest
outbox events, page/profile summaries, import visible counts and archive totals against
facts. Repairs regenerate recoverable metadata and queue current page state without
unhiding or deleting visits. Pending valid outbox entries are not corruption and are not
pruned. Structural damage and unknown schema versions are reported, not fabricated away.
Legacy intelligence checks/repairs delegate to their existing functions; any repair,
including a derived-only repair, retains its native run ledger entry.

Desktop contracts (all payload fields are camelCase):

| Command / response                                                   | Native read contract                                                                                                                                                                        |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `query_history({ query })`                                           | Existing response fields and `time\|id` cursor. Timeline `page > 1` requires a cursor; there is no OFFSET. Page size is 1–1,000 (default 150). Search requests use the owner search engine. |
| `load_recent_runs({ cursor?: number, limit?: number })`              | `{ items: BackupRunOverview[], nextCursor: number \| null }`, run ID descending, default page size 12. The compatibility system run is excluded.                                            |
| `load_import_batches({ cursor?: string, limit?: number })`           | `{ items: ImportBatchOverview[], nextCursor: string \| null }`, unfinished (`reverting`, `restoring`, `interrupted`) first, then ID descending, default page size 16.                       |
| `preview_import_batch({ batchId, cursor?: string, limit?: number })` | Existing detail plus `previewNextCursor: string \| null`. Visible visits sort by time/ID descending. Default page size remains 24.                                                          |
| `AppSnapshot`                                                        | Adds `recentRunsNextCursor` and `recentImportBatchesNextCursor`; arrays remain the first pages. `directories.archiveDatabasePath` agrees with the native archive status path.               |
| `DashboardSnapshot`                                                  | Adds `recentRunsNextCursor`; existing counters and arrays retain their types.                                                                                                               |
| `HealthRepairReport`                                                 | Adds `repairedTotalCounters`, `repairedBatchTotals`, `refreshedSearchPages`, `queuedSearchUpdates` (numbers, default 0 when decoding older payloads). Existing fields remain.               |

Run/import/review limits are page sizes (maximum 1,000), never total-result caps. Continue
with the returned cursor; do not reconstruct it. Core OG prefetch/refetch reads likewise
return `{ items, nextCursor }` and accept an optional cursor; worker fetch budgets still
choose one work page. Scalar latest/earliest/best-image selectors retain `LIMIT 1` because
they select one value, rather than enumerate a truncated record set.

Doctor adds the stable check codes `archive-integrity`, `source-evidence-integrity`,
`intelligence-integrity`, `agent-integrity`, `canonical-references`, `cached-totals`,
`search-outbox`. Existing artifact/visibility/schema codes remain. Frontend integration
and the en/zh-CN/zh-TW labels are documented in the
[read-port validation report](research/2026-10-turso-archive-reads.md).

### Turso write path and ownership

`Vault` opens one shared `Store` with the migration registry. Each durable database
starts with a consolidated v1, including all archive read and search indexes. Writers maintain per-source counters with the visible facts. Archive includes resumable visibility state, the batch
URL index, indexed preview-image eviction, independent source cursors, cached totals,
and canonical URL indexes. No released Turso schema needs a backfill. Fact transactions
maintain the counters incrementally. Public writes receive `&Vault`; transaction helpers
receive `&Session`. The ported paths do not bootstrap schemas or seed search on reads.

Backup and import submit at most 10,000 source rows per `Priority::Batch` transaction.
Each archive batch commits facts, URL bounds, affected search documents, profile
memberships, source visit counters, outbox events and run/import progress together. The producer waits for its
commit, and interactive stars, notes and tags can overtake a queued batch. A second
backup request in the same owner is rejected as `backup-already-running`. A colliding
scheduled request is deferred with the existing `write-lock` reason. Checkpoint replay
shares this guard.

Visit IDs and download IDs advance with their own committed rows. Favicon timestamps
and search terms' URL timestamps use independent inclusive cursors so ties stay reachable.
Each URL batch advances its URL-time watermark and stores a rowid scan position.
Chromium, Firefox and Safari page their URL tables by `id > position ORDER BY id`.
The scan retains its original time filter and visit-ID range so partial commits cannot
change the selection of unread rows. Resume seeks past committed rows. Visits added
since the interrupted scan began can bring in a parent URL behind that position,
regardless of its timestamp; those parent upserts are idempotent. A full first scan
without new source visits does not rescan the visits table. Completing all source phases
clears the scan position. Visit/download streams walk source IDs; search terms and
favicons use one streaming source query rather than re-sorting for each emitted batch.
Browser-direct imports update `last_source_batch_id` without advancing live backup
cursors. Takeout retains a watermark for its separate synthetic profile.

Source evidence is a separate database. Its bounded chunks commit before the archive
watermark references them. A crash can leave unreferenced evidence, but cannot commit a
cursor past evidence that was never saved. Browser files still use rusqlite staging and
parsers. Browser-import schema fingerprints describe schema observations, not whole-file
contents. Takeout file and ZIP readers emit batches before reaching the end of input,
avoiding an allocation proportional to the complete input file; there is no plaintext
JSON evidence spool. The parser continues to accept trailing bytes as it did before this port. Imports
still drain the input with cancellation checks to validate ZIP checksums and late I/O
errors before completion. A late error leaves a failed batch with its committed prefix.

Runs store `running` while active, `success` on completion, `failed` on an ordinary
error, and `interrupted` on cancellation or crash. Errors retain committed counts,
completed profile summaries and collected warnings. The run overview uses the same
status, timestamps and requested profile scope as its row. On startup, leftover active write runs and import batches become
interrupted. A partial import retains its batch ID and exposes `batch.status` in its
list/detail response; it is never reported as fully imported. In-flight previews do not
write final audit artifacts. The run links its `importBatchId` from the initial transaction.

Revert and restore commit 1,000 affected URL IDs at a time. The batch stores `reverting`
or `restoring`, the last committed URL ID, its operation run, and its pre-revert status.
Each chunk changes visits, page documents, visible profile summaries and outbox rows
in one transaction. Cancellation is checked between chunks; invoking the same action
resumes it. A second active producer or the opposite action on an unfinished operation
is refused. Restore returns a partial batch to `interrupted` or `failed` when that was
its original status. These runs use the existing UI trigger `manual`; `run_type` remains
`rollback` or `restore`. Archive and per-batch visible totals change in the same fact
transactions. Run summaries and manifests read these counters and indexed visit bounds;
they do not scan the archive for totals. Successful import and visibility finalization
update the run and batch together. Failed imports retain file reports and notes.
Startup snapshots include unfinished `reverting`/`restoring` batches even when they are
older than the recent-batch list; repeating that action resumes from its committed cursor.

`pages` remain exact-URL anchors shared across profiles. Their indexed `canonical_url`
is computed by the same normalizer used by stars and enrichment. It handles tracking
parameters, default ports, empty paths, IDN hosts and unreserved percent escapes;
reserved path delimiters remain escaped. Variant lookups use indexed equality.
New visits increment profile summaries only for actual inserts. Each affected page's
text and annotations are read once per batch refresh and publish one outbox entry.
Enrichment resolves all matching page IDs on a reader, then publishes their text and
outbox rows in a short transaction. No page match before fetching leaves the job in
`awaiting_archive` with artifact code `archive-page-missing`, without fetching or recording
a success/failure. Candidate generation walks Turso pages by canonical URL without a
candidate-pool ceiling. It reoffers `awaiting_archive` jobs when the canonical page exists,
including pages that do not yet have a visit. `page_content_enrichments` stores one result
per `(canonical_url, content_source)`, so existing and future visits share the cache.
The cache result and a `contentStored` handoff marker commit together in Intelligence;
Archive publication then updates every matching page document and its outbox. Recovery
replays this local handoff without fetching again. A fresh request clears the old handoff,
and a cache-only publication leaves already-matching documents unchanged. An absent search
document produces no outbox event. No write path rebuilds an index; the separate
`vault-search` consumer applies the durable outbox independently of archive writes.

Star lists paginate every sort with a complete tie-breaker: visit count when requested,
star time, entity kind and key. Each item includes its canonical page's newest nonempty
note and combined tags, loaded in bulk in the same read transaction. Domain stars have
empty notes/tags. Page limits do not cap the reachable set. AI tools return a page and
cursor and state when more items exist.

Annotation list/search responses are `{ items, nextCursor }`. Both accept an optional
`cursor`; it carries the last `(updated_at, URL)` pair, so tied timestamps and rows beyond
5,000 remain reachable. The frontend must consume this page contract.

The app reserves ownership at startup. `Vault::open_owned` shares that reservation with
Store; CLI, MCP, scheduled tasks and the sidecar remain owner-IPC clients. Session open
and explicit close run on blocking threads, without holding the session mutex. App Lock
is a UI gate: it retains the Vault, password, data key and running AI chats. Scheduled
backups continue. Concurrent openers and UI reads wait for startup open to finish; the
owner endpoint binds before opening starts. Connected requests wait without a response
timeout; the client's deadline covers connecting only. A typed password submitted during
startup is validated after that attempt, even if startup failed. Panics end the opening
transition and wake waiters. Endpoint bind failures are logged and shown in a native
dialog before exit.
An explicit archive-session clear or wipe closes Store outside the session mutex.
Store close revokes retained handles and finishes admitted requests; the app's shared
ownership reservation survives until the app exits, including across a wipe.

The encrypted archive uses `archive/keys.envelope` from `vault-crypto`. `Vault::open_password`
unwraps an existing envelope. If no Turso database exists, it can create a random data key
wrapped with Argon2id and AES-256-GCM; an existing SQLCipher archive must first accept the
password. Before any database file is created, the envelope is written to
`keys.envelope.pending` and both file and directory are synced. A successful Store open
promotes it atomically to `keys.envelope`. An interrupted first open recovers through
the pending envelope with the same password; a wrong password changes neither file.
Existing Turso files without either envelope return `envelope-missing` and are never
assigned a new key.
The native keychain entry stores the data key as 64 hexadecimal digits, replacing the
password. Old password entries are authenticated through the password path and replaced
in the backend. Only startup and explicit keychain unlock read this secret. Data keys
and caller-owned key buffers are zeroized; no data key is returned
to the WebView or over owner IPC. Each database uses its `KeyPurpose` HKDF sub-key,
including scratch; `TempCryptIo` protects spill files. Plaintext supplies no Store keys
and retains the compatibility password state expected by unported callers. Password changes rewrap the envelope without rewriting databases.

Desktop command contracts (camelCase payloads):

- `set_session_database_key({ databaseKey: string }) -> null` opens with the typed password.
- `keyring_get_database_key({}) -> boolean` reports presence only.
- `unlock_archive_from_keychain({}) -> null` opens in the backend without a password round trip.
- `keyring_store_database_key({ value: string }) -> KeyringStatusReport` treats `value` as
  the archive password, unwraps the existing envelope and stores only its data key.
- `list_stars({ kind: "url" | "domain" | null, sort: "recently_starred" | "most_revisited", cursor: string | null, limit: number | null }) -> StarPage`.

`StarPage` is `{ items: StarListItem[], nextCursor: string | null }`. An item is
`{ entityKind, entityKey, starredAt, domain, title, visitCount: number, notes: string, tags: string[] }`;
all other fields are strings. `kind`, `cursor` and `limit` may also be omitted. Reuse the
cursor with the same kind and sort; invalid cursors/page sizes return `invalid-cursor`
and `invalid-page-size`. The default page size is 500.

- `list_url_annotations({ limit?: number | null, cursor?: string | null }) -> AnnotationPage`.
- `search_url_annotations({ query: string, limit?: number | null, cursor?: string | null }) -> AnnotationPage`.

`AnnotationPage` is `{ items: UrlAnnotation[], nextCursor: string | null }`; `UrlAnnotation`
is `{ url, notes, tags: string[], createdAt, updatedAt, sourceProfile: string | null }`.
All other annotation fields are strings. Pass `nextCursor` unchanged to the same command.
Unlock errors use `CommandError` (`message`, optional `code`, `actionHint`, `retryHint`),
including `archive-password-wrong`, `archive-password-required`, `envelope-missing`,
`keychain-key-missing`, `keychain-unavailable`, `archive-open-panicked`, `archive-busy`
(close only), and `storage-migration-incomplete`.
The frontend adopts these contracts separately. Keychain-only backups and imports succeed but
report a migration warning instead of launching follow-up SQLCipher reads without a password.

Recorded checkpoint replay requires a `snapshots` ledger entry, uses the recorded single-profile scope (and preserves the directory-name fallback for
older or multi-profile snapshots), preserves the original snapshot creation time, and records
source hashes and manifest provenance. A directory supplied without a ledger entry is
not a valid restore source.

The old `ArchiveOpGate` / `ArchiveWriteLock` remain for legacy fixture helpers, retention and wipe. Native whole-file maintenance uses Store admission and the owner lock.
Their native SQLite tests use independent fixtures, never translated copies of Turso
writer results.

- `archive/history-vault.sqlite`
  - 只保存 canonical facts 與 immutable audit facts
  - 包含 `runs`、`source_profiles`、`urls`、`visits`、`downloads`、`search_terms`、`favicons`、`profile_watermarks`、checkpoint / import / manifest / snapshot trace
- `archive/source-evidence.sqlite`
  - 保存 cold archived source-native evidence
  - 包含 `source_batches`、schema / capability observation、typed evidence tables 與 `native_entities`
  - 這是 archive contract，不是 derived-state；remote/local bundle 必須和 canonical archive 一起打包
- `derived/history-search.sqlite`
  - 只保存 lexical recall projection 與 bounded rollups
  - Legacy artifact only; native search does not open this file.
- `derived/search-index/`
  - Versioned, rebuildable Tantivy generations; encrypted when the Vault is encrypted.
- `derived/intelligence.db`
  - Turso 儲存 queue、canonical enrichment metadata、deterministic read model、runtime state 與 compact semantic metadata / rebuild accounting；加密模式使用此 DB 專屬的 Vault sub-key。
- `derived/agent.db`
  - assistant chat 持久化面（W-AI-3/7）：`conversations`、`messages`，以及 `agent_runs`、`agent_steps`、`agent_citations` 的 run/step/citation trace
  - 舊檔保留供轉換；原生 agent 資料庫包含在加密 portable export 中
- `derived/vectors/`
  - 自製 `FlatVectorIndex` 的 on-disk plane：`.pkvec`（raw f32 source）、`.pkmap`（visit→content_key dedup map，W-AI-4c）、`.pkbin`（binary-recall plane）、`.pki8`（int8-rescore plane）
  - 全屬可重建衍生狀態（14.4M 尾端的 f32 約 59 GB），**排除出 export bundle**，在 target 重嵌而非搬運
- `models/`
  - in-app candle embedding 下載的模型權重（SHA-256 pin）；可重新下載，**排除出 export bundle**
- `sidecars/intelligence-blobs/`
  - W-ENRICH content-fetch 抓回的 readable body / caption blob（content-addressed）；可重抓、可能大，**排除出 export bundle**（已投影的 bounded enrichment text 隨 archive 的 `search_documents` 搬移，目的地重建索引後仍能離線搜尋；intelligence DB 本身不進 native bundle）

### Page encryption

Every encrypted Turso database (archive, source evidence, Insights, agent, scratch) encrypts
each page with **AES-256-GCM** under a key derived from the data key; Turso writes a fresh
random nonce and the tag into each page.

Decided 2026-10-07, replacing AEGIS-256. Turso's SDK enables its `pure-rust-crypto` feature,
and with it AEGIS uses hardware AES only when the binary is compiled for it (`target_feature =
"aes"`). That holds on Apple Silicon. On x86_64 every page went through bitsliced software AES:
74% of the busy samples in the 14.4M Insights rebuild profile
(`research/2026-10-turso-area4-vm-benchmark.md`). Compiling for AES-NI would make the app
crash on CPUs without it. AES-256-GCM (RustCrypto) checks for AES-NI and carry-less multiply
at run time, on every CPU. Measured with `vault-store/tests/benchmark.rs`
(`encrypted_page_throughput`, Modal VM, x86_64, release):

| Cipher                       | 4,000 commits of 100 rows | 40 commits of 10,000 rows | Cold scan of 800k rows |
| ---------------------------- | ------------------------: | ------------------------: | ---------------------: |
| AEGIS-256, software AES      |                    7.30 s |                    1.23 s |                 0.53 s |
| AEGIS-256, built with AES-NI |                    6.25 s |                    0.79 s |                 0.12 s |
| AES-256-GCM, run-time detect |                    6.15 s |                    0.80 s |                 0.15 s |

Random 96-bit nonces put a limit on how many pages one key may encrypt: about 2³² keeps
the chance of a repeated nonce below 2⁻³². A full Insights rebuild at 14.4M visits writes on
the order of 10⁷ pages, so the limit is hundreds of full rebuilds under one data key. Changing
the archive password re-wraps the key without changing it. If that limit ever matters, the
remedy is to rotate the data key (a full copy, as an encryption mode switch does), not to
change the cipher.

The encrypted search index uses AES-256-GCM as well (`vault-crypto`).

### Retained SQLCipher files

SQLCipher inputs retained by conversion keep their original password and at-rest
mode. A later native password change rewraps the native envelope only; it does not
modify those retained originals. Their deletion is a separate user decision.
While the converter ships, rusqlite uses `bundled-sqlcipher-vendored-openssl` across
the whole executable; browser SQLite files are read without a key. See ADR-012.

Native snapshots pause admission between committed batches, TRUNCATE every WAL,
then copy the four main files and envelope/config together. Their checksummed
`snapshot.json` identifies the directory as one restore unit. Browser source
checkpoint replay continues to use area 1's recorded-checkpoint path. Old archive-only
SQLCipher safety snapshots are converted before native restoration; they cannot
recover source evidence or agent data that the old snapshot never contained.

- **Legacy artifacts / separate sidecars**：殘留的 `history-search.sqlite`、`history-intelligence.sqlite`、`agent.sqlite` 可能仍含明文；原生搜尋不開啟舊 FTS 檔。原生 `intelligence.db`、`agent.db` 與 search-index generations 隨 Vault 加密；`derived/vectors/` 的 flat planes 仍是獨立 sidecars。舊檔與 sidecar 的整體 at-rest 處理屬後續維護範圍，見 [[WORK-ENCRYPT-AT-REST-ALL backlog]]。

Native mode changes, bundle imports and snapshot restores share the existing
`archive/.pk-import-journal.json` location. The extended journal records each
relative destination, whether an original existed, staging/backstop roots and the
publication decision. Startup rolls back an uncommitted generation or finishes
cleanup after publication. The owner-lock inode is never replaced. Abandoned
`staging/maintenance-*` copies are removed on startup; the separately named
SQLCipher conversion staging tree remains available to resume.

### Run ledger 與 rollback visibility

- canonical schema 採 **unified run ledger**：`backup`、`import`、`revert`、`doctor`、`snapshot_restore` 共用 `runs` 表，以 `run_type` 和 `trigger` 區分語義。
- `manifests`、`snapshots`、checkpoint trace 視為 immutable audit facts，不做 soft-delete。
- `visits`、`downloads`、`search_terms` 等 user-visible facts 用 `reverted_at` / `reverted_by_run_id` 表示 rollback visibility。
- `urls` 和 `source_profiles` 作為 canonical anchors，不直接以 rollback 做 destructive delete；read model 以關聯 facts 的可見性來判斷是否顯示。

### Search documents and index

`search_documents` has one row per canonical page. Its text includes every visible
profile title, visible search terms, notes, tags and the approved enrichment
projection. `search_document_profiles` records visible profile membership;
`search_outbox` is a durable ordered handoff, committed with the facts that changed
search. Revocation refreshes all three in the same archive transaction.

The owner indexer applies complete outbox prefixes to `vault-search` and then
prunes acknowledged rows with a Batch write. The index commit records the highest
sequence. Reopen replays unacknowledged changes. Rebuild streams page IDs and
publishes a generation atomically; initial builds expose partial coverage and
healthy rebuilds keep serving the old generation. Writer defaults are two threads,
64 MiB arena and 128 MiB decrypted cache, matching the engine measurements.

Search reads and hydration use Turso only. Eligibility binds date, profile and
browser to the same visible visit. Ranking and exact counting are separate
resumable walks over all matches. Revocation is checked in canonical storage even
before the index catches up. There are no search windows, candidate limits,
newest-first fallback, match-set cache or preparation phase. Regex scans complete
canonical visit ranges. AI fusion uses complete candidate streams, and every
transport shares session cursors. See [search.md](search.md) for ownership,
recovery, cancellation and exact command/channel shapes.

### Aggregation strategy

- canonical v1 不把 `daily_visit_counts`、`domain_daily_counts`、heatmap density、timeline buckets 當作 source of truth。
- M1 先以 on-demand query + bounded bucket strategy 支撐 UI；如果之後需要 materialized tables，它們也只能是 **可重建的 derived state**。
- 任何 materialized aggregation table 都必須能從 canonical facts 重新產生，rollback 後靠 rebuild / invalidate 修正，不在 aggregation table 內複製 rollback metadata。

### AI derived-state ledger / queue

- AI queue、assistant trace、semantic metadata、deterministic runtime state 都不再由 canonical archive bootstrap 建表；它們透過 `derived/intelligence.db` 持久化。
- `ai_index_ledger` 以 `(provider_id, model)` 為 key，記錄 `sidecar_table`、`index_version`、`state`、`source_watermark`、`last_run_id`、build started / finished、clear time 與 failure reason。
- semantic metadata、AI queue 與 deterministic read model 落在 `derived/intelligence.db`。Turso 僅保留 compact metadata / runtime accounting；**向量 payload 不進 SQLite**，而是寫到 `derived/vectors/` 的 `.pkvec/.pkmap/.pkbin/.pki8` sidecar plane（W-AI-4/5）。
- assistant chat / agent run trace 落在獨立的 `derived/agent.db`（W-AI-3/7）：`conversations` / `messages` 加上 `agent_runs` / `agent_steps`（append-only journal，支援崩潰續跑）/ `agent_citations`（釘住 evidence 的 `history_id` / `canonical_url`）。它和瀏覽歷史一樣是用戶資料、無法重建，所以會隨安全快照備份，也會以匯出密碼加密後放進 portable bundle 搬到新電腦（用戶 2026-10-07 決定：聊天記錄不比瀏覽記錄更私密，要一起備份和遷移；推翻先前「排除出 portable export」的設計）。
- vector sidecar 與 agent.db 都可以整個刪除後再依 `ai_index_ledger` / canonical archive facts 重建（agent chat 刪除即重開新對話）；刪這些衍生面不應修改任何 canonical facts。
- 2026-06 AI redesign closeout：上面 2026-04/05 寫下的「optional AI / vector / MCP 全部 disabled、移到 v0.3」結論已被 AI redesign 2026 program（W-AI-1..9）取代並落地——streaming chat、in-app + external embedding、`FlatVectorIndex` 語義/混合檢索、agent harness、code-mode、MCP 對外面都已交付且 reachable。整個 AI 面 **off by default + consent-gated**（master `ai.enabled` + 各 sub-flag），boundary 與 threat model 見 [ai-security-posture.md](ai-security-posture.md)。

### Source-native evidence boundary

- `archive/source-evidence.sqlite` 的 typed evidence tables 目前至少預留：
  - `visit_search_evidence`
  - `visit_navigation_evidence`
  - `visit_engagement_evidence`
  - `visit_context_evidence`
- `native_entities` 用來保存非 visit 粒度、尚未 promotion、或 browser-family-specific 的來源資料，例如 Firefox `moz_inputhistory` / metadata、Chromium clusters / annotations / task graph、Safari tombstones / tags、Takeout Session / tab navigation。
- 大型 import 的 source-native evidence 必須是 parser batch → bounded spool builder 的流式 contract；parser 可為 read-only `parse_history` API 保留完整 report，但 execute/import path 不得把全量 `TypedEvidenceBatch` / `native_entities` 常駐到 `StreamedHistory`。
- extractor 遇到新欄位時，預設應保留到 typed/native evidence；只有明確證明低價值且能從 retained raw artifact 無損重建的資料，才可不進 source-evidence archive。
- source-evidence 讀路徑預設是 cold path：rebuild、explainability、debug attach、field promotion / re-extract 可以讀；Explorer / Dashboard / default shell query path 不得直接退化成掃 native payload。

### Enrichment / insight derived-state boundary

- `AppConfig.enrichment.plugins[*]` 是 enrichment plugin 的設定 surface，至少保存 `id`、`enabled`、`version`。缺漏設定必須能從 built-in defaults 回補，避免舊 config 因為新增 plugin 而失真。
- `AppConfig.deterministic.modules[*]` 是 Core Intelligence module 的設定 surface，至少保存 `id`、`enabled`、`version`。2026-04-15 hard reset 後，正式 shipping 的 built-ins 改為 `visit-derived-facts`、`daily-rollups`、`sessions`、`search-trails`、`refind-pages`、`activity-mix`、`search-effectiveness`、`domain-deep-dive`；缺漏設定同樣必須能從 built-in defaults 回補。
- M5-A 起內建 enrichment plugin 固定有兩個：`title-normalization`（`m5-v1`、local-only）與 future-facing `readable-content-refetch`（`m4-v1`、network-backed）。v0.2.0 只預設啟用 `title-normalization`；`readable-content-refetch` 保留設定 / runtime row，但預設 disabled，且不得在 v0.2.0 抓取或保存網頁正文。
- `search_engine_rules` 現在是 Settings derived-state surface 擁有的 deterministic config table，持久化在 `derived/intelligence.db`。built-in search-engine rules 由 runtime 合成，只有使用者新增/修改的 custom 規則真正落表；但 visit classification、search-event extraction 與 Settings rule editor 都必須讀到同一份 merged enabled rule set，避免 UI 與 rebuild contract 漂移。
- enrichment queue contract 掛在 `derived/intelligence.db` 的 `intelligence_jobs`，並以 `job_type = 'enrichment-plugin'`、`plugin_id`、`run_id`、payload / artifact trace、`lease_owner`、`lease_expires_at`、`heartbeat_at`、`stop_requested` 區分；額外 trigger 關聯則落在 `intelligence_job_triggers`。這是 derived-state runtime policy，不是 canonical ingest schema 的一部分。
- built-in enrichment runtime 目前是 first-party only；Settings / Insights 可以 review、retry、cancel 內建 job，但 third-party plugin execution 仍保持 deferred，不進 shipping runtime。
- `page_content_enrichments` 保存在 `derived/intelligence.db`，以 `(canonical_url, content_source)` 共用 readable-text evidence；它服務 optional AI / assistant / enrichment flows，不是 legacy insights snapshot。
- 2026-04-15 hard reset 後，deterministic/Core Intelligence 的可重建 derived tables 改為：`visit_derived_facts`、`domain_daily_rollups`、`category_daily_rollups`、`engine_daily_rollups`、`daily_summary_rollups`、`sessions`、`search_trails`、`search_trail_members`、`search_events`、`search_event_terms`、`query_families`、`refind_pages`、`habit_patterns`、`reopened_investigations`、`path_flows`、`compare_sets`（2026-10-08 起在 aggregate 建置時算好，不再每次讀取現算）。`clear_derived_intelligence_state` 只會清掉這些 Core Intelligence rows 與對應 runtime trace，不會動 canonical archive facts；回報欄位也必須按 stage 分組為 `clearedVisitDerivedFactRows`、`clearedDailyRollupRows`、`clearedStructuralRows`、`clearedRuntimeRows`，只有真的清 enrichment 時才額外回 `clearedEnrichmentRows`。
- `visit_derived_facts` 是新的 visit-level explanation state，持久化 `session_id`、`trail_id`、`registrable_domain`、`canonical_url`、`domain_category`、`page_category`、`search_engine`、`search_query`、`is_new_domain`、`is_search_event`、`evidence_tier`、`taxonomy_source`、`taxonomy_pack`、`taxonomy_version`。這些都屬 derived explanation state，而不是 canonical archive fact。
- desktop / worker app snapshot 的 canonical runtime readiness 欄位現在是 `intelligenceStatus`，對應型別是 `IntelligenceStatus`；repo 已不再保留 `insightStatus` / `InsightStatus` alias，新 surface 必須直接使用 canonical naming。
- Core Intelligence read path 不再依賴 `insight_snapshot_payloads` 這種整包 JSON snapshot。前台應以實體 / rollup query（sessions、trails、rollups、refind、domain deep dive、digest）為主路徑；若保留舊 snapshot payload，只能視為 legacy inert data，不再是 accepted contract。
- Core Intelligence 的 persisted read model 以 `profile_id`、時間範圍、和對應實體 id（例如 `session_id`、`trail_id`、`family_id`、`canonical_url`）分區；single-profile / all-profile rebuild 不得共享同一批實體 row 或互相清空。
- `get_search_queries` 現在是從 `search_events`、`query_families` 與 `search_trails` 組出的 additive read model：同一視窗內會先 dedupe 最新一筆 `(search_engine, normalized_query)`，但 row 仍必須保留 reusable `family_id`、`trail_id` 與 `profile_id`，讓 `/intelligence` 的 `Recent Queries` 與後續 external-output payload 可直接跳到 shared insights destination，而不是回退成 consumer-local parsing。
- `compare_set_id` 現在是 compare-page aggregate 的 canonical reusable identity，`get_compare_set_detail` 會從這個 id 還原 compare-set summary、related trail/session context、pages 與 recent days；`path_flows` 則必須帶 stable `flow_id` 與 typed `steps`，前端不得再從 human-readable pattern label 逆推 route target。
- `get_intelligence_embed_cards`、`get_intelligence_widget_snapshot`、`get_intelligence_public_snapshot` 現在屬於 read-only backend payload providers。它們已經接到 Settings 的 manual review / copy-export surface，並作為 `preview_intelligence_local_host` / `build_intelligence_local_host` 的唯一資料來源；這仍不等於 OS widget、localhost API、或其他完整 embed/widget/public host integration 已完成。
- trusted external-output payload 現在可帶 structured `primaryTarget` / `secondaryTargets` 供 Settings 與 trusted local host 復用 app-link；`public snapshot` 則必須維持 redacted，不得下放 `family_id`、`trail_id`、`compare_set_id`、`canonical_url` 等 internal reusable IDs。
- `preview_intelligence_local_host` 只在記憶體中 materialize 可 review 的 artifact；`build_intelligence_local_host` 則固定把 `browser-snippet-v1` 寫到 `app_root/integrations/core-intelligence/browser-snippet-v1/{index.html,bundle.json}`。這兩個檔案都是 rebuildable local artifact，不屬於 canonical archive 或 source-evidence archive 的 contract。
- `bundle.json` 是 trusted local host 的 typed machine contract：至少包含 `bundleVersion`、`hostId`、`generatedAt`、`locale`、`dateRange`、`profileId`、`embedCards`、`widgetSnapshot`、`publicSnapshot`、`trustedOnlyCardIds`、`trustedOnlyCardCount` 與 `boundaryNotes`；`index.html` 直接內嵌同一份 bundle data 靜態渲染，不再從 `file://` 另行 fetch JSON。
- `deterministic_module_runtime` 是 module-registry trace table，不是 canonical truth。它只保存 module version、status、dependencies、derived tables、last run / built / invalidated time、stale reason 與 notes，供 Settings / Insights 誠實顯示 rebuild-required state。
- derived clear / rebuild 絕不能修改 canonical `visits`、`downloads`、`search_terms`、`runs`、`manifests` 或 rollback visibility 欄位。任何 derived maintenance 都只能留下 trace，不可改寫 source facts。2026-04-12 起，deterministic rebuild 的 live snapshot 也不得先清空再等待後續 commit；同 scope 的 derived rows、snapshot payload 與 module runtime 必須在同一個 intelligence transaction 內替換完成，避免留下半清空狀態。
- refetch freshness / fetch status / snippet / readable text 都屬 derived evidence，而不是 source of truth。W-ENRICH content-fetch 已交付但 **egress hard-default-OFF**（`content_fetch_enabled`，預設關），且 SSRF-guarded（見 [ai-security-posture.md](ai-security-posture.md) §4）；這些資料可因 plugin disable、full rebuild、clear derived state 或 pipeline version 升級而被重新計算或刪除。

### Data migration bundle contract (`.pathkeep-bundle`)

Native bundles use format 2. Every export has a fresh random data key wrapped by
an explicit export password. The ZIP contains `archive/history-vault.db`,
`archive/source-evidence.db`, `archive/agent.db`, `archive/keys.envelope`,
`config/config.json`, `archive/portable-artifacts.pkf`, the manifest and its
SHA-256 sidecar. The config and streaming TAR of audit, raw snapshots and retained
sidecars use the authenticated bundle-artifacts codec; the six-entry inventory
stays bounded as the provenance history grows. Source-machine artifact paths in
database rows are relocated after verification; immutable audit JSON keeps its
original hash and resolves through imported root aliases. Agent conversations
are included. The `derived/` tree is excluded; search is rebuilt on import and
intelligence remains rebuildable. This replaces the old SQLCipher bundle layout.

Export copies and verifies database rows before writing the ZIP, and atomically
publishes the completed bundle at the requested path. Import validates manifest
and entry digests in staging, authenticates the export password, checks database
integrity, builds search, then closes the old owner and performs the journaled
installation. Existing native files are retained in `maintenance.bak-*`. Import
works with an empty project root and requires `confirmOverwrite` when replacing
an existing native archive. App Lock policy stays with the destination's local
secret; an empty destination starts without App Lock rather than asking for an
excluded source-machine passcode.

The command contracts and en/zh-CN/zh-TW phase labels are in
[the maintenance contract](research/2026-10-turso-maintenance.md).

---

## 3. 長期容量設計原則

重度瀏覽器使用者（每天 ~2,500 visits）在 20 年間可能累積近 2000 萬筆記錄。核心 archive 本身不會是瓶頸（預估僅 40-80 GB），但 AI 相關資產如果設計不當會急速膨脹。

- **AI 資產（embedding、向量索引、chat transcript、enrichment 文本、insight 衍生表）是可重建的衍生狀態**，不是核心數據。清空重跑不會丟失任何原始歷史紀錄。
- **向量 payload 不進 SQLite**，而是寫到 `derived/vectors/` 的 `FlatVectorIndex` sidecar plane；SQLite 只保留 compact metadata / rebuild accounting。
- **語義搜尋完整覆蓋向量集合**。搜尋逐段計算全部向量的分數，再按 canonical page 合併與篩選；候選數量不能限制可達資料。向量生成與 AI queue 的覆蓋由 area 4 負責。
- **AI 資產不能拖慢核心 archive**。search index、intelligence runtime、embedding / 向量索引、正文 blob 都和 canonical archive 隔離。
- **設定頁面應顯示各類資產的磁碟佔用**：core archive、search projection、intelligence projection、semantic / blob sidecars、快照等，讓用戶清楚知道空間花在哪裡，以及最近的增長趨勢。
- 產品層的 storage analytics summary 現在先分成兩個 top-level bucket：`core history`（`history-vault.sqlite` + `source-evidence.sqlite`）與 `other data`（search / intelligence projection、semantic index、content blobs、audit artifacts、exports、temporary files）。這是給 Dashboard / Intelligence / Settings 共用的使用者心智，不改變底層 storage planes。
- 更細的 storage detail 仍保留到 detail surface：`core history` 可再拆成 canonical archive / source evidence；`other data` 可再拆成 search projection、intelligence projection、semantic index、content blobs、audit artifacts、exports、temporary files。更細的 per-plugin / per-model accounting 仍可在後續里程碑再補。
- **跨 profile 的時間序列表要有自己的索引**。History 列表按時間讀所有 profile 的可見 visit；migration 016 的 `idx_visits_visible_time_id (visit_time_ms, id) WHERE reverted_at IS NULL` 讓第一頁與 cursor 頁只讀約一頁的索引條目，而不是先排序全部 visit（量測與代價見 [ipc-performance.md](ipc-performance.md)）。
- **快照必須有保留上限**。一個 100 GB 的 archive 保留 8 份快照就是 800 GB。預設保留最近 4-8 個 archive 快照，用戶可調。

### Turso intelligence, indexing and queue contract

Area 4 public storage operations receive the session's `Vault`. SQL remains inside
`vault_store::Session` callbacks. Intelligence, queue state, taxonomy edits and embedding
metadata use `Db::Intelligence`; chats and journals use `Db::Agent`; canonical facts,
AI run records and search publication use `Db::Archive`. Observed browser metrics use
the SourceEvidence read pool. Reads do not install schemas or recover jobs.

The consolidated Intelligence v1 schema exposes sixteen logical read views over
physical generation tables. A full build retains published generations until replacement
stages commit. Archive facts are read in timestamp/ID pages; facts, checkpoints and stream
state commit in bounded `Priority::Batch` writes. Cancellation is checked between batches.
Restart resumes a matching unfinished build. A changed archive or rule recipe discards
that unfinished generation; late visits and visibility changes require a full chronological
replay. Ordinary appends reuse the fact/daily/navigation generation and build replacement
aggregates. Published old generations are reclaimed in bounded writes.

`session_domains` persists per-session domain counts with navigation assignments and their
cursor. A resumed long session reads these counts instead of replaying every earlier visit.
`session_paths` holds each session's sites in visit order, a row per local day and at most 256
sites of its own, each row starting with the last three sites of the one before. Common paths
for a date range count the rows of those days, each path in the row of its last site, instead
of replaying the range's visits.
Assignments store their navigation ordinal, which session and trail details use for paging.
Refind construction streams one canonical page at a time. Its bounded evidence sample is
storage bookkeeping: public evidence reads page the complete canonical visit set.

Overview snapshots are warmed after publication, outside the render path, with cancellation
checkpoints between bands. Their fingerprint includes the requested end date, archive and
stage watermarks, classification recipe and local date/timezone. A changed fingerprint
prevents publishing an obsolete cache. Reads refresh runtime metadata and observed evidence.
The `overview` progress phase reports preparation of these views.

Queue claims capture their trigger watermark. Requests arriving during a running claim
remain queued after its completion. Module invalidations have a monotonic revision, so
completion cannot overwrite a newer stale state. Owner startup requeues in-flight jobs;
terminal jobs remain terminal. Malformed payloads commit their failed/quarantined state
before the decoding error is returned.

AI backfill captures an upper visit ID and walks all candidate batches. `limit` is a batch
size, including working-set builds; it is not total coverage. Working-set enumeration
pages canonical URLs and evaluates stars, notes/tags, recency and frequency without the
old 50,000/12,500 pools. Scores determine queue priority. Vectors remain in the existing
vector sidecars. `ai_embedding_totals` tracks item and byte counts in the same transaction
as metadata replacement and removal, including doctor repair.

Every changed wire shape, cursor rule and coverage-limit outcome is listed in
[the area-4 report](research/2026-10-turso-area4.md). Cursors are opaque and must be reused
with the same filters and sort. Empty observed-interaction pages can still have a cursor.
Agent-store behavior and its existing cursor contracts remain unchanged in this continuation.

Entity explanations return evidence visit IDs in cursor pages. Their factors describe the
whole entity on every page. Path-flow explanations still describe the latest occurrence;
they seek backward from its stored timestamp and retain only its domain window, rather
than buffering a profile's visit history.
