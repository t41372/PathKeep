# INTELLIGENCE CURRENT STATE — 2026-04-13 Shipping Truth

> 這份文檔不是在描述「理想上想做什麼」，而是在描述 **PathKeep 現在實際上已經做了什麼、怎麼做、哪些地方只做到一半、哪些舊文檔已經不夠準**。  
> 如果你要重新設計 intelligence UI，或要先搞清楚現在 repo 裡的 intelligence 到底是什麼，先讀這份。

---

## 1. 先講結論

PathKeep 現在的 intelligence，不是一個單一功能，而是三層東西疊在一起：

1. **deterministic intelligence baseline**
   - 不靠 LLM、不靠 embedding，也能提供可 shipping 的洞察。
   - 這是現在真正的基線，也是 M5 已接受的正式方向。
2. **optional AI layer**
   - semantic search、assistant、MCP、provider 測試、index build。
   - 這些功能存在，而且 default desktop build 會一起 shipping，但它們預設是 optional。
3. **runtime / queue / review layer**
   - 背景重建、plugin runtime、module runtime、retry / cancel / pause / resume、recovery。
   - 這一層不是 debug 小工具，而是正式產品 surface。

換句話說，現在的 intelligence 不是「只有 Insights 頁」。  
它實際上分散在：

- Dashboard
- Explorer
- Assistant
- Insights
- Jobs
- Settings
- sidebar footer background strip

---

## 2. 設計師應該先看哪些文檔

### 如果你要理解「現在產品實際上長什麼樣」

建議閱讀順序：

1. [intelligence-current-state.md](intelligence-current-state.md)
2. [deterministic-intelligence.md](deterministic-intelligence.md)
3. [intelligence.md](intelligence.md)
4. [../design/screens-and-nav.md](../design/screens-and-nav.md)
5. [../architecture/decisions/006-deterministic-intelligence-boundary.md](../architecture/decisions/006-deterministic-intelligence-boundary.md)
6. [../architecture/data-model.md](../architecture/data-model.md)
7. [../architecture/desktop-command-surface.md](../architecture/desktop-command-surface.md)

### 如果你要開始做 intelligence UI redesign

在上面的閱讀順序之外，再讀：

1. [../design/intelligence-ui-redesign-brief.md](../design/intelligence-ui-redesign-brief.md)
2. [../design/jobs-ui-redesign-brief.md](../design/jobs-ui-redesign-brief.md)

### 每份文檔現在適合拿來做什麼

| 文檔                                          | 建議用途                                                    | 現況                                |
| --------------------------------------------- | ----------------------------------------------------------- | ----------------------------------- |
| `docs/features/deterministic-intelligence.md` | deterministic baseline 的正式 source of truth               | **目前最準**                        |
| `docs/features/intelligence.md`               | optional AI / assistant / MCP / queue / shipping truth note | **可用，但混有 legacy 段落**        |
| `docs/design/screens-and-nav.md`              | 畫面 IA、導航、queue grammar、Insights / Jobs 分工          | **大致準確**                        |
| `docs/architecture/decisions/006-...`         | 為什麼 deterministic baseline 要這樣切                      | **準確**                            |
| `docs/plan/m3-*` / `docs/plan/m5-*`           | 歷史 WBS 與 closeout 背景                                   | **拿來看脈絡，不要當 feature spec** |

---

## 3. 現在我們說的 intelligence，到底是在做什麼

白話一點說，PathKeep 現在的 intelligence 目標不是「幫你做很玄的 AI 分析」。

它實際上在做三件更務實的事：

1. **把一堆零碎瀏覽紀錄整理成比較像人能讀的結構**
   - 例如：這些搜尋是同一條問題的演化、這幾個頁面其實是同一條研究線、這個頁面是你一直回來看的穩定參考頁。
2. **把背景重建與補資料過程變得誠實**
   - 例如：現在還在排隊、剛剛卡在哪一段、哪個 plugin 失敗、哪個 job 可以重試。
