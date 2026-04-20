# Search Activity 改成 Search Keywords Browser — Trade-off 決策

> **狀態：Accepted**
> **日期：2026-04-20**
> **範圍：** `/intelligence` 的 `Search Activity`、`/intelligence/domain/:domain`
> **關聯文檔：**
>
> - [screens-and-nav.md](screens-and-nav.md)
> - [ui-review-guardrails.md](ui-review-guardrails.md)
> - [../features/intelligence-current-state.md](../features/intelligence-current-state.md)
> - [../features/core-intelligence-ultimate-design.md](../features/core-intelligence-ultimate-design.md)

---

## 1. 問題定義

目前 shipped `Search Activity` 有兩個已被真實資料證偽的問題：

1. `Top Search Concepts` 用詞雲呈現，但詞雲同時放大了噪音 query token。
   - 使用者已明確回報 `https`、`asu` 這類看起來更像貼上網址或導覽輸入的字串，出現在最前面。
   - 問題不只是「圖不好看」，而是 keyword-facing surface 沒有先把 navigational noise 排除。
2. `Recent Queries` 雖然補上了 reusable identity，但目前仍偏向 additive card list。
   - 它缺少 bounded review surface 應有的 pagination / page-size / nested date filter。
   - 同樣的 search-keyword browsing 能力也還沒進 `Domain Insights`，導致 search-engine domain drilldown 只剩入口比例，沒有實際查過什麼。

這代表目前的 truth drift 同時出在 **資料口徑** 與 **workbench 層級**。

---

## 2. 約束

- 不新增新的 route grammar；仍沿用既有 shared `query-family` / `trail` / Explorer evidence contract。
- `Search Keywords` 仍維持 distinct `(search_engine, normalized_query)` row，不退回每一次 search event 各自成列。
- `/intelligence/domain/:domain` 仍沿用 overview 的 `range` / `start` / `end` / `profileId` query contract，不為內嵌 keyword browser 再長第二套 URL state。
- `Search Activity` 依然是 summary-card family，而不是把整個 Explorer query workbench 重新塞回 `/intelligence`。
- Domain route 的 archive-wide / scoped honesty 仍要保留，但不能再用 oversized callout 吃掉首屏。

---

## 3. 候選方案

### 方案 A — 保留詞雲，只補 stop words

**做法**

- 繼續用詞雲顯示 concept。
- 只在 tokenizer 補更多 stop words，如 `http` / `https` / `www` / `com`。

**優點**

- 改動最小。

**缺點**

- 詞雲仍然不利於精準比較排名。
- 問題不只在 stop words；還有整筆 query 本身其實是 pasted URL / hostname-like navigation。
- 無法順便解決 search history browser 太輕量、Domain Insights 缺 keyword list 的問題。

**結論**

- 不接受。

### 方案 B — Search Activity 保持 aggregate + additive，但把 keyword browsing 升格成真正的 paged browser

**做法**

- `Top Concepts` 保留 aggregate summary，但改成 horizontal bar chart。
- `Recent Queries` 改名為 `Search Keywords`，補齊 bounded browser controls：
  - text filter
  - engine filter
  - nested date subrange
  - sort
  - explicit pagination / page size
- 同一個 browser surface 重用到 `Domain Insights`，只在 search-engine domain 有資料時顯示。

**優點**

- 圖表改成更適合比較排名的 familiar form。
- keyword browsing 仍維持在 summary/workbench 邊界內，不需要重開 Explorer route。
- `Domain Insights` 能補上真正有意義的 search-engine-specific evidence。

**缺點**

- 需要前後端同時調整 request/filter contract。
- 需要定義 nested date filter 與 page reset 行為。

**結論**

- 採用。

### 方案 C — 直接把 search keyword history 做成獨立 route 或 Explorer 新 view

**做法**

- 新增 `/intelligence/queries`、`/explorer?view=queries` 或類似 workbench route。

**優點**

- 最完整。

**缺點**

- scope 明顯超出這輪問題修復。
- 重新打開 URL grammar、route ownership 與 page-level IA 決策。
- 和 M6–M8 已接受的 shared insights / Explorer 邊界衝突。

