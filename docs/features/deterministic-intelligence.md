# DETERMINISTIC INTELLIGENCE — Evidence-First Analytics

> **狀態：Superseded (2026-04-15)**  
> 這份文檔曾是 `ADR-006` 的正式 feature spec。自 2026-04-15 起，PathKeep 的 deterministic / Core Intelligence baseline 已改由 [core-intelligence-ultimate-design.md](core-intelligence-ultimate-design.md) 接管。這份文檔保留作為 M5 evidence-first hardening 的歷史背景，不再是新的 product contract。

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
- source-batch capability snapshot 與 evidence coverage stats

### 3.1.1 Capability families

deterministic / optional intelligence module 的 feature gate 必須建立在 capability family，而不是 browser/version：

- `search.native_terms`
- `search.url_extracted_query`
- `nav.from_visit`
- `nav.opener_visit`
- `nav.redirect_edges`
- `engagement.foreground_ms`
- `engagement.view_metrics`
- `context.task_graph`

同一個 capability 可以來自不同 browser family，但 explainability / degrade copy 必須誠實反映 coverage 與 evidence tier。

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
- source batch / capability snapshot
- referrer / transition hints

Normalization 要求：

- 去除常見 tracking params，例如 `utm_*`
- 保留產品需要的語義參數，例如搜尋 query、商品 id、issue id、video id
- host 與 registrable domain 分開存取
- registrable domain 就是「網站」，照瀏覽器的方式切：Mozilla 的 Public Suffix List，ICANN 與私有兩段都用（`vault-core/vendor/publicsuffix/`，`bun run update:public-suffix-list` 更新）。`rust-lang.github.io` 與 `mio-rs.github.io` 是兩個網站，`news.bbc.co.uk` 在 `bbc.co.uk`；IP 位址、單一標籤的主機（`localhost`）與本身就是公共後綴的主機（`github.io`）各是自己的網站。清單的版本是 URL 規則版本的一部分，清單一換，洞察就整個重建。（2026-10-10 之前 app 實際上沒有載入任何清單，是「最後兩段」加 11 個手寫的兩段後綴。）
- 對 IDN domain 做標準化處理

### 5.2 Taxonomy classification

分類 precedence 固定為：

1. user override（使用者替整個網站選的類別不蓋過清單另外分類的部分，見下方 2026-10-11 note）
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
- module runtime 也必須記錄本次 build 使用到的 capability family 與最低 coverage 判斷，避免 UI 把 partial-support 結果包裝成 fully-supported baseline。

2026-10-08 note（precedence 不變）：

- exact domain rule 先比對造訪的完整主機，再依序比對上層網域，到網站擁有者在 ICANN 規則下註冊的網域為止：`mail.google.com` 先於 `google.com`；`tokio.readthedocs.io` 是自己的網站，但也走到 `readthedocs.io`，所以 Read the Docs 的規則涵蓋那裡的每個文件網站；不會比對到 `co.uk` 這種公共後綴。之前只比 registrable domain，所以 Google 的文件、郵件、Gemini 都算成搜尋。
- 站點清單在 `vault-core/src/visit_taxonomy/sites/`，每個 pack 一個檔案，只收分類一看就清楚的網站；入口網站與包山包海的 app（`yahoo.co.jp`、`naver.com`、`qq.com`）不收，只收其中分類清楚的主機（`news.naver.com`、`mail.qq.com`）。exact rule 只定網站分類，頁面種類留 unknown（文件站與搜尋引擎除外），因為頁面種類會影響 compare sets。
- 要靠路徑分頁面種類的網站（GitHub、YouTube、Reddit、Amazon、Taobao）不寫 exact rule（會蓋掉路徑規則），改在它們的路徑規則後面加一條只看主機的規則，其餘頁面也有網站分類。測試保證：每條規則都比對得到、同一網域不重複、exact rule 不會蓋掉路徑規則。
- title / query lexicon 只決定頁面種類，不決定網站分類（網站分類留 unknown）：頁面標題說的是這一頁，不是整個網站；之前「tokio vs smol」這種部落格文章因為 "vs" 被歸成購物、「訓練計畫」因為 "plan" 也是。英文詞要整個詞相符（"api" 不算出現在 "rapid" 裡、"plan" 不算出現在 "explanation" 裡）；中文、日文標題詞之間沒有空格，照子字串比對。
- 只有 http(s) 網頁屬於某個網站。擴充功能的頁面（`chrome-extension://<id>/…`）、本機檔案這類造訪算進總數，但不算網站：不出現在網站排名、網站數、新網站與網站之間的路線裡（2026-10-09，之前擴充功能的 id 被當成網站）。
- 一個網站一天的分類取當天多數造訪的分類，平手時已知分類優先，再依名稱。之前取名稱最小的，一個被歸成 docs 的頁面就讓一整天的搜尋引擎變成 docs。

