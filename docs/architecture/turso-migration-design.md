# Turso 與 Tantivy 遷移設計

> **狀態：** 設計（2026-10-05），依 [ADR-012](decisions/012-turso-storage-engine.md)：關聯式資料用 Turso，全文搜尋用獨立的 Tantivy 索引。
> **範圍：** PathKeep 自己的資料庫、擁有資料庫的行程、寫入模型、搜尋、維護操作、測試。
> **不在範圍：** 讀瀏覽器原始檔（照舊用 rusqlite，只是改成一般的 bundled SQLite）。

## 1. 資料庫檔

| 檔案                         | 內容                                                   | 說明                                                                                                  |
| ---------------------------- | ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------- |
| `archive/history-vault.db`   | canonical archive，加上每頁一份的搜尋文件和搜尋 outbox | 搜尋文件、outbox 和訪問寫在同一個交易裡。不再有獨立的搜尋 SQLite 資料庫，也不再有 `ATTACH … KEY ''`。 |
| `derived/search-index/`      | Tantivy 索引（加密 Directory）                         | 可以從 canonical 重建。                                                                               |
| `archive/source-evidence.db` | 冷資料：原始 payload                                   | 大檔，少讀。                                                                                          |
| `derived/intelligence.db`    | 可重建的衍生資料                                       | 重寫頻繁，獨立成一個檔，不讓 canonical 檔膨脹。                                                       |
| `derived/agent.db`           | AI 對話                                                | 獨立。                                                                                                |

- 四個檔都用 Turso 原生加密，各用一把從 data key 推導出的子金鑰（見 §3），全部 WAL 模式。
- 不用 MVCC，不用 TEMP 表，不用 `VACUUM INTO`。
- 暫存用 ATTACH 一個加密的 scratch 資料庫（`derived/scratch.db`，啟動時清空）。

## 2. 唯一擁有者：PathKeep app 行程

- **擁有者：** 只有 PathKeep app 行程開資料庫。它可以有視窗，也可以只在選單列（tray）。
- **擁有權鎖：** 擁有者在 `archive/.pk-owner.lock` 上持有 OS 獨佔鎖。拿不到鎖的行程不准開資料庫檔，`vault-core` 的 open 函數會檢查這一點。
- **其他入口一律是 client**，透過本機 IPC 和擁有者溝通。macOS / Linux 用 Unix domain socket，Windows 用 named pipe。
  - **排程備份：** launchd 或 Task Scheduler 照樣啟動 `<app> --worker backup --due-only`。這個行程不碰資料庫：app 正在跑就把請求轉給它；沒在跑就以背景模式（無視窗，只有 tray）啟動 app，等 socket 就緒後再轉。
  - **MCP server：** 照樣是 stdio 行程，每個 tool call 轉給擁有者。app 沒在跑時，以同樣方式啟動它。
  - **CLI：** `doctor`、`ai-index`、`ai-queue` 也一樣經由 IPC。
- **Tauri 單一實例：** 同一個使用者只會有一個 app 實例；第二次啟動會把參數轉給第一個。
- **IPC 協定：** 有版本號的 request/reply。只開放 client 需要的操作，不是把整個命令面都開出去。

這樣就同時解決了兩件事：

- 多行程 WAL 那個 `.tshm` frame 上限造成的卡住，因為根本只有一個行程；
- 排程備份和 GUI 同時寫入的衝突，因為所有寫入都經過同一個寫入佇列。

實作與協定：[owner-process.md](owner-process.md)。

## 3. 金鑰

- **Data key：** 隨機 256-bit，開啟所有資料庫。
- **密碼信封** `archive/keys.envelope`：
  - Argon2id 從密碼推導出 KEK，用 AES-256-GCM 包裝 data key；
  - 每個資料庫、搜尋索引、暫存檔各用 HKDF-SHA256 從 data key 推導出自己的子金鑰，標籤固定，不共用金鑰材料；
  - 暫存檔的金鑰再加一個只存在記憶體裡的隨機 salt，所以殘留的溢出檔即使有密碼也解不開；
  - KDF 參數存在信封裡，並且受認證；
  - 目標機上的參數另外量（驗收時用 m=256 MiB、t=3、p=4）。
- **Keychain：** 存 data key 本身，取代今天存的 archive 金鑰，所以自動解鎖的安全性和今天一樣。
- **改密碼：** 只重新包裝信封，資料檔不動。
- **明文 archive 模式（用戶選擇不加密）：** 照樣支援，不設 key。
- **記憶體裡的金鑰：** 鎖定時清掉 data key，並關閉所有連線。

## 4. 存取層：`vault-store`（新 crate）

**職責：**

