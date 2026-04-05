# Browser History Vault 長期容量與效能分析

日期：2026-04-05  
目的：檢驗「20 年以上、重度瀏覽器使用者、所有功能啟用、每日備份」下的設計可行性，並評估 60 年尺度是否仍成立。

## 1. 先講結論

### 短結論

- **Archive 核心本身不會先爆。**
- **真正會先爆的是 embedding 儲存與 semantic search 查詢策略。**
- **如果沿用目前的 embedding 設計，20 年就已經偏危險，60 年不建議。**
- **如果把 embedding、FTS、enrichment、snapshot retention 做對，20 年很穩，60 年仍然可做。**

### CTO 判斷

要撐到 20-60 年，這個專案需要達成下面四件事：

1. embedding 不再用 JSON text 存
2. semantic search 不再 full-scan 全表 cosine
3. enrichment / FTS / vector index 改成分層資料設計，而不是重複把大文本塞進多張表
4. snapshot 必須有硬性的 retention policy

如果這四件事沒做到：

- **20 年可用，但資料庫會變胖，AI 功能會變慢**
- **60 年在重度使用者情境下會開始不健康**

如果這四件事做到：

- **20 年完全沒問題**
- **60 年仍然可以做，只是必須接受資料目錄會是「數十 GB 到數百 GB」級別，而不是幾 GB**

---

## 2. 分析假設

## 2.1 使用者模型

這次用的是「重度瀏覽器使用者」模型：

- 工作與生活都高度依賴瀏覽器
- 幾乎每天使用
- 長期多 profile / 多站點 / 搜尋密集
- 開啟全部功能：
  - 每日備份
  - Recall / FTS
  - enrichment
  - semantic search
  - insights
  - Ask My History

## 2.2 硬體假設

以 **2026-04-05** 這個時間點的主流個人電腦做保守假設，並假設之後技術**完全停滯**：

- CPU：8-12 個有效核心
- RAM：16-32 GB
- SSD：1-2 TB NVMe
- GPU：不假設一定有離散 GPU

也就是說，我們不能把未來可用性建立在「10 年後電腦自然會快很多」這種前提上。

## 2.3 流量模型

我用三個量級來看：

| 模型 | 每日 visit events |
|------|-------------------|
| Heavy | 2,000 |
| Very Heavy | 2,500 |
| Extreme | 5,000 |

對應的總 visit 數：

| 模型 | 20 年 | 60 年 |
|------|------|------|
| 2,000/day | 14.6M | 43.8M |
| 2,500/day | 18.25M | 54.75M |
| 5,000/day | 36.5M | 109.5M |

其中 `2,500/day` 是我認為最接近「重度、長期、工作生活都很依賴瀏覽器」的標準模型。

---

## 3. 現有設計的主要容量風險

## 3.1 embedding 現在的存法過胖

目前 `ai_embeddings` 是這樣存的：

- `content TEXT`
- `embedding_json TEXT`
- `dimensions INTEGER`

也就是：

- 向量不是 binary，而是 JSON 字串
- 文字內容又在 embedding 表裡重複存一次

對應代碼：

