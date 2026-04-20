# 里程碑

> 從 [vision-and-requirements.md](vision-and-requirements.md) 抽出。
>
> ⚡ **半里程碑 work block 與工作進度追蹤** → 見 [plan/README.md](plan/README.md)

---

## M0 — 重構基礎（新增）

第一個重寫里程碑：直接拆掉舊骨架，建立新架構。

- 舊版前端全部移除，照設計師新版設計稿建立前端骨架
- Rust workspace 重組：新增 `browser-history-parser` 獨立 crate
- Schema 審查與 migration 系統規劃
- 統一產品名稱為 PathKeep

📋 詳細待辦 → [plan/m0-foundation/README.md](plan/m0-foundation/README.md)

## M1 — Solid Archive

核心目標：把「長期保存與可恢復」做對。

- 正式 schema migration 系統（編號 SQL 檔 + migration 表）
- 增量備份（Chromium）完全可用
- 排程設定（macOS LaunchAgent）
- Run / Import / Rollback 正確的 operation model（軟刪除）
- 基本加密/不加密選擇
- 審計 manifest + hash chain
- Archive 快照 safety net（含 retention 上限）
- 歷史紀錄瀏覽和搜尋（FTS5 + Regex toggle）
- HTML/JSONL 匯出

📋 詳細待辦 → [plan/m1-solid-archive/README.md](plan/m1-solid-archive/README.md)

## M2 — Recall & Trust

- Google Takeout 導入（含 dry-run, quarantine, 完整可回滾）
- 多瀏覽器支持（Firefox）
- Doctor 完整性檢查
- Run 歷史與回滾 UI
- Preview/Manual/Execute 全面落地
- i18n（en, zh-CN, zh-TW）
- Windows / Linux 排程正式驗證與支持

📋 詳細待辦 → [plan/m2-recall-and-trust/README.md](plan/m2-recall-and-trust/README.md)

## M3 — Intelligence

- AI provider 配置 UI
- AI 計算任務系統（Job Queue）
- Embedding pipeline（rig.rs + LanceDB sidecar）+ 語義搜尋
- 基礎洞察：Topic timeline, On This Day, Site Analytics, 定期總結
- Ask My History（AI 問答，rig.rs 驅動 agentic RAG）
- MCP server + AI IDE Skill

📋 詳細待辦 → [plan/m3-intelligence/README.md](plan/m3-intelligence/README.md)

## M4 — Full Intelligence & Polish

- 完整洞察套件：Thread detection, Open Loops, Contrastive Summary, Explore/Exploit 等
- Enrichment 插件系統（arXiv, GitHub, YouTube 等）
- S3 遠端備份
- 地理位置記錄（實驗性）
- 多平台完整驗證
- Loading States & Skeleton Screens：全頁面 loading 狀態規範，參見 `docs/design/ux-principles.md` §4
- Profile-Scoped Insights：洞察系統支援 profile 級別篩選，參見 `docs/features/intelligence.md` §Profile-Scoped Insights

### Post-M4 / Blocked

- [!blocked] App Lock（Biometric / Password）：應用程式鎖定功能，阻於 `PG-RD-PLAT-006` 安全研究。參見 `WORK-M4-C`、`docs/features/archive.md` §8

📋 詳細待辦 → [plan/m4-full-polish/README.md](plan/m4-full-polish/README.md)

## M5 — Deterministic Intelligence / Runtime & Extensions

- 用 evidence-first deterministic baseline 取代 session / dwell-centric 假設
- Query groups、reformulation ladders、reference pages、source effectiveness、open loops 正式成為 no-AI 也可 shipping 的 intelligence surface
- deterministic modules、enrichment runtime、queue review、evidence controls 與 shared freshness / provenance drawer 已正式收斂成 shipping contract
- current-host `14.4M / 60y` long-horizon benchmark signoff 已落地；後續若要再補更深的 entity reuse，不再回頭重開 M5，而是進入新的 follow-up milestone

📋 詳細待辦 → [plan/m5-runtime-and-extensions/README.md](plan/m5-runtime-and-extensions/README.md)

## M6 — Shared Day And Domain Insights

- 把 `day` 與 `domain` 升格成 first-class shared insights entity
- 新增 `/intelligence/day/:date`，並把 `/intelligence/domain/:domain` 正式視為 `Domain Insights`
- backend 新增 `get_day_insights` typed read model；frontend 收斂 shared href grammar
- Dashboard、Intelligence、Explorer 優先接入 route-first day/domain entry，而不是各處各自重做完整 detail

📋 詳細待辦 → [plan/m6-shared-insight-surfaces/README.md](plan/m6-shared-insight-surfaces/README.md)