- 開啟資料庫；
- 擁有連線；
- schema migration；
- 寫入佇列；
- 讀取池；
- 暫存檔加密 IO；
- 合作式取消；
- 錯誤分類。

**不負責：** 任何業務 SQL。

**連線：**

- 每條連線只屬於一個執行緒。
- **寫入執行緒：** 每個資料庫一條，處理寫入佇列。佇列有兩個優先級：「互動」（星號、筆記、標籤、設定、預覽狀態）和「批次」（備份、匯入、重建）。每個批次交易結束後，先處理互動寫入，再開下一批。
- **讀取執行緒池：** 大小由記憶體量測決定。每條讀取只開短交易，以免卡住 checkpoint。
- 對外是同步的 request/reply 介面，接上 Tauri 的 `spawn_blocking`。不把 async 往上傳。

**取消：** 長操作（匯出、搜尋 collector、regex 掃描、批次寫入）每批之間檢查取消旗標。Turso 沒有公開的中斷介面。

**暫存 IO：** 用 `with_io_impl` 包一層，用只存在記憶體裡的金鑰（AES-256-CTR）加密 `tursodb_temp_file`。Apple Silicon 要開 `--cfg aes_armv8` 和 `--cfg polyval_armv8`。

**Checkpoint：**

- 每個批次之後做 PASSIVE。
- 寫入佇列空下來，而且讀取都結束時，做 TRUNCATE。
- 不依賴自動 checkpoint 的設定，0.8.1 會忽略它。

**Migration：**

- 每個資料庫一份有版本號的 migration 清單，每一步在一個交易裡跑。
- 只在擁有者啟動時跑一次，不在讀取路徑上跑。

**錯誤：** 引擎的錯誤轉成 PathKeep 的分類：鎖定中、忙碌、損毀、金鑰錯誤、取消。前端遇到忙碌會自己恢復。

## 5. 寫入模型

- **備份和匯入分批提交。** 每批一個交易，包含：
  - 訪問；
  - URL；
  - 搜尋文件的 upsert 和搜尋 outbox（Tantivy 由索引執行緒在交易之後追上）；
  - profile watermark；
  - run 的進度。
- **Run 的可見性：**
  - run 一開始就是 `running`；已提交的批次立刻可見，因為它們就是真實的歷史。
  - 全部批次完成後，run 才標成 `completed`。
  - 中斷（崩潰、取消）的 run 會標成 `interrupted`。下次備份從 watermark 接著做，已提交的批次不重做。
  - 撤銷匯入照舊以 batch id 為單位。
- **一次匯入 14.4M**（例如 Chrome 一年半、Firefox 十年的一次性匯入）也走同樣的分批，有進度，可以取消。
- **使用者的寫入**在批次之間進行，不會等到整個備份結束，也不會在 5 秒後失敗。
- **不再有：**
  - 「備份後重建整個搜尋索引」；
  - 「讀取路徑去 seed 索引」；
  - 「冷啟動用 `BEGIN IMMEDIATE` 擋住讀取」。

## 6. 搜尋

- **搜尋文件：**
  - 每個頁面（URL 字串，跨瀏覽器合併）一份，存在 archive 的 `search_documents` 表。
  - 欄位：十個權重欄位，加上篩選和排序要用的欄位（網域、最新可見訪問、所屬 profile）。
  - 寫入訪問、標註或 enrichment 時，在同一個交易裡更新它，並寫一筆 `search_outbox`（頁面 id、變動序號）。
- **Tantivy 索引：**
  - 欄位：詞欄位（十個，各有權重）、3-gram 欄位（錯字容忍、未分詞的中文），以及 fast fields（最新可見訪問、網域、profile 位元）。
  - 擁有者行程裡的索引執行緒消化 outbox：每批讀出變動頁面，更新 Tantivy，commit 時把已處理到的序號寫進 commit payload，然後刪掉已處理的 outbox 列。
  - 崩潰後從 payload 記的序號重放。索引不見、損毀，或 schema、分析器版本不符時，從 canonical 整個重建，有進度，搜尋照常可用。
- **加密 Directory：**
  - 檔案切成 64 KiB 區塊，每塊用 AES-256-GCM 加密；檔名、區塊位置和明文長度一起認證。
  - 金鑰：data key 用 HKDF 推導出的子金鑰。
  - 寫入、合併、meta 檔從建立起就是密文，不留明文暫存。
  - 解密快取有上限，鎖定時清掉。
  - 安全審查後才能發佈。
- **正規化：** 照舊在 Rust 裡做：ICU NFKC、去重音、OpenCC 簡繁折疊、中日韓切分。
- **查詢：** 用 Tantivy 的查詢物件直接組（詞、前綴、片語、排除、欄位權重），不經文字解析。
- **排序：**
  - 自訂 collector：先相關度，再最新可見訪問，最後頁面 id。
  - 篩選依選擇性推進查詢；日期和瀏覽器篩選，要以同一筆可見訪問為準，這一步由 SQL 算 eligibility 或驗證。
  - 計數和排序分開算，精確。