- [ai.rs](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/crates/vault-core/src/ai.rs#L71)
- [ai.rs](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/crates/vault-core/src/ai.rs#L84)

這個設計在小數據集上簡單，但在長壽命 archive 上很不划算。

### 為什麼危險

1536 維 float32 向量的原始大小只有：

- `1536 * 4 bytes = 6144 bytes`，約 6 KB

但如果存成 JSON：

- 每個數字可能會變成 6-12 個字元
- 再加上逗號、括號、SQLite text overhead
- 實際常見會膨脹到 **12-18 KB / 向量**

也就是說，**單是序列化格式就可能把向量體積放大 2-3 倍**。

## 3.2 semantic search 現在是 full scan

目前 semantic search 的路徑是：

1. 把該 provider/model 的 embedding rows 全部載出
2. JSON parse 每一條 embedding
3. 在應用層做 cosine similarity

對應代碼：

- [ai.rs](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/crates/vault-core/src/ai.rs#L892)

這在幾萬筆時沒問題，但在幾千萬筆時不可互動。

### 粗算

如果是 `43.8M` visits、每筆 `1536` 維：

- 每次查詢要做大約 `43.8M * 1536 = 67.3B` 維度乘加

這還**不包含**：

- SQLite 讀盤
- JSON parse
- 分數排序
- Rust allocation

所以目前這條路在 20-60 年尺度下不成立。

## 3.3 insights 現在的實作本來就不是長期全量設計

目前 insights 有兩個明確訊號：

- enrichment 文字上限是 `12,000` 字元
- 預設分析窗口只跑最近 `600` 筆

對應代碼：

- [insights.rs](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/crates/vault-core/src/insights.rs#L22)
- [insights.rs](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/crates/vault-core/src/insights.rs#L25)

這代表現在的 insights 實作其實還是「產品原型模式」，不是「60 年全量數據模式」。

## 3.4 snapshot 如果不控 retention，最終會比主庫更大

真正長壽命系統常見的坑不是主資料表，而是：

- DB snapshots
- raw browser checkpoints
- audit exports
- remote bundles

如果 snapshot 不設 retention：

- 主 archive 可能 150 GB
- 但快照保留 8 份就是 1.2 TB

所以 snapshot 一定要是**有上限的安全網**，不能是無限累積的第二套 archive。

---

## 4. 容量估算

以下我分成三種設計狀態來估：

## 4.1 設計 A：接近目前方向的 naive all-on

假設：

- core archive：`1.5 KB / visit`
- enrichment stored text：`1.2 KB / visit`
- FTS / recall projections：`0.8 KB / visit`
- embeddings：`16 KB / visit`
- insight derived state：`0.15 KB / visit`

合計約：

- **19.65 KB / visit**

這裡最大的問題就是 embedding。

### 容量結果

| 使用量 | 20 年 | 60 年 |
|-------|------|------|
| 2,000/day | 267.2 GB | 801.6 GB |
| 2,500/day | 334.0 GB | 1002.0 GB |
| 5,000/day | 668.0 GB | 2003.9 GB |

### 解讀

- 20 年已經偏胖
- 60 年接近或超過 1 TB
- 這還**沒把多份 full snapshots** 算進去

所以這個版本我判定為：

- **20 年勉強可存**
- **60 年不健康**

## 4.2 設計 B：我建議的 long-horizon 設計

假設：

- core archive：`1.3 KB / visit`
- enrichment stored text：`0.6 KB / visit`
- FTS / recall：`0.6 KB / visit`
- embeddings：`0.5 KB / visit`
- insights derived：`0.15 KB / visit`

合計約：

- **3.15 KB / visit**

這個模型對 embedding 做了兩件事：

- 不對每一個 visit 都保留高成本向量
- 向量不再用 JSON text

### 容量結果

| 使用量 | 20 年 | 60 年 |
|-------|------|------|
| 2,000/day | 42.8 GB | 128.5 GB |
| 2,500/day | 53.5 GB | 160.6 GB |
| 5,000/day | 107.1 GB | 321.2 GB |

### 解讀

這個結果是健康的。

就算是 60 年、2,500/day：

- 主資料大約 `160 GB`

這在 1-2TB SSD 的假設下完全可以接受。

## 4.3 設計 C：每個 visit 都做 embedding，但做對格式

假設：

- 仍然每個 visit 都存 embedding
- 但 embedding 改成約 `1.6 KB / visit` 等級的緊湊格式

合計約：

- **4.25 KB / visit**

### 容量結果

| 使用量 | 20 年 | 60 年 |
|-------|------|------|
| 2,000/day | 57.8 GB | 173.4 GB |
| 2,500/day | 72.2 GB | 216.7 GB |
| 5,000/day | 144.5 GB | 433.4 GB |

### 解讀

這其實也還可以接受。

所以真正的結論不是「embedding 一定太大」，而是：

> **目前 embedding 的儲存格式與查詢方式太大、太慢。**

---

## 5. 逐項回答你的問題

## 5.1 embedding 會不會過大？

### 以目前設計來看：會

原因：

- `embedding_json TEXT`
- 每個 visit 一筆 embedding
- query 時全量讀出

如果是 `2,500/day` 的重度使用者：

- 20 年光 embedding 就大約 **271.9 GB**
- 60 年光 embedding 就大約 **815.8 GB**

這是主庫最不健康的一塊。

### 以修正後設計來看：不一定

如果改成：

- binary / float16 / quantized vector
- 只保留一個 active embedding model
- 不把同一份文本重複存進 embedding 表
- 不做 exact full scan

那 embedding 在 20-60 年尺度下仍然可以接受。

我的判斷是：

- **embedding 不是不能做**
- **但不能照現在這個 schema 與查詢模型做**

## 5.2 數據庫會不會過大？

### Archive core：不會先成為瓶頸

真正的 core archive，即便 60 年、2,500/day，估算也仍在：

- `60-80 GB` 級別的核心事實表

這完全不是 SQLite 的理論極限，也不是 SSD 無法承受的量級。

### 會讓資料庫胖起來的元兇

1. embedding JSON
2. enrichment 大文本重複儲存
3. FTS 把太多全文全部重複索引
4. snapshots 無上限保留

所以問題不是 SQLite 不行，而是**我們怎麼花掉空間**。

## 5.3 數據量太多會不會導致卡頓？

### Lexical / FTS：可以不卡，但有前提

只要做到：

- FTS5 獨立索引表
- query 永遠走 pagination / virtualized list
- timeline 用預先聚合 bucket，而不是每次拖動都掃 `visit_events`

那麼 20-60 年級別資料量仍可保持互動性。

### Semantic search：目前會卡

現在的 full-scan cosine 在幾千萬筆時一定卡。

所以：

- **現在的 semantic search 設計不通過**
- **必須改成 ANN index**

### Insights：全量重算會卡

如果每次都想從 4000 萬筆原始 visit 重新推導 topics / threads / cards，會慢。

但如果改成：

- incremental jobs
- windowed recomputation
- aggregates/materialized summaries

那就沒有問題。

## 5.4 60 年也不會出問題嗎？

### 回答：可以，但要限定「哪一版設計」

如果你問的是**目前這版實作方向**：

- **我不會承諾 60 年都很健康**

如果你問的是**經過這次報告要求的重構後**：

- **我認為可以承諾 60 年仍然是技術上健康的**

但這裡的「健康」不是說：

- 永遠只佔幾 GB
- 所有重建都幾秒完成

而是說：

- 主 archive 尺寸仍在單機可承受範圍
- 查詢延遲仍可做桌面互動
- 背景作業仍可在今天水準的 CPU/RAM 下完成
- 不需要依賴未來硬體性能自動拯救設計

---

## 6. 效能壓力評估

## 6.1 每日備份

每日備份本身不是問題。

原因：

- 來源瀏覽器只保留近 90 天左右的活躍資料
- 每天新增量相對小
- 我們是增量 ingest，不是每日重建 archive

就算是 `2,500/day`：

- 每天新增也只是數千 rows 到數萬 rows 級別的寫入

對 SQLite 來說是小工作量。

## 6.2 FTS rebuild

FTS 不應該每天 full rebuild。

應該：

- 增量寫入新 documents
- 只在 schema / tokenizer 重大變更時 full rebuild

full rebuild 在 20-60 年資料量下仍可能是「數十分鐘到數小時」的背景任務，但這可以接受，前提是它不是互動路徑。

## 6.3 Embedding pipeline

Embedding 計算量其實比儲存問題更容易解。

原因：

- 即使 2,500 visits/day，也不代表 2,500 條都值得 embedding
- 很多是重複 domain、短停留、無內容價值頁面

建議：

- 只對高價值頁面做 embedding
- 每日 budget 化，例如 `200-1000` 個新 documents/day
- 背景慢慢追平，而不是要求「今天的每個頁面今晚都要全部 semantic-ready」

## 6.4 Timeline / charts

時間軸若直接掃主表，60 年資料一定變慢。

但如果每天增量維護：

- `daily_visit_counts`
- `domain_daily_counts`
- `topic_daily_counts`

那時間軸與圖表查詢量其實非常小。

因為：

- 60 年也只有約 `21,900` 個日期 bucket

這個量級極小。

---

## 7. 真正要改的設計

## 7.1 embedding storage 必須重做

### 不應再做

- `embedding_json TEXT`
- 每個 provider/model 都複製整份 content

### 應改成

- 向量獨立存 binary blob 或專用向量索引
- 文件內容單獨存在 document/enrichment 層
- 向量只透過 `document_id` 關聯
- 只允許少量 active embedding spaces

## 7.2 semantic search 必須改成 ANN

可行方向：

- SQLite 的 ANN extension
- 或獨立 ANN sidecar index

核心原則只有一個：

- **不能再每次查詢都把全部向量拉出來算 cosine**

## 7.3 FTS 只索引「召回需要的文本」

不要把所有 enrichment 原文無差別塞進 FTS。

應該分層：

- metadata FTS：URL / title / search terms / normalized labels
- optional content FTS：只對有價值的頁面內容建立索引

## 7.4 snapshots 必須有 retention ceiling

建議：

- full archive snapshots：保留最近 `N=4~8`
- quarterly raw browser checkpoints：保留最近 `N=8~16`
- 更老的快照只允許使用者手動匯出到冷儲存

## 7.5 insights 必須增量化

不可以每次都重掃全歷史。

應改成：

- session / thread assignment incremental
- topic aggregates windowed recompute
- cards/materialized summaries overwrite latest window

---

## 7.6 如果 embedding 很大不可避免，必須把它們從主 archive 剝離

你這次提出的約束很關鍵：

- 我們想基於 embedding 做很多功能，不只是 semantic search
- 我們今天無法可靠定義什麼叫「重要文件」
- 所以「只 embed 少數高價值文件」不能作為唯一前提

這代表我們必須接受一件事：

> **大規模 embedding 也許不可避免，但它們不能成為主 archive 的負擔。**

因此，正確方向不是「想辦法讓主 SQLite 同時扛所有內容、FTS、embedding、ANN、insights」，而是把資料分層：

1. **Core archive DB**
   - source of truth
   - 使用 SQLCipher / SQLite
   - 只放 visit、URL、title、時間、session/thread 關聯、輕量 metadata

2. **Content snapshot store**
   - 壓縮後的 markdown / readable text / chunked text
   - 可以是獨立 sidecar SQLite，也可以是 content-addressed file store
   - 不是互動查詢的熱路徑

3. **AI assets / vector sidecar**
   - embedding
   - ANN index
   - rerank cache
   - clustering / centroid / graph / neighborhood 等衍生結構
   - 必須被視為「可重建的 derived state」，不是 canonical data

4. **Optional FTS sidecar**
   - 如果全文非常大，FTS 也應該從主庫分離
   - 避免 WAL、VACUUM、hot query 被大文本拖累

這樣做的真正目的，不只是容量，而是故障域隔離：

- embedding 再大，也不能拖慢 core archive 的 browse / timeline / backup
- ANN index 損壞，也不能傷到核心歷史資料
- AI 資產可以重建、清理、搬遷、升級，而不需要動主庫

這是我對 20-60 年設計最重要的補充結論。

## 7.7 Chroma 能不能幫忙？

### 可以幫忙的地方

Chroma 的確能幫我們把 vector retrieval workload 從主庫拆出去。

根據官方文件：

- Chroma 提供 persistent client / local path
- collections 可以同時存 `documents`、`embeddings`、`metadata`
- schema 可以配置索引，並可選 customer-managed encryption keys (CMEK)

參考：

- [Chroma Clients](https://docs.trychroma.com/docs/run-chroma/clients)
- [Adding Data to Chroma Collections](https://docs.trychroma.com/docs/collections/add-data)
- [Chroma Schema](https://docs.trychroma.com/reference/typescript/schema)

### 但它不是我心中的首選主方案

原因有四個。

#### 1. 它主要解的是 retrieval workload，不是整體 archive 架構

Chroma 很適合做 retrieval 層，但它不會自動解決：

- 主庫膨脹
- content snapshot 管理
- insight materialization
- 長期備份與壽命管理

也就是說，它能幫我們把「embedding 與 query 壓力」搬出去，但不會替我們完成整個長壽命分層設計。

#### 2. 對我們這種 Tauri + Rust 桌面應用，進程模型不夠乾淨

官方文件明確寫到：

- TypeScript client 需要連到一個運行中的 Chroma server
- Rust client 也需要連到一個運行中的 Chroma server

也就是說，在我們的桌面產品裡，Chroma 比較像一個 sidecar service，而不是可以自然內嵌進現有 archive 進程模型的儲存層。

對桌面產品來說，這會增加：

- 安裝與升級複雜度
- 本地 background process 管理
- 資源占用與 crash handling

#### 3. Chroma 本身不會減少原始 embedding 體積

官方 add-data 文檔說得很清楚：你可以把 `documents`、`embeddings` 一起存進 collection，也可以只存 embedding。

所以如果我們不自己做量化、分層、近重複處理、snapshot 壓縮：

- Chroma 只是在另一個地方把同樣大的東西存起來

它幫你做檢索，但不會自動幫你做長壽命容量治理。

#### 4. 本地桌面「可加密 sidecar」這件事，官方路線不夠明確

官方 schema 文檔有提到 CMEK，但從文檔呈現來看，這更像 Chroma indexing / managed environment 的能力接口，不像 SQLCipher 那種對本地單機檔案非常直接的 at-rest story。

對我們來說，最難的不是「可不可以把向量放進 Chroma」，而是：

- 能否保證本地加密
- 能否保證 source-of-truth 與 sidecar 一致性
- 能否在壞掉時快速重建

### 我的判斷

Chroma：

- **可以作為原型驗證或可選 retrieval backend**
- **不適合做我們長期桌面產品的第一優先核心架構**

我不會把它當作唯一解法，更不會把 canonical archive 放到 Chroma 裡。

## 7.8 比 Chroma 更接近需求的，是「支援量化與磁碟型 ANN 的 sidecar」

如果你的核心問題是：

- embedding 很大
- query 不能 full scan
- 不能讓主庫被拖慢

那比起 Chroma，我會優先考慮能明確提供下列能力的向量層：

- on-disk / memmap
- quantization
- 可控的 ANN index
- 本地 sidecar 部署

在這個方向上，Qdrant 與 LanceDB 都比 Chroma 更貼近我們的核心問題。

## 7.9 Qdrant 類方案：對大 embedding 更有幫助，但仍然只能當 sidecar

### 為什麼它更有幫助

Qdrant 的官方文件對大規模向量壓力有更直接的支援：

- vector storage 可設 `on_disk`
- 有 memmap storage，讓資料主要留在磁碟，由 page cache 協助存取
- payload 也可 `on_disk`
- 有 scalar / binary / product quantization

參考：

- [Qdrant Storage](https://qdrant.tech/documentation/storage/)
- [Qdrant Quantization](https://qdrant.tech/documentation/manage-data/quantization/)

這非常符合我們的需求，因為我們怕的不是「不能查」，而是：

- RAM 被吃光
- 主庫被大向量拖慢
- 幾千萬筆向量讓互動查詢不穩

Qdrant 官方文檔給出的能力，正好對準這些問題。

### 對容量的實際意義

官方量化文檔提到：

- scalar quantization 可把 `float32 -> uint8`，約 4x 壓縮
- binary quantization 可到約 32x 壓縮
- 某些模式還能做到更高速度

這代表如果我們真的要在 60 年尺度保留海量 embedding，Qdrant 類路線至少提供了明確的工程槓桿。

### 但它仍然不是 source of truth

即便如此，我還是只會把 Qdrant 當成：

- ANN / retrieval layer
- semantic neighborhood / clustering acceleration layer

而不是：

- 核心 archive DB
- canonical content store

原因很簡單：

- 向量索引可以重建
- archive 事實表不能承擔重建風險

### 安全與加密注意點

Qdrant 官方 security 文檔也很直接：

- self-deployed instance 預設並不安全
- 需要自己開 API key、bind、TLS 等

參考：

- [Qdrant Security](https://qdrant.tech/documentation/guides/security/)

這說明一件事：

> **Qdrant 可以幫我們解效能與容量，但不會天然替我們解本地加密與安全模型。**

如果要在桌面應用使用它，我們仍然需要：

- 把它限制在本地 loopback / 私有進程
- 對其資料目錄提供可靠的 at-rest 加密策略
- 把它視為可丟棄、可重建的 sidecar

## 7.10 LanceDB 也值得看，而且比 Chroma 更像嵌入式候選

LanceDB 的優點在於：

- OSS 可以直接連本地路徑，作為 embedded library 使用
- 官方支持 Rust / TypeScript / Python
- 文件明確強調 disk-based indexing
- 支持量化，包括 `IVF_PQ` 與 `IVF_RQ`

參考：

- [LanceDB Quickstart](https://docs.lancedb.com/quickstart)
- [LanceDB Vector Indexes](https://docs.lancedb.com/indexing/vector-index)
- [LanceDB Quantization](https://docs.lancedb.com/indexing/quantization)
- [LanceDB Indexing Overview](https://docs.lancedb.com/indexing)

這比 Chroma 更符合我們的桌面應用型態，因為它不是天然要求一個獨立 server process 才能運作。

### 但也有保留

1. OSS 路線需要自己管理 index lifecycle  
官方文件明確提到，OSS 模式要手動建立、更新與調參 index。

2. 官方明確寫出 at-rest encryption 的，是 Enterprise 安全頁  
Enterprise 文檔清楚提到 encryption at rest，但 OSS 本地單機加密故事沒有 SQLCipher 那麼直接。

參考：

- [LanceDB Enterprise Security](https://docs.lancedb.com/enterprise/security)

### 我的判斷

如果我們真的要做「本地、嵌入式、可 Rust 集成、偏向自管」的向量 sidecar：

- **LanceDB 比 Chroma 更值得實驗**

但我仍然不建議把它當成 archive core，本質上它更像：

- AI 資產層
- 向量與 hybrid retrieval 引擎

## 7.11 SQLite Vec1 值得跟，但今天還不適合單押

SQLite 官方現在有 Vec1：

- 走 SQLite virtual table
- 支援 ANN
- 用 BLOB 作為 native vector format
- 目前基於 IVFADC

參考：

- [SQLite Vec1](https://sqlite.org/vec1)
- [Vec1 Introduction](https://sqlite.org/vec1/doc/trunk/doc/vec1intro.md)

這條路非常吸引人，因為它跟我們的整體技術棧高度相容：

- 本地
- 單機
- SQLite 生態
- BLOB vector 而不是 JSON text

### 但今天還不夠成熟

官方 roadmap 仍列出多個 release 前需要完成的項目，並且有不少後續能力還在 roadmap 上，例如：

- 更多 SQL integration
- 其他資料型別
- OPQ
- graph-based index alternative

所以我的結論是：

- **Vec1 非常值得跟進**
- **但今天還不適合作為唯一押注**

它比較適合列為：

- 第一優先觀察路線
- 第二階段或第三階段替代方案

## 7.12 LEANN 值得研究，但它比較像「極小型搜尋索引」而不是完整 embedding 平台

你提到的 LEANN 是這輪研究裡最有意思的部分之一。

論文摘要與 repo README 的核心主張是：

- 不直接存全部 embedding
- 查詢時做 on-the-fly recomputation
- 讓 index 壓到原始資料的很小比例
- 相較傳統 index 可顯著減少儲存量

參考：

- [LEANN Paper](https://arxiv.org/abs/2506.08276)
- [LEANN GitHub](https://github.com/yichuan-w/LEANN)

### 為什麼它很吸引人

如果我們最害怕的是：

- 幾千萬到上億個 embedding 佔用太大
- 本地設備裝不下
- 備份與搬遷成本太高

那 LEANN 的方向正好直擊痛點。

### 但我不建議把它理解成「embedding 不用存了，所有問題都解了」

因為我們的 use case 不只是 retrieval。

你自己已經講中了重點：

- insights 系統也要基於 embedding
- 不只是 search
- 還包含 thread / topic / cluster / pattern / similarity-based analytics

這些能力與純 retrieval 不完全一樣。

如果我們要做的是：

- top-k semantic search
- RAG retrieve
- 類似 recall 的搜尋層

那 LEANN 很有吸引力。

但如果我們還要做：

- 全域 clustering
- topic centroid
- long-range similarity analytics
- repeated neighborhood updates
- vector-derived summaries

那單靠「查詢時重算 embedding」不一定合適。

### 我的判斷

LEANN 很可能適合做：

- **retrieval-facing compressed index**

但不太像可以單獨承擔：

- **所有 analytics-grade embedding 工作負載**

也就是說：

- **它可能是很好的搜尋層**
- **但不是完整 AI 資產層的唯一答案**

最實際的用法可能是：

- 用 LEANN 或 LEANN 類技術做第一階段召回
- 對熱資料、近期資料、被頻繁引用的資料，仍保留較高精度向量或衍生摘要

## 7.13 不知道什麼是「重要 document」，就不要把架構建立在二元 gating 上

這點我很同意你的質疑。

很多系統會說：

- 只 embed 高價值文件
- 低價值文件丟掉

但對瀏覽歷史產品來說，這在真實世界裡往往行不通。因為：

- 重要性是事後才知道的
- 今天看似低價值的頁面，半年後可能因為某個問題變得關鍵
- 使用者對「重要」的定義高度個人化、動態化

所以我更建議的是**分層 embedding 策略**，不是二元開關。

### Tier 0：所有 documents 都保留 lexical 與輕量 metadata

每個 document 至少有：

- title
- URL / domain / path signature
- timestamps
- normalized excerpt
- token count
- content fingerprints

這層最便宜，也最穩。

### Tier 1：所有 documents 都有「便宜的語義表徵」

這裡不是說每份都要保留 full float32 embedding，而是：

- 小模型 embedding
- 低精度向量
- PQ / binary / RaBitQ / LEANN 類召回表示
- 或其他適合大規模召回的壓縮表徵

重點是：

- **讓所有資料都有 semantic recall 能力**
- **但不要讓所有資料都佔一樣高的成本**

### Tier 2：熱資料 / 最近資料 / 被查過的資料 升級為高精度表示

只要某些 document：

- 最近常被查
- 被多次命中
- 進入 session / thread / insight pipeline
- 被使用者 pin / save / revisit

就可以升級為：

- 更高精度 embedding
- 更多 chunk embeddings
- 更完整的 derived graph / neighborhood state

### Tier 3：持久化的 derived outputs

對最昂貴的洞察運算，不要每次都從 raw embeddings 重算。

應持久化：

- topic assignment
- cluster id
- centroid summaries
- session/thread labels
- relationship edges

這層通常遠小於原始向量，但對產品體驗非常重要。

這個架構的好處是：

- 不需要先知道誰重要
- 所有資料都可被召回
- 成本隨使用熱度自然上升

## 7.14 content hash 不夠，就做近重複與版本鏈

你說得對：

- 網頁轉 markdown 後，完全相同文本其實不多
- exact content hash 去重，效果一定有限

所以不要把 dedupe 理解成「只有 bit-identical 才算重複」，而應該分成幾層。

### 1. exact hash

還是要有，因為它最便宜。

### 2. normalized hash

在 hash 前先做輕量 normalization，例如：

- 去掉明顯 boilerplate
- 統一空白
- 去掉追蹤參數
- 弱化時間戳、cookie banner、導航區塊

### 3. near-duplicate fingerprint

即使不是完全相同，也可以判斷為：

- 同頁不同版本
- 高度近似頁
- layout 改了但正文差不多

工程上可用：

- shingles
- SimHash / MinHash 類 fingerprint
- block / chunk-level overlap

### 4. version chain，而不是「重複就丟掉」

很多網頁不是重複，而是小改版。

所以更好的模型是：

- `document`
- `document_version`
- `document_chunk`
- `fingerprint`

我們要做的是把內容串成版本鏈，而不是只問「這次要不要存」。

這對 60 年壽命很重要，因為它能降低：

- 重複 snapshot 儲存
- 重複 embedding
- 重複 FTS indexing

## 7.15 我對最終技術方案的建議

### 必做原則

1. **Core archive 與 AI assets 完全分離**
2. **大文本 snapshot 壓縮並冷存**
3. **向量不再以 JSON text 儲存**
4. **semantic retrieval 不得 exact full scan**
5. **所有 AI 資產視為可重建 derived state**
6. **設定頁面必須可觀測各類資產大小**

### 我會怎麼排優先級

#### 第一優先：先做我們自己可控的分層 sidecar 架構

- core archive：SQLCipher SQLite
- content snapshots：壓縮 sidecar store
- vector assets：獨立 encrypted sidecar
- insights：只持久化 derived outputs

這一步不依賴任何特定向量庫，也最符合長壽命設計。

#### 第二優先：向量 sidecar 接上真正的 ANN / quantization 後端

從需求匹配度看：

- **Qdrant 類方案**：最適合處理大向量與量化 / on-disk
- **LanceDB**：很適合做嵌入式本地候選，值得實驗
- **Chroma**：適合做 prototype / optional backend，不是第一選擇

#### 第三優先：把 LEANN 當成 retrieval 壓縮實驗，而不是一開始就當核心

如果實驗結果漂亮，它很可能能把：

- semantic search
- recall
- Ask My History 的第一階段召回

做得非常省空間。

但我不會讓整個 insights 體系一開始就建立在 LEANN 這個假設上。

## 7.16 Settings 顯示大小，不是 fallback，而是應該一開始就做

你提到「如果真的沒辦法，就在設置界面顯示數據庫 / embedding / 其他資產大小」。

我想補一句：

> **這不只是退路，而是長壽命產品一開始就應該具備的能力。**

應該至少顯示：

- core archive DB 大小
- content snapshots 大小
- FTS 索引大小
- vector index 大小
- embeddings 依 model / provider 分組大小
- insights / derived state 大小
- snapshots / backups 大小
- 最近 30 / 90 天增長量
- 可清理空間

這件事的價值很高，因為它能直接提升：

- 使用者信任
- 問題診斷
- 空間治理能力
- 「為什麼今天變慢」的可解釋性

對 20-60 年產品而言，這不是 nice-to-have，而是基礎運維能力。

---

## 7.17 DuckDB 很適合做 insights sidecar，但不適合取代 archive core

如果只問一句：

> **DuckDB 適不適合我們？**

我的答案是：

- **非常適合做 analytics / insights sidecar**
- **不適合直接取代我們的主 archive DB**

### 為什麼它很適合 insights

DuckDB 的定位本來就是嵌入式分析型資料庫。官方 concurrency 文檔講得很直接：

- 它優化的是 bulk operations
- many small transactions 不是主要設計目標
- 多進程同寫不是自動支援的主要目標

參考：

- [DuckDB Concurrency](https://duckdb.org/docs/current/connect/concurrency)

這跟我們的需求剛好對上：

- browse / timeline / backup / ingest 是主 archive 的事
- 重掃、聚合、window functions、長時間 OLAP query 是 insights 的事

也就是說，DuckDB 很適合承接：

- session / thread / topic 重新聚合
- domain / tag / cluster 趨勢分析
- 長時間窗口的 cohort / revisit / dwell-time analytics
- embedding-derived aggregates
- 離線 materialized views / feature tables

### 它甚至可以直接讀 SQLite

DuckDB 官方 SQLite extension 支援：

- 直接 attach SQLite 檔案
- 直接讀寫 SQLite tables
- 底層資料在查詢時直接從 SQLite 讀出
- 與 SQLite 的並行存取由 SQLite 的鎖來處理

參考：

- [DuckDB SQLite Extension](https://duckdb.org/docs/current/core_extensions/sqlite)

這一點非常重要，因為它意味著：

> **我們不一定要把 archive 從 SQLite 遷走，也能用 DuckDB 做重 analytics。**

也就是：

- 主 archive 繼續用 SQLCipher / SQLite
- 定期把可分析表 expose 給 DuckDB
- 或增量複製到 DuckDB analytics sidecar

但這裡要保守說一句：

- DuckDB 官方 SQLite extension 文檔明確提到 SQLite / Turso 相容性
- **我沒有找到官方文檔明確承諾 SQLCipher 相容**

因此，在今天這個資訊基礎上，我不會把「DuckDB 直接 attach 我們加密後的主 archive」當成可靠前提。

更穩的做法是：

- 從主 archive 增量匯出 analytics 需要的表
- 或產生只讀 analytics replica / Parquet snapshot 給 DuckDB

### DuckDB 對 encryption at rest 也變得更像樣了

截至 **2025-11-19** 發布的官方資料，DuckDB 自 **1.4.0** 起支援 database encryption。官方 `ATTACH` 文檔寫明：

- 加密覆蓋 main DB
- 也覆蓋 WAL
- 也覆蓋 temporary files
- 預設是 AES-256-GCM

參考：

- [DuckDB ATTACH / Encryption](https://duckdb.org/docs/current/sql/statements/attach)
- [DuckDB Encryption Announcement (2025-11-19)](https://duckdb.org/2025/11/19/encryption-in-duckdb)

但同一份官方文檔也明確寫到：

- **尚未達到官方 NIST requirements**

所以我的判斷是：

- **拿來做本地 analytics sidecar 加密，非常值得考慮**
- **若要把它當唯一核心安全邊界，仍需保守**

### 為什麼我不建議它取代主 archive

原因不是 DuckDB 弱，而是工作負載不對。

對我們這種產品，主 archive 需要：

- 持續小量增量寫入
- browser ingest
- metadata upsert
- 長壽命穩定 canonical store
- 與 app UI 緊密互動

而 DuckDB 官方自己說：

- 許多小交易不是主要設計目標
- 多進程寫入不是主要設計目標

所以把 canonical archive 完全換成 DuckDB，會讓我們在最重要的熱路徑上，逆著它的最佳使用場景走。

### 對 20-60 年問題，它能幫什麼？

DuckDB 能幫的不是「讓 embedding 變小」，而是：

- **把大量 insights query 從主庫搬走**
- **把 expensive analytics 變成 columnar / vectorized workload**
- **讓我們把 derived marts 壓到分析 sidecar**

換句話說，它緩解的是：

- insight 重算卡頓
- 大量 group-by / window / cohort query
- 長時間跨年份聚合

它不能單獨解決：

- embedding 體積
- ANN index 壓力
- 即時 semantic recall

## 7.18 DuckDB 的 FTS / VSS 很有意思，但今天不應該當核心押注

### DuckDB FTS：對 batch analytics 有用，但不適合做主搜尋層

DuckDB 有官方 FTS extension，但官方文檔明確警告：

- **FTS index 不會在表更新時自動更新**
- workaround 是重建 index

參考：

- [DuckDB FTS](https://duckdb.org/docs/current/core_extensions/full_text_search)

所以它比較適合：

- 離線分析
- 小型分析型文本檢索
- 在 DuckDB marts 上做研究性 query

但不適合取代我們產品級、持續變動的主搜尋索引。

### DuckDB VSS：很酷，但離 production-grade desktop vector core 還有距離

DuckDB 官方 `vss` extension 很吸引人，因為它提供：

- HNSW index
- top-k aggregate acceleration
- index-accelerated lateral joins
- fuzzy join / vss_join

參考：

- [DuckDB VSS](https://duckdb.org/docs/current/core_extensions/vss)
- [What's New in DuckDB VSS](https://duckdb.org/2024/10/23/whats-new-in-the-vss-extension)

對 insights 來說，這很有想像空間，因為它可以把很多：

- nearest-neighbor join
- related-session matching
- cluster seed expansion
- document-to-document 相似群組分析

都寫成 SQL。

但官方文檔同時也寫得很明白：

- persistence 目前仍有已知問題
- WAL recovery 對 custom indexes 還不完整
- 官方仍建議不要在 production 使用這個 persistence 路徑
- index 必須能放進 RAM
- persistent index 每次 checkpoint 會整個序列化覆寫

這些限制對我們這種 20-60 年、海量 embedding、本地桌面應用來說太重了。

所以我的判斷是：

- **DuckDB VSS 很適合原型、研究、離線分析**
- **不適合當我們長壽命產品的主向量核心**

## 7.19 2026 年更適合我們 use case 的，不是單一資料庫，而是最佳化組合

這輪看下來，我不認為存在一個「把 SQLite 換掉就全解」的神奇方案。

真正貼合我們 use case 的，是下面這種組合。

### 方案 A：我目前最推薦的組合

1. **Core archive：SQLCipher / SQLite**
   - 最適合 canonical source of truth
   - 最適合小交易、長壽命、本地穩定存放

2. **Insights mart：DuckDB**
   - 專門承接 OLAP / 趨勢 / cohort / aggregate / recompute
   - 可做 rebuildable analytics sidecar

3. **Vector layer：LanceDB 或 Qdrant Edge / Qdrant**
   - 專門承接 ANN / quantization / hybrid retrieval / multivector

4. **Content snapshots：壓縮 sidecar content store**
   - markdown / readable text / chunk store

這個組合的優點是，每個元件都在它擅長的區域工作。

### 方案 B：如果想更激進地押「酷炫 analytics」

可以做：

- SQLite core
- DuckDB insights sidecar
- DuckDB marts 內做一部分 brute-force vector analytics
- 真正 retrieval 還是交給 LanceDB / Qdrant

也就是說，DuckDB 可以參與 embedding 洞察，但不該單獨扛整個向量生命週期。

### 方案 C：未來觀察名單

1. **SQLite Vec1**
   - 如果成熟度上來，它有機會把本地向量能力重新拉回 SQLite 生態

2. **DuckDB VSS**
   - 如果 persistence / WAL / RAM 壓力問題被實質解掉，會很有吸引力

3. **LEANN**
   - 如果我們把它定位成 retrieval layer，而不是萬能 embedding substrate，它可能很有價值

## 7.20 如果你問我「有沒有非常適合我們的 2026 方案」

有，但答案不是某一個單品，而是一個非常清楚的 stack：

> **SQLCipher SQLite 做 archive core，DuckDB 做 insights mart，LanceDB 或 Qdrant Edge 做 vector retrieval sidecar，content snapshots 做壓縮冷存。**

這是我目前看到最符合下面這些條件的組合：

- 本地桌面應用
- 長壽命資料
- 加密 at rest
- 大規模 embedding
- insights 很重
- 不能讓 AI 資產拖垮主 archive

如果硬要在「單一新方案」裡挑一個最值得引進的，我會選：

- **DuckDB，作為 insights sidecar**

如果硬要在「單一向量方案」裡挑一個最值得實驗的，我會先看：

- **LanceDB**

如果硬要在「極度看重向量壓縮 / 邊緣設備」裡挑一個，我會看：

- **Qdrant Edge / Qdrant + quantization**

---

## 8. 最終判斷

### 8.1 Chroma 能不能緩解問題？

可以緩解一部分，特別是把 retrieval workload 與主庫隔離。

但它不是我建議的主方案，因為：

- 它不會自然解決 embedding 太大
- 它不會自動解決長壽命容量治理
- 在我們的桌面 Rust/Tauri 架構裡，它偏向 sidecar server，而不是自然嵌入層
- 本地加密故事沒有 SQLCipher 那麼直接

### 8.2 LEANN 適不適合我們？

值得深度跟進，而且很可能非常適合：

- semantic recall
- RAG 式召回
- 第一階段檢索

但對於：

- insights
- clustering
- topic analytics
- 長期 embedding 衍生分析

它比較像部分解，而不是全部解。

### 8.3 真正穩的方案是什麼？

我現在最有信心的答案是：

> **把 archive core、content snapshots、FTS、vector assets 明確分層；把 embedding 當成可重建 sidecar 資產；對所有 documents 提供便宜的語義層，再對熱資料做升級。**

這樣就算 embedding 很大，也不會：

- 損壞主 archive
- 拖慢主 archive 查詢
- 讓整個資料庫因為 AI 功能而失去可控性

### 8.4 60 年能不能成立？

可以，但前提不是「找到一個神奇向量庫」。

前提是：

- 有正確的資料分層
- 有壓縮
- 有 ANN / quantization
- 有近重複與版本鏈
- 有觀測與清理能力

只靠換成 Chroma，不夠。

只靠 LEANN，也不夠。

**靠的是整體架構收斂。**

---

## 9. 補充參考

- [DuckDB Concurrency](https://duckdb.org/docs/current/connect/concurrency)
- [DuckDB SQLite Extension](https://duckdb.org/docs/current/core_extensions/sqlite)
- [DuckDB ATTACH / Encryption](https://duckdb.org/docs/current/sql/statements/attach)
- [DuckDB Encryption Announcement (2025-11-19)](https://duckdb.org/2025/11/19/encryption-in-duckdb)
- [DuckDB FTS](https://duckdb.org/docs/current/core_extensions/full_text_search)
- [DuckDB VSS](https://duckdb.org/docs/current/core_extensions/vss)
- [What's New in DuckDB VSS](https://duckdb.org/2024/10/23/whats-new-in-the-vss-extension)
- [Chroma Clients](https://docs.trychroma.com/docs/run-chroma/clients)
- [Run a Chroma Server](https://docs.trychroma.com/docs/cli/run)
- [Adding Data to Chroma Collections](https://docs.trychroma.com/docs/collections/add-data)
- [Chroma Schema](https://docs.trychroma.com/reference/typescript/schema)
- [Qdrant Storage](https://qdrant.tech/documentation/storage/)
- [Qdrant Quantization](https://qdrant.tech/documentation/manage-data/quantization/)
- [Qdrant Security](https://qdrant.tech/documentation/guides/security/)
- [Qdrant Edge Quickstart](https://qdrant.tech/documentation/edge/edge-quickstart/)
- [LanceDB Quickstart](https://docs.lancedb.com/quickstart)
- [LanceDB Indexing Overview](https://docs.lancedb.com/indexing)
- [LanceDB Vector Indexes](https://docs.lancedb.com/indexing/vector-index)
- [LanceDB Quantization](https://docs.lancedb.com/indexing/quantization)
- [LanceDB Enterprise Security](https://docs.lancedb.com/enterprise/security)
- [SQLite Vec1](https://sqlite.org/vec1)
- [Vec1 Introduction](https://sqlite.org/vec1/doc/trunk/doc/vec1intro.md)
- [LEANN Paper](https://arxiv.org/abs/2506.08276)
- [LEANN GitHub](https://github.com/yichuan-w/LEANN)
