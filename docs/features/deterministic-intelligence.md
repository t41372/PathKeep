# DETERMINISTIC INTELLIGENCE — Evidence-First Analytics

> **狀態：Accepted (M5 baseline, 2026-04-10)**  
> 這份文檔是 `ADR-006` 的正式 feature spec。M5 之後，PathKeep 的 deterministic baseline 以這裡為準；[intelligence.md](intelligence.md) 仍保留 optional AI / assistant / MCP contract 與現行 shipping truth note，但不再是新的 deterministic heuristic source of truth。

---

## 1. 為什麼需要這份 spec

PathKeep 已經有一部分 deterministic insights，但 repo 內仍殘留幾個不夠誠實或不夠長期的假設：

- 把 `visit_duration` 當作可用的停留時間 proxy
- 把 `session` 同時拿來代表時間容器、task、研究線
- 讓 thread / topic baseline 過度依賴 embedding 或隱含的 semantic merge
- 用少量英文網站 heuristic 假裝是全球 taxonomy

M5 的目標不是把 AI 拿掉，而是把「**沒有 AI 也很好用**」這一層做成正式、可 shipping、可重建、可維護的 intelligence baseline。

---

## 2. Non-Goals

以下能力**不屬於** deterministic intelligence baseline：

- 估算 dwell time
- 估算 session duration
- 估算 focus time / foreground time
- 估算 deep reading / engagement
- 推斷使用者是否真的在看畫面
- 依賴 browser-private clustering annotations
- client-side per-user training
- 在 backup / import transaction 內同步重算完整 intelligence
- 未經 sandbox / permission 設計就開放第三方模組直接執行

如果產品要呈現「時間花在哪裡」一類資訊，只能在未來另開明確研究，不得偷渡進 deterministic baseline。

---

## 3. 可用證據與證據等級

### 3.1 Canonical inputs

Deterministic intelligence 可以使用：

- `visited_at_ms` / `visit_time`
- `url`
- `title`
- `browser_kind`
- `profile_id`
- `from_visit`
- `transition`
- `external_referrer_url`
- canonical `search_terms` rows（若存在）
- 從 search-result URL extract 出來的 query
- import / rollback / visibility / run metadata

### 3.2 Evidence tiers

不是所有來源都同樣強，因此需要明確區分：

- `tier_a`
  - canonical `search_terms`
  - 明確 search-result URL 且 query 參數可穩定抽出
  - 可追溯的 referrer / redirect chain
- `tier_b`
  - host / path / title / query token overlap
  - 同 domain family / page-category 的結構性延續
- `tier_c`
  - 單純時間相近
  - 僅能用於 `burst`，不可單獨支撐 thread / open-loop / source-effectiveness 結論

產品與 explainability panel 必須保留 evidence tier，避免把弱證據包裝成強結論。

---

## 4. 名詞定義

### `visit`

單一筆 canonical history row。是所有 derived intelligence 的最小單位。

### `burst`

時間上相近、結構上相鄰的一段 visit container。

- 用途：便宜的局部分段、pagination / visualization / workflow adjacency
- 限制：不是 task，不是 research thread，不代表完成度

### `query_group`

由一次搜尋或一組明確 query reformulation 展開的一串相關 visits。

- 優先建立在 `tier_a` 搜尋證據上
- 允許吸附與 query family、page category、referrer chain 一致的落地頁
- 是 deterministic intelligence 的主要產品單位

### `thread`

跨 burst、跨 query group、可跨天 reopen 的同一條研究線。

- 以 query family、reference-page reuse、domain/page mix、reopen evidence 建立
- 可以有 `confidence`
- 不是 canonical truth，可隨規則與 evidence 改版重建

### `domain_category`

網站的大類世界觀，例如 `search`、`docs`、`developer`、`community`、`social`、`video`、`shopping`、`news`、`finance`、`education`、`work`、`ai`、`travel`、`entertainment`。

### `page_category`

具體頁型，例如 `search_results`、`docs_page`、`repo`、`issue`、`pull_request`、`forum_thread`、`product_page`、`category_page`、`video_page`、`article_page`、`profile`、`dashboard`、`home`。

### `interaction_kind`

這頁在研究路線中通常扮演的角色，例如 `discover`、`compare`、`resolve`、`discuss`、`watch`、`learn`、`transact`、`manage`。