3. **在有 AI 時加值，但沒有 AI 也不能整塊變空**
   - 所以 deterministic surfaces 必須能自己成立，LLM 和 embedding 只是加法，不是地基。

---

## 4. 當前 intelligence 功能總表

下面這張表是「實際有沒有做」的盤點，不是理想藍圖。

| 能力                                      | 使用者在哪裡看到                              | 目前狀態                           | 實際做法                                                                       |
| ----------------------------------------- | --------------------------------------------- | ---------------------------------- | ------------------------------------------------------------------------------ |
| On This Day                               | Dashboard、Insights                           | **Shipping**                       | 用 canonical history 做本地日曆日比對，只回看過去年份                          |
| Site Analytics                            | Insights                                      | **Shipping**                       | 讀 `canonical.topDomains`，顯示 top domains 與 deep-link                       |
| Periodic Summary                          | Dashboard、Insights                           | **Shipping**                       | deterministic template summary，沒有 AI 也會顯示                               |
| Contrastive Summary                       | Insights                                      | **Shipping**                       | 比較當前視窗與前一個等長視窗                                                   |
| Topic Timeline                            | Insights                                      | **Shipping，但不是舊文檔寫的那套** | 目前是 deterministic topic aggregation，不是 embedding cluster + LLM naming    |
| Query Groups                              | Insights                                      | **Shipping**                       | 用 search evidence、token overlap、landing continuity 分群                     |
| Query Ladders / Query Evolution           | Insights                                      | **Shipping**                       | 從 query group 的 steps / stages 產生                                          |
| Threads / Open Loop                       | backend、Insights cards / stats / explain     | **部分前台呈現**                   | backend 有完整 thread summary / detail；UI 目前沒有 thread list 作為主 section |
| Reference Pages                           | Insights                                      | **Shipping**                       | 找被跨 query group / thread 重用的穩定頁面                                     |
| Source Effectiveness                      | Insights                                      | **Shipping**                       | 看某個來源是否常成為穩定落點                                                   |
| Insight Cards                             | Insights                                      | **Shipping**                       | 從 summary / open loop / reference page 產生可 explain 的卡片                  |
| Explainability                            | Insights                                      | **Shipping**                       | deterministic explanation + citations，不靠 LLM                                |
| Profile-scoped insights                   | shell scope + Insights / Assistant / Explorer | **Shipping**                       | shared scope 會影響 insight fetch 與 deep-link                                 |
| Storage analytics                         | Dashboard、Insights                           | **Shipping**                       | 用 dashboard storage snapshot，不是從 deterministic pipeline 現場重算          |
| Latest growth signal                      | Insights、Settings                            | **Shipping**                       | 連回 Audit run                                                                 |
| Semantic search                           | Explorer                                      | **Shipping，optional**             | semantic / hybrid / keyword，同一路由                                          |
| AI Assistant                              | Assistant                                     | **Shipping，optional**             | queue-backed assistant with citations                                          |
| AI provider test                          | Explorer、Assistant、Settings                 | **Shipping**                       | test provider connection，不只是 boolean                                       |
| AI queue review                           | Explorer、Assistant、Jobs、sidebar            | **Shipping**                       | `ai_jobs` queue read model                                                     |
| Deterministic / enrichment runtime review | Insights、Jobs、Settings、sidebar             | **Shipping**                       | `intelligence_jobs` + module / plugin runtime read model                       |
| Sidebar background strip                  | shell footer                                  | **Shipping**                       | compact queue summary + progress bar + Jobs link                               |
| Jobs page                                 | `/jobs`                                       | **Shipping**                       | 專門看 background work、retry、cancel、recovery                                |
| Settings derived-state panel              | Settings                                      | **Shipping**                       | plugin / module / runtime / rebuild / clear review surface                     |
| Workflow map                              | backend snapshot                              | **backend-only**                   | snapshot 有資料，但當前 Insights UI 沒有 render                                |
| Profile facets                            | backend snapshot                              | **backend-only**                   | snapshot 有資料，但當前 Insights UI 沒有 render                                |
| Thread detail route/surface               | backend command                               | **backend-ready, UI-light**        | `load_thread_detail` 已存在，但目前前台不是一等公民 surface                    |

