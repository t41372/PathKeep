# Intelligence Generic Entity Navigation Trade-off

> **狀態：Accepted**
> **日期：2026-04-19**
> **範圍：** `/intelligence` overview、`/intelligence/query-family/:familyId`、`/intelligence/refind/:canonicalUrl`、`/intelligence/session/:sessionId`、`/intelligence/trail/:trailId`，以及 Explorer / Settings external outputs / shared entity CTA
> **關聯文檔：**
>
> - [intelligence-entity-route-tradeoff.md](intelligence-entity-route-tradeoff.md)
> - [screens-and-nav.md](screens-and-nav.md)
> - [ui-review-guardrails.md](ui-review-guardrails.md)
> - [../features/intelligence-current-state.md](../features/intelligence-current-state.md)
> - [../features/core-intelligence-ultimate-design.md](../features/core-intelligence-ultimate-design.md)
> - [../plan/m7-reuse-audit/README.md](../plan/m7-reuse-audit/README.md)
> - [../plan/m8-aggregate-entity-identity/README.md](../plan/m8-aggregate-entity-identity/README.md)

---

## 1. 問題定義

M6 已經把 `day` / `domain` 收斂成 first-class shared route，但 app 內仍有大量 active intelligence entity 停留在 consumer-local 行為：

1. `query family`、`refind page`、`session`、`trail` 還沒有 single-source destination
2. `reopened investigation`、`stable source`、`habit`、`friction`、`compare set`、`path flow` 等卡片仍各自決定要不要跳、跳去哪裡
3. Settings external outputs、Explorer grouped view、`/intelligence` overview 各自手寫 action hierarchy

結果是：

- 同一個 entity 在不同 surface 上點擊行為不一致
- 各頁反覆手搓 `/explorer` query 或局部 detail CTA
- reusable route grammar、scope/window carry-through、evidence CTA 無法成為真正的 single source of truth

---

## 2. 約束

- 不推翻 M6 已接受的 `Insights first` baseline；M7 是在這條路上繼續推進，不是回頭做 page-local detail
- 不擴新的分析算法或 synthetic identity layer
- `session` / `trail` 在 Explorer 仍是 browse-first canonical grouped view；M7 只能補 shared insights entry，不能把 grouped Explorer 改成 route-only workflow
- `domain` route query contract 不加新的 `focus` / context-highlight param；若未來需要更細 context，留到後續 milestone
- `refind page` route 直接使用 encoded `canonicalUrl` 作 path param，不新建 synthetic ID

---

## 3. 候選方案

### 方案 A — 保持 entity-specific local composition

**做法**

- 每個 consumer 自己決定 entity 是否可點、跳到 `/explorer`、還是打開局部 explain/evidence

**缺點**

- 繼續放大跨 app 不一致
- shared scope / window grammar 無法收斂
- M6 打下的 `Insights first` baseline 會再次被 consumer-local 拼裝侵蝕

**結論**

- 不接受

### 方案 B — 只再升格一兩個 route，其餘仍用 local CTA

**做法**

- 只把最顯眼的 `query family` 或 `refind page` 做成 route，其他 entity 暫留原狀

**缺點**

- 會繼續留下一批「看起來很像已抽象、實際上仍各自為政」的混合狀態
- 無法真正形成 reusable entity contract

**結論**

- 不足以完成 M7 的 reuse audit 目標

### 方案 C — 所有 active intelligence entity 都先有 shared destination，再決定是否需要獨立 read model

**做法**

- 抽 shared entity target / href grammar / CTA chrome
- `query family`、`refind page`、`session`、`trail` 升格成 first-class shared insights route
- 其餘 entity 也必須解析到單一 shared destination：
  - `reopened investigation` → anchor route
  - `habit` / `stable source` / `friction` / `multi-browser domain` → `domain insights`
  - `compare set` → `trail insights`
  - `path flow` 只有在 step 可穩定解析到 registrable domain 時才提供 domain insights CTA

**優點**

- active entity 全部有一致的 single-source destination
- shared CTA hierarchy、scope/window、evidence 行為可跨 app 復用
- 後續若要再升格更多 entity，可以在既有 contract 上 additive 擴充

