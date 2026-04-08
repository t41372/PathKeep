# BACKLOG — 後續工作塊佇列

> 這裡只放 **half-milestone 粒度**的 work blocks。  
> `STATUS.md` 清空或完成當前 block 後，才從這裡把下一個未被阻塞的 block 搬上去。

---

## 工作塊佇列（按順序）

### Program — Quality Closeout Before M4

### M1 — Solid Archive

### M2 — Recall & Trust

### M3 — Intelligence

### M4 — Full Polish

> 2026-04-08：release closeout 的 `mutation:rust` 預演已把 parser / AI deep-check 缺口升級為 `WORK-M4-D`，並直接 promoted 到 `STATUS.md` 當前 focus；此處只保留尚未 promoted 的後續 backlog 項目。

- **WORK-M4-G** — Large Archive Performance & Profiling
  - 設計規格：`docs/features/recall.md` §大數據量下的效能設計、`docs/features/archive.md` §Enrichment、`docs/design/ux-principles.md` §Loading States & Skeleton Screens
  - Why：`WORK-M4-B` 的 release-ready closeout 雖然已把 blocking gate、docs、platform validation 和 debug desktop build 跑通，但用真實大型 Chromium profile（Yi-Ting 的 Chrome profile）做 manual backup 後，整個 app 仍會明顯不流暢，代表「可發版」和「在真實 archive 規模下可用」之間還有一個高優先缺口。這個 block 的目的不是再做一次抽象 polish，而是把 PathKeep 拉回大型真實資料下也能操作的 baseline。
  - Focus problem：
    1. Chromium ingest 仍有全量 materialization / row-by-row write amplification，真實大 profile 會把 CPU、記憶體和 SQLite write path 一次打滿。
    2. Explorer day-one keyword recall 仍走 `LIKE` 而不是 FTS5，匯入完成後的主要 read path 會跟著 archive size 一起退化。
    3. Dashboard / shell refresh 雖已收斂掉一部分同步阻塞與重複 `COUNT(*)`，但還缺 whole-app profiling artifact，沒法誠實量化哪一段最慢、也沒法把 trace 直接交給 LLM 做下一輪分析。
    4. 2026-04-08 已先做過第一輪止血：manual backup 不再同步卡住 insights rebuild、busy overlay 已補 phase/detail、canonical ingest 已減少 payload 重複序列化與 per-visit URL bounds update、dashboard totals 改成優先讀 cached run stats。下一位 agent 應以這個 baseline 往前推，不要回頭重做同一批 UI spinner / derived follow-up 修補。
  - 讀先：
    - `docs/features/recall.md`
    - `docs/features/archive.md`
    - `docs/design/ux-principles.md`
    - `docs/architecture/data-model.md`
    - `docs/plan/m4-full-polish/platform-release-and-polish.md`
  - 範圍：
    1. 建立 whole-app profiling runbook：webview runtime、Tauri command wall time、Rust CPU / allocation、SQLite query plan 與 archive-size regression fixture
    2. 為大型 Chromium / Chrome profile backup 補 progress event / phase log contract，避免長任務只剩 opaque spinner
    3. 把 Explorer day-one keyword recall 從 `LIKE` 收斂回 `FTS5` 契約，補 query benchmark 與 visibility / rollback correctness 驗證
    4. 盤整 canonical ingest hot path，優先處理 parser 全量 materialization、row-by-row write amplification，以及 derived follow-up 對 UI responsiveness 的影響
  - 建議子任務分配：
    1. Profiling / artifact lane：產出可重放的真實操作流程、Performance trace、Rust profile、SQLite query plan，並把輸出整理成 LLM-friendly artifact bundle
    2. Ingest lane：針對 `browser-history-parser` Chromium path 與 `vault-core` ingest 做 streaming / batching / fewer round-trips 的 hot-path 收斂
    3. Read-model lane：把 Explorer keyword path 拉回 FTS5，驗證 rollback / restore visibility correctness，避免大 archive 匯入後主畫面依然卡
    4. UX / trust lane：把 progress overlay 升級成真正的 phase log / progress contract，讓大型 backup 期間可觀察、可判斷是否卡住
  - 完成訊號：
    1. 有一份下次 agent 可直接重跑的 profiling runbook，且至少包含一組真實大型 profile artifact
    2. Explorer keyword recall 不再依賴 `LIKE` 作為 day-one fast path
    3. 大型 Chromium profile backup 不再讓 shell 在完成後長時間維持「可見但明顯卡頓」狀態
    4. `bun run check`、`bun run build`，以及任何本 block 新增的 targeted perf / regression 驗證都通過

- [!] **WORK-M4-C** — Secure App Lock And Profile Partitions `[!blocked: 先完成 PG-RD-PLAT-006，釐清 biometric / passcode / session-key security model]`
  - 設計規格：`docs/features/archive.md` §8、`docs/design/screens-and-nav.md` §App Lock
  - 讀先：
    - `docs/vision-and-requirements.md`
    - `docs/features/archive.md`
    - `docs/features/recall.md`
    - `docs/features/intelligence.md`
    - `docs/design/screens-and-nav.md`
    - `docs/plan/program/research-and-decisions.md`
  - 範圍：
    1. 定義 app lock 保護範圍：僅 UI session、資料庫解鎖、或兩者結合
    2. 研究 macOS / Windows / Linux 的 biometric / passcode / keyring fallback 與 recovery story
    3. 決定 shared profile scope 是否升級為真正的 per-profile partition，特別是 Insights / Dashboard 的 read model 邊界

- **WORK-M4-E** — Loading States & Skeleton Screens
  - 設計規格：`docs/design/ux-principles.md` §4
  - 範圍：
    1. Dashboard / Explorer / Insights / Import / AI 操作的 skeleton screen 實作
    2. `var(--border)` pulse animation + `prefers-reduced-motion` fallback
    3. Progress overlay 包含進度數字與可讀狀態說明

- **WORK-M4-F** — Profile-Scoped Insights
  - 設計規格：`docs/features/intelligence.md` §Profile-Scoped Insights、`docs/design/screens-and-nav.md` §Profile-Scoped Insights
  - 範圍：
    1. Insights 頁面接入 shared profile scope，可篩選 surface 切換為 scoped view
    2. Scoped vs archive-wide callout 顯示
    3. 與 Explorer / Assistant 的 scope 語法一致性驗證

---

## 依賴關係圖

```
WORK-M0-A ──┐
WORK-M0-B ──┴── WORK-M1-A → WORK-M1-B → WORK-M2-A → WORK-M2-B → WORK-M3-A → WORK-M3-B → WORK-QC-A → WORK-QC-B → WORK-M4-A → WORK-M4-B → WORK-M4-D → WORK-M4-G / WORK-M4-E / WORK-M4-F
                                                                                                                                   └── WORK-M4-C [!blocked: PG-RD-PLAT-006]
```

---

## 維護規則

1. `STATUS.md` 清空時，從這裡頂部取最多 2 個未被阻塞的 work blocks
2. 新工作一律先放進這裡，除非使用者明確要求立刻調整 `STATUS.md`
3. 如果某個 block 需要再拆，只在 milestone 文檔的 WBS 裡拆，不把 `STATUS.md` / `BACKLOG.md` 再退回原子 task
4. 依賴解除後，直接更新行尾的 `[!blocked: ...]`