---

## 5. 沒有 LLM、沒有 embedding 時，現在到底有哪些洞察

這一段只講 deterministic intelligence，也就是你特別關心的那層。

### 5.1 On This Day

設計目的：

- 讓使用者快速回看「過去幾年的同一天，我那時候在看什麼」。
- 它是一個回顧卡，不是 timeline 全覽。

現在的實際行為：

- 只看過去幾年的同一天。
- 不會把當前年份今天的紀錄混進來。
- 用本地 timezone 的日曆日，不是 UTC 假裝對齊。
- Dashboard 和 Insights 都會用到它。
- 每一筆都可以 deep-link 回 Explorer。

這張卡現在是可信的 deterministic surface，不需要 AI 才成立。

### 5.2 Site Analytics

設計目的：

- 回答「最近這段時間，我主要在哪些網站花力氣」。
- 它不是做人格分析，也不是判斷網站好壞。

現在的實際行為：

- 顯示最近視窗內的 top domains。
- 以簡單排名與條形寬度呈現相對量級。
- 點 domain 可以回 Explorer 重新看 evidence。

這是 canonical summary，不靠 AI。

### 5.3 Periodic Summary

設計目的：

- 用人能直接讀懂的段落，幫使用者知道最近一段時間大概在研究什麼。

現在的實際行為：

- backend deterministic 直接產出 `periodic-summary`。
- 也會產出 `contrastive-summary`，用來講「這段時間和前一段時間比，有沒有變化」。
- 即使 AI 關掉，summary 仍會顯示。
- 有 AI 時，未來可以再把 deterministic output 改寫得更像人話，但基底不是 AI。

### 5.4 Query Groups

設計目的：

- 把一次問題搜尋與後續點進去看的頁面，收斂成同一組研究動作。
- 不再把每個搜尋頁、每個 landing page 都看成互相無關。

現在的實際行為：

- 優先吃 canonical search term 與 search-result URL 的 query。
- 也看 query token overlap、referrer / redirect chain、landing continuity。
- 會記住：
  - root query
  - latest query
  - steps
  - stages
  - confidence
  - evidence tier
- Insights 頁直接把它當一個正式 section 顯示。

這是目前 deterministic baseline 最重要的 shipping 單位之一。

### 5.5 Query Ladders / Query Evolution

設計目的：

- 讓使用者回看「我的問題是怎麼一步一步變具體的」。

現在的實際行為：

- 從 query groups 裡的 `steps` / `stages` 生成 ladder。
- UI 會把 stage 翻成人可讀 label。
- Chromium search term evidence 最強，其它來源如果只是從 URL 猜 query，可信度要低一級看待。
- 現在已能 deep-link 回 Explorer。

### 5.6 Topic Timeline

設計目的：

- 回答「我最近在關注哪些主題」。

現在的實際做法，和舊文檔最不同：

- **現在不是 embedding 聚類加 LLM 命名。**
- 現在是用 thread title、query group title 的 token similarity 做 deterministic aggregation。
- 換句話說，topic 目前比較像「從現有 thread / query group 摘出來的主題彙整」，不是完整的 semantic topic clustering 系統。
- UI 會用簡單條形與 count 呈現，不是高解析度時間序列圖。

所以如果設計師要重做這個區塊，不能拿舊文檔那種「向量 topic cluster」去設計過度複雜的操作模型。

### 5.7 Threads / Open Loops

設計目的：