**結論**

- 不接受。

---

## 4. 最終決定

採用 **方案 B**。

### 4.1 Search Activity 的新 truth

- `Search Activity` 固定保留四個 tab：
  - `engines`
  - `concepts`
  - `search keywords`
  - `families`
- `Top Concepts` 是 aggregate summary，不再用詞雲；改為 ranked horizontal bar chart。
- `Search Keywords` 是 bounded browser surface，不是 additive 「多載幾張卡」。

### 4.2 Keyword-facing surface 的資料口徑

- `search_events` 需要明確區分：
  - 真正的 keyword search
  - URL-like / navigational noise
- `Top Concepts` 與 `Search Keywords` 只讀 keyword-eligible rows。
- `Query Family` 仍可保留既有 deterministic merge，但 keyword-facing summary 不再讓噪音 query 主導排名。

### 4.3 Domain Insights 的新 truth

- `Domain Insights` 對 search-engine domains 要顯示 domain-scoped `Search Keywords` section。
- 這個 section 重用同一個 paged keyword browser，只是 request 多帶 `domain` filter。
- 若當前 domain 沒有 keyword-eligible rows，就誠實隱藏，不做空佔位。

### 4.4 Domain route 的 scope honesty

- archive-wide / profile-scoped note 改成 compact inline strip。
- 它仍是 honesty chrome，但不再用 full-height orange callout 佔掉首屏。

---

## 5. 接受的介面行為

1. `Recent Queries` 正式改名為 `Search Keywords`。
2. `Search Keywords` 預設顯示 distinct normalized keyword rows，而不是每一次 search event。
3. nested date filter 只允許縮小到當前 route window 內部，不允許超出頁面時間窗。
4. page / page-size / sort / filter 任一改變時，都要回到第 1 頁。
5. row 若已有 `familyId`，primary CTA 仍是 shared `query-family insights`；其餘 drilldown 只允許 `trail insights` 與 Explorer evidence。
6. Domain Insights 內若同一 host 映到多個 engine id，row 必須保留 engine label，且 domain-scoped browser 仍可按 engine filter。

---

## 6. 風險與緩解

### 風險 1：heuristic 過嚴，把合法 query 當成 navigational noise

**緩解**

- 判斷不能只靠 token stop words；還要參考 query shape 與 trail landing continuity。
- 保留 Rust regressions 覆蓋 Google / BiliBili / GitHub / custom rule 的合法 query。

### 風險 2：Search Keywords browser 變成半套 Explorer

**緩解**

- 明確限制它只做 distinct keyword rows + bounded filters + pagination。
- 不新增新的 route、不做 evidence-side grouped detail rail。

### 風險 3：Domain Insights 首屏再次被 support chrome 壓過

**緩解**

- scope note 固定降成 compact strip。
- entity hero、KPI 與 domain-specific evidence 保持優先閱讀順序。

---

## 7. 回滾策略

如果未來要調整 `Search Keywords`，可接受的回滾只有：

1. 保留 keyword/noise classification，但改變 browser controls 的呈現形式。
2. 把 Domain Insights 的 keyword section 重新排序或改成 secondary disclosure。

不允許回滾到：

- keyword-facing surface 混入 URL-like navigation noise
- `Top Concepts` 再次回到詞雲
- 為了做 keyword history 重新打開新的 Explorer / route grammar

---

## 8. 使用者確認記錄

2026-04-20，使用者明確要求：

- 修正 `Search Activity` 裡把 `https` / `asu` 之類導覽型字串排進常搜概念的算法問題
- 詞雲改為和其他 insights 卡片一致語法的長條圖
- `Search Activity` 補上可排序 / 篩選 / 動態載入的 search-keyword browsing
- `Domain Insights` 對 search-engine domains 額外顯示該網站上的搜索關鍵詞
- Domain route 頂部過大的橘色 box 要縮小

因此本文件作為 accepted docs 更新前的 trade-off 記錄，後續 source-of-truth 應以此決定為準。
