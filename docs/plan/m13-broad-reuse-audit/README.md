# M13 — Broad Reuse Audit Across Support, Trust, And Workflow Surfaces

> 目標：延續 M12 的 support-action / diagnostics single-source 方法，把 reuse audit 從單一路由或單一 primitive 擴大到全 app 的 support、trust、workflow surface，而不是再讓 Settings mega-route 或 transport parity 單獨吃掉下一輪。
>
> **2026-04-20 pause note:** 使用者在 M13 開工後直接插單 `WORK-PERF-A`，要求先修 `/intelligence` large-archive 凍結與 route revisit 卡頓。M13 scope 沒被取消，但在 perf stop-ship block 收尾前不再往 reuse extraction 繼續推進。
>
> **2026-04-20 insert note:** 使用者後續又插單 `WORK-PERF-B`，要求先修 Onboarding 初始化 / 手動備份 / Takeout scan-import 造成的整體 UI freeze。該 block 已獨立完成，並以 off-main-thread Tauri command facade + import paint-first yield 收口；M13 scope 本身沒有變更。
>
> **2026-04-20 insert note:** 使用者另行插單 `WORK-CI-R`，要求先收斂 Search Activity keyword truth 與 search-engine domain deep-dive；該 block 已獨立完成並 append 到 `CHANGELOG.md`，不視為 M13 deliverable 的一部分。

---

## M13 的完成定義

- [ ] 盤點 app-wide support / trust / workflow surface 的 reusable grammar 與 owner map
- [ ] 決定哪些 Jobs / Settings / Import / Audit / Explorer follow-through 值得正式抽成 shared composition
- [ ] 收斂一輪高價值 shared support / workflow composition
- [ ] 把 transport parity 保持在 subordinate inventory，而不是重開 codegen / manifest 專案

---

## 首批範圍

- Jobs plugin / module summary rows與 runtime health composition
- Settings mega-route 的下一輪 owner split
- Import / Audit / Schedule / Security / Lock 的 workflow follow-through grammar
- Explorer export / support quick action 與其他 trust-repair affordance
- support / trust surface 的 code comments、`TODO: M13`、與 planning/source docs 對齊

---

## 建議工作塊

- `WORK-M13-A` — Broad Reuse Inventory Across Support / Trust / Workflow Surfaces
- `WORK-M13-B` — Shared Support / Workflow Composition Extraction

---

## 邊界

- 不回退 M6–M12 已 accepted 的 route / payload / review-shell / support-action boundary
- 不讓 Settings route split 壟斷整個 milestone；它只是一個 consumer，不是唯一主題
- transport parity 只在 inventory 證明 owner drift / maintenance cost 持續上升時才進下一步，不預設升格成主線

---

## Single-Source Map

| 契約 / 能力                                   | canonical owner                                             | M13 結論                                                                                                  |
| --------------------------------------------- | ----------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| neutral review shell                          | `src/components/review/`                                    | 延續 M11，不重開 owner                                                                                    |
| support-action / clipboard grammar            | `src/components/review/`                                    | 延續 M12，不重開 owner                                                                                    |
| runtime-boundary review card grammar          | `src/components/review/runtime-boundary-card.tsx`           | M13 正式升格；Jobs 與 Settings derived runtime review 共用                                                |
| Jobs route orchestration                      | `src/pages/jobs/index.tsx`                                  | 保留 route owner，只移走 runtime health / boundary composition                                            |
| Jobs runtime health composition               | `src/pages/jobs/runtime-health-section.tsx`                 | M13 第一輪 extraction 已落地                                                                              |
| Settings derived runtime detail orchestration | `src/pages/settings/derived-runtime-review.tsx`             | 仍是 consumer owner，但已改吃 shared runtime-boundary card shell                                          |
| shell bootstrap provider                      | `src/app/shell-data.tsx`                                    | 保留 public provider facade；不改 `useShellData()` 對外 contract                                          |
| shell runtime polling owner                   | `src/app/shell-runtime-status.ts`                           | M13-B follow-up 已落地；Sidebar / Jobs / digest runtime truth 仍走 shell shared source                    |
| Security workflow follow-through              | `src/pages/security/use-security-workflow.ts`               | M13-B follow-up 已落地；route shell 保留 fallback / focus / panel composition                             |
| Dashboard route fallback owner                | `src/pages/dashboard/route-fallback-*`                      | M13-B follow-up 已落地；resolver / renderer / archive-access probe 同屬 fallback owner                    |
| Browsing Rhythm card state owner              | `src/components/intelligence/browsing-rhythm-card-state.ts` | M13-B follow-up 已落地；API load / selected-year-day state / derived calendar model 不再混在 render shell |
| transport parity / worker pass-through        | existing Rust owners                                        | 維持 subordinate inventory，不升格成 M13 主線                                                             |

