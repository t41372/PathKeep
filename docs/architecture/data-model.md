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
- Raw capture 層保留原始瀏覽器的時間值不做轉換（存在 `raw_row_versions` 的 JSON payload 中）。
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

### PathKeep v1 起點

- PathKeep 的 canonical archive schema v1 採 **fresh schema** 策略，不在舊 `browser-history-backup` archive schema 上繼續疊 migration。
- 既有 legacy archive DB 透過**一次性升級工具**轉入新的 canonical schema v1；原始 DB 或 migration 前 snapshot 必須保留，作為 recoverability 安全網。
- 從 canonical schema v1 開始，後續所有 PathKeep schema 調整才進入正式的 migration ledger。

### 核心策略：追蹤最新，兼容歷史

我們的策略是**永遠追蹤最新版本的瀏覽器數據格式，同時向後兼容所有舊版格式**。

- **前向追蹤**：每當瀏覽器發布新版本，主動研究其 History / Favicons DB 的 schema 變化。新增的表、欄位、關聯關係都應該被評估和捕獲。
- **向後兼容**：parser 必須能處理舊版瀏覽器的數據庫。新功能的欄位不存在時 gracefully degrade，不影響備份。
- **深度研究**：在實現或更新各瀏覽器的 parser 時，必須**深度研究該瀏覽器當前版本能提供的所有數據**，不要只提取最基本的 visits + urls。瀏覽器不斷在歷史紀錄中加入更多有價值的信息，我們應該盡可能捕獲。

### 設計要點

- 兩層處理：
  - **Raw capture 層**：永遠動態讀取所有欄位並落盤。即使 Chrome 加了新欄位或改了結構，raw capture 只要 SQLite 能打開就能繼續工作。
  - **Derived normalizer 層**：把 raw data 映射到我們的統一 schema。新增未知欄位時只降級該欄位，不丟全部數據。
- Archive schema 獨立版本管理，用編號 SQL migration。每次升級前自動備份 archive DB。
- 關鍵時刻保存完整原生快照（壓縮保存 History/Favicons DB 原檔）：
  - 首次備份
  - 來源 schema 變更時
  - 每季 checkpoint
- 記錄每次備份的瀏覽器版本、schema 指紋、profile metadata。

### Migration 系統

- migration ledger 適用於 **PathKeep canonical schema v1 之後** 的演化；legacy archive → v1 走一次性 upgrade path，不和正式 migration 編號混用。
- 新增 `schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT, checksum TEXT, backup_path TEXT)` 表。
- Migration 採編號 SQL 檔。
- 每次 migration 前自動做 archive snapshot。
- Migration report 寫入審計工件。

---

## 3. 長期容量設計原則

重度瀏覽器使用者（每天 ~2,500 visits）在 20 年間可能累積近 2000 萬筆記錄。核心 archive 本身不會是瓶頸（預估僅 40-80 GB），但 AI 相關資產如果設計不當會急速膨脹。

- **AI 資產（embedding、向量索引、enrichment 文本、insight 衍生表）是可重建的衍生狀態**，不是核心數據。清空重跑不會丟失任何原始歷史紀錄。
- **Embedding 不能用 JSON 文本存儲**。向量應使用 binary blob 或專用向量格式（LanceDB 原生格式），避免 2-3 倍的序列化膨脹。
- **語義搜尋不能做全表掃描**。幾千萬行 embedding 全量 cosine 計算不可能保持互動性，需要 ANN（近似最近鄰）索引。LanceDB 提供 disk-based IVF-PQ 索引。
- **AI 資產不能拖慢核心 archive**。Embedding 和向量索引存在獨立的 LanceDB sidecar 中，與主 SQLite archive 完全隔離。
- **設定頁面應顯示各類資產的磁碟佔用**：core archive、FTS 索引、embedding / 向量索引、enrichment、快照等，讓用戶清楚知道空間花在哪裡，以及最近的增長趨勢。
- **快照必須有保留上限**。一個 100 GB 的 archive 保留 8 份快照就是 800 GB。預設保留最近 4-8 個 archive 快照，用戶可調。