### `source_role`

對這個使用者來說，某個來源在實際行為中扮演的個人角色，例如入口、背景補充、正式參考、解法來源、比價來源。

### `open_loop`

代表「反覆重開、尚未穩定收斂」的研究線 signal，不代表真實世界的任務是否完成。

### `reference_page`

被跨天回訪、跨 group / thread 重用、且常作為穩定落點的高價值頁面。

---

## 5. Pipeline

### 5.1 Visit normalization

每筆 visit 進入 deterministic pipeline 前，至少要提取：

- normalized URL
- host
- registrable domain
- subdomain
- path
- normalized query params
- probable search engine / search-result flag
- extracted search query（若有）
- browser / profile
- referrer / transition hints

Normalization 要求：

- 去除常見 tracking params，例如 `utm_*`
- 保留產品需要的語義參數，例如搜尋 query、商品 id、issue id、video id
- host 與 registrable domain 分開存取
- 對 IDN domain 做標準化處理

### 5.2 Taxonomy classification

分類 precedence 固定為：

1. user override
2. exact domain rule
3. host / path / query rule
4. title / query lexicon
5. optional model fallback
6. `unknown`

高信心規則不可被低信心模型結果覆蓋。`unknown` 必須能沿整個 UI 與 explainability 合法存在。

2026-04-10 implementation note：

- `vault-core::deterministic` 已作為 taxonomy v2 的唯一 foundation home。
- `visit_insight_features` 現在會持久化 `domain_category`、`page_category`、`interaction_kind`、`evidence_tier`、taxonomy source / pack / version / reason，供 explainability、unknown review 與後續 rebuild contract 使用。
- deterministic feature scoring 已移除 `duration_ms` 權重，避免把 estimated dwell / session-duration proxy 再包裝成 baseline truth。

### 5.3 Burst construction

`burst` 是低成本時間容器：

- 同 profile
- gap 在 bounded threshold 內
- referrer / `from_visit` / tab-local continuity 若存在可提高信心

限制：

- `burst` 不能直接宣稱是 task
- `burst` 不可拿來輸出「你花了 X 分鐘」

### 5.4 Query groups

起點：

- canonical `search_terms`
- 可穩定解析 query 的 search-result URL
- 明確 page-category = `search_results`

吸附條件：

- query token overlap
- referrer / redirect chain 關聯
- 常見 research landing page 類型：`docs_page`、`repo`、`issue`、`forum_thread`、`article_page`、`product_page`、`video_page`
- 與 group 已存在 domain / page mix 明顯相近

結束條件：

- gap 超過 threshold
- 出現高信心的新搜尋
- 接續多筆 visits 與現有 group 無關

Query reformulation：

- `sqlite wal`
- `sqlite wal too large`
- `sqlite wal checkpoint not working`

這種 query family 應保留在同一條 ladder / group，而不是每改一個詞就硬切。

### 5.5 Query reformulation ladder

這是 deterministic baseline 的核心 shipping surface：

- 顯示 query 如何逐步變具體
- 支援 `broad -> narrow`
- 支援平台限制、error code、version、`site:` restrict、compare intent
- Chromium search term evidence 最強；其他來源若只有 URL 抽取，必須誠實標示 evidence tier

2026-04-10 implementation note：

- `run_insights` 現在已正式按 `visit features -> burst -> query group -> thread -> reference/source/summaries` 順序執行。
- `visit_insight_features` 會持久化 `burst_id` 與 `query_group_id`；shipping derived tables 也已補上 `insight_bursts`、`insight_query_groups`、`insight_query_group_members`、`insight_reference_pages` 與 `insight_source_effectiveness`。
- `query_ladders` 現在優先從 persisted query-group surface 載入；如果 runtime 尚未有對應 group rows，才回退到 visit-level feature reconstruction。

### 5.6 Thread merge

Thread 只建立在下列 evidence 上：

- query token family overlap
- reference-page reuse
- domain / page-category mix similarity
- reopen evidence
- shared canonical search / navigation anchors

禁止：

- 單憑時間相近就跨天併 thread
- 單憑熱門 token 就把寬泛主題都併在一起
- 沒有 evidence / confidence 就輸出強結論

### 5.7 Source role / source effectiveness

`source_role` 回答：

