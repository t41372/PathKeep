# M8 — Aggregate Entity Identity And Context Reuse

> 目標：在 M7 已建立 generic insight-entity navigation 與 promoted routes 之後，繼續清理仍缺 stable identity、context focus、或 aggregate digest reuse 的 intelligence entity surface。

---

## M8 的完成定義

- [x] 補齊 M7 故意 deferred 的 aggregate entity identity 缺口
- [x] 讓 remaining entity surface 不再依賴 ad-hoc label、best-effort parsing、或 page-local context focus
- [x] 把 reusable entity IDs / context focus / aggregate digest slot 再往 shared contract 收斂一輪
- [x] 所有 `TODO: M8` 與 source docs / status / backlog tracking 保持一致

---

## 首批範圍

- path flow stable step identity
- compare set full detail / promotion boundary
- domain / promoted entity context highlighting
- external-output payload 內更多 reusable entity IDs
- aggregate digest / slot reuse 與 evidence hierarchy 再抽象

---

## 當前待辦

- [x] `M8-001` 盤點目前仍靠 best-effort parsing 或弱 anchor 才能落到 shared destination 的 entity surface
- [x] `M8-002` 決定哪些 aggregate entity 值得補 stable ID / detail read model，哪些維持解析到既有 route
- [x] `M8-003` 設計 context focus / highlight contract，避免 route 因 consumer-local state 再分裂
- [x] `M8-004` 收斂 external-output payload reuse 與 remaining `TODO: M8` tracking

## 2026-04-19 salvage note

- `Search Activity` 的 `Recent Queries` 現在已成為第一個明確的 M8 seed：query-history rows 不再只靠 raw label，而是直接攜帶 reusable `familyId` / `trailId` / `profileId`。
- search-engine rule matching 也已從 branch salvage 成目前的 Settings derived-state surface；custom host/path/query-param rules 會和 built-ins 一起參與 deterministic rebuild，但沒有重開 Explorer `queries` view 或任何新的 URL grammar。
- `WORK-M8-A` 現已完成：`compare set` 正式升格成 first-class shared route、path-flow 改成 stable `flowId` + typed `steps`、shared non-overview entity routes additive 支援 `focusType` / `focusId`，而 trusted external-output payload 也補上 structured entity targets；`public snapshot` 維持 redacted 邊界。

---

## 已知種子與參考

- [../m7-reuse-audit/README.md](../m7-reuse-audit/README.md)
- [../../design/intelligence-generic-entity-navigation-tradeoff.md](../../design/intelligence-generic-entity-navigation-tradeoff.md)
- [../../design/screens-and-nav.md](../../design/screens-and-nav.md)
- [../../features/intelligence-current-state.md](../../features/intelligence-current-state.md)

---

## 邊界

- 不推翻 M6 / M7 已接受的 entity-first route baseline
- 不為了 context focus 重新引入 consumer-local fetch / URL grammar 分叉
- 不新增新的分析算法里程碑；M8 仍以 identity / reuse / navigation contract 為主

---

## 2026-04-19 closeout note

- compare-set 現在已正式升格成 `/intelligence/compare-set/:compareSetId`，並補齊 dedicated detail read model、shared hero/actions、recent compare days、以及 generic `explain_entity` ownership。
- shared non-overview routes 現在可 additive 接受 `focusType` / `focusId`，目前只開放 `compare-set` 與 `path-flow`；overview 會在返回時主動清掉 focus，不再讓 consumer-local state 漫延。
- `path flow` 不再由前端 parsing `flowPattern` label 決定 routeable step；backend 現在會回 stable `flowId` 與 typed `steps`。
- Settings external outputs 與 `browser-snippet-v1` trusted local host 現在都優先根據 structured entity targets 產生 app links；`public snapshot` 明確不帶 `familyId` / `trailId` / `compareSetId` / `canonicalUrl` 這類 internal reusable IDs。
- 下一輪 milestone 改為 [M9 — Cross-App Reuse Audit And Shared Composition](../m9-cross-app-reuse/README.md)，用來全面盤點剩餘 consumer-local composition、shared digest / CTA / evidence 抽象，以及新的 `TODO: M9` / `TODO: M10` 邊界。