2026-10-11 note（使用者選的網站類別）：

- 網站頁可以改網站類別（`set_site_category`）：存成一筆 user override，只帶網站類別，跟 exact rule 一樣只決定網站類別，頁面種類與 interaction 照規則（GitHub 的 issue 在使用者叫它「工作工具」的網站上仍是 issue）。帶了頁面種類與 interaction 的 override 照舊整個由它決定。override 比對網站（registrable domain），網站的主機都跟著改，清單另外分類的部分除外：清單的 exact rule 或路徑規則單獨列出的、在網站之下的主機（`google.com` 裡的 `mail.google.com`、`docs.google.com`，`ycombinator.com` 裡的 `news.ycombinator.com`）和它們底下的主機保留清單的類別；網站本身、`www.` 和清單沒列的主機照使用者選的。網站頁列出存檔裡有頁面的這些部分與類別。選回「PathKeep 清單」就刪掉這筆。2026-10-11 用戶決定（side agent 提醒在 `google.com` 選一個類別會連 Gmail、Google 文件一起改掉）；放棄的做法：整個網域都改、改之前先提醒（照原本「override 永遠優先」的字面，但多用途網站一改就沒了清單的區分），以及每個部分都能各自改（範圍變大，清單只收分類一看就清楚的主機，需要的機會少）。
- 改了之後只重新分類那個網站的頁：每次建置記下當時的 override，下一次跟現在的比，只有整個網站的類別變了（新增、改、拿掉）就只走那些網站的頁（`build_reclassify`，`store-schema.md`）；rule pack 換了或 override 指的是 host、網址開頭時，照舊每頁都分一次。建置中又改了另一個網站，同一個工作在建完後再跑一輪那個網站；app 中途關掉的建置照樣接著做，不因類別變了而整個重建。打開 app 時要補的建置不再強制 full，由建置自己決定。
- 建置記錄的規則版本包含 matcher 版本與每個 pack 的版本；改清單就改該 pack 的 version。打開 archive 時，若有設定檔的 Insights 是用別的規則建的（而且最近一次 Insights 工作是成功的），就排一次重建，不等下次備份。

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

實作（2026-10-09，`structural-rebuild-v5`）：同一個 session、距 trail 上一個頁面 15 分鐘內的頁面，只有在「從 trail 的某個造訪點開」（`from_visit`）、「在 trail 已經去過的網站」或「標題和這串搜尋有共同的詞」時才算進 trail；連續 3 個不相關的頁面就結束這條 trail。之前同一 session 裡接著的每個頁面都算進去，真實資料上平均每次搜尋「打開」23 個頁面。研究型頁面類別本身不算吸附條件（任何文件頁都會被吸進來）。

Query reformulation：

- `sqlite wal`
- `sqlite wal too large`
- `sqlite wal checkpoint not working`

這種 query family 應保留在同一條 ladder / group，而不是每改一個詞就硬切。

