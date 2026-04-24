# Jobs UI Redesign Brief

> 這份 brief 是給設計師重做 `Jobs` 介面的輸入文件。  
> 目標不是把它做得更像監控後台，而是把它做成一個使用者能快速 triage、快速判斷、快速處理的 review surface。

---

## 1. 先讀什麼

開始畫之前，請先看：

1. [../features/intelligence-current-state.md](../features/intelligence-current-state.md)
2. [../features/intelligence.md](../features/intelligence.md)
3. [screens-and-nav.md](screens-and-nav.md)

---

## 2. Jobs 頁現在在產品裡的角色

Jobs 不是 debug page。  
它是正式 shipping route，負責讓使用者看懂：

1. 現在背景到底在做什麼
2. 哪些只是排隊，哪些真的失敗
3. 哪些地方需要我處理
4. 如果剛剛 app 中斷，系統恢復到了哪裡

它要和：

- sidebar footer background strip
- Insights 頂部 runtime digest
- Maintenance derived-state panel

一起形成同一套 queue grammar。

---

## 3. 為什麼現在的 Jobs UI 讓人痛苦

### 3.1 它其實有很多正確資料，但沒有做出清楚 triage

現在頁面上同時有：

- page status callout
- overview hero
- AI queue summary
- derived-data queue summary
- readable-content backlog
- title normalization
- deterministic modules
- recovery notes
- plugin runtime status
- module runtime status
- recent AI jobs
- recent derived-data jobs

問題不是資料不夠。  
問題是使用者打開後不一定能立刻回答三個最重要的問題：

- 什麼正在跑
- 什麼只是排隊
- 什麼需要我處理

### 3.2 兩套 queue plane 對一般使用者不夠直覺

現在真相是：

- `ai_jobs`
- `intelligence_jobs`

這兩套都存在。

但大多數使用者不想知道資料表名稱。  
他們想知道的是：

- AI 問答有沒有卡住
- deterministic rebuild 有沒有完成
- 網頁正文抓取為什麼還很多排隊

### 3.3 readable-content backlog 容易看起來像「全部失敗」

實際上很多情況是：

- deterministic rebuild 先跑
- network fetch 後補
- 只有少部分 failed

但如果設計沒有把 `deferred backlog` 和 `failed items` 清楚分開，使用者會以為整套系統都壞了。

### 3.4 頁面像 operations dump，不像 review surface

現在太多區塊都長得像：

- 一個 panel
- 一組 counts
- 幾句說明

結果是所有訊息都在同一層級講話。

---

## 4. Jobs 頁必須先回答的問題

使用者一打開 Jobs，5 秒內應該知道：

1. **現在正在做什麼**
2. **哪裡需要我處理**
3. **哪些工作只是等一下才會做**

如果做不到這三點，就算所有資料都有，也是失敗的 UI。

---

## 5. 設計目標

### 5.1 triage first

頁面第一屏必須明確區分：

- Running now
- Needs review
- Deferred backlog

### 5.2 用人話描述 queue，而不是先暴露原始狀態碼

例如：

- `PDF / sign-in redirect / rate limit`

應該比：

- `unsupported-content`
- `fetch-error`

更早被看到。

### 5.3 讓 deterministic rebuild 成為真正的主線工作

因為現在產品邏輯上：

- deterministic rebuild 是 baseline intelligence
- readable-content refetch 是後補 enrichment

所以視覺上不能讓大批 network backlog 搶走 deterministic rebuild 的主角位置。

### 5.4 把「系統健康」和「最近活動」分層

plugin / module health 很重要。  
但它不該跟「現在這一刻最該處理哪個 job」混在一起。

---

## 6. 非目標

這次 redesign 不要做：

- 把 Jobs 做成工程監控 dashboard
- 露出更多內部 ID / 版本號 / runtime marker 當主文案
- 把 Settings 的所有控制搬來 Jobs
- 把 AI queue 與 derived-data queue 硬湊成一張毫無邊界的大表

---

## 7. 建議的資訊架構

### Layer 1 — Current Focus

這一層要非常明確地回答：

- 現在最重要的 active work 是什麼
- 它走到哪一步了
- 還要不要我介入

建議內容：

- 一張主 hero
- 主進度條或 phase indicator
- 一個 primary active job
- 一句非常白話的說明

### Layer 2 — Needs Review

這一層只放：

- failed
- retryable
- cancelled but unfinished implications

它應該是醒目的，但數量要可壓縮。  
不要讓整頁一打開就像 20 個失敗警報。

### Layer 3 — Waiting / Deferred

這一層專門處理：

- queued
- paused
- deferred network fetch

尤其 `readable-content-refetch` 大 backlog，應該先被包裝成「後補工作池」，而不是失敗牆。

### Layer 4 — Queue Families

這一層才展開不同工作家族，例如：

