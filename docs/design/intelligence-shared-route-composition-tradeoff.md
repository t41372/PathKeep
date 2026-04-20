# Intelligence Shared Route Composition Trade-off

> **狀態：Accepted**
> **日期：2026-04-19**
> **範圍：** `/intelligence` overview 與 promoted routes 的 shared metric strip、query-family card、compare-set page list、section heading + evidence/freshness badge 對齊，以及 Settings trusted-output target label reuse
> **關聯文檔：**
>
> - [screens-and-nav.md](screens-and-nav.md)
> - [ui-review-guardrails.md](ui-review-guardrails.md)
> - [../features/intelligence-current-state.md](../features/intelligence-current-state.md)
> - [../features/core-intelligence-ultimate-design.md](../features/core-intelligence-ultimate-design.md)
> - [../plan/m9-cross-app-reuse/README.md](../plan/m9-cross-app-reuse/README.md)
> - [../plan/m10-workbench-reuse/README.md](../plan/m10-workbench-reuse/README.md)

---

## 1. 問題定義

M6–M8 已把 `day` / `domain` / `query family` / `refind page` / `session` / `trail` / `compare set` 的 destination grammar、focus grammar 與 structured target contract 收成 shared truth，但 UI 仍殘留一批高頻 consumer-local composition：

1. section title 與 `證據與新鮮度` badge 沒有共用 header grammar，導致 badge 會佔整行、hover hitbox 過大、與 card title 對不齊
2. route-level metric strips 在 `day`、`query family`、`refind`、`session`、`trail`、`compare set` 間大量重複
3. `query-family-card` 與 compare-set page rows 在 overview / detail routes 之間各自手寫，易 drift
4. Settings trusted output 的 structured entity targets 雖然已有 shared href grammar，label 卻還停在 consumer-local raw `kind`

結果是：

- route destination 已 shared，render composition 卻還是多份 implementation
- badge / card chrome 的桌面真機細節會在不同 section 間持續漂移
- 下次要再抽 shared workbench surface 時，會被這一層 presentation duplication 持續干擾

---

## 2. 約束

- 不推翻 M6–M8 已接受的 entity-first / focus / trusted-output 邊界
- 不新增新的 backend read model、desktop command、或 query grammar
- 不為了抽象而把所有 UI 都塞進單一 mega-component
- 這一輪優先處理 **route-level shared composition**，不是 Explorer / Settings / Jobs 全面視覺重設

---

## 3. 候選方案

### 方案 A — 維持 consumer-local render

**做法**

- 保留各 route / section 各自 render metric strip、query-family card、compare-set rows
- 只用小修 CSS 解掉 badge 問題

**缺點**

- destination shared 了，composition 還是重複
- 下次改 copy / CTA / focus carry-through 仍要多處同步
- 無法真正完成 M9「shared composition」的 closeout

**結論**

- 不接受

### 方案 B — 直接把 reuse 一路推到 backend / transport glue

**做法**

- 這輪順手重拆 Tauri command / worker bridge / TS invoke wrapper 的 repetitive pass-through
- 同時做前端 route component extraction

**缺點**

- 範圍過大，會把 M9-B 從 route composition 擴成 transport refactor
- 風險與 review 面積明顯超出本輪目標
- 容易在沒有 product 驅動的情況下重寫 desktop contract glue

**結論**

- 不接受

### 方案 C — 先抽 route-level shared composition，transport hygiene 留給下一輪

**做法**

- 以 front-end shared primitives 收掉最常重複的 route/header/card composition
- badge 佈局與 hover box 一起收斂到 shared header grammar
- backend / worker / bridge glue 延後到下一輪獨立處理

**優點**

- 直接命中目前最可見、最容易 drift 的 user-facing duplication
- 不改 accepted route / payload contracts
- 能把 M9 關在「shared composition」而不是「又一輪 architecture rewrite」

**缺點**

- `refind` detail/workbench rows、Explorer/Settings review rows、desktop transport glue 仍會留待下一輪

**結論**

- 採用

---

## 4. 最終決定

採用 **方案 C**。

### 4.1 本輪正式升格成 shared primitives 的內容

- route-level metric strip
- `query-family-card`
- compare-set page list / landing badge / focused-domain link row
- structured target label helper
- section heading + evidence/freshness badge 的 inline-end header grammar

### 4.2 Section meta badge 的 accepted UI 行為

- badge 是 section header chrome，不是獨立一列內容
- 在 summary card 上，badge 必須縮成 inline-end anchor，與 section title 同列
- hover / focus 命中區只允許包住 badge 自身，不得吃滿整個 card header row
- panel 仍維持 floating review panel，不在卡內展開 mutation controls

### 4.3 本輪明確不做的事

- 不重寫 `src-tauri/src/commands/intelligence.rs` / `worker_bridge` / `vault-worker` 的 pass-through glue
- 不把 `refind` / `session` / `trail` / Explorer detail/workbench rows 全部組件化
- 不重開新的 route promotion 或新的 focus type

---

## 5. 後續里程碑

下一輪正式進入 [M10 — Workbench Reuse And Transport Hygiene](../plan/m10-workbench-reuse/README.md)：

- 處理仍未抽出的 workbench/review rows（尤其 `refind`、Explorer detail/session/trail、Settings richer review surfaces）
- 再盤點是否值得拆 route / desktop transport glue
- 清理本輪留下的 `TODO: M10`