> 這個來源對這個使用者通常拿來做什麼？

`source_effectiveness` 回答：

> 這個來源對這個使用者，是否常成為有用的穩定落點？

可用 signal：

- 是否常出現在 query group 後半段
- 是否常是 group / thread 的穩定落點
- 看完之後 reformulation 是否下降
- 是否被跨天重新打開
- 是否出現在多個 query group / thread 中

不可用 signal：

- dwell
- foreground time
- scroll / click / typing

### 5.8 Open loops

Open loop 只能輸出：

- score
- confidence
- reasons

不可輸出：

- 「完成」/「未完成」二元強判定

較好的 user-facing wording：

- 反覆回來看的問題
- 仍在持續探索的主題
- 多次重開的研究線

### 5.9 Resurfacing / reference pages

重點不是停留時間，而是：

- 多次回訪
- 跨天回訪
- 出現在不同 query group
- 出現在不同 thread
- 經常被重新找回來

這類頁面可以支撐：

- 常重找的參考頁
- 值得 pin 的頁
- 某主題下最常回訪的資料

2026-04-10 implementation note：

- Insights / Settings 現在正式 shipping `query groups`、`reference pages`、`source effectiveness` 與 `template summaries` surface。
- `thread` summary 現在也持久化 `query_group_count`、`confidence` 與 `evidence_tier`，讓 explainability panel 不再只剩 reopen / open-loop 分數。

### 5.10 Template summaries

M5 的 deterministic summaries 先採模板生成：

- 最近常查的問題
- 哪些主題反覆被重開
- 哪些來源常作為穩定落點
- 近一段時間偏探索還是偏深挖

LLM 只能在 optional layer 上把 deterministic outputs 改寫成人話，不可替代 evidence generation。

---

## 6. Regional taxonomy strategy

### 6.1 Coverage priority

v1 taxonomy packs 以以下順序維護：

1. 中國大陸
2. 美國
3. 台灣
4. 日本
5. 韓國
6. 歐洲
7. 俄羅斯
8. International common sites

### 6.2 Rule pack layout

建議以版本化資料包維護：

- `global-core`
- `cn-core`
- `us-core`
- `tw-core`
- `jp-core`
- `kr-core`
- `eu-core`
- `ru-core`

每個 pack 至少可包含：

- exact domain rules
- host / subdomain rules
- path-pattern rules
- search-engine parameter extraction rules
- title / query lexicons

### 6.3 Taxonomy governance

為避免維護性危機：

- taxonomy pack 必須有 version
- 每次新增 / 修改規則都要有 fixture
- 使用者 override 永遠優先
- unknown rate、top unmatched domains、top misclassified domains 要可 review

2026-04-10 runtime guardrail：

- 在 `PG-RD-AI-010` 完成前，shipping runtime 只允許 checked-in heuristic rule packs 與 script-aware tokenization baseline。
- external registrable-domain / tokenizer / language-ID / optional model assets 只允許留在 research / offline evaluation 討論，不可先偷渡進桌面 bundle。

---

## 7. Tokenization and language strategy

Day-one baseline：

- Unicode normalization
- script-aware lower-risk tokenization
- 對 Latin script 使用 word boundaries
- 對 CJK script 允許 n-gram / script-aware segmentation baseline

Optional helpers：

- language ID 可用於 lexicon / tokenizer selection
- 更重的中文 / 日文斷詞器或模型資產，必須先經過 bundle-size / supply-chain / license review

重要原則：

- language ID 是 hint，不是 truth
- tokenize 錯了，最多降級成較差 recall，不應導致偽精準分類

---

## 8. Runtime wheel strategy

建議優先使用成熟、可審查的標準輪子：

- URL / domain parsing：`url`、`idna`、`publicsuffix` 或 `psl`
- rule matching：`regex` / `regex-automata`、`aho-corasick`
- static maps：`phf`
- baseline tokenization：`unicode-segmentation`

可考慮但暫不作 day-one baseline：

- language ID：`whatlang` 這類小型庫
- ONNX runtime：`ort`
- FastText bindings

只適合 offline seed / benchmark，不適合目前桌面 runtime baseline：

- `Tranco`：熱門網域排序種子
- `Curlie`：類別種子與驗證集
- `Homepage2Vec`：對照模型與離線評估

任何外部輪子、模型或資料集在進入 runtime 前，都需要：

