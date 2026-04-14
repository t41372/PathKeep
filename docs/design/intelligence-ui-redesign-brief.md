# Intelligence UI Redesign Brief

> 這份 brief 是給設計師重做 intelligence 介面的輸入文件。  
> 它描述的是 **現在產品實際已有的 intelligence 能力，要怎麼重新整理成好懂、好用、可解釋的 UI**。  
> 它不是要發明一套完全脫離現實的「AI 感」介面。

---

## 1. 先讀什麼

開始畫之前，請先看：

1. [../features/intelligence-current-state.md](../features/intelligence-current-state.md)
2. [../features/deterministic-intelligence.md](../features/deterministic-intelligence.md)
3. [../features/intelligence.md](../features/intelligence.md)
4. [screens-and-nav.md](screens-and-nav.md)

---

## 2. 這次 redesign 的範圍

主要重做：

- `Insights` route 主頁
- Dashboard 上 intelligence 相關入口與摘要區塊
- Insights 與 Explorer / Assistant / Jobs 的 deep-link grammar
- Insights explainability surface

這次不主張一起重做：

- Explorer 的完整 canonical results UI
- Assistant 對話體驗本身
- Settings 全頁 IA

但新的 Intelligence UI 必須和這些頁面接得上。

---

## 3. 為什麼要重做

現在的 intelligence 介面有三個根本問題。

### 3.1 真的有很多資訊，但沒有把主次講清楚

使用者打開 Insights 之後，同時看到：

- overview hero
- runtime digest
- On This Day
- Site Analytics
- storage analytics
- periodic summaries
- query groups
- topic timeline
- query evolution
- reference pages
- source effectiveness
- deterministic module health
- insight cards
- explainability panel

問題不是這些東西不該存在。  
問題是它們現在太常用相同的 panel 語法與相近的視覺重量出現，導致頁面沒有很清楚地回答：

- 這次我最該先看哪一件事？
- 今天的重點是什麼？
- 哪些是分析結論，哪些是系統健康？

### 3.2 backend 最有價值的結構沒有被 UI 正確放大

現在最有價值的 deterministic structure 其實是：

- query groups
- threads
- reference pages

但現行 UI 對 threads 的呈現很弱，topic 與 card 反而更顯眼。  
這使得產品看起來像很多聰明小卡片，而不是一套能幫人重新理解自己研究脈絡的工具。

### 3.3 runtime 與 analysis 仍然互相打架

產品現在很誠實，這是優點。  
但誠實不代表要把 queue、retry、cancel、plugin status 全部擠進 analysis 主頁的注意力中心。

現在的設計需要做的是：

- 保留 honesty
- 但把 runtime review 放回正確層級

也就是：

- `Insights` 以「看懂結果」為主
- `Jobs` 以「處理背景工作」為主

---

## 4. 這個頁面必須回答什麼問題

使用者在 10 秒內應該得到這些答案：

1. 最近這段時間，我最值得注意的研究重點是什麼？
2. 這些結論是根據哪些行為算出來的？
3. 如果我要繼續追，下一步去哪裡？
4. 現在這些洞察新不新鮮？有沒有還在重建？

如果一個版本的設計讓使用者先看到的是：

- 大量 controls
- 大量機器狀態
- 一排排同權重卡片

那就是設計順序錯了。

---

## 5. 設計目標

### 5.1 讓 Insights 真正變成「分析頁」，不是 runtime 牆

Insights 應該先顯示：

- 這段時間的重點
- 研究線
- 搜尋演化
- 穩定參考來源

而不是先顯示整頁 queue / plugin / module 運維資訊。

### 5.2 把 thread 拉回第一排角色

現在 thread 是 backend 核心概念，但 UI 幾乎退居幕後。  
這一輪 redesign 應該考慮：

- thread 要不要成為獨立 section
- query groups 與 thread 怎麼形成層級關係
- topic 是否只做概覽，而不是主體

### 5.3 保留可解釋性，但用更自然的閱讀節奏

每個重大 insight 都仍然要能：

- explain
- show evidence
- jump to Explorer

但 explain 不應該讓整頁變成技術審計台。  
它應該像「展開這個結論的由來」，而不是另一個工程診斷頁。

### 5.4 shared profile scope 必須永遠誠實

如果使用者現在看的是某個 profile scope：

- UI 必須一眼講清楚這是 scoped view
- 哪些數字仍然是 archive-wide，也要明講

---

## 6. 非目標

這次 redesign 不要做：

- 假裝有大量生成式摘要其實 backend 沒有
- 假裝 topic 是完整 semantic workspace
- 用「時間花在哪裡」這種不誠實語法包裝 deterministic data
- 把 Jobs 的完整操作搬回 Insights
- 在主產品介面突出內部版本號、plugin runtime marker、milestone code name

---

## 7. 建議的頁面資訊架構

下面不是唯一答案，但應該是設計探索的起點。

### Layer 1 — Snapshot

第一屏先回答：

- 這段時間的分析範圍
- 現在資料是否新鮮
- 最重要的 1-2 個洞察 headline

建議內容：

- page title
- profile scope honesty
- generated at / freshness
- compact runtime digest
- 1 個 primary CTA：`Refresh`
- 1 個 secondary CTA：`Open Jobs`

### Layer 2 — Highlights

這層是「今天先看什麼」。

建議內容：

- On This Day
- Periodic / contrastive summary
- 1-2 張真正值得看的 highlight cards

這一層應該最像產品，而不是最像資料結構。

### Layer 3 — Research Lines

這層是 intelligence 的核心。

建議內容：

- threads
- query groups
- topic overview

