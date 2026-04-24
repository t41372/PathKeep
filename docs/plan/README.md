> **⚡ 找下一個要做的 work block？直接看 [STATUS.md](STATUS.md)**

# PathKeep — 工作計劃與進度追蹤

> **Status:** Living document · **Rebuilt:** 2026-04-05  
> 本目錄是 PathKeep 的實作層 source of truth。  
> 產品願景、需求和設計定義在 [vision-and-requirements.md](../vision-and-requirements.md) 與它的子文檔裡；這裡回答的是 **接下來怎麼做、先做什麼、哪些事情卡住了、每個里程碑拆到哪一層**。

### 工作塊追蹤系統

| 檔案                         | 用途                           | Agent 何時讀        |
| ---------------------------- | ------------------------------ | ------------------- |
| [STATUS.md](STATUS.md)       | 當前 work block（通常 1-2 個） | 每次開工            |
| [BACKLOG.md](BACKLOG.md)     | 後續 work block 佇列 + 依賴圖  | STATUS.md 清空時    |
| [CHANGELOG.md](CHANGELOG.md) | 已完成 work block 紀錄         | 不需要讀，只 append |

這一層不再追求原子 task。`STATUS.md` / `BACKLOG.md` 的單位改成**半個 milestone 左右的 work block**：

- 一個 work block 可以橫跨多個 docs / code 子任務，但要有單一可驗收成果。
- work block 可以拆成多個可 review commit；不要把「work block 變大」誤解成「commit 也要巨大」。
- 更細的 WBS 仍保留在各 milestone 文檔裡，作為 block 內部的拆解參考，而不是每日追蹤單位。