- **可見性：** 結果一律以 Turso 的 canonical 資料驗證，索引追趕期間也不會顯示已撤銷的資料。新寫入的頁面在 outbox 消化後就搜得到；在那之前，畫面上標明索引正在更新。
- **記憶體上限：** 寫入堆積、解密快取、reader 重新載入策略都有明確設定，數字用 14.4M 實測決定。
- **呈現（用戶要求）：**
  - 排序分時間片跑；每片回傳進度。
  - 排完前顯示進度和目前最好的結果，標明「仍在排序」。
  - 排完才換成最終順序，之後不再重排。
  - 計數陸續到。
- **Regex：** 照舊在訪問上分段掃描並串流；Tantivy 的 3-gram 欄位可以預先篩掉不含字面片段的頁面。
- **刪掉：**
  - 25,000 頁窗口、由新到舊退路、符合集合快取；
  - 「準備中」階段、分段計數；
  - 模糊搜尋的 200 候選上限。
- **其他會讓記錄搜不到的上限一起修**（Codex 稽核出的 13 處，見 STATUS），例如：
  - AI 搜尋的池子上限；
  - code mode 只看最新 1,000 筆；
  - 命令面板不續掃；
  - 標註 API 的上限。

### §6 implementation: `vault-search` (2026-10-05)

The independent engine is implemented in
[`vault-search`](../../src-tauri/crates/vault-search/README.md); vault-core's
projection/outbox consumer and UI integration remain separate work.

- Open/create accepts `Option<&vault_crypto::DataKey>`. `EncryptedDirectory` owns
  sub-key derivation; vault-search implements no encryption.
- Ten weighted positioned fields, Unicode trigrams, page ID, recency, domain and
  multi-valued profile fast fields. Schema/analyzer versions accompany the durable
  outbox sequence in every encrypted commit payload.
- Exact and prefix token namespaces share the ten fields. Complete Unicode prefix
  aliases provide BM25 frequency/length scoring without query-time expansion caps.
  Field norms include the aliases. This preserves the projection weights, not
  numerical FTS5 scores. The measured storage/build cost is documented in the crate.
- A bounded heap ranks all eligible matches by score, recency and page ID. Domain
  and profile estimates choose pushed zero-score intersections or fast-field
  predicates; canonical same-visit eligibility remains caller-owned.
- Session-owned snapshots and continuation keys keep every result reachable during
  writes and rebuilds. Ranking and counting retain their segment position between
  time slices; provisional results become exact only at completion.
- Rebuild streams new segments alongside the published commit, then atomically
  replaces Tantivy metadata. Cancelled/failed input never publishes. Crash replay
  starts after the committed sequence; fresh writers first collect uncommitted
  deletion files that would otherwise collide with replayed opstamp filenames.

The crate [README](../../src-tauri/crates/vault-search/README.md) defines caller
obligations and API contracts; the
[measurements](research/2026-10-vault-search-benchmark.md) justify its defaults.

## 7. 維護操作

| 操作            | 新做法                                                                               |
| --------------- | ------------------------------------------------------------------------------------ |
| 改密碼          | 重新包裝信封（§3）                                                                   |
| 加密 ↔ 明文切換 | 明確 ATTACH 一個新設定的目的資料庫，完整複製，驗證後再換檔；Tantivy 索引用新金鑰重建 |
| 搬家匯出包      | 同上，複製到用匯出密碼加密的目的地。`derived/` 不放進包，到目的地重建                |
| 匯入搬家包      | 驗證後換檔，沿用現有的中斷恢復 journal                                               |
| 安全快照        | 寫入佇列暫停，TRUNCATE checkpoint，複製檔案（本來就是加密的）                        |
| 清除全部資料    | 照舊                                                                                 |
| doctor / repair | 經由擁有者執行                                                                       |
| 匯出歷史        | 不排序的查詢形狀（測試要確認計畫裡沒有 sorter），合作式取消                          |

## 8. 從 SQLCipher 轉換

- 現有的 SQLCipher archive，用「升級 archive」畫面轉換：
  - 預覽要多少磁碟、會動到哪些檔；
  - 用戶確認後才開始；
  - 有進度，可以取消；
  - 驗證計數和內容摘要後，才換檔；
  - 舊檔留到驗證完成。
- 轉換器是一個獨立的 crate（`vault-convert-sqlcipher`）：唯一仍依賴 SQLCipher 的地方。主程式其他部分不再連結 SQLCipher。
- 什麼時候拿掉轉換器另外決定。

