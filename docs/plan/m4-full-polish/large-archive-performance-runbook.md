# Large Archive Performance Runbook

> 目標：讓下一次 large-profile triage 不必再從零開始。這份 runbook 定義要收哪些 artifact、怎麼重跑，以及哪些輸出可以直接丟給下一位 agent / LLM。

---

## 何時使用

- manual backup 用真實大型 Chromium / Chrome profile 時，shell 在執行中或完成後明顯卡頓
- Dashboard ↔ `/intelligence` 切換在大型 archive 上出現 1s+ 空白、整頁凍住，或需要等全部 query 跑完才看到首屏
- onboarding / import 最後一步的 loading overlay 看似在轉，但其實整個動畫停住，等 backend 回來後才一次補幀
- Explorer keyword recall 在大型 archive 上退化
- 需要判斷問題主要卡在 webview、Tauri command、Rust CPU / allocation，還是 SQLite query plan

---

## Artifact Bundle

每次 profiling 都建一個獨立資料夾，建議命名：

`artifacts/perf/<YYYY-MM-DD>-large-archive-<short-note>/`

至少放這些檔案：

- `context.md`
  - commit hash
  - OS / machine
  - profile source（例如 Chrome Default / Arc Profile 2）
  - 估計資料量（History DB bytes、Favicons DB bytes）
  - 執行的是 manual backup 還是 Explorer query
- `webview-trace.json`
  - Tauri webview / DevTools Performance trace
- `desktop-command-metrics.json`
  - 從 `window.__PATHKEEP_DESKTOP_COMMAND_METRICS__` 匯出的每次 desktop invoke 記錄（command、duration、request/response bytes、recordedAt）
- `rust-sample.txt`
  - Rust process CPU sample / stack snapshot
- `sqlite-query-plan.txt`
  - `EXPLAIN QUERY PLAN` 輸出
- `notes.md`
  - 肉眼觀察：何時開始卡、何時恢復、是否是 backup 結束後才卡

---

## Rehearsal

1. 啟動桌面 app：

```bash
bun run desktop:dev
```

2. 先做 route-read triage：

- 從 Dashboard 進 `/intelligence`
- 記錄首屏是否立刻看到 runtime digest / primary skeleton
- 等 primary cards 穩定後再往下滾，看 secondary grid 是否是分段補上，而不是整頁一起卡住
- 返回 Dashboard，再重進 `/intelligence`，確認 revisits 不會再重複放大 foreground fan-out

3. 再做長任務 triage：

- 在 Dashboard 觸發 manual backup，或在 Import / onboarding 走到真正的 execute step
- 整段操作中觀察 overlay / skeleton 是否持續 repaint、log lines 是否更新、route chrome 是否仍可回應

3. 同步收三份 artifact：

- Webview trace
  - 開 Tauri DevTools / Web Inspector
  - Performance 錄影，從 route enter 或按下 execute 前開始，到 primary UI 穩定 / background work settle 為止
- Desktop command metrics
  - 在 DevTools console 匯出 `window.__PATHKEEP_DESKTOP_COMMAND_METRICS__`
  - 儲存為 `artifacts/perf/<run>/desktop-command-metrics.json`
- Rust sample
  - macOS 可用：

```bash
sample <PATHKEEP_PID> 15 -file artifacts/perf/<run>/rust-sample.txt
```

- SQLite query plan
  - 對 backup 後的 archive DB 執行：

```bash
sqlite3 "$ARCHIVE_DB" <<'SQL' > artifacts/perf/<run>/sqlite-query-plan.txt
EXPLAIN QUERY PLAN
SELECT visits.id
FROM visits
JOIN urls ON urls.id = visits.url_id
JOIN source_profiles ON source_profiles.id = visits.source_profile_id
JOIN history_search ON history_search.rowid = urls.id
WHERE visits.reverted_at IS NULL
  AND history_search MATCH '"example"*';
SQL
```

---

## Explorer Query Check

用同一份 archive，再補一輪 Explorer keyword recall 檢查：

1. 在 Explorer 輸入一組應該命中的 keyword
2. 記錄 first result latency 與 page-to-page latency
3. 如果 query plan 沒出現 `VIRTUAL TABLE INDEX`，先修 FTS path，不要回頭調 UI spinner

---

## Interpretation