實作（2026-10-09，`structural-rebuild-v10`）：一次搜尋依序這樣歸組（都在同一設定檔、同一搜尋引擎內）：(1) 同一種說法已經在某組裡，就進那組；(2) 它跟前一次搜尋在同一條 trail 裡（§5.4 上面的 trail 規則：15 分鐘內、跟最新一種說法共用至少一半的詞或一方全在另一方裡），就進前一次搜尋那組；(3) 否則跟每組的**第一次搜尋**比，共用至少一半的詞、而且至少兩個（或兩邊的詞完全一樣）就可以加入，可以加入的不只一組時進最像的那組（共用的比例最高，一樣高時最早建立的）；(4) 都不行就自成一組。所以「台北天氣預報」先出現時，「台北天氣」（五個字組裡共用三個）進同一組，緊接著改寫的「台北天氣 v2」「台北天氣 v2 eg」順著 trail 也進這組；上面 `sqlite wal` 的例子照原順序搜會在同一組。(1) 優先於 (2)：trail 中途改回另一組已有的說法時，進那組，trail 之後的改寫跟著它。放棄的：同一串改寫隔了一段時間、倒過來搜（先「sqlite wal checkpoint not working」，幾天後才搜「sqlite wal too large」），中間沒有 trail 接著，跟第一次搜尋也只共用兩個詞，會另成一組。
v9（同日稍早）讓一次搜尋跟組裡**任何一種**說法比，結果一組順著說法一路串下去：只搜一個詞（例如「python」）的那次進了某組之後，所有含這個詞的搜尋都算像它、全進這組。合成的 57.6 萬次搜尋（1440 萬筆瀏覽的 4%，詞頻照 Zipf；`intelligence/tests/family_scale.rs`）裡，30.5 萬種說法有 16.5 萬種被串進同一組，建置要 46 分鐘，而且每多一批搜尋就更慢。再之前只跟這組的第一次搜尋比、只取第一個夠像的組：第一次剛好是較長的說法時，緊接著的「台北天氣 v2 eg」另成一組，同一件事在首頁出現兩次；「fujifilm x-t5 vs a7c ii v2 eg」也因為跟另一組的改寫共用 a7c、ii、v2、eg 被分走。v10 用 trail 接住緊接著的改寫，只跟第一次搜尋比、取最像的，兩個問題都不會再發生；一組只順著同一條 trail 裡緊接著的改寫延伸，之後的搜尋只跟第一次搜尋比，不會再順著組裡別的說法串下去。同一次的第一版 v10 還留著「一方的詞全在另一方裡也算」：很短的第一次搜尋會收進所有含它的詞、再長也一樣的搜尋，同一份合成資料上一組由兩個詞開頭的收了 6,683 種說法，由一個詞開頭的組合計占全部搜尋的 13%。拿掉之後，只搜一個詞（中文兩個字也是一組字）的那次，除非接在 trail 裡或一模一樣，不跟別的搜尋同組；規格的三個例子都靠「共用一半」就成立。同一份資料上建置 6.3 s，最大的一組 3,589 種說法（由兩個詞開頭：合成資料的詞彼此獨立抽，三、四個詞的搜尋裡剛好含有最常見那兩個詞的就有這麼多；真實的搜尋裡常一起出現的詞多半是同一個主題），由一個詞開頭的組最多 12 種說法（同一個詞重搜，加上 trail 裡緊接著的改寫）。trail 的改寫規則不變（仍算一方全在另一方裡），因為那只在 15 分鐘內接著搜的時候用。

比較詞的時候不算虛詞（英文的 this、what、my、vs…，中文的 的、是、什麼、如何…，日文的 の、は、です…，完整清單在 `intelligence_shared.rs`）和網址片段（www、com…）：真實資料上，只搜「this」的那次被算成所有含 this 的搜尋的改寫，一共 31 次；「搜尋最多的詞」裡也不再出現它們。中文、日文的搜尋不再整句算一個詞（2026-10-09，`structural-rebuild-v8`；見 §7）。之前只按空格切，換個說法就跟原本的搜尋一個詞也不共用，改寫、query family 都分不到一起，「搜尋最多的詞」列出整句話。現在分兩種用途：顯示與計數（「搜尋最多的詞」）用字典切出的詞，「台北天氣」是 台北 / 天氣；比較兩次搜尋（改寫、query family、trail 拿標題比搜尋）用去掉虛詞後剩下的字、每相鄰兩字一組，因為字典會按前後文把同一句切成不同的詞（「什麼是非同步」切成 是非 / 同步，「非同步是什麼」切成 非同 / 步），兩兩一組的「非同」「同步」兩邊都有。拉丁字母照舊在非字母數字處切開。

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