- license review
- bundle-size impact
- update cadence
- supply-chain / trust review
- fallback / removal story

---

## 9. Module contract

每個 deterministic intelligence module 至少要宣告：

- `id`
- `version`
- `enabled`
- required inputs
- derived outputs
- rebuild mode
- invalidation triggers
- evidence schema
- UI surfaces

建議的 internal interface：

1. `collect(inputs) -> candidate rows`
2. `score(candidates, context) -> scored rows`
3. `persist(rows, run_context)`
4. `explain(id) -> evidence + reasons + confidence`
5. `clear()`

限制：

- module 不可改寫 canonical facts
- module 不可在 backup / import hot path 同步執行全量重建
- third-party execution 仍屬 deferred，直到 sandbox ADR 存在
- 2026-04-10 closeout：repo 現在正式 shipping first-party-only enrichment runtime registry（`title-normalization` + `readable-content-refetch`）與 queue review surface；這是 internal runtime boundary，不是 third-party plugin promise
- 2026-04-10 `WORK-M5-B` closeout：deterministic built-ins 現在也有正式 module registry trace，至少涵蓋 `query-groups`、`threads`、`reference-pages`、`source-effectiveness`、`template-summaries`。Settings / Insights 會顯示 `ready` / `stale` / `disabled` / `idle`、dependency list、derived tables、last built time 與 stale reason。

---

## 10. Rollback / restore / visibility contract

以下事件必須讓 deterministic derived state 至少標為 stale：

- rollback
- restore
- import revert / restore
- visibility change
- taxonomy version upgrade
- plugin / module enable-state change

重建策略：

- taxonomy / visit-level feature rows 可 incremental rebuild
- query groups / threads / open loops / reference pages 視 affected window incremental 或 full rebuild
- clear derived state 必須留下 run-linked report，不可靜默刪除

2026-04-10 implementation note：

- `clear_derived_intelligence_state` 與 archive visibility repair 現在都會清掉 M5-B derived tables，並把 deterministic module registry 明確標成 `stale`。backup / import 成功後必須自動排入新的 deterministic rebuild job；若是 manual clear / repair 導致的 stale state，UI 也必須明講目前狀態與下一步，而不是讓舊 surface 假裝仍然新鮮。

---

## 11. Testing fixtures

M5 至少要有這些 fixture family：

1. 搜尋後點 docs / forum / repo 的 query-group fixture
2. 連續 reformulation fixture
3. 中日韓 title / query fixture
4. 同 host 不同 page type fixture
5. rollback 後 group / thread 斷裂 fixture
6. restore 後 rebuild fixture
7. 中國大陸 / 美國核心網站分類 fixture
8. unknown / ambiguous taxonomy fixture
9. large-window deterministic rebuild fixture

測試要求：

- 規則新增要帶 fixture
- explainability 要可驗證
- `unknown` 與 low-confidence 要有 snapshot / golden coverage

---

## 12. Shipping phases

### Phase 1 — Foundation

- URL normalization
- registrable domain extraction
- search URL parser
- page-type rules
- lexicon framework

### Phase 2 — High-value deterministic intelligence

- query groups
- reformulation ladders
- reference pages
- source role / source effectiveness
- open loops v1

### Phase 3 — Cross-day research lines

- thread merge
- reopen detection
- explore vs exploit
- template summaries

### Phase 4 — Optional enhancements

- language ID
- small classification model fallback
- LLM / semantic enhancements on top of deterministic outputs

---

## 13. Acceptance bar

M5 只有在以下條件成立時才算完成：

- deterministic insights 不再依賴 dwell / session duration / focus proxies
- taxonomy precedence、invalidations、module contract 都有正式文檔與測試
- no-AI mode 仍可提供高價值 insights
- explainability 能回到 canonical Explorer evidence
- 60-year / heavy-user envelope 至少對 deterministic pipeline 有 replayable benchmark 與 cost accounting

## 相關

- [intelligence.md](intelligence.md)
- [../architecture/decisions/006-deterministic-intelligence-boundary.md](../architecture/decisions/006-deterministic-intelligence-boundary.md)
- [../architecture/data-model.md](../architecture/data-model.md)
- [../plan/m5-deterministic-intelligence/README.md](../plan/m5-deterministic-intelligence/README.md)