推薦邏輯：

- `thread` 當主體
- `query group` 當 thread 的展開或配套結構
- `topic timeline` 當高階概覽，而不是唯一主角

### Layer 4 — Search Behavior

建議內容：

- query evolution / ladders
- search refinement patterns

這一層應該幫使用者理解「我是怎麼把問題越問越具體」。

### Layer 5 — Evidence Library

建議內容：

- reference pages
- source effectiveness

這層比較像「你反覆依賴哪些頁面與網站」。

### Layer 6 — Health

建議內容：

- deterministic module status
- coverage
- storage analytics
- growth signal

這層要保留，但應該是頁面後段或 secondary tab，不應和 highlights 搶第一視線。

---

## 8. 推薦的主要交互模型

### 模型 A：單頁分層

適合：

- 保留現有 route 結構
- 快速迭代

做法：

- 同一頁，但用更清楚的 section hierarchy
- thread / query group 可以用 expandable rows
- explain 用 side panel / drawer

### 模型 B：總覽 + drilldown

適合：

- thread 想變成一等公民
- 想把單頁壓力降下來

做法：

- Insights 首頁只做總覽
- 點 thread / group / reference page 進 detail drawer 或 detail subview

目前我更推薦 **模型 B**。  
原因是現在 backend 已經有 `thread detail` 結構，但 UI 還沒真正用起來。

---

## 9. 每個主要區塊的內容契約

### 9.1 Snapshot hero

必須包含：

- time window
- profile scope
- generated at
- freshness / queued rebuild honesty

不應包含：

- 整排 runtime 操作按鈕
- plugin 細節
- 長段 queue 文案

### 9.2 Highlight cards

必須包含：

- 標題
- 短摘要
- 為什麼值得看
- 可 explain

不應包含：

- 過多 raw metrics
- 工程狀態語言

### 9.3 Threads

應成為主 section 候選。

每個 thread row 至少可包含：

- title
- last seen
- query group count
- reopen count
- confidence / signal strength
- explain
- open in Explorer

### 9.4 Query groups

每個 row 至少可包含：

- root query
- latest query
- step count
- confidence
- evidence tier
- explain
- open in Explorer

### 9.5 Topic timeline

請把它當「概覽圖」設計，不要當「完整 topic workbench」設計。

現在它實際上的資料只有：

- label
- visit count
- revisit count
- trend score
- burst score

所以不要假設已經有高精度 topic genealogy。

### 9.6 Evidence library

reference pages 與 source effectiveness 建議並排或分頁：

- `Reference pages`
  - 強調具體頁
- `Source effectiveness`
  - 強調網站角色

### 9.7 Explainability

建議用 drawer 或 side panel，不要直接把頁面拉長成另一段報告。

必須包含：

- explanation text
- citations
- deep-link to Explorer

---

## 10. 狀態設計要求

設計稿必須覆蓋：

- zero data
- new archive
- scoped view
- AI disabled
- deterministic rebuild queued
- deterministic rebuild running
- runtime failed but insights still可讀
- explanation failed
- storage / growth signal unavailable

---

## 11. 文案與語氣要求

### 必須做到

- 用人話講清楚結論
- 把 `why this matters` 放前面
- 讓 CTA 看 label 就知道會做什麼
- 對 scoped view、archive-wide metrics、rebuild freshness 講清楚

### 不要再出現

- 內部版本字串
- 過度技術化 raw status 當主 copy
- 空泛的「智能分析中」「正在處理」沒有上下文
- 誇大 deterministic 能力的語氣

---

## 12. User Stories

### Core

- 作為一個回來看舊研究的使用者，我想一打開就知道最近最值得回看的主題是什麼，而不是先讀一整頁系統狀態。
- 作為一個想搞懂自己研究脈絡的使用者，我想看見哪些 query 其實屬於同一條研究線，這樣我才知道自己是怎麼一步一步走到現在的。
- 作為一個不信黑盒結論的使用者，我想知道每個洞察是根據哪些真實瀏覽紀錄推來的，這樣我才會信任它。
- 作為一個只看某個 profile 的使用者，我想一眼知道現在是不是 scoped view，以及哪些數字沒有跟著 scope 切換。

### Secondary

- 作為一個沒有開 AI 的使用者，我仍然想從 Insights 得到有價值的結論，而不是看到一堆 disabled state。
- 作為一個想繼續追某條線索的使用者，我希望能從 card / thread / group 一步跳回 Explorer。
- 作為一個在重建期間使用產品的人，我希望知道資料是不是還在刷新，但不想被一整頁 queue 打斷閱讀。

---

## 13. 驗收標準

新的 Intelligence UI 至少要做到：

- 使用者 10 秒內能分辨 `重點洞察`、`研究線`、`系統健康`
- thread 成為明確可見的一等概念，不能只剩一個 count
- runtime digest 明顯變次級，不再像第二個 Jobs
- explainability 的入口與結果更自然
- scoped vs archive-wide honesty 變得一眼可見
- no-AI mode 依然看起來完整有價值

---

## 14. 交付給設計師的建議產物

- desktop high-fidelity main view
- thread detail / explain drawer
- scoped view state
- zero-data / rebuilding / failed state
- component inventory
- content hierarchy annotations

---

## 15. 與 Jobs 的邊界

為了避免兩頁繼續互相污染，請設計時堅守這條線：

- `Insights` 回答：我看到了什麼、為什麼值得看、去哪裡繼續追
- `Jobs` 回答：現在系統在做什麼、哪裡需要我處理、哪個工作卡住了

如果一個稿看起來把這兩頁混成一頁，那就偏掉了。
