# ADR-012 — PathKeep 自己的資料庫改用 Turso

## 狀態

Proposed（2026-10-04）。用戶決定：只要 Turso 通過「驗收條件」一節列出的硬性要求，就整個換過去。驗收測試正在跑；結果出來後，本 ADR 改為 Accepted，或寫明哪一條沒過、怎麼處理。

取代 [large-result-search-tradeoff.md](../large-result-search-tradeoff.md) 的建議。那份文檔把遷移成本和工作量當成取捨理由，還提了過渡方案，違反核心原則 6。

## 背景

- **搜尋必須覆蓋整個 archive**（用戶 2026-10-04 決定）。搜不出來的記錄，對用戶來說就是不存在。
- **常見詞的相關度排序太慢。** SQLite FTS5 會替每個符合項打分：在 14.4M 筆訪問上，「topic」第一頁 10.5 秒，精確計數 26.6 秒。過去為了繞過這個問題，做了窗口、由新到舊的退路、符合集合快取、分段計數，每一項都是 workaround，窗口甚至讓結果搜不到。
- **加密 archive 很慢。** SQLCipher 讀每一頁都要解密：按頁面分組的查詢明文 6.8 秒，加密 26.8 秒；全量匯出 30 秒對 84 秒。
- **at-rest 保護不完整。** 搜尋、intelligence、AI 對話三個資料庫是明文（data-model.md 記載的例外，backlog `WORK-ENCRYPT-AT-REST-ALL`）。
- **備份時 app 用不了。** 調查結論：原因不在 SQLite 的讀取（WAL 下讀不會被擋），在 PathKeep 自己：
  1. 備份後重建整個搜尋索引；
  2. 讀取路徑在首次備份時會自己去重建索引；
  3. 冷啟動用 `BEGIN IMMEDIATE` 做 schema bootstrap；
  4. 前端讀取失敗後不會重試；
  5. 單一寫入者：備份的長交易進行時，加星、筆記、標籤等使用者寫入要等 5 秒，然後失敗。

## 決策

PathKeep 自己的五個資料庫（archive、source evidence、search、intelligence、agent）全部改用 Turso。

1. **全部加密。** 用 Turso 原生加密：按頁加密，全文索引也在加密資料庫裡。不再有明文的衍生資料庫。
2. **金鑰管理。**
   - 應用程式用 Argon2id 從密碼推導出 key-encryption key，用它包裝一把隨機的 256-bit data key，所有資料庫都用這把 data key 開啟。
   - 改密碼只需要重新包裝 data key，資料檔不用重寫。
   - keychain 存的是 data key 的包裝，或沿用現有做法，由驗收結果決定。
3. **全文搜尋用 Turso 原生 FTS。**
   - 每個頁面（以 URL 字串為單位，跨瀏覽器合併）一份搜尋文件，在寫入同一個交易裡更新。
   - 相關度排序走索引的 top-k 剪枝；相關度相同時按最新訪問排；計數直接從索引和頁面文件算。
   - 篩選（瀏覽器、日期、網域）必須作用在同一筆訪問上；具體查詢形態由驗收測試決定。
   - 不再有窗口、由新到舊的退路、符合集合快取、「準備中」階段、分段計數，也不再在備份後重建整個索引。
4. **Regex 照舊分段掃描並串流顯示。** Regex 用不了索引，掃 14.4M 筆一定需要時間；串流呈現是對的設計，不是 workaround。
5. **並行寫入。**
   - 使用者的寫入（星號、筆記、標籤、預覽狀態、設定）用 MVCC（`BEGIN CONCURRENT`），不必等備份的長交易。
   - 連線模式要避開「同一連線上所有 statement 共用一個 MVCC 交易」的已知限制。
6. **讀瀏覽器原始檔仍用 rusqlite。**
   - Chrome、Firefox、Safari 的歷史是瀏覽器自己的明文 SQLite 檔，常常正在使用中。用 SQLite 的 online backup API 拿快照，再寫進 Turso。
   - 只需要一般的 bundled SQLite，不再需要 SQLCipher 和 vendored OpenSSL。
7. **永遠不用 `VACUUM INTO` 複製加密資料庫**，它輸出的是明文。搬家用的匯出包，用明確 ATTACH 一個加密的目的資料庫再完整複製。
8. **和引擎無關、也要一起修的問題：**
   - 冷啟動不能用 `BEGIN IMMEDIATE` 擋住所有讀取；
   - 讀取路徑永遠不重建索引；
   - 前端遇到暫時性的忙碌錯誤，要能自己恢復，不能停在錯誤畫面。

## 驗收條件（全部要有可執行的測試，結果記在 RESULTS.md）

1. **全文查詢語法與現在一致。** 包括單一詞的前綴（`topi*`、`42*`）、邊打邊搜（最後一個詞是前綴）、片語、排除、十個欄位的權重、ngram 的錯字容忍和中文。
2. **在 14.4M、加密的情況下，帶篩選、每個頁面一行的相關度 top-k 正確**（和窮舉計算比對），計數精確，延遲可接受。
3. **MVCC 能和加密、FTS、ATTACH、多行程同時運作。** 備份的長交易進行時，小寫入不必等它，讀取不受影響，FTS 保持一致，不卡死。
4. **找出之前原型裡「建完索引後行程停住」的根本原因**，並修掉或繞開。
5. **加密設計可行。**
   - 金鑰包裝；
   - 加密複製（輸出確認是密文）；
   - 暫存或溢出檔不寫明文；
   - kill -9 之後能恢復，完整性檢查通過。
6. **Rust 介面能做到現在依賴的事。** 跨執行緒中斷查詢（匯出取消）、`RETURNING`、忙碌處理。
7. **rusqlite（讀瀏覽器）和 Turso 能共存在同一個執行檔裡。**

任何一條沒過，如果是 Turso 本身的 bug 或缺功能，就在 repo 裡記下根本原因和最小重現，並在 PathKeep 自己的代碼裡繞過（例如在 FTS 的結果流上自己做 top-k）。不在 Turso 倉庫開 issue 或 PR，也不以任何形式聯絡上游；若唯一的修法是改 Turso 本身，先問用戶。不退回 SQLCipher，也不做過渡方案。

## 後果

- 搜尋、加密、並行寫入這三類問題，由引擎的能力解決，不再靠應用層的 workaround。
- 要刪掉的代碼：
  - 窗口、由新到舊退路、符合集合快取、分段計數；
  - 明文搜尋資料庫的 `ATTACH … KEY ''`；
  - 備份後的整個索引重建。
- 現有的 SQLCipher archive 要轉換。現在幾乎沒有真實用戶，轉換流程照現有的「升級 archive」畫面做（預覽、可取消、有進度、驗證後再切換），不為舊格式長期保留相容層。
- 執行檔裡會同時有 Turso 和一般的 SQLite（只用來讀瀏覽器）。
- Turso 的加密、FTS、多行程在 0.8.1 標為 experimental。按用戶要求不以成熟度做取捨；靠上面的驗收測試和 E2E 守住行為。

## 相關

- [research/2026-10-turso-evaluation.md](../research/2026-10-turso-evaluation.md)
- [research/2026-10-large-result-search.md](../research/2026-10-large-result-search.md)
- [large-result-search-tradeoff.md](../large-result-search-tradeoff.md)（已被本 ADR 取代）
- [data-model.md](../data-model.md)、[lexical-recall-v2.md](../lexical-recall-v2.md)：決策 Accepted 後要改寫