- Deterministic rebuild
- Readable content fetch
- Title normalization
- Assistant / AI jobs

### Layer 5 — Health And Recovery

這一層放：

- plugin runtime status
- module runtime status
- restart / recovery notes

### Layer 6 — Recent History

這一層放：

- recent AI jobs
- recent runtime jobs

這是「我想回頭查剛剛做過什麼」時才需要的層，不應搶第一屏。

---

## 8. 推薦的主介面模型

### 模型 A：三段式 triage dashboard

上半部三塊：

- Running now
- Needs review
- Waiting

下半部再展開 families / health / recent history。

### 模型 B：inbox workflow

最上面先是一個「待處理事項收件匣」：

- failed jobs
- blocked jobs
- retry suggestions

再往下才是 background overview。

目前我比較推薦 **模型 A**，因為它比較符合現在已有的 queue grammar，也比較容易和 sidebar / Insights digest 對齊。

---

## 9. 每個區塊的內容契約

### 9.1 Current focus hero

必須包含：

- 正在跑的 job family
- phase / progress
- 粗粒度 percent 或 current / total
- 人話 detail
- pause / resume 或 open details

不應包含：

- 大量 secondary job list
- 內部 runtime note 當主標題

### 9.2 Needs review

每筆至少顯示：

- 問題是什麼
- 為什麼會這樣
- 可不可以 retry
- retry 會影響什麼

### 9.3 Waiting / deferred

這區要特別清楚區分：

- queued because workers are busy
- queued because queue is paused
- deferred because deterministic rebuild has priority

### 9.4 Queue family cards

每個 family 至少可包含：

- boundary
- queued / running / failed
- last completed
- last error summary

### 9.5 Module / plugin health

這區是 secondary review layer。

module 至少顯示：

- status
- last built
- stale reason
- derived tables

plugin 至少顯示：

- local or network
- queue counts
- stored records
- last completed

### 9.6 Recent history

近期 job list 應更像「活動紀錄」，不是主頁主體。

每列至少顯示：

- summary
- state
- timestamp
- retry / cancel if valid

---

## 10. 重要的文案約束

### 必須做到

- 失敗原因優先翻成人話
- queue paused / queued / running 的差別要一眼看懂
- 對 deferred backlog 要明講「不是失敗」
- 對 running job 要明講「現在卡在哪一段」

### 不要再做

- 用 raw status 直接當主文案
- 用單一句「正在處理」打發所有長任務
- 讓使用者自己推理 deterministic rebuild 和 network fetch 的優先關係

---

## 11. User Stories

### Core

- 作為一個普通使用者，我想一打開 Jobs 就知道現在系統是在重建洞察、抓正文、還是在處理 AI 問答。
- 作為一個想判斷是否需要介入的使用者，我想快速分辨哪些工作只是排隊，哪些是真的需要我重試或修設定。
- 作為一個看到大量正文抓取 backlog 的使用者，我想知道這是不是正常的後補工作，而不是以為整個功能全都失敗了。
- 作為一個在 app 意外中斷後回來的使用者，我想知道系統有沒有幫我恢復先前中斷的工作。

### Secondary

- 作為一個想確認 deterministic rebuild 是否仍在前進的使用者，我想看到 phase、heartbeat、粗粒度進度，而不是只有轉圈。
- 作為一個想調整背景工作節奏的使用者，我想在同一頁 pause / resume，不必去別處找。
- 作為一個需要 support 的使用者，我想看到 recent jobs 與 human-readable error，但不想先被系統細節淹沒。

---

## 12. 必須覆蓋的狀態

- archive not initialized
- archive locked
- queue paused
- running deterministic rebuild
- large readable-content backlog with few failures
- mixed running + failed state
- no jobs
- recovered-after-restart note
- retryable failed jobs
- cancellable running jobs

---

## 13. 與其他頁面的邊界

### 與 sidebar footer 的關係

sidebar footer 只做：

- compact summary
- one-line status
- quick jump

Jobs 才做完整 triage。

### 與 Insights 的關係

Insights 只保留：

- runtime digest
- open Jobs

完整 retry / cancel / queue family review 應留在 Jobs。

### 與 Settings 的關係

Settings 保留：

- enable / disable
- rebuild / clear
- provider config
- integration preview

Jobs 不需要重新變成第二個 Settings。

---

## 14. 驗收標準

新的 Jobs UI 至少要做到：

- 使用者 5 秒內能回答 `現在在做什麼`
- 使用者 5 秒內能回答 `哪裡需要我處理`
- 大量 deferred backlog 不再被看成全面失敗
- deterministic rebuild 明顯成為第一優先的背景工作
- plugin / module / recent history 被放到正確層級

---

## 15. 建議交付產物

- desktop high-fidelity main Jobs page
- running / failed / paused / idle variants
- backlog-heavy scenario
- restart recovery scenario
- component inventory
- content hierarchy annotations
