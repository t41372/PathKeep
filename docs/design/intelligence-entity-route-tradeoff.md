# Intelligence Day / Domain Entity Route Trade-off

> **狀態：Accepted**
> **日期：2026-04-19**
> **範圍：** `/intelligence`、`/intelligence/day/:date`、`/intelligence/domain/:domain`、Dashboard、Explorer 的 day/domain deep-link contract
> **關聯文檔：**
>
> - [screens-and-nav.md](screens-and-nav.md)
> - [ui-review-guardrails.md](ui-review-guardrails.md)
> - [../features/intelligence.md](../features/intelligence.md)
> - [../features/intelligence-current-state.md](../features/intelligence-current-state.md)
> - [../features/core-intelligence-ultimate-design.md](../features/core-intelligence-ultimate-design.md)
> - [../plan/m6-shared-insight-surfaces/README.md](../plan/m6-shared-insight-surfaces/README.md)
>
> **2026-04-19 M7 follow-up:** 更通用的 `query family` / `refind page` / `session` / `trail` entity promotion 已由 [intelligence-generic-entity-navigation-tradeoff.md](intelligence-generic-entity-navigation-tradeoff.md) 接手；本文件現在只定義 M6 的 day/domain baseline。

---

## 1. 問題定義

Core Intelligence 的計算層其實已經有 shared rollup / section envelope / registry 基礎，但 UI 仍長期卡在「實體沒有被正式建模」：

1. `domain` 只有孤立的 `/intelligence/domain/:domain` route
   - 不同卡片有的能跳、有的不能跳、有的又跳去 `/explorer`
2. `day` 長期停留在 `Browsing Rhythm` 的 page-local detail
   - 同一種「某一天的完整洞察」沒有 single source of truth
3. 多數 surface 仍直接拼 `/explorer` URL
   - route grammar、scope/window、primary CTA 行為在各處不一致

這造成的不是單純 UX 不整齊，而是：

- day/domain 的完整資訊被分裂到多個局部實作
- card 自己拼 read model，重複造輪子
- 使用者在不同地方點同一個 domain / day，結果卻不一致

---

## 2. 約束

- 不新增新的 intelligence schema plane，也不為 day/domain 再做 snapshot-first payload
- 優先重用既有 rollups / entities / derived tables
- mutation control 仍留在 Settings / Jobs，不在 analysis route 上擴寫
- 共享 profile scope 與 route query grammar 必須保持一致
- 這次只先把 `day` / `domain` 升格；更通用的 entity navigation 抽象留到下一輪

---

## 3. 候選方案

### 方案 A — 保持 overview inline detail + 各卡片各自 deep-link

**做法**

- `Browsing Rhythm` 仍在卡內顯示 selected-day detail
- `domain` 是否可點、點去哪裡，由各卡片自己決定
- `/explorer` 繼續作為 day/domain 的主要 evidence 出口

**優點**

- 短期改動最小
- 不需要新增 day route

**缺點**

- day/domain 仍然沒有 first-class shared contract
- 卡片之間會繼續各自實作 date/domain drilldown
- 使用者行為不一致，維護成本持續上升

**結論**

- 不能解掉這次最核心的耦合問題

### 方案 B — 把 day / domain 升格成 first-class shared routes

**做法**

- 新增 `/intelligence/day/:date`
- 保留 `/intelligence/domain/:domain`，但正式視為 `Domain Insights`
- shared helper 統一產生：
  - `dayInsightsHref`
  - `domainInsightsHref`
  - exact-day Explorer evidence href
- overview / dashboard / explorer 只保留 entry / digest / secondary CTA

**優點**

- day/domain 都有唯一完整頁面與唯一 link grammar
- 可以把 read model 與 i18n / scope / evidence 行為收斂成 single source of truth
- 其他 surface 只需接入口，不必再重組完整 detail

**缺點**

- 需要新增 route 與 shared helper 層
- 舊測試與舊文檔裡的「卡內 day detail」契約都要同步更新

**結論**

- 最符合這次目標，也最容易為 M7 的 generic entity navigation 打底

### 方案 C — 保持 overview route，但改成 drawer / modal 顯示完整 entity detail

**做法**

- day/domain 都仍掛在當前頁面上，用 drawer / modal 承接完整 detail

**優點**

- 視覺上連續
- 不需要讓使用者真的切 route

**缺點**

- state、URL、refresh、shareability 都會變得更複雜
- 很容易又回到「完整邏輯藏在 consumer local state」的舊問題
- 與 Explorer / Dashboard 的跨頁共用契約更難收斂

**結論**

- 不適合作為這次消耦合的 baseline

---

## 4. 最終決定

採用 **方案 B**：

- `day insights` 與 `domain insights` 都是 first-class entity surface
- `/intelligence` overview、Dashboard、Explorer 只做 digest / entry / secondary CTA
- day/domain 的 primary interaction 一律先進完整 insights route
- Explorer evidence 降為 secondary CTA，而不是主點擊

---

## 5. 具體接受的行為變更

1. `Browsing Rhythm` 點日格後，預設進 `/intelligence/day/:date`
   - 不再以卡內 selected-day detail 作為主工作流
2. `/intelligence/domain/:domain` 的 user-facing IA 正式升格為 `Domain Insights`
3. `/intelligence` 頂部新增 `Insight Access` strip
   - 直接打開 day/domain 完整頁面
4. Explorer detail rail 新增 `Open day insights` / `Open domain insights`
5. 凡是已經顯示 domain/day 實體且不需要新增每卡獨立 fetch 的 active surface，都應優先接 shared href helper

---

## 6. 風險與緩解

### 風險 1：overview 失去「就地看 detail」的感覺

**緩解**

- route-first 不是砍掉 detail，而是把完整 detail 移到可重整、可分享、可複用的頁面
- overview 仍保留 digest / CTA / context

### 風險 2：shared entity helper 還不夠泛化

**緩解**

- 本輪只先收斂 day/domain
- 更通用的 query family / refind page / source / trail entity navigation 留到 M7

### 風險 3：仍有零星 surface 尚未接入

**緩解**

- M6 當時必須在代碼中留下 `TODO: M7`
- 這些 follow-up 現已由 M7 收口；剩餘更深的 identity / context reuse gap 改由 M8 追蹤

---

## 7. 回滾策略

若未來證據顯示 entity-first route 不適合某些 surface，回滾不能退回「各卡片各自拼 day/domain detail」。

正確回滾路徑只能是：

1. 保留 shared route / shared helper / shared read model
2. 只在個別 surface 上調整 primary CTA 呈現方式

不允許回到 consumer-local composition 重新分裂 route grammar。

---

## 8. 後續演進

M6 只處理 `day` / `domain` baseline；後續 generic entity promotion 已在 M7 完成第一輪 closeout。

- `query family`、`refind page`、`session`、`trail` 的 first-class route 見 [intelligence-generic-entity-navigation-tradeoff.md](intelligence-generic-entity-navigation-tradeoff.md)
- 剩餘的 aggregate identity / context focus gap 已轉交 [M8](../plan/m8-aggregate-entity-identity/README.md)

---

## 9. 使用者確認記錄

2026-04-19，使用者已明確要求：

- day/domain insights 必須成為全局共用基礎模組
- 不能再讓各處各自實作、各自決定點擊行為
- `Browsing Rhythm` 不該再以卡內 detail 作為主工作流
- 後續還要以本輪經驗為基礎，進一步做跨 app reuse audit

因此本文件作為 accepted day/domain baseline 記錄；generic entity promotion 的後續 accepted contract 需與 M7/M8 文檔一起閱讀。