- Webview trace 慢，但 Rust sample 空閒：
  - 優先看 frontend refresh / repeated queries / oversized payload
- `desktop-command-metrics.json` 顯示 route entry 仍一次打出十多個相似 invoke：
  - 先修 overview batching / staged loading / shared polling，再回頭看單條 SQL
- Rust sample 熱在 archive ingest：
  - 優先看 parser materialization、per-row inserts、JSON serialization
- SQLite query plan 沒走 `VIRTUAL TABLE INDEX`：
  - 先修 FTS contract 或 filter 組合把 planner 拉回全文索引
- backup 執行中可觀察，但完成後仍卡：
  - 優先查 dashboard refresh、post-run read model、derived follow-up
- overlay 動畫停住，但 log / sample 顯示 backend 還在工作：
  - 優先檢查前端是否把 shell refresh、route hydration 或 post-run read-model refresh 放在同一個 blocking await 鏈上

---

## Current Baseline (2026-04-09)

- Explorer day-one keyword recall 已切回 FTS5 `history_search` projection，不再走 `LIKE` 當 fast path
- backup overlay 會收到 profile-scoped phase progress event，不再只剩 opaque spinner
- `/intelligence` route 現在改成 staged overview：primary overview 先批次載入並 prime section cache，secondary grid 在 first paint / idle 後再補
- 2026-04-20 hot-path recovery：overview batch 現在同一批只重用一條 intelligence SQLite connection / attached archive 與一份 runtime snapshot，不再為每個 module-backed section 重複載入 runtime metadata
- 2026-04-20 warm-cache recovery：前端現在有 scope-keyed warm cache + in-flight dedupe；`domain/day/entity route -> back -> /intelligence` 的同 scope revisit 會先用現有 cache 畫出卡片，再做 background revalidate，而不是先整頁 cold skeleton
- 2026-04-20 Search Activity prewarm：`Recent Queries` 與 `Query Evolution` 不再等點 tab 才第一次發 request；首屏穩定後會在 idle 階段自動 prewarm，避免使用者第一次切 tab 又撞冷啟
- sidebar、Dashboard 與 Intelligence runtime digest 現在共享同一個 shell-level runtime polling source，不再各自重複輪詢 queue/runtime
- `Browsing Rhythm` 初次進頁不再自動抓同日 detail；只有使用者真的選某一天，或 primary overview 已經穩定後才會做額外 detail fetch
- import / onboarding finalization 現在支援 typed progress stream（`phase/current/total/percent/detail/logLines`），follow-up backup / refresh 也改成 background-style 收尾，而不是把 overlay 凍住到最後
- canonical ingest 已減少 `source_profiles` / `urls` 的額外 SQLite round-trip，優先用 `RETURNING id`
- shell route 現在已做 route-level code splitting；checked-in shell artifact bundle 位於 `artifacts/perf/2026-04-09-large-archive-shell-scaling/`
- desktop invoke 也會持續記錄到 `window.__PATHKEEP_DESKTOP_COMMAND_METRICS__`，方便 real-data triage 時直接看 command count、payload size 與 latency，而不用每次重新加 ad hoc instrumentation
- `bun run perf:artifact:shell` 現在會從最新 production build 重新生成 `context.md`、`shell-payload-summary.json`、`route-chunk-breakdown.md`、synthetic `sqlite-query-plan.txt`，以及誠實標記的 placeholder `webview-trace.json` / `rust-sample.txt`
- 目前 checked-in artifact 顯示：
  - base shell approx bytes：`580261`
  - largest approx first-route bytes：`629465`（`settings` route）
  - synthetic Explorer keyword query plan 仍有 `VIRTUAL TABLE INDEX`
- checked-in bundle 目前只對 shell scaling 與 synthetic FTS query plan 背書；`webview-trace.json` / `rust-sample.txt` 目前仍是 placeholder，不是一次真實 large-profile replay 的產物
- 在這台 workspace 上，`bun run verify` 已經可以重新跑到全綠；目前剩下的 gap 是真實 large-profile replay artifact，而不是 CI / build gate
- 剩餘 hot spot 若再出現，優先考慮 `browser-history-parser` 的真正 streaming API 與 `archive/mod.rs` 的分模組化重整
- 下一輪若要對外背書「60 年資料量仍流暢」，至少還要補一份真實 large-profile replay artifact bundle，而不是只靠 synthetic shell / query-plan summary。