> **2026-04-14 closeout note**：`WORK-QC-R` 已完成。repo 的正式 storage truth 現在是 canonical/search/intelligence/sidecars 四層結構；legacy archive upgrade、runtime compatibility patching、hot-path raw row persistence 都不再是 active strategy。
>
> **2026-04-15 closeout note**：`WORK-QC-T` 已完成。deterministic product contract 已從 legacy Insights snapshot hard-cut 到 Core Intelligence baseline；`core-intelligence-ultimate-design.md` 現在是 accepted source of truth，`/intelligence` 與新的 Tauri/worker query surface 取代舊 `run_insights_now` / `load_insights` / `explain_insight` 主路徑。
>
> **2026-04-17 continuation note**：Core Intelligence 的實際完成度已經超過最初的 P1/P2 口頭分工；若要讓 fresh agent 續接 frontend/backend 工作，請先讀 [core-intelligence-progress.md](core-intelligence-progress.md) 與 [core-intelligence-handoff.md](core-intelligence-handoff.md)，不要只靠 pre-reset `m5-deterministic-intelligence/` 文檔猜目前狀態。
>
> **2026-04-18 closeout note**：`WORK-CI-C` 已完成。repo 現在只接受 registry-backed Core Intelligence module ids、canonical table names、runtime reports 與 benchmark evidence；`artifacts/benchmarks/2026-04-18-intelligence-long-horizon-signoff/` 已補齊 current-host `14.4M / 60y` full replay 與 expired-lease recovery artifact。current-host signoff 就是目前 stop point；若未來還想補第二台主機 benchmark parity，必須重新立項，而不是預設待辦。
>
> **2026-04-18 desktop truth follow-up note**：source 之後又補上 locked-archive shell snapshot degradation、Security candidate-key fail-fast、sidebar locked-state polling gate，以及 compact `version · short-sha[+]` build diagnostics；但 fresh `bun run desktop:dev` 在這台主機上仍可能顯示舊的 generic dashboard copy 與不帶 SHA 的 shell chrome。這要先視為 current-host stale WebView / bundle cache drift，再決定是否重開 frontend regression。
>
> **2026-04-19 M6 closeout note**：`WORK-M6-A` 已完成。`day` 與 `domain` 現在都已升格成 first-class shared insights entity：新增 `/intelligence/day/:date`、保留但正式升格 `/intelligence/domain/:domain`、shared href grammar、`Insight Access` strip，以及 Dashboard / Intelligence / Explorer 的 route-first entry。下一輪 active 規劃改成 `M7 — Cross-App Reuse Audit And Insight Entity Consolidation`，用來盤點其餘仍然 consumer-local 的 intelligence entity surface。
>
> **2026-04-19 M7 closeout note**：`WORK-M7-A` 已完成。repo 現在正式有 generic insight-entity navigation contract、shared entity CTA chrome，以及 `query family` / `refind page` / `session` / `trail` 四條 first-class shared insights route；非 promoted entity 也都已收斂到 shared destination。下一輪 active 規劃改成 `M8 — Aggregate Entity Identity And Context Reuse`，專門處理 path-flow stable identity、compare-set full detail、context focus 與更多 reusable entity IDs。
>
> **2026-04-19 M8 closeout note**：`WORK-M8-A` 已完成。repo 現在正式有 `compare set` first-class insights route、受限的 shared `focusType` / `focusId` query grammar、typed `path flow` identity，以及 trusted external-output payload 的 structured entity targets；`public snapshot` 仍維持 redacted。下一輪 active 規劃改成 `M9 — Cross-App Reuse Audit And Shared Composition`，用來全面盤點剩餘 consumer-local composition 與 shared extraction 機會。
>
> **2026-04-19 M9 closeout note**：`WORK-M9-A` 與 `WORK-M9-B` 已完成。repo 現在正式有 shared route-level metric strip、`query-family-card`、compare-set page list、structured target label，以及 inline-end section-meta header chrome；`證據與新鮮度` badge 不再獨占一整行，也不再把 hover hitbox 擴成整張卡 header。下一輪 active 規劃改成 `M10 — Workbench Reuse And Transport Hygiene`，專門處理仍未抽出的 review rows 與 route / desktop glue decomposition。
>
> **2026-04-19 M10 closeout note**：`WORK-M10-A` 與 `WORK-M10-B` 已完成。repo 現在正式有 shared `refind` workbench shell、Explorer session/trail shared group-card/member-row primitive、Settings external-output/local-host shared review chrome，以及 split 的 promoted routes / Core Intelligence API / Tauri command + worker-bridge intelligence facade；對外 route / payload contract 維持不變。下一輪 active 規劃改成 `M11 — App-Wide Reuse And Shared Review Grammar`，用來全面盤點剩餘 mixed helper、dev mirror、與跨 route review / PME / diagnostics drift。
>
> **2026-04-19 M12 closeout note**：`WORK-M12-A` 與 `WORK-M12-B` 已完成。repo 現在正式有 app-wide shared support-action / clipboard grammar，Settings general diagnostics / App Lock、Audit、Import、Schedule、Security / Lock 與 Explorer export path 都已接到 `src/components/review/` 的 canonical owner；Jobs plugin / module summary rows與 transport parity follow-up 則明確移交給 `M13 — Broad Reuse Audit Across Support / Trust / Workflow Surfaces`。
>
> **2026-04-21 M13 inventory note**：`WORK-M13-A` 已完成。repo 現在除了既有 neutral review 與 support-action grammar，也正式有 shared runtime-boundary card grammar；Jobs runtime health / plugin / module summary 與 Settings derived runtime review 是第一批 consumer。`WORK-M13-B` 仍維持 active，下一輪 priority 改成 shell-data owner、Security / Import workflow follow-through、Dashboard fallback owner 與 `Browsing Rhythm` layering。
>
> **2026-04-21 backend decomposition note**：使用者另行開啟平行的 backend hotspot 拆分軌道，source of truth 在 [backend-hotspot-decomposition.md](backend-hotspot-decomposition.md)。這條軌道和 `WORK-M13-B` frontend reuse 並行，但要求保持 transport / schema contract 穩定，並以 `takeout` / parser / archive ingest 這條大數據量 import boundary 作為第一個 execution slice。
>
> **2026-04-21 archive ingest follow-up note**：backend 軌道的第二個 execution slice 已落地。`src-tauri/crates/vault-core/src/archive/mod.rs` 現在把 canonical ingest boundary 下沉到 `archive/ingest/{mod,parser,writes}.rs`，整體從 `2159` 行降到 `1299` 行；但 parser/import 仍維持 collect-then-ingest contract，且 `takeout/import_flow.rs` 仍超過 `600` 行，所以真正的 streaming/import batching 收尾已在 `BACKLOG.md` 立成後續 block。
>
> **2026-04-21 takeout import follow-up note**：backend 軌道的第三、第四個 execution slice 也已落地。Takeout import 現在不再沿用 inspection preview helper 去額外建一整份 preview rows；`takeout/import_flow.rs` 已下沉 payload parse/write 細節到 `takeout/payload_import.rs`，並把 parser report ownership 直接交給 source-evidence plan。同時 batch review read/audit repair 也已拆到 `takeout/batch_review.rs`，讓 `takeout/batches.rs` 回到 write-side revert/restore ownership。剩餘 backend 風險現在集中在更深一層的 parser/import streaming contract 和 `archive/mod.rs`，而不再是 Takeout boundary 的文件尺寸本身。
>
> **2026-04-21 execute-path note**：Takeout execute path 的雙重解析也已經拿掉。現在 `import_takeout` 在 non-dry-run 下不會再先做一輪完整 `inspect_takeout`；它改成單次掃描檔案、邊寫 canonical rows 邊累積 batch metadata，最後再從 persisted `preview_import_batch` 回填 review payload。這讓後續真正需要攻克的問題只剩 parser crate 本身的 full-batch materialization，以及 archive ingest 邊界的進一步 streaming 化。
>
> **2026-04-21 archive backup follow-up note**：backend 軌道的第六個 execution slice 已落地。`src-tauri/crates/vault-core/src/archive/mod.rs` 現在又把 backup orchestration 與 manifest/snapshot support helper 下沉到 `archive/{backup,run_support,artifacts}.rs`，parent module 已進一步縮到 `406` 行。這表示 archive-side giant-file triage 基本完成；剩餘後端 stop-ship 風險回到 parser/import collect-then-ingest contract，而不是 `archive/mod.rs` 本身。
>
> **2026-04-22 source-evidence follow-up note**：backend 軌道的第七個 execution slice 已落地。backup ingest 與 Takeout import 現在不再在 canonical commit 後保留第二份完整 `ParsedHistory` 只為了寫 cold source evidence；`archive::source_evidence` 已改成消費更窄的 `typed_evidence + native_entities` payload。這先拿掉了 post-commit duplicate retention，但 parser family 本身仍會先把 URL/visit/download/search-term/favicon vectors 整批 materialize，所以真正的下一輪重點仍是 parser-side streaming contract。
>
> **2026-04-22 Chromium streaming note**：backend 軌道的第八個 execution slice 已落地。`browser-history-parser::chromium` 現在新增 chunked `stream_history` path，`archive::ingest` 也已把 Chromium live-backup path 接到這條新 contract；這表示主力 backup 路徑現在會邊 parse staged Chromium DB 邊寫 canonical archive，而不是等整份 parser batch 完成後才開始寫入。剩餘 stop-ship 風險因此進一步縮到 Firefox / Safari / Takeout 仍 full-batch materialize，以及 cold source-evidence 尚未完全 spool/stream 化。
>
> **2026-04-22 cross-family streaming note**：backend 軌道又向前推了一輪。`browser-history-parser::firefox` 與 `::safari` 現在也補上相同的 streamed parser contract，`archive::ingest` 已把三個 live browser backup family 全部接到同一條 streamed canonical-ingest path。這讓 ordinary local backup runs 都不再等整份 parser batch 才開始寫 archive；剩下的 full-batch path 主要集中在 Takeout / restore-preview，以及 cold source-evidence 仍先在記憶體累積到 post-commit 才寫入。
>
> **2026-04-22 bounded-memory note**：backend 軌道這一輪再補了一刀。`archive::source_evidence` 現在會把超過閾值的 deferred cold payload spill 到 `staging/source-evidence-spool/`，所以 backup / import 不必把所有 post-commit native payload 一直留在 RAM 裡等 cold archive write 開始；`snapshot_restore` preview 也改成 direct checkpoint row counts，而不是為了估算 replay 再 materialize 一整份 parser batch。multi-profile backup 下 checkpoint preview 還順手修掉了 Safari / Firefox 會誤套第一個 `profile_scope` 的歸屬漂移。這讓剩下的 backend full-batch 風險更明確地只剩 Takeout parser/import 本身。
>
> **2026-04-22 Takeout parser note**：backend 軌道這一輪又向前推了一整個子路徑。`browser-history-parser::takeout` 現在不再是單個 `728` 行 giant-file，而是 split 成 `browser_history`、`json_stream`、`payloads`、`source`、`tests` 等 focused modules；同時它也新增 payload-level streaming contract，讓 `vault-core::takeout::payload_import` 能在 BrowserHistory payload 還在解析時就開始寫 canonical URL/visit rows。這表示 Takeout import 的 canonical write path 也已 streamed，剩下的 full-batch 風險主要縮到單一 payload 內的 source-native evidence 與 inspection preview accumulation。
>
> **2026-04-22 Takeout preview note**：同一天的後續 execution slice 又把 `vault-core::takeout::inspect_takeout` 切到 payload-level streamed preview contract。dry-run preview 現在不再先 materialize 一整份 BrowserHistory payload report，也不再在 inspection path 上累積 `typed_evidence` / `native_entities`；剩餘 Takeout full-batch 風險因此更明確地只剩 import path 內單一 payload 的 source-native evidence retention。
>
> **2026-04-22 Takeout source-evidence note**：後續又再往前推一刀。`browser-history-parser::takeout` 現在可把 source-native evidence 以 chunk 形式交給 consumer，`vault-core::takeout::payload_import` 則透過新的 `archive::source_evidence_builder` 邊接收邊 spill 到 `staging/source-evidence-spool/`。這表示 Takeout import 的 canonical rows、preview rows、以及 cold source-evidence 都已經進入 bounded-memory path；backend 軌道的下一個實際 focus 因此改成 `intelligence_runtime.rs` queue/recovery/runtime snapshot ownership。
>
> **2026-04-22 intelligence runtime note**：同一天的後續 execution slice 又把 `src-tauri/crates/vault-core/src/intelligence_runtime.rs` 從單個 `2222` 行 giant-file 拆成 `intelligence_runtime/{mod,enqueue,claims,job_control,recovery,snapshot,tests_queue,tests_runtime}.rs`。queue writes、lease recovery、runtime snapshot read model、以及回歸測試現在都有明確 owner；下一個 backend giant-file focus 因此正式轉到 `intelligence/mod.rs`，之後才是 `vault-worker/src/intelligence.rs`。
>
> **2026-04-22 intelligence read-model note**：同一天的下一個 execution slice 又把 `src-tauri/crates/vault-core/src/intelligence/mod.rs` 的 route-facing read-model layer 抽成 `intelligence_{overview,summary,domain,outputs}.rs`。`/intelligence` staged overview、digest/stable-source/search-effectiveness、domain/deep-dive/discovery/on-this-day、以及 embed/widget/public snapshot payload 現在都有明確 owner；`intelligence/mod.rs` 本體也先從 `11043` 行降到 `9761` 行，接下來的 backend giant-file focus 更集中在 explain 與 rebuild 路徑。
>
> **2026-04-22 intelligence explain note**：同一天的再下一個 execution slice 又把 `intelligence/mod.rs` 的 refind/detail/explain surfaces 拆成 `intelligence_{refind,explain,explain_helpers}.rs`。refind page detail、`explain_entity` 與 explanation-only visit-id / entity-id helper 現在都各有 owner，`intelligence/mod.rs` 因而再降到 `8848` 行；剩餘真正的 giant-file 風險更明確地只剩 schema/bootstrap 與 rebuild-stage orchestration。
>
> **2026-04-22 intelligence rebuild note**：同一天的後續 execution slice 又把 `intelligence/mod.rs` 的 schema/bootstrap 與 rebuild orchestration 抽成 `intelligence_{schema,schema_sql,rebuild}.rs`。migration/bootstrap、derived-state clear、public rebuild entrypoints、legacy scoped fallback、以及 runtime-ready update ownership 現在都有獨立 module owner；`intelligence/mod.rs` 也因此再降到 `7703` 行，剩餘 backend giant-file 風險更集中在 structural rebuild internals 與 query/read-model helpers。
>
> **2026-04-22 backend structural closeout note**：`WORK-BE-A` 現在也已把 structural rebuild internals 從 `intelligence/mod.rs` 拆成 `intelligence_structural_{state,build,aggregates,persist,stream,stage}.rs`。這讓 parent module 再降到 `5561` 行，且所有新 structural modules 都回到 `600` 行硬限制內；下一個 active backend block 已切到 `WORK-BE-B`，focus 改成剩餘 query/read-model helper clusters，以及 `vault-worker/src/intelligence.rs` / `ai.rs` 的 mixed ownership。
>
> **2026-04-22 backend read-model note**：`WORK-BE-B` 的第一個 execution slice 也已落地。`intelligence/mod.rs` 的 session/trail detail、navigation path/hub pages、search metrics/rules/concepts、recent-search/query-family surface，現在已拆到 `intelligence_{sessions,navigation,search_metrics,search_queries}.rs`，而 domain-only helper 也已回到 `intelligence_domain.rs`。這讓 parent module 再降到 `4508` 行；後續 BE-B focus 則更明確地只剩 residual helper clusters、`vault-worker/src/intelligence.rs` 與 `ai.rs`。