## Inventory Snapshot

### 本輪已證明值得抽離

- Jobs 的 plugin / module summary rows 與 runtime health summary，不再維持 Jobs-local review shell。
- Settings derived runtime review 與 Jobs runtime boundary 都依賴同一組 title / status / metric / notes / action row grammar。
- `src/components/review/` 現在不只承接 neutral review 與 support actions，也承接 runtime-boundary review shell。

### 仍維持後續 priority 的 hotspot

- 無。`src/components/ui.tsx` 已無 `PathRow` export 或 active consumer；path/copy/open grammar 目前由 `src/components/review/support-actions.tsx` 的 `ReviewPathActionRow` 承接。

### 這輪刻意不做

- 不把 `ShellDataContextValue` 對外 shape 改成新 contract。
- 不把 Settings route 再拆成另一輪 mega-route cleanup。
- 不碰 `backend.ts`、transport codegen、或 worker parity automation。

## 2026-04-21 WORK-M13-A Closeout

- inventory 已正式收斂成 app-wide owner map，而不是只有「下一個想拆什麼檔案」的口頭清單。
- `components/review` 現在明確多了一層 runtime-boundary card grammar；Jobs runtime health composition 與 Settings derived runtime review 是第一批 consumer。
- 下一輪 extraction priority 也已固定：先 shell owner / workflow-heavy routes，再處理 dashboard 與 browsing-rhythm layering。

## 2026-04-21 WORK-M13-B Slice Note

- 第一輪高價值 extraction 已落地：Jobs runtime health / plugin / module summary rows 不再維持 Jobs-local review shell。
- [`src/pages/jobs/index.tsx`](../../src/pages/jobs/index.tsx) 現在降到可接受的 route-shell 尺寸；runtime health / boundary section 已拆到 [`src/pages/jobs/runtime-health-section.tsx`](../../src/pages/jobs/runtime-health-section.tsx)。
- 2026-04-22 Import follow-through slice landed：`/import` 現在不再只把 wizard / workflow explainer / recent batch review 疊成同質化面板。route 會以 `new import wizard -> grouped scan report -> recent imports / selected batch / doctor repair` 的順序呈現，並直接吃 backend 提供的 `will-import / known-but-ignored / needs-review / parse-error` file classification、preview time range 與 detected locale；這讓 Takeout UI 終於能把「現在會導入什麼」與「為什麼某些檔案沒被導入」講清楚。
- 2026-04-23 shell runtime owner slice landed：`src/app/shell-data.tsx` 不再內嵌 queue/runtime refresh、in-flight dedupe 與 active/idle polling cadence；這些責任已抽到 `src/app/shell-runtime-status.ts`，並新增 focused hook tests 保護 locked neutral state、dedupe、fallback error 與 3s/15s polling backoff。
- 2026-04-23 Security workflow owner slice landed：`src/pages/security/index.tsx` 不再內嵌 posture load、unlock/keyring、lock 與 rekey mutation state machine；這些責任已抽到 `src/pages/security/use-security-workflow.ts`，route shell 現在只保留 fallback、deep-link focus、path-copy feedback 與 panel composition。
- 2026-04-23 Dashboard fallback owner slice landed：Dashboard bootstrap error path 不再在 route shell 內直接探 Security status；`src/pages/dashboard/route-fallback-access.ts` 現在 owns archive-access probe，與 existing fallback resolver / renderer 合成同一個 fallback owner。
- 2026-04-23 Browsing Rhythm state owner slice landed：`src/components/intelligence/browsing-rhythm-card.tsx` 不再同時 owns API load、selected-year/day state、calendar derivation 與 JSX composition；這些 state / derived-model responsibilities 已抽到 `src/components/intelligence/browsing-rhythm-card-state.ts`，Dashboard 與 `/intelligence` 的 public behavior 不變。
- 2026-04-23 PathRow retirement audit landed：legacy `PathRow` 已不是 active source；repo search 只剩 M13 planning references 與 `ReviewPathActionRow` 的 explanatory comment，因此 M13-B 不再需要新增 code slice 來 retire it。
- `WORK-M13-B` 已收口；`BACKLOG.md` 目前沒有可提升的未阻塞 work block。