- 把跨 burst、跨 query group、可跨天 reopen 的研究線收斂起來。
- 幫使用者看出哪些問題是反覆回來看的，而不是一次就結束。

現在的實際行為：

- backend 會建立 thread summary、thread detail、open-loop score、confidence、evidence tier。
- Insights page 目前會用：
  - thread count
  - open-loop card
  - explainability
  - reference/source surfaces
    來間接表達 thread 結果。
- **但目前前台沒有把 threads 做成一個真正的一等 section。**

這是現在 UI 與 backend 之間最值得設計重做的一個缺口。

### 5.8 Reference Pages

設計目的：

- 找出那些你會反覆回來看的高價值頁面。
- 它們通常是文件、repo、討論串、工具頁、文章，而不是一時點進去的路過頁。

現在的實際行為：

- 看 revisit 次數、cross-day revisit、出現在多少個 query groups / threads。
- Insights 頁把它做成正式 section。
- 每一列都可以 explain，也可以回 Explorer。

### 5.9 Source Effectiveness

設計目的：

- 回答「哪些網站對你來說常常真的有幫助，而不是只有被你打開很多次」。

現在的實際行為：

- 看它是不是 query group 後段的穩定落點。
- 看它是不是被 reference pages 與 reopen support 反覆支撐。
- 現在 UI 會顯示 domain、source role、group / reference / landing counts。

### 5.10 Insight Cards

設計目的：

- 讓使用者不用讀完整頁，就先抓到幾個值得看的重點。

現在的實際行為：

- 從 template summaries 先取前幾張 card。
- 額外補：
  - open-loop card
  - reference-page card
- 每張卡都能 explain。

### 5.11 Explainability

設計目的：

- 不讓洞察變成一句「看起來很合理但不知道從哪來」的黑盒話術。

現在的實際行為：

- `thread`
- `query-group`
- `reference-page`
- `template-summary`
- `topic`
- 一般 `card`

都可以拿到 deterministic explanation。

而且目前 explain 是：

- `usedLlm = false`
- 有 citation
- 有 notes

也就是說，這層 explainability 現在是可 shipping 的 deterministic contract。

---

## 6. optional AI intelligence 現在做到哪

### 6.1 Semantic Search

現在在 Explorer 裡。

使用者可切：

- `keyword`
- `semantic`
- `hybrid`

現在實際做法：

- semantic retrieval 先查 LanceDB sidecar。
- sidecar 不可用就誠實退回 lexical。
- 不允許在 request path 偷偷依賴 SQLite metadata 當假 semantic。
- result 會顯示：
  - score band
  - match reason
  - visited time
  - profile
  - deep-link

### 6.2 AI Assistant

現在是 queue-backed，不是同步黑盒聊天。

它的真實狀態有：

- queued
- completed
- insufficient-evidence
- failed
- cancelled

UI 目前也真的把這些 state 做出來了，而且保留：

- jobId
- runId
- providerId
- embeddingProviderId
- citations

### 6.3 MCP / integration preview

現在主要在 Settings 做 preview。

它的定位是：

- 給使用者 review command / JSON / skill markdown / manual steps
- 不是假裝已自動安裝

---

## 7. intelligence runtime 現在的真實樣子

這一層很重要，因為現在很多 UI 痛點不是演算法本身，而是 runtime review 沒被整理好。

### 7.1 其實有兩套 queue

#### AI queue

用途：

- semantic index build / clear
- assistant jobs

資料表：

- `ai_jobs`

前台 surface：

- Explorer runtime panel
- Assistant
- Jobs page
- sidebar strip（聚合後）

#### intelligence runtime queue

用途：

- deterministic rebuild
- enrichment plugin jobs

資料表：

- `intelligence_jobs`
- `deterministic_module_runtime`

前台 surface：

- Insights runtime digest
- Jobs page
- Settings derived-state panel
- sidebar strip（聚合後）