#### 網站怎麼被打開（2026-10-09，`structural-rebuild-v6`）

網站頁的「怎麼來到這裡」與「接著去了哪裡」只看瀏覽器記下的 opener（`from_visit`：打開這頁的那次造訪），不靠時間相鄰猜：

- 打開它的頁面不在這個網站上，這次造訪才算「來到這裡」；在同一網站的頁面之間移動不算。
- 沒有 opener（Chromium / Firefox 寫 `0`）：輸入網址、書籤、新分頁或從別的 app 打開，算「輸入網址或書籤」。
- opener 是搜尋結果頁：算「從搜尋來」，記下搜尋引擎的網站。
- opener 是別的網站的頁面：算「從連結來」，記下那個網站；那個網站同時多一個「接著去了哪裡」。
- opener 不在最近走過的 512 次造訪裡（很久以前開著的分頁、隱藏的造訪）：仍算「從連結來」，但不寫是哪個網站。
- 瀏覽器根本不記 opener（Safari、Google Takeout，`from_visit` 是 `NULL`）：這次造訪不算進任何一項，不能當成「輸入網址」；另記為「沒記錄」（`structural-rebuild-v9`），網站頁據此說「無法知道」，而不是「沒有從別處來的瀏覽」；兩種都有時，寫出有幾次沒算進來。

v0.3 的版本把有 trail 的造訪算「搜尋」、有 `from_visit` 的算「連結」、其餘算「輸入」，三者會重疊；它的來源與去向網站只看同一網站的造訪彼此相鄰，所以從來沒有資料。

#### 重新打開的頁面（2026-10-11，`structural-rebuild-v11`）

一頁（同一個 canonical URL、同一設定檔）在不只一天、不只一條搜尋 trail，或不只一次直接輸入（沒有 opener）時打開過，就是 refind page（`refind_pages`）；搜尋結果頁不算。

KPI「重新打開的頁面」數的是範圍內某一天被打開過的 refind page，跟 day 頁「回來再看的頁面」列的是同一批：所有瀏覽器時同一頁算一次，選了設定檔就只看它。之前數的是第一次到最後一次造訪的區間跟範圍重疊的頁，沒有任何造訪的一天也寫出幾十頁。建置時把每一天被打開的 refind page 存進 `refind_day_pages`（一天一列，本地日子按洞察的時區）；追加時只把頁加進新的日子（新的造訪都晚於已存的，只有已存的最後一天可能重複）。

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
- 2026-04-12 runtime rewrite：`query_groups`、`threads`、`reference_pages`、`source_effectiveness` 與 `cards` 的 persisted read model 必須按 `profile_scope + window_days` 分區，禁止 30-day / 365-day 或 single-profile / all-profile rebuild 互相清掉對方的結果。

### 5.10 Template summaries

M5 的 deterministic summaries 先採模板生成：

- periodic summary（目前 window 的 deterministic overview）
- contrastive summary（目前 window vs 前一個等長 window）
- 最近常查的問題
- 哪些主題反覆被重開
- 哪些來源常作為穩定落點
- 近一段時間偏探索還是偏深挖

LLM 只能在 optional layer 上把 deterministic outputs 改寫成人話，不可替代 evidence generation。

2026-04-12 truth note：