> **2026-04-22 worker boundary note**：`WORK-BE-B` 的後續 execution slice 也已把 `src-tauri/crates/vault-worker/src/intelligence.rs` 從 `1636` 行 giant-file 拆成薄 façade `intelligence.rs` (`789` 行) 與 `intelligence/{ai_queue,runtime}.rs`。AI queue / assistant / semantic-search orchestration，還有 deterministic runtime queue / retry / cancel / snapshot ownership，現在都不再混在同一個 parent module 裡；既有 worker export surface 與 `archive_flows` 使用的 `maybe_spawn_*` background helpers 都維持不變。

> **2026-04-22 AI boundary note**：同一個 `WORK-BE-B` block 的下一刀也已把 `src-tauri/crates/vault-core/src/ai.rs` 從 `2116` 行 giant-file 拆成薄 façade `ai.rs` (`199` 行) 與 `ai/{control,provider,indexing,ledger,search,read_model}.rs`。provider probe / validation、semantic index build + ledger、semantic search、assistant tool orchestration 現在都有獨立 owner；既有 worker caller、assistant run payload、semantic search payload、與 read-model contract 都維持不變。

> **2026-04-22 worker read-surface note**：同一個 `WORK-BE-B` block 的後續 execution slice 又把 `src-tauri/crates/vault-worker/src/intelligence.rs` 的 residual query passthrough 拆成 `intelligence/{route_queries,section_queries}.rs`。worker parent file 現在只剩 shared helper + re-export façade (`124` 行)，route-first entity reads 與 section-style wrappers 各自有明確 owner；既有 worker export surface 與 frontend consumer contract 仍維持不變。

