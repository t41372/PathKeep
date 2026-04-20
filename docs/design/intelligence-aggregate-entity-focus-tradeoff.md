# Intelligence Aggregate Entity Focus And Compare-Set Promotion Trade-off

> **狀態：Accepted**
> **日期：2026-04-19**
> **範圍：** `/intelligence/compare-set/:compareSetId`、shared non-overview insights routes 的 `focusType` / `focusId` query grammar、path-flow typed identity、以及 trusted external-output payload 的 structured entity targets
> **關聯文檔：**
>
> - [intelligence-generic-entity-navigation-tradeoff.md](intelligence-generic-entity-navigation-tradeoff.md)
> - [screens-and-nav.md](screens-and-nav.md)
> - [../features/intelligence-current-state.md](../features/intelligence-current-state.md)
> - [../features/core-intelligence-ultimate-design.md](../features/core-intelligence-ultimate-design.md)
> - [../plan/m8-aggregate-entity-identity/README.md](../plan/m8-aggregate-entity-identity/README.md)
> - [../plan/m9-cross-app-reuse/README.md](../plan/m9-cross-app-reuse/README.md)

---

## 1. 問題定義

M7 已經把 active intelligence entity 收斂到 shared destination，但刻意留下三個缺口：

1. `compare set` 仍只解析到 `trail insights`，aggregate identity 會在落地時被吃掉
2. `path flow` 與 compare-set 相關的 context highlighting 仍靠 consumer-local state 或 label parsing 維持，URL 不可分享、重整後也不可信
3. Settings external outputs 雖然已有 manual review / trusted local host，但 trusted payload 仍主要靠 raw `href`，缺少 reusable structured entity target

結果是：

- compare-set 仍不像真正的 first-class shared entity
- domain / day / trail 等 shared route 無法誠實表達「我是從哪個 aggregate context 進來的」
- path-flow 仍維持脆弱的 best-effort parsing
- trusted outputs 難以成為真正的 reusable app-link source of truth

---

## 2. 約束

- 不推翻 M6 / M7 已接受的 `Insights first` / entity-first route baseline
- 不回退成 consumer-local drawer、ephemeral store、或各頁自拼 deep-link grammar
- focus contract 必須是 additive、shareable、可重整的 URL state，而不是 overview 自己吞一套 page-local context
- focus 只屬於 non-overview shared entity routes；回 `/intelligence` overview 時必須清掉 focus，避免 overview 變成第二套 local workflow
- 不新增新的分析演算法；M8 只補 stable identity、detail read model、context reuse 與 payload contract
- reusable entity IDs 只允許進 trusted outputs；`public snapshot` 必須維持 redacted，不能把 internal entity ID 偷變成 public API

---

## 3. 候選方案

### 方案 A — 維持 M7 baseline

**做法**

- `compare set` 繼續只跳到 `trail insights`
- `domain` / `day` / `trail` 不增加 focus query
- path-flow 仍由前端 parsing label 決定是否可跳
- trusted outputs 只保留 raw `href`

**缺點**

- compare-set identity 仍會在落地時消失
- aggregate context 無法跨 route 分享或重整保存
- path-flow 仍維持脆弱的 best-effort parsing
- trusted payload 難以被其他 consumer 穩定重用

**結論**

- 不接受

### 方案 B — 用 consumer-local state 補 context

**做法**

- compare-set / path-flow card 點擊後，把 context 存在頁面 state、global store、或 local-only search param
- 共享 route 本身不承接 focus grammar

**缺點**

- URL 不可分享；重整後 context 消失
- 各 consumer 又會長回自己的 state contract
- 與 M6/M7 的 shared route baseline 衝突

**結論**

- 不接受

### 方案 C — compare-set 升格為 first-class route，shared routes additive 支援 focus grammar

**做法**

- 新增 `/intelligence/compare-set/:compareSetId`
- 非 overview 的 shared entity routes additive 支援 `focusType` / `focusId`
- `path flow` 改成 stable `flowId` + typed `steps`
- trusted outputs 改傳 structured entity targets；public snapshot 維持 redacted

**優點**

- compare-set 真正成為可分享的 single-source destination
- context highlight 可跨 route、跨 consumer、跨重整保持一致
- path-flow 不再靠人類可讀 label 逆推結構
- Settings external outputs 與 trusted local host 可重用同一份 typed target contract

**缺點**