## M7 — Cross-App Reuse Audit And Insight Entity Consolidation

- 全面盤點 app 內仍然重複造輪子的 intelligence entity surface
- 抽出 generic insight-entity navigation / digest component / route grammar
- 清理 M6 留下的 `TODO: M7` 與其對應 docs/backlog tracking
- 為 query family、refind page、source、session/trail、category mix、external-output review surface 建立一致的 single source of truth

📋 詳細待辦 → [plan/m7-reuse-audit/README.md](plan/m7-reuse-audit/README.md)

## M8 — Aggregate Entity Identity And Context Reuse

- 補齊 M7 故意 deferred 的 aggregate entity identity 與 context focus 缺口
- 決定哪些 aggregate entity 值得補 stable ID / full detail read model，哪些維持解析到既有 shared route
- 收斂 external-output payload 內更多 reusable entity IDs 與 aggregate digest slot reuse
- 把所有新的 deferred reuse gap 明確改用 `TODO: M8` 與 source docs / status 對齊

📋 詳細待辦 → [plan/m8-aggregate-entity-identity/README.md](plan/m8-aggregate-entity-identity/README.md)

## M9 — Cross-App Reuse Audit And Shared Composition

- 全面盤點 app 內仍然重複造輪子的 UI composition、helper、read-model glue 與 review chrome
- 抽出 shared digest / CTA / evidence / focus / review composition，降低 Dashboard / Intelligence / Explorer / Settings 間的 drift
- 把新的 reuse debt 改記 `TODO: M9` / `TODO: M10`，並同步回寫 `STATUS.md`、`BACKLOG.md` 與 source docs
- 延續 M6–M8 已接受的 entity-first / focus / trusted-output 邊界，不回退成 consumer-local state

📋 詳細待辦 → [plan/m9-cross-app-reuse/README.md](plan/m9-cross-app-reuse/README.md)

## M10 — Workbench Reuse And Transport Hygiene

- 收斂仍然 consumer-local 的 workbench / review row composition，尤其 `refind`、Explorer detail/session/trail 與 richer Settings review surfaces
- 決定哪些 intelligence route / desktop glue 值得正式拆分，哪些只保留 inventory 而不重構
- 清理 M9 留下的 `TODO: M10`，避免 reusable UI 與 transport hygiene 再度混在同一輪裡
- 延續 M9 的 route-level shared composition baseline，不回退成各頁各自拼 CTA / review chrome

📋 詳細待辦 → [plan/m10-workbench-reuse/README.md](plan/m10-workbench-reuse/README.md)

## M11 — App-Wide Reuse And Shared Review Grammar

- 盤點全 app 仍然重複造輪子的 review / PME / diagnostics surface，建立 single-source map
- 決定 `src/lib/intelligence.ts`、dev IPC mirror、以及剩餘 `vault-worker` pass-through 是否值得繼續正式拆分
- 抽出一輪跨 route 的 shared review / code-preview / target-link / verify-result grammar
- 延續 M10 的 workbench reuse / transport hygiene baseline，不回退成各頁再手搓 trust / review chrome

📋 詳細待辦 → [plan/m11-app-wide-reuse/README.md](plan/m11-app-wide-reuse/README.md)

## M12 — Shared Support Actions And Diagnostics Decomposition

- 延續 M11 的 neutral review primitive，把 copy / open-path / support action 再收斂成 shared grammar
- 抽出 diagnostics rows / support summary，降低 Settings / Import / Audit / Jobs 的 page-local drift
- 決定 Settings mega-route 還有哪些 slices 值得繼續拆，而不是再靠同一支 mega-file 吸收新 surface
- 評估是否需要更輕量的 transport parity automation，而不是直接重開 codegen / manifest 專案

📋 詳細待辦 → [plan/m12-support-actions-and-diagnostics/README.md](plan/m12-support-actions-and-diagnostics/README.md)

## M13 — Broad Reuse Audit Across Support, Trust, And Workflow Surfaces

- 以 support / trust / workflow surface 為主題繼續盤點 reusable grammar，而不是再把 reuse 只局限在單一路由或單一 primitive
- 延續 M12 的 support-action single-source 方法，擴到 Jobs summary、Settings split、Explorer / Import / Audit / Schedule follow-through
- 讓 transport parity 繼續保持 subordinate inventory；除非 owner drift / maintenance cost 證據升高，否則不重開 codegen / manifest 主線

📋 詳細待辦 → [plan/m13-broad-reuse-audit/README.md](plan/m13-broad-reuse-audit/README.md)