這兩套 queue 同時存在，是現在產品真相。  
如果未來 UI 要重做，不能把它們硬藏成一套「萬用背景任務」而失去邊界，也不能讓使用者被兩套 queue 的細節淹死。

### 7.2 built-in enrichment plugins

現在正式 shipping 的 built-ins 只有兩個：

1. `title-normalization`
   - local-only
   - 目的是把 noisy title 收斂成更穩定 evidence label
2. `readable-content-refetch`
   - network-backed
   - 目的是補可讀正文與 site-adapter evidence

它們都屬於 derived state runtime，不改 canonical archive facts。

### 7.3 deterministic modules

現在正式 shipping 的 built-ins 有五個：

1. `query-groups`
2. `threads`
3. `reference-pages`
4. `source-effectiveness`
5. `template-summaries`

每個 module 現在都已有：

- id
- version
- enabled
- dependsOn
- derivedTables
- status
- lastBuiltAt
- staleReason
- notes

也就是說，module registry 已經不是文檔構想，而是實作中的 runtime contract。

### 7.4 progress / heartbeat / cancel

deterministic rebuild 現在不是只有抽象的 `running`。

worker 會回寫：

- phase
- detail
- completedSteps / totalSteps
- processedItems / totalItems
- progressPercent

cancel 也不是假裝立即中止，而是 cooperative stop：

- UI 設 stop request
- worker 在 phase / chunk 邊界停
- runtime 留下 cancelled trace

### 7.5 recovery

如果 app 上次中斷：

- deterministic rebuild running jobs 會被 recover/requeue
- enrichment running jobs 也會 recover/requeue
- runtime notes 會誠實告訴使用者有 recovered jobs

這一點已經有 backend 實作和測試，不只是文檔要求。

---

## 8. 前端各頁目前各自扮演什麼角色

### Dashboard

目前不是完整 intelligence 頁，而是入口與摘要頁。

它現在做的事：

- 顯示整體 AI / intelligence status
- 顯示 On This Day
- 顯示 periodic summary
- 提供去 Explorer / Assistant / Insights 的快速入口

### Explorer

目前是：

- canonical evidence 主場
- semantic / hybrid recall 的入口
- AI runtime panel 的修復與 queue review 入口

### Assistant

目前是：

- queue-backed Q&A surface
- provider probe
- queued job reload / run / cancel
- citation review

### Insights

目前是：

- deterministic analysis snapshot 主頁
- runtime digest 的次級入口
- explainability 主頁

目前實際 section 順序是：

1. scoped-view callout
2. refresh queued callout
3. runtime digest
4. overview hero
5. runtime mini panel
6. spotlight
7. storage analytics
8. periodic summary
9. research signals
10. evidence / health
11. explainability panel

### Jobs

目前是：

- background work 的完整 review surface
- pause / resume
- plugin / module / recent jobs / recovery

### Settings

目前是：

- control tower
- provider config
- integration preview
- derived-state rebuild / clear
- plugin / module enable-disable
- runtime recent jobs

### Sidebar footer strip

目前是：

- 永遠可見的 compact background status
- progress bar
- 快速跳 Jobs

---

## 9. backend 已經有，但 UI 還沒好好用上的東西

這些都很重要，因為它們會直接影響 redesign 是否要補畫面、補 IA、補 interaction。

### 9.1 Thread detail 已存在，但沒有成為前台主角

backend 已有：

- `load_thread_detail`
- persisted `insight_threads`
- persisted `insight_thread_members`

但目前前台沒有：

- thread list 主 section
- thread detail drawer / panel / route

### 9.2 workflow map / profile facets 已存在 snapshot，但目前沒 render

snapshot type 與 backend 都已經有：

- `workflowMap`
- `profileFacets`

但當前 Insights UI 沒把它們畫出來。  
這代表設計師如果要重做，可以決定：

- 要不要把它們正式上架
- 還是先留 backend-only