## 9. 測試

- **E2E 是合約。** 現有 28 個 E2E 全部要過，另外新增：
  - 備份進行中加星、加筆記，立刻成功，並且在搜尋裡找得到；
  - 常見詞搜尋：整個 archive 都覆蓋，計數精確，有進度呈現；
  - 排程備份在 app 開著時，經由 IPC 交給 app 執行；
  - 從 SQLCipher archive 轉換後，資料一致。
- **Rust 測試：** 和引擎行為耦合的測試（檔案交換、SQLCipher 標頭、`ATTACH ':memory:'` 的 fixture）照新架構重寫，先寫失敗模式，再寫測試。不移植只為了測舊引擎行為的測試。
- **升級 Turso 時必須重跑：** 前綴、組合排序、暫存檔加密、多行程上限，四個回歸探測。

## 10. 工作切分（依序；同一層可以平行）

1. 平行進行：
   - `vault-store`：開啟、金鑰信封、連線擁有、寫入佇列、讀取池、migration 框架、暫存加密 IO、checkpoint、取消、錯誤分類。
   - `vault-search`：Tantivy 加密 Directory、schema、索引執行緒、outbox 消化與重放、重建、查詢組合、collector、計數、記憶體上限。
2. schema：四個資料庫的 schema 和 migration，包含搜尋文件和 outbox。
3. 唯一擁有者和 IPC：擁有權鎖、socket 或 named pipe、排程備份、MCP、CLI client、背景啟動 app、Tauri 單一實例。
4. 平行進行：
   - 寫入路徑：備份、匯入、撤銷和恢復、標註、預覽圖、intelligence projection、AI 佇列。
   - 搜尋：查詢產生器、collector、計數、分時呈現的 API、regex、模糊、AI 搜尋、MCP、匯出、其他上限。
   - 讀取路徑：History、Insights、intelligence 讀取、agent store。
   - 維護：改密碼、模式切換、搬家匯出和匯入、快照、清除、doctor。
5. SQLCipher 轉換器。
6. 前端：搜尋的分時呈現、忙碌時自己恢復、轉換畫面（我做）；E2E 全套。

後端交給 Codex，每塊附明確範圍和驗收條件。前端、E2E 和審查由 Claude 做。

## 11. vault-core 怎麼拿到資料庫

- **`vault_core::Vault`：** vault-core 對外只有這一個 handle。它包著擁有者行程裡唯一的 `Arc<vault_store::Store>`，以及 `ProjectPaths` 和設定。
  - app 在解鎖時建立它，鎖定時關掉它（同時清掉金鑰和解密快取）。
  - Tauri 命令從 app state 拿 `&Vault`，再交給 `vault_worker` 和 `vault_core`。
- **公開函數一律收 `&Vault`，**不再收 `(paths, config, key)`，也不再自己開連線。
  - 讀取寫成 `vault.store().read(Db::Archive, |s| ...)`；寫入用 `write`，並指定優先級。
  - 長操作（備份、匯入、重建）每批送一個 `Batch` 寫入，每批之間檢查 `CancelToken`。
- **內部函數收 `&vault_store::Session`。** 和今天收 `&rusqlite::Connection` 的位置一樣，只是換型別。
- **測試用 `Vault::open_temp()`：** 開一個暫時的 Store（明文，或帶 32-byte key），schema 由 `store_schema::migration_registry()` 建好。
- **遷移期間的做法：**
  - 一次移植一個區域：改簽名、改 SQL、改測試，讓那個區域的測試在 Turso 上通過。
  - 還沒移植的區域照舊用 rusqlite，以便編譯和跑舊測試。
  - 全部移植完成後，app 才切換成用 `Vault`，同時刪除 rusqlite 的開檔函數。
  - 分支上一定會有一段「app 還不能跑」的時間；分支的驗收是最後的 E2E 全綠，不是中間的每一步。
- **區域與順序**（同一層可以平行；每個區域都附自己的測試）：
  1. archive 寫入：開檔、ingest、備份（分批）、Takeout 和瀏覽器匯入、撤銷和恢復、標註、星號、預覽圖、source evidence。
  2. archive 讀取：History 時間軸、read models、source stats、URL 詳情、doctor、匯出。
  3. 搜尋（等 vault-crypto 完成）：`vault-search` crate、索引執行緒和 outbox、查詢、collector、AI 搜尋、MCP。
  4. intelligence、AI 佇列、agent store。
  5. 維護：改密碼、加密模式切換、搬家匯出和匯入、快照、清除、啟動恢復。
  6. app 接線：解鎖建立 `Vault`、擁有者生命週期、刪掉 rusqlite 開檔函數。最後跑 E2E 全套。