**缺點**

- 需要明確區分「first-class route」與「解析到現有 route 的 shared destination」
- 需要把 remaining identity gap 誠實寫回下一輪 milestone，而不是假裝這輪全解

**結論**

- 採用

---

## 4. 最終決定

採用 **方案 C**。

### 4.1 First-class shared routes

M7 正式升格四個 entity 為 first-class insights route：

- `/intelligence/query-family/:familyId`
- `/intelligence/refind/:canonicalUrl`
- `/intelligence/session/:sessionId`
- `/intelligence/trail/:trailId`

其中：

- `query-family`、`refind` 新增 dedicated read model
- `session`、`trail` 直接重用既有 detail read model
- 這四條 route 一律沿用 `range` / `start` / `end` / `profileId` query grammar

### 4.2 Generic entity contract

所有 active intelligence entity 必須經過 shared entity contract 解析成單一 target：

- 統一 `kind`
- 統一 path/query grammar
- 統一 primary CTA / secondary evidence CTA / explainability slot
- 統一 scope carry-through

### 4.3 Non-promoted entities 仍需有 shared destination

M7 不接受「先保留 static chip，以後再說」。

因此：

- `reopened investigation` 解析到既有 anchor route
- `habit`、`stable source`、`friction`、`multi-browser diff` 這類 domain-based entity 一律走 `domain insights`
- `compare set` 一律走 `trail insights`
- `path flow` 只有當 step 可穩定解析成 registrable domain 時才接 `domain insights`；否則保持誠實的非互動 text，並把缺口移到 M8

---

## 5. 接受的行為變更

1. `/intelligence`、day/domain pages、Explorer grouped/detail surfaces、Settings external outputs 不再各自手寫 entity action strip
2. `Refind` 類 surface 的 primary CTA 改為 refind insights route；Explorer evidence 與 domain insights 降為 secondary CTA
3. Explorer grouped `session` / `trail` header 保留 inline expand，但要新增明確的 `Open ... insights` CTA
4. `Search Effectiveness` hardest topics、day-insights query families、`Search Activity` query families 改成 query-family insights route
5. Settings external outputs 的 top domain / discovery-date chips 改吃 shared entity href，而不是留 static label
6. `domain insights` route 暫不增加 context/focus query；context highlighting 留到 M8

---

## 6. 風險與緩解

### 風險 1：session/trail route 會和 Explorer canonical view 打架

**緩解**

- 明確規定 grouped Explorer 仍是 browse-first canonical surface
- shared insights route 只承接 reusable detail / explainability / evidence CTA

### 風險 2：entity promotion 變成無止境地新增 route

**緩解**

- 只有具備獨立 detail read model，或低成本可補齊者，才升格成 first-class route
- 其他 entity 先解析到既有 shared destination，不新增 page-local 拼裝

### 風險 3：`path flow` / compare-set / context highlighting 仍有 identity 缺口

**緩解**

- 明確立項 `M8 — Aggregate Entity Identity And Context Reuse`
- 所有 deferred code comments 一律改記 `TODO: M8`

---

## 7. 回滾策略

若未來要調整這套 entity navigation，不能回滾成 consumer-local 拼裝。

可接受的回滾方式只有：

1. 保留 shared entity contract 與 shared destination
2. 調整某些 surface 的 CTA 形式或預設 landing page
3. 在有充分證據時，把某個 promoted route 再收斂回既有 shared destination

不允許回到每張卡各自決定 URL grammar / scope carry-through / evidence CTA。

---

## 8. M8 Follow-up

M7 完成後，下一輪只保留真正還缺 stable identity 或 context reuse 的部分：

- `path_flow` stable step identity
- `compare_set` full detail read model 是否值得升格
- `domain insights` / promoted entity route 的 context highlighting
- external-output payload 內更多 reusable entity IDs
- aggregate entity digest / slot reuse 再往前抽

這些後續全部收斂到 [M8 README](../plan/m8-aggregate-entity-identity/README.md)，不再回頭用 `TODO: M7` 漂浮追蹤。