### 9.3 topic 現在比較像「彙總結果」，不是互動式 topic workspace

UI 只有：

- label
- bar
- count
- deep-link

沒有：

- topic detail
- explain button
- timeline drilldown

---

## 10. 哪些文檔已經過時，哪些沒有

### 10.1 `docs/features/deterministic-intelligence.md`

判斷：

- **大致準確**

原因：

- 它跟 `ADR-006` 對得上。
- 它講的 burst / query group / thread / module / invalidation / no-AI baseline，跟當前 backend 是一致的。

### 10.2 `docs/features/intelligence.md`

判斷：

- **部分過時，部分準確**

最主要的過時點：

- `Topic Timeline` 仍殘留 embedding cluster + LLM naming 的描述，但目前 shipping 不是這樣。
- 文檔裡同時保留了 M3 optional AI、M4/M5 shipping truth note、以及一些舊 deterministic 想像，讀起來容易混。

仍然準確的部分：

- optional AI / assistant / MCP / queue / runtime surface 的大方向
- Jobs / Insights / Settings / sidebar 的 queue honesty contract

### 10.3 `docs/design/screens-and-nav.md`

判斷：

- **大致準確**

原因：

- 它對 Jobs / Insights 的分工、queue grammar、shared profile scope、runtime digest 的規範，和現在前台很接近。

### 10.4 `docs/plan/m3-*`、`docs/plan/m5-*`

判斷：

- **不算過時，但不是現在最該拿來做設計的文檔**

原因：

- 它們比較像「當時怎麼做、做完了哪些 work package」，不是當前 UX source of truth。

---

## 11. 這次盤點後，我認為最值得立刻重做的地方

### 11.1 Insights 的核心問題不是資料不夠，而是層次太亂

現在它把：

- summary
- storage
- runtime
- query groups
- topic timeline
- reference pages
- source effectiveness
- module health
- cards
- explainability

全部塞進同一種 panel 語法裡。  
資訊是真的，但頁面不會自己告訴使用者「先看哪裡」。

### 11.2 Jobs 的核心問題不是功能缺，而是沒有把 triage 畫面做對

現在它其實已經有：

- running now
- queued
- failed
- plugin status
- module status
- recovery
- recent AI jobs
- recent runtime jobs

但視覺上還是太像一整頁 operations dump，而不是一個讓人快速判斷優先順序的 review surface。

### 11.3 Threads 現在是 backend 核心概念，但 UI 不是

這是目前 intelligence 設計最可惜的一點。

現在真正有結構意義的是：

- query group
- thread
- reference page

但 UI 目前比較強調：

- cards
- topic bars
- flat lists

所以 designer 這一輪很值得重新思考：

- thread 是否應該成為一等主體
- topic 是否應該退到比較輕的概覽層

---

## 12. 本次調查的結論

如果只用一句話總結：

**PathKeep 現在已經有一套可用、可解釋、可重建的 deterministic intelligence 系統；真正需要大改的，不是「有沒有 intelligence」，而是「怎麼把這套已存在的 intelligence 和 queue truth 整理成使用者第一次看就懂的 UI」。**

這也是為什麼接下來的設計重點應該是：

- 不是再 invent 新功能名詞
- 而是重新整理頁面層次、區塊主次、thread 的主角地位、queue triage 的閱讀順序，以及 explainability 的可達性

---

## 相關文檔

- [deterministic-intelligence.md](deterministic-intelligence.md)
- [intelligence.md](intelligence.md)
- [../design/intelligence-ui-redesign-brief.md](../design/intelligence-ui-redesign-brief.md)
- [../design/jobs-ui-redesign-brief.md](../design/jobs-ui-redesign-brief.md)
- [../design/screens-and-nav.md](../design/screens-and-nav.md)
- [../architecture/decisions/006-deterministic-intelligence-boundary.md](../architecture/decisions/006-deterministic-intelligence-boundary.md)