- backend 現在必須直接生成 periodic / contrastive summary，讓 AI disabled 時仍有 summary card。
- `Important but Unsaved` 仍 deferred，直到 canonical archive 真正 ingest bookmark / saved-page facts；在那之前 deterministic layer 不能假裝知道「是否已保存」。

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
- 使用者 override 永遠優先：同一個名稱上，使用者的先於清單；使用者替整個網站選的類別不蓋過清單另外分類的部分（主機更細，2026-10-11 用戶決定，見 §5.2 note）。指定主機或網址開頭的 override 照舊先於清單
- unknown rate、top unmatched domains、top misclassified domains 要可 review（2026-10-11：「哪類網站」卡的「未分類」列打開範圍內不在清單上的網站，瀏覽最多的在前〔`get_sites_in_category`〕，連到網站頁改類別；分錯的網站一樣在網站頁改）

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

2026-10-09 的 review（搜尋詞的中文 / 日文斷詞）：用 ICU4X 的 `icu_segmenter`（`WordSegmenter::new_auto`：中日文用字典，泰、寮、高棉、緬文用小型 LSTM），只切含中日文字元的那一段，拉丁字母不變。顯示用字典切出的詞；比較用去掉虛詞後的相鄰兩字（字典切法隨前後文變，見 §5.4 下方）。

- supply chain：Unicode Consortium 的 ICU4X，與已在用的 `icu_normalizer`、`icu_collator` 同一套、共用已鎖定的 `icu_provider` 等；新增的只有 `icu_segmenter`、`icu_segmenter_data` 與 ICU4X 維護者的 `core_maths`（依賴已有的 `libm`）。
- license：Unicode-3.0（與 `icu_normalizer` 相同）。
- bundle size：release 二進位增加約 2.4 MB（同樣 release 設定下有用與沒用的差，量自 Modal；`tech-stack.md`）。
- 只用在搜尋詞（改寫、query family、「搜尋最多的詞」、trail 用標題比對搜尋的詞）；頁面種類的詞（`visit_taxonomy::tokenize_text`）仍照子字串比對中日文，不受影響。
- 只用 bigram 不行：能把同一件事分到一起，但「搜尋最多的詞」會變成「麼是」「是非」這種片段；只用字典詞也不行：同一句換個語序切法就變。所以顯示用詞、比較用字典去掉虛詞後的兩字一組。

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
- 2026-10-13：import revert / restore 之前沒有照上面的清單標 stale，也沒排重建；撤銷後洞察照舊算著那些造訪。現在跟匯入走同一條（`vault-worker` 的 `refresh_after_visibility_change`），只排這次動到的設定檔。
- 2026-10-13：一次刷新按設定檔與階段排好幾個 job（先全部設定檔的 visit-derive，再 daily-rollup，再 structural）。之前第一個設定檔的 job 做完就把它的 module 標成 ready，別的設定檔的同一階段還在排隊；Insights 讀到 ready 就停止重讀，最後一個設定檔的數字進來時畫面沒更新。現在只要還有排隊中的 job 會重建某個 module，它就維持 stale（`intelligence_rebuild.rs` 的 `modules_still_queued`），由最後一個做完的 job 標成 ready。
- deterministic rebuild queue 現在採 lease + heartbeat + cooperative cancel：claim 必須 compare-and-set，running cancel 只設 stop request，worker 需在 phase / chunk 邊界自行結束並留下 cancelled trace；success 不得覆蓋已被 cancel / failed 的 job。

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
  - 2026-04-12 current replayable artifacts:
    - Native Turso: `CARGO_NET_OFFLINE=true cargo test --manifest-path src-tauri/Cargo.toml -p vault-worker --test archive_scale_bench turso_intelligence::encrypted_intelligence_at_scale -- --ignored --nocapture`. This uses one encrypted archive, enforces the 15 GB data budget, and removes it afterwards.

## 相關

- [intelligence.md](intelligence.md)
- [../architecture/decisions/006-deterministic-intelligence-boundary.md](../architecture/decisions/006-deterministic-intelligence-boundary.md)
- [../architecture/data-model.md](../architecture/data-model.md)
- [../plan/m5-deterministic-intelligence/README.md](../plan/m5-deterministic-intelligence/README.md)