> **2026-04-22 backend helper-cluster closeout note**：`WORK-BE-B` 現在也已把 `intelligence/mod.rs` 的 residual helper clusters 抽成 `intelligence_{shared,visit_records,visit_derive,daily_rollup_state,daily_rollups,core_persist}.rs`。shared date/query heuristics、visit-derived stage、daily-rollup stage、以及 scoped full-rebuild persistence 現在都已有 focused owner，`intelligence/mod.rs` 因而進一步降到 `2583` 行，只剩 exported surface、core record types、batch cursors、常數與 regression suite；下一個 active backend block 已切到 `WORK-BE-C`，focus 改成剩餘 oversized support files 與 `intelligence/mod.rs` 內嵌 regression suite。

> **2026-04-23 visit-taxonomy boundary note**：`WORK-BE-C` 的第一個 execution slice 已把原 `deterministic` module 改名並拆成 `src-tauri/crates/vault-core/src/visit_taxonomy/{mod,types,url,text,rules,classification,tests}.rs`。`crate::visit_taxonomy::*` public façade、taxonomy version、URL normalization、query extraction、tokenization 與 built-in rule semantics 都維持不變；下一個 backend support hotspot 改成 `intelligence/site_dictionary.rs`。

> **2026-04-23 site-dictionary boundary note**：`WORK-BE-C` 的下一個 execution slice 已把 `src-tauri/crates/vault-core/src/intelligence/site_dictionary.rs` 拆成 `site_dictionary/{mod,types,overrides,search_rules,classification,tests}.rs`。Settings search-rule DTO、override schema、search-query extraction、display-name fallback 與 visit classification semantics 都維持不變；下一個 backend support hotspot 改成 `models/core_intelligence.rs`。

