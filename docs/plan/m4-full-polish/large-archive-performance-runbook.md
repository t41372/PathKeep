# Large Archive Performance Runbook

> 目標：讓下一次 large-profile triage 不必再從零開始。這份 runbook 定義要收哪些 artifact、怎麼重跑，以及哪些輸出可以直接丟給下一位 agent / LLM。

---

## 何時使用

- manual backup 用真實大型 Chromium / Chrome profile 時，shell 在執行中或完成後明顯卡頓
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

2. 在 Dashboard 觸發 manual backup，整段操作都不要切換到其他 route。

3. 同步收三份 artifact：

- Webview trace
  - 開 Tauri DevTools / Web Inspector
  - Performance 錄影，從按下 backup 前開始，到 Dashboard totals / recent runs 穩定為止
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
- Rust sample 熱在 archive ingest：
  - 優先看 parser materialization、per-row inserts、JSON serialization
- SQLite query plan 沒走 `VIRTUAL TABLE INDEX`：
  - 先修 FTS contract 或 filter 組合把 planner 拉回全文索引
- backup 執行中可觀察，但完成後仍卡：
  - 優先查 dashboard refresh、post-run read model、derived follow-up

---

## Current Baseline (2026-04-08)

- Explorer day-one keyword recall 已切回 FTS5 `history_search` projection，不再走 `LIKE` 當 fast path
- backup overlay 會收到 profile-scoped phase progress event，不再只剩 opaque spinner
- canonical ingest 已減少 `source_profiles` / `urls` 的額外 SQLite round-trip，優先用 `RETURNING id`
- 剩餘 hot spot 若再出現，優先考慮 `browser-history-parser` 的真正 streaming API 與 `archive/mod.rs` 的分模組化重整
- 2026-04-09 補充：`bun run verify` 已重新全綠，但 production build 仍會對單一 main chunk（約 702 kB minified）發出 warning，且 repo 仍沒有真實 large-archive artifact bundle。下一輪若要對外背書「60 年資料量仍流暢」，至少要把 route-level payload / refresh 行為和真實 perf artifacts 一起補齊。