- 需要明確限制 focus scope，避免 route grammar 無止境膨脹
- compare-set 需要補 detail read model / explainability ownership

**結論**

- 採用

---

## 4. 最終決定

採用 **方案 C**。

### 4.1 Compare Set 正式升格為 first-class shared route

- 新 route：`/intelligence/compare-set/:compareSetId`
- compare-set 需要 dedicated detail read model：至少包含 compare-set summary、related trail/session context、compared pages、recent days
- compare-set 也正式納入 generic `explain_entity` ownership，不再只停留在 consumer-local summary

### 4.2 Shared focus contract 採 additive query grammar

- focus query 只加在 **non-overview shared insights routes**
- grammar：`focusType=<type>&focusId=<id>`
- M8 正式支援兩種 focus type：
  - `compare-set`
  - `path-flow`
- `/intelligence` overview 不承接 focus state；回 overview 時必須主動清掉 focus
- 既有 scope/window grammar 不變：
  - `day` route 繼續只用 exact-day path + optional `profileId`
  - 其餘 shared entity route 繼續沿用 `range` / `start` / `end` / `profileId`

### 4.3 Path Flow 改用 typed identity，不再 parsing label

- `get_path_flows` 現在必須輸出 stable `flowId`
- 同時輸出 `steps[]`，每一步明確描述：
  - label
  - 是否可 route
  - registrable domain（若存在）
- 前端不得再從 `flowPattern` 字串拆出 domain chip

### 4.4 Trusted outputs 改用 structured entity targets

- `embed cards` / `widget snapshot` / trusted local host bundle 可帶：
  - `primaryTarget`
  - `secondaryTargets`
- Settings external-output review surface 與 `browser-snippet-v1` 一律優先用 structured target 產生 app links
- raw `href` 只保留為 fallback，不再是唯一真相

### 4.5 Public snapshot 維持 redacted

- `public snapshot` 不新增 internal routing IDs，例如：
  - `familyId`
  - `trailId`
  - `compareSetId`
  - `canonicalUrl`
- 允許保留 human-readable labels / aggregates，但不得把 redacted payload 變成隱性 internal entity API

---

## 5. 接受的行為變更

1. compare-set card / CTA 的 primary destination 改為 compare-set insights route，不再只是 trail route 的影子入口
2. Trail / Day / Domain shared routes 現在可顯示 compare-set 或 path-flow context callout / highlight strip
3. `path flow` domain chips 現在帶著 `focusType=path-flow&focusId=<flowId>` 進 shared domain insights
4. compare-set 相關的 day/domain/trail drilldown 現在帶著 `focusType=compare-set&focusId=<compareSetId>`，不再靠 consumer-local state 猜上下文
5. Settings external outputs 現在從 structured entity targets 產生 `Open insights` links；trusted local host bundle 也沿用同一份 contract

---

## 6. 風險與緩解

### 風險 1：focus query grammar 膨脹成任意 consumer-local context

**緩解**

- M8 只接受 `compare-set` / `path-flow`
- overview 不承接 focus
- 新 focus type 必須重開 trade-off / milestone，而不是偷加 query string

### 風險 2：compare-set route 與 trail route 重疊

**緩解**

- compare-set route 只承接 aggregate identity、compared pages、recent compare days、shared context CTA
- trail route 仍維持 browse / explain / evidence detail
- 兩者透過 focus contract 協作，而不是互相取代

### 風險 3：trusted output 與 public payload 邊界再次模糊

**緩解**

- structured targets 只出現在 trusted outputs
- public snapshot 明確保持 redacted，不能把 internal IDs 下放

---

## 7. 回滾策略

若未來要調整 compare-set promotion 或 focus contract，可接受的回滾只有：

1. 保留 shared route 與 structured entity contract
2. 調整哪些 route 接受哪些 focus type
3. 把 compare-set route 的 UI 收斂或簡化

不允許回到 consumer-local state、label parsing、或重新把 focus grammar 分散到各頁私有實作。

---

## 8. 後續里程碑

M8 完成後，下一輪正式進入 [M9 — Cross-App Reuse Audit And Shared Composition](../plan/m9-cross-app-reuse/README.md)：

- 全面盤點剩餘 consumer-local composition 與 duplicated helpers
- 收斂共享 digest / CTA / evidence / focus composition
- 把新的 deferred reuse gap 改記 `TODO: M9` / `TODO: M10`，不再沿用 `TODO: M8`