---

## 先看哪裡

| 如果你關心                                        | 先看這份                                                                           |
| ------------------------------------------------- | ---------------------------------------------------------------------------------- |
| 整體節奏、里程碑順序、依賴關係                    | [program/README.md](program/README.md)                                             |
| 現在這個 repo 和新 vision 的距離                  | [program/repo-baseline.md](program/repo-baseline.md)                               |
| 哪些技術決策還沒落地、哪些研究要先做              | [program/research-and-decisions.md](program/research-and-decisions.md)             |
| 現行 quality gate、blocking path、deep checks     | [program/quality-matrix.md](program/quality-matrix.md)                             |
| 某份需求/設計文檔應該對應哪份實作計劃             | [program/traceability-map.md](program/traceability-map.md)                         |
| Core Intelligence hard-reset 的真實進度與剩餘工作 | [core-intelligence-progress.md](core-intelligence-progress.md)                     |
| Core Intelligence frontend/backend 續作 handoff   | [core-intelligence-handoff.md](core-intelligence-handoff.md)                       |
| M0 重構基礎                                       | [m0-foundation/README.md](m0-foundation/README.md)                                 |
| M1 Solid Archive                                  | [m1-solid-archive/README.md](m1-solid-archive/README.md)                           |
| M2 Recall & Trust                                 | [m2-recall-and-trust/README.md](m2-recall-and-trust/README.md)                     |
| M3 Intelligence                                   | [m3-intelligence/README.md](m3-intelligence/README.md)                             |
| M4 Full Intelligence & Polish                     | [m4-full-polish/README.md](m4-full-polish/README.md)                               |
| M5 Deterministic Intelligence                     | [m5-deterministic-intelligence/README.md](m5-deterministic-intelligence/README.md) |
| M5 Runtime & Extensions                           | [m5-runtime-and-extensions/README.md](m5-runtime-and-extensions/README.md)         |
| M6 Shared Insight Surfaces                        | [m6-shared-insight-surfaces/README.md](m6-shared-insight-surfaces/README.md)       |
| M7 Reuse Audit                                    | [m7-reuse-audit/README.md](m7-reuse-audit/README.md)                               |
| M8 Aggregate Entity Identity                      | [m8-aggregate-entity-identity/README.md](m8-aggregate-entity-identity/README.md)   |
| M9 Cross-App Reuse / Shared Composition           | [m9-cross-app-reuse/README.md](m9-cross-app-reuse/README.md)                       |
| M10 Workbench Reuse / Transport Hygiene           | [m10-workbench-reuse/README.md](m10-workbench-reuse/README.md)                     |
| M11 App-Wide Reuse / Shared Review Grammar        | [m11-app-wide-reuse/README.md](m11-app-wide-reuse/README.md)                       |
| 產品願景、需求、畫面結構                          | [../vision-and-requirements.md](../vision-and-requirements.md)                     |

---

## 這一版計劃是怎麼來的

這不是把舊的 todo 清單換個排版，而是重新掃過整個 repo 和整份 `docs/` 後，做的一次真正 re-baseline。這次至少確認了幾件事：

- 新的 vision、features、architecture、design 文檔已經成形，而且方向很清楚。
- 現在的代碼庫不是「完全不能用」，而是**還在穩定驗證一套舊產品假設**。
- 舊 UI 不只是視覺上不好看，而是整個資訊架構、導航和狀態模型都還綁在舊產品上。
- Rust 端其實已經有很多功能，但大量功能長在錯的地方，巨型檔案和責任混寫很明顯。
- 有些決策其實還沒真的落地，例如 schema reset strategy、migration story、rollback visibility、AI sidecar 邊界、跨平台排程。

所以這份 plan 的目的不是「幫我們記得做哪些功能」，而是先把**正確的實作順序、決策順序和刪舊代碼的順序**講清楚。

---

## 2026-04-06 基線結論

