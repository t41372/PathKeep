# ADR-006 — Deterministic Intelligence Uses Honest Evidence-First Boundaries

## 狀態

Proposed

## 背景

PathKeep 現行的 intelligence 文檔與實作，混雜了三種不同層次的能力：

1. optional 的 embedding / LLM intelligence
2. 可重建的 deterministic derived-state
3. 帶有強假設的 session / dwell / topic-clustering heuristic

這在 M3 / M4 先做出功能 slice 時可以成立，但到了長期產品邊界與 60-year support envelope，就暴露出幾個不能再模糊帶過的問題：

- PathKeep 的跨瀏覽器 baseline 並沒有可靠的前景停留時間、專注時間、深度閱讀時間，`visit_duration` 也不等於真實 attention。
- Firefox、Safari、Takeout、Chromium 變種之間，可穩定取得的 signal 並不對稱；Chrome 專屬 enhancement 不能被寫成產品 baseline。
- 現行 intelligence 實作仍集中在巨型 `insights.rs`，若再把 taxonomy、query grouping、thread merge、open-loop、reference page 等規則繼續往裡堆，會直接放大維護風險。
- 產品要求 AI 與 embedding 都是 optional；heavy user 在 4-core / 8 GB RAM 的機器上，沒有 AI 時也必須有高價值 analytics。
- 使用者已明確收斂產品硬邊界：PathKeep 不能估算 dwell time、session duration、focus time，且 deterministic taxonomy 要以中國大陸與美國為優先，同時兼顧台灣、日本、韓國、歐洲、俄羅斯與 international 常見網站。

因此，在直接覆寫既有 accepted intelligence contract 之前，需要一份正式決策文檔，把「哪些 signal 可以當 baseline、哪些不行、模組怎麼拆、哪些 enhancement 只能當 optional layer」講清楚。

## 決策

### 1. Deterministic baseline 只建立在 honest canonical evidence 上

M5 提議中的 deterministic intelligence baseline，只能使用以下 evidence family：

- canonical `visit_time` / `visited_at`
- URL 與其可重建的 normalization / host / domain / path / query 特徵
- title
- browser / profile
- `from_visit`、referrer、transition 等結構性跳轉線索（若來源瀏覽器提供）
- search term / query extraction（若 canonical archive 或 search-result URL 可提供）
- import / rollback / visibility / run metadata

以下 signal 明確列為 deterministic baseline 的 non-goals：

- dwell time
- session duration
- foreground time / focus time
- deep reading
- click / scroll / typing activity
- browser-private clustering annotations
- client-side per-user training

### 2. 名詞與建模單位改為 burst / query group / thread

為了避免把不同層次的 grouping 混成同一個 `session`，M5 提議採用這組 vocabulary：

- `burst`
  - 只代表時間上相近、結構上連續的一段 visit container
  - 是便宜、低風險的 grouping primitive，不代表 task 完成度
- `query_group`
  - 以明確搜尋證據或 search-result URL 為起點，向後吸附相關落地頁與 reformulation
  - 是 deterministic intelligence 的主要產品單位
- `thread`
  - 跨 burst、跨 query group、可跨天 reopen 的研究線
  - 只能建立在 query family、reference-page reuse、domain/page mix、reopen evidence 等結構性 signal 上

`session` 可以保留為低層 implementation helper，但不再作為 PathKeep intelligence 的主要產品語義。

### 3. Taxonomy 採 layered classifier，而不是單一 host label

PathKeep 的 site intelligence 應拆成至少三個維度：

- `domain_category`
- `page_category`
- `interaction_kind`

分類 precedence 固定為：

1. user override
2. exact domain rule
3. host / path / query rule
4. title / query lexicon rule
5. optional model fallback
6. `unknown`

高信心規則不得被 optional model 覆寫。`unknown` 是合法結果，不可為了 UI 完整感而強行猜測。

### 4. Query groups、threads、open loops、reference pages 都屬 derived state

以下 intelligence outputs 一律視為 rebuildable derived state，而不是 canonical archive truth：

- taxonomy classification
- bursts
- query groups
- reformulation ladders
- threads
- source role / source effectiveness
- open-loop / reopen / resurfacing / reference-page signals
- deterministic summary cards

它們必須：

- 不阻塞 backup / import hot path
- 在 rollback / restore / visibility change 後可 invalidate / rebuild
- 保留 evidence 與 confidence，而不是只存 summary text
- 能 deep-link 回 Explorer 的 canonical filters