根據這次掃描和驗證，目前可以先這樣理解 repo：

- 前端入口 [`src/main.tsx`](../../src/main.tsx) 已切到 [`src/app/index.tsx`](../../src/app/index.tsx)；`AppNew` 與舊 `App.css` 已退場。
- 新 shell / route tree / sidebar / topbar / page skeleton 已建立，入口資訊架構已對齊新 prototype，而不是舊 setup-first shell。
- [`src/lib/backend.ts`](../../src/lib/backend.ts) 仍帶有 legacy / compatibility 成分，但正式 typed IPC wrapper 已移到 [`src/lib/ipc/bridge.ts`](../../src/lib/ipc/bridge.ts)，preview data 也已從主 bridge 分離。
- Rust 端目前的主要複雜度集中在幾個 archive / intelligence 主檔裡：[`src-tauri/crates/vault-core/src/archive/mod.rs`](../../src-tauri/crates/vault-core/src/archive/mod.rs)、[`src-tauri/crates/vault-core/src/intelligence/mod.rs`](../../src-tauri/crates/vault-core/src/intelligence/mod.rs)、[`src-tauri/crates/vault-core/src/intelligence_runtime.rs`](../../src-tauri/crates/vault-core/src/intelligence_runtime.rs)、[`src-tauri/crates/vault-core/src/ai.rs`](../../src-tauri/crates/vault-core/src/ai.rs)。`chrome.rs`、`vault-worker/src/lib.rs` 與 legacy `insights.rs` 已不再是同級 hotspot truth。
- canonical archive 已有正式 migration ledger 與 schema foundation；M1 的主題不再是「先把 schema 生出來」，而是接上可信 archive engine。
- PathKeep 命名已完成 public / build metadata sweep；剩餘舊名字串只應存在於 explicit legacy alias 或 migration 註記。
- 設計師的 prototype 現在已經落成 production shell 的 token、layout 與 smoke target；prototype gap list、deep-link 與 non-prototype state coverage 也已回寫成 source docs，剩餘的全站 accessibility / release polish 留在 M4。

---

## 已做過的基線驗證

這一輪 plan 不是純主觀判斷。下面這些命令已重新執行並回寫：

- `bun run typecheck`：通過
- `bun run test:unit`：通過
- `bun run test:unit:desktop-contract`：通過
- `bun run coverage:js:desktop-contract`：通過，desktop contract slice 維持 100% coverage
- `bun run mutation:js:desktop-contract`：通過，desktop contract slice 維持 100% mutation score
- `cargo test --manifest-path src-tauri/Cargo.toml --workspace --all-targets --quiet`：通過
- `bun run test:e2e`：通過，驗證新 shell / onboarding / dashboard smoke
- `bun run check`：通過，repo-wide Markdown / Prettier debt 與驗收途中浮出的 JS ESLint、Rust Clippy 基線問題已清理
- `bun run build`：通過
- `bun run coverage:js`：通過，living M0-M3 JS quality surface 維持 100% coverage
- `bun run coverage:rust`：通過，Tauri desktop command / bridge quality surface 維持 100% coverage
- `bun run mutation:js`：通過，living M0-M3 JS quality surface 的 mutation score 恢復到 blocking threshold 之上

2026-04-07 品質 closeout：

- repo 現在有一份正式的 [quality matrix](program/quality-matrix.md)，把 mainline blocking path、scheduled / release deep checks，以及 desktop / preview 驗收邊界全部寫清楚。
- desktop contract slice 仍然存在，但它現在是 `bun run check` 裡的一條 targeted sub-gate，不再冒充整個產品 UI 或所有 desktop flows 都已驗收。
- 2026-04-07 closeout：`WORK-QC-B` 已把 prototype / doc parity、desktop-vs-preview 邊界、dashboard / onboarding trust copy 與 timezone-sensitive On This Day 行為重新對齊；M4 現在可從 `WORK-M4-A` 啟動。
- 2026-04-08 closeout：`WORK-M4-A` 已把 enrichment / derived-state v1、storage analytics / growth evidence、以及 remote backup 的 bundle / verify / PME 閉環正式落地；`WORK-M4-B` 隨後也已完成，正式補齊 release / support 文檔、platform validation runbook、release workflow preflight 與 Settings diagnostics。blocking path、coverage、`mutation:js`、browser-preview smoke 與 debug desktop build 都已通過；其後 `WORK-M4-D` 把 Rust mutation baseline 收斂成 parser crate + AI status/helper slice 的 signed-off contract，並把 whole-workspace `mutation:rust:full` 保留作 exploratory triage，而 `WORK-M4-C` 的安全研究也已在 ADR-005 / App Lock 實作中正式 close out。
- 2026-04-08 性能 closeout：`WORK-M4-G` 已把 Explorer day-one keyword recall 從 `LIKE` 收斂到 FTS5 `history_search` projection，manual backup 也改為透過 desktop progress event 顯示 profile-scoped phase log；同時補齊 [large-archive-performance-runbook.md](m4-full-polish/large-archive-performance-runbook.md)，讓之後的大型 archive triage 有固定 artifact bundle，而不是再靠一次性的口頭記錄。
- 2026-04-08 UI closeout：`WORK-M4-E` 已把 Dashboard / Explorer / Insights / Import / AI action 的 loading grammar 收斂成 skeleton + readable progress contract；`WORK-M4-C` 也補上 App Lock route、session guard、MCP refusal path 與 source-of-truth docs。M4 當前已切好的 work blocks 全部收口，下一輪需要從剩餘 docs/plan 開放項重新切出新的 half-milestone block。
- 2026-04-09 audit closeout：`WORK-QC-D` 與 `WORK-M1-C` 已完成，當時 closeout environment 的 `bun run verify` / `bun run check` / `bun run build` 已重新回綠；但這次審核也確認 repo **不能**聲稱「所有設計文檔需求都已完成」。M4 仍保留兩個真正的未完成主線：`WORK-M4-J`（60-year performance proof）與 `WORK-M4-I`（advanced intelligence shipping）。`WORK-M4-J` 現在已重新補回可重跑的 shell-scaling artifact script 與 checked-in bundle，但 final signoff 仍需要真實 large-profile replay，不是 synthetic bundle 即可代替。
- 2026-04-10 recoverability closeout：`WORK-M1-D` 已完成。repo 現在正式 shipping checkpoint-based `snapshot_restore` preview / execute、manual-first local retention prune，以及 run-ledger-backed rekey audit summary；M1 不再把 recoverability 留在「truth-only 文檔收尾」。仍保留 manual-first 的只剩 archive-file safety snapshot 在需要舊 key 時的恢復邊界，以及 M4 的剩餘主線 `WORK-M4-K` / `WORK-M4-L`。
- 2026-04-10 boundary / release closeout：`WORK-M4-K` 與 `WORK-M4-L` 已完成。PathKeep 現在正式 shipping macOS Touch ID session unlock、consented frontend analytics、Settings updater review / install surface、`com.yi-ting.pathkeep` clean-break namespace、single-script version bump，以及 release size / code-health artifact docs。plugin sandbox、獨立 enrichment queue family 與 longer-horizon intelligence 已明確移回 `WORK-M5-A` / `WORK-M5-B`；M4 因此 truthfully close out，而不是停在模糊 `[~]`。
- 2026-04-10 deterministic unblock：使用者已明確 sign off `ADR-006`，M5 從 proposal / blocked 轉為 active。repo 也同步落下第一版 `vault-core::visit_taxonomy` URL normalization / search parser foundation，並補修 onboarding archive-mode IPC mismatch 與 insights refresh queue regression，避免 M4 closeout 遺留的契約錯位繼續污染 M5 起點。
- 2026-04-10 deterministic grouping closeout：`WORK-M5-B` 已完成。PathKeep 現在正式 shipping query groups / ladders、cross-day thread merge、reference pages、source effectiveness、template summaries、deterministic module registry 與 stale/invalidation honesty。
- 2026-04-10 packaging closeout：使用者已明確 sign off 保留 default desktop build 內建 optional AI / MCP / semantic runtime；`WORK-QC-F` 因此以 [ADR-009](../architecture/decisions/009-default-desktop-optional-intelligence-shipping.md) 與 `artifacts/release/2026-04-11-size-audit/` artifact bundle 正式收口，不再作為 active blocker。

這個結果很重要，因為它代表 repo 現在不只保住 desktop entry + typed IPC contract，也重新把 living M0-M3 quality surface 的 coverage、build、e2e 與 deep-check 分層拉回可兌現狀態。

2026-04-06 審查修正：M1 的 archive feature baseline 已經落地，但 milestone 本身仍有 closeout 要完成。當時非前端剩餘重點收斂到 `M1-DB` / `M1-OPS` 的 acceptance matrix、security mode taxonomy、retention / audit summary；這些 gap 現已由 `WORK-M1-D` 收口。前端 shell / route / sidebar 的驗收仍不能借用舊的 shell slice 敘事，必須由前端 owner 補上獨立驗收。

---

## 進度符號

- `[ ]` 未開始
- `[/]` 進行中
- `[x]` 已完成
- `[~]` 已有部分實作，但不符合新 vision，需要重做或重構
- `[!]` 阻塞中，必須先做研究或決策

---

## WBS 根節點

```
PG  Program / Baseline / Research
M0  Foundation reset
M1  Solid Archive
M2  Recall & Trust
M3  Intelligence
M4  Full Intelligence & Polish
M5  Deterministic Intelligence / Runtime & Extensions
M6  Shared Day And Domain Insights
M7  Cross-App Reuse Audit And Insight Entity Consolidation
M8  Aggregate Entity Identity And Context Reuse
M9  Cross-App Reuse Audit And Shared Composition
M10 Workbench Reuse And Transport Hygiene
M11 App-Wide Reuse And Shared Review Grammar
M12 Shared Support Actions And Diagnostics Decomposition
M13 Broad Reuse Audit Across Support / Trust / Workflow Surfaces
BE  Backend Hotspot Decomposition And Rustdoc Hardening
```