### 5. 模組化先做 internal module registry，不直接承諾 third-party plugin runtime

M5 提議所有 deterministic insights 都走可插拔 module contract，但 day-one 的安全邊界是：

- 先做 first-party internal module registry / dependency injection
- 先把 module interface、enable / disable、rebuild / clear、evidence contract 做穩
- third-party plugin execution 仍維持 deferred，直到有獨立 sandbox / permission / data-access ADR

換句話說，PathKeep 可以先做到「模組化、可關閉、可測試」，但不能把「任何第三方模組都能安全讀你的 archive」提前寫成既成事實。

### 6. 多語言策略以 script-aware tokenization + region packs 為主，language ID 為輔

Deterministic intelligence 的 day-one multilingual baseline 應優先採用：

- script-aware tokenization / normalization
- versioned taxonomy rule packs
- locale / region lexicons

language ID 可以作為 tokenizer / lexicon selection hint，但不是 truth source。中國大陸與美國是 taxonomy v1 的核心 coverage，台灣、日本、韓國、歐洲、俄羅斯與 international 常見網站作為第二圈。

### 7. Optional models 只能補 coverage，不是主判官

若未來加入 language ID 或小型 page-classifier model：

- 只能在 rule / lexicon 無法下結論時補 coverage
- 不得推翻高信心 deterministic 規則
- 需要明確的 packaging、bundle-size、license、upgrade 與 supply-chain review

`Homepage2Vec` 之類網站分類模型可用於 offline evaluation / benchmark 對照，但不適合作為目前桌面 runtime baseline。

## 理由

- **Trust & Transparency**：不承諾拿不到的 signal，比提供漂亮但假的時間數字更符合產品核心原則。
- **Cross-browser honesty**：把 baseline 限縮在 Safari / Firefox / Chromium / Takeout 都能相對誠實取得的 evidence，能避免 Chrome-first 假設污染產品定義。
- **Performance envelope**：query group、rule-based taxonomy、reference-page resurfacing 這類 deterministic intelligence，比 embedding-first topic / thread pipeline 更容易在 8 GB / 4-core 的機器上維持可預期成本。
- **Maintainability**：多維 taxonomy、module registry、derived-state invalidation 明確化後，才有機會把 intelligence 從單一巨型檔案拆開，而不是繼續把 heuristic 越寫越散。
- **Internationalization**：region packs、script-aware tokenization、`unknown` fallback，比硬寫單一英文 host map 更符合未來多語擴張。

## 後果

### 正面

- deterministic intelligence 會更誠實、可解釋、可測試
- query groups / reformulation ladders / reference pages 能在沒有 LLM / embedding 時提供高價值功能
- taxonomy / source role / open-loop signals 有清楚 precedence 與 invalidation contract
- intelligence modules 可先走 internal plugin 化，再逐步討論 sandbox

### 負面

- 中國大陸 / 美國核心、再加上台灣 / 日本 / 韓國 / 歐洲 / 俄羅斯 / international 的 taxonomy rule packs，會形成持續的內容維運成本
- 沒有 embedding 時，thread merge 與 long-horizon topic family 的 recall 上限會比較保守，必須接受較多 `unknown` / `low-confidence`
- search-term evidence 的跨瀏覽器 coverage 不均；query group 必須設計 evidence tier，而不能假裝所有資料源都同等強
- 若未來要開放 third-party insight plugins，仍需另做 sandbox 與 permission boundary 決策

## 需要被後續 supersede 的現況

在本 ADR 被接受之前，repo 內這些現行假設仍然存在，但不應再被擴寫：

- `features/intelligence.md` 中的 estimated dwell、session duration、embedding-first thread / topic baseline
- `vault-core/src/insights.rs` 內以 `duration_ms`、session merge、topic centroid 為主的 deterministic pipeline
- 單一 `source_role` / `page_type` 的 global heuristic map

一旦此 ADR 轉為 `Accepted`，上述內容需要由 M5 文檔與實作正式 supersede，而不是局部修字眼後繼續沿用。

## 相關

- [../../features/deterministic-intelligence.md](../../features/deterministic-intelligence.md)
- [../../features/intelligence.md](../../features/intelligence.md)
- [../data-model.md](../data-model.md)
- [../module-boundary-map.md](../module-boundary-map.md)
- [../../plan/m5-deterministic-intelligence/README.md](../../plan/m5-deterministic-intelligence/README.md)
- [../../plan/program/research-and-decisions.md](../../plan/program/research-and-decisions.md)