每個里程碑目錄都有：

- `README.md`：這個里程碑在做什麼、何時算完成、有哪些工作包
- 2 到 4 份工作包文檔：把待辦拆到更細的功能域和實作層
- `STATUS.md` / `BACKLOG.md`：以 half-milestone work block 追蹤目前真正要做的範圍

---

## 里程碑入口

| 里程碑 | 目標                                                                                                  | 狀態  | 入口                                                                                           |
| ------ | ----------------------------------------------------------------------------------------------------- | ----- | ---------------------------------------------------------------------------------------------- |
| `PG`   | 盤清 repo 現況、建立決策 backlog、維護文檔導覽和依賴關係                                              | `[/]` | [program/README.md](program/README.md)                                                         |
| `M0`   | 切斷舊 UI 和舊產品骨架，建立新的前端、後端和資料平面起點                                              | `[x]` | [m0-foundation/README.md](m0-foundation/README.md)                                             |
| `M1`   | 把 Archive、Audit、Schedule、Security、Explorer v1 做成可信的基礎                                     | `[x]` | [m1-solid-archive/README.md](m1-solid-archive/README.md)                                       |
| `M2`   | 補齊導入、回滾、Doctor、多瀏覽器、PME、i18n 和跨平台排程                                              | `[x]` | [m2-recall-and-trust/README.md](m2-recall-and-trust/README.md)                                 |
| `M3`   | 在穩定 archive 之上加入 optional AI provider、index、assistant、insights                              | `[x]` | [m3-intelligence/README.md](m3-intelligence/README.md)                                         |
| `M4`   | 補齊 enrichment、進階洞察、remote backup、release polish 和多平台驗證                                 | `[x]` | [m4-full-polish/README.md](m4-full-polish/README.md)                                           |
| `M5`   | 以 honest evidence 重建 deterministic intelligence baseline 與 runtime                                | `[x]` | [m5-runtime-and-extensions/README.md](m5-runtime-and-extensions/README.md)                     |
| `M6`   | 將 `day` / `domain` 升格成 first-class shared insights entity surface                                 | `[x]` | [m6-shared-insight-surfaces/README.md](m6-shared-insight-surfaces/README.md)                   |
| `M7`   | 全面盤點 cross-app reuse，抽出 generic insight-entity navigation                                      | `[x]` | [m7-reuse-audit/README.md](m7-reuse-audit/README.md)                                           |
| `M8`   | 補齊 aggregate entity identity、context focus 與 reusable entity IDs                                  | `[x]` | [m8-aggregate-entity-identity/README.md](m8-aggregate-entity-identity/README.md)               |
| `M9`   | 全面盤點剩餘重複造輪子處，並收斂 shared composition / extraction strategy                             | `[x]` | [m9-cross-app-reuse/README.md](m9-cross-app-reuse/README.md)                                   |
| `M10`  | 收斂 workbench review row reuse，並盤點 route / desktop glue decomposition                            | `[x]` | [m10-workbench-reuse/README.md](m10-workbench-reuse/README.md)                                 |
| `M11`  | 從全 app 角度盤點 reusable review / PME / diagnostics grammar 與剩餘 mixed helper / transport drift   | `[x]` | [m11-app-wide-reuse/README.md](m11-app-wide-reuse/README.md)                                   |
| `M12`  | 收斂 shared support actions / diagnostics rows，並盤點 Settings 與 transport parity 的下一輪拆分      | `[x]` | [m12-support-actions-and-diagnostics/README.md](m12-support-actions-and-diagnostics/README.md) |
| `M13`  | 以 support / trust / workflow surface 為主題，延續 broad reuse audit 與 shared composition extraction | `[/]` | [m13-broad-reuse-audit/README.md](m13-broad-reuse-audit/README.md)                             |

---

## 與其他文檔的關係

```
docs/vision-and-requirements.md   WHY + WHAT
  ├── docs/architecture/          技術原則與資料長期設計
  ├── docs/features/              功能需求詳細規格
  ├── docs/design/                UX 原則與畫面結構
  ├── docs/milestones.md          里程碑概覽
  ├── docs/standards.md           品質標準
  └── docs/plan/                  HOW + WHEN + WBS
       ├── README.md
       ├── program/
       ├── m0-foundation/
       ├── m1-solid-archive/
       ├── m2-recall-and-trust/
       ├── m3-intelligence/
       ├── m4-full-polish/
       ├── m5-deterministic-intelligence/
       ├── m5-runtime-and-extensions/
       ├── m6-shared-insight-surfaces/
       ├── m7-reuse-audit/
       ├── m8-aggregate-entity-identity/
       ├── m9-cross-app-reuse/
       ├── m10-workbench-reuse/
       ├── m11-app-wide-reuse/
       ├── m12-support-actions-and-diagnostics/
       └── m13-broad-reuse-audit/
```
