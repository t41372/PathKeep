# STATUS.md — 當前工作

> Agent 每次開工讀這個檔案。一次只做第一個 `[ ]` work block。
> 2026-10 之前已完成的 blocks 見 `CHANGELOG.md`，或 git 歷史裡舊版的本檔（`main` 上的 `docs/plan/STATUS.md`）。

**當前 Milestone：M18 — Frontend redesign（Claude Design prototype → shadcn）**

---

## CURRENT FOCUS

- [ ] **WORK-FRONTEND-REDESIGN-2026-10** — 用新 prototype 整個替換前端（用戶 2026-10-02 指示）
  - 分支：`redesign/v0.4`（從 `main@551f851d` 開出）。第一個遠端 agent 的工作以 `4bbd8d62` 一次導入，它自己的 commit 歷史沒有保留。
  - 讀先：
    `docs/design/prototype-2026-10/`（prototype HTML + 截圖；用 `python3 -m http.server` 開）
    `AGENTS.md`（測試規則：E2E 為主、不跑 mutation、不強制 coverage）
    `docs/architecture/desktop-command-surface.md`
    `docs/plan/program/quality-matrix.md`
  - 用戶要求：
    - 新前端用 shadcn，盡量用 pre-built components；charts 用 evilcharts（`npx shadcn add @evilcharts/...`，Recharts）；heatmap 若前者沒有，用 amicro mono-charts，視覺風格要一致。
    - prototype 缺的東西順手補上；後端 dead code 順手刪；IPC / 卡頓問題看到就修或記錄。
    - E2E 為主，每次 E2E 產出可驗證、可重現的 artifact。
  - 步驟：
    - [x] 0. 分支、prototype 存進 `docs/design/prototype-2026-10/`
    - [x] 1. 改 gate：刪 mutation / coverage gate，AGENTS.md 加新規則，quality-matrix / TESTING 改寫，E2E artifact 設定
    - [x] 2. 後端：補齊前端已在呼叫的命令、IPC 性能修復、刪死命令，結論寫進 `docs/architecture/`（`desktop-command-surface.md` §2026-10、`ipc-performance.md`）
    - [x] 3. 新前端地基：tokens、窗口外殼 + nav rail、i18n、主題、shadcn 組件、數據層
    - [x] 4. 頁面：Home ✓、History ✓、Insights ✓、Ask ✓、Backup ✓、Lock ✓、Command palette ✓、Onboarding ✓（`bun run dev:demo -- --first-run --fresh`）、Settings ✓（General / Security / AI / Storage / About；舊設定的取捨見 commit `2f6f5880`）
    - [ ] 5. 刪舊前端殘留和沒人用的後端命令
    - [x] 6. E2E：`bun run test:e2e`，7 條場景在真 Rust 後端上全綠（first-run → read → change → wipe），產物在 `artifacts/e2e/`
    - [ ] 7. 文檔同步（features / design / architecture）、CHANGELOG、`bun run check` 全綠
  - 已知缺口（2026-10-02 盤點）：
    - ~~前端呼叫但 Rust 沒註冊的命令~~：已補（`load_source_stats`、`get_url_detail`、`preview_wipe_all_data` / `wipe_all_data`），Tauri 與 dev bridge 都有。
    - ~~開機時啟動、選單列圖示~~：已做（用戶 2026-10-02 決定補做）。macOS 實機看過；**Windows / Linux 的 tray、登入啟動、語言偵測從沒編譯過**，release 前要在 CI（Linux 跑 `bun run check`、Windows 手動跑 Platform Native workflow）或實機上編譯並點過一次。
    - ~~IPC 性能問題~~：已修並量測（14.4M：History 列表第一頁約 26 s → 1.9 ms；加密 `app_snapshot` 237 → 97 ms），見 `docs/architecture/ipc-performance.md`。剩下：常見詞的關鍵字搜尋第一頁 14.4M 約 1.1 s；`app_snapshot` 每次查 keychain 未量測。
    - 「刪除所有資料」會保留已安裝的自動備份排程（文案已寫明）。要不要讓刪除也一併移除排程，待用戶決定。
    - 前端 `HistoryQueryResponse` 型別（`src/lib/types/archive.ts`）缺 `totalExact`，`history/queries.ts` 目前用交集型別補。
    - 30 多個前端未用的 intelligence 讀取命令保留待第 5 步決定，清單見 `desktop-command-surface.md` §2026-10。
    - 打包時 Recharts 被併成一個名叫 `heatmap` 的 546 kB chunk（Home 和 Insights 共用、lazy load），不算問題。
  - 2026-10-02 試跑時記下、還沒修的：
    - History 搜尋把同一頁的每一次訪問都列出來，重訪多的頁面會把其他結果擠掉（搜 "tokio"：前 2,000 多條全是同一頁）。要後端給 `query_history` 加「同一網址收成一行」的選項，前端顯示「N 次訪問」。
    - Insights「常搜尋」的次數是整個 query family 的歷史總數，不受日期範圍限制（會大於同範圍的搜尋 KPI）。要後端給一個按範圍統計的命令。
    - ~~排程器沒有沙盒~~：已補 `PATHKEEP_PLATFORM_TEST_SANDBOX_DIR`（只在 debug build 生效），dev:demo 與 E2E 都會設。E2E 現在仍選 Manual，之後可以補一條真的安裝 / 移除排程的場景。
    - `export_history` 先把整個結果集載入記憶體再寫檔，1440 萬條會撐爆 8 GB；`doctor_report` / `repair_health` 用 `visit_id NOT IN (SELECT id FROM archive.visits …)`。兩者都要改成串流 / anti-join。
    - 已接受文檔與新 UI 不一致：`docs/features/archive.md` §8 與 `docs/design/screens-and-nav.md` 要求 App Lock 面板有 Touch ID 開關、recovery hint、config 路徑、上次解鎖時間，新 Settings 沒做（新鎖屏沒有生物辨識路徑）。step 7 要嘛補上、要嘛寫 trade-off 給用戶決定。
    - ~~合成 Firefox profile 被 Floorp / LibreWolf / Waterfox 各認一次~~：已修，override 只套用在 Firefox。
    - Home「一年的瀏覽」載入中會先顯示「0 次瀏覽」。
  - 試跑：`bun run dev:demo`（真後端 + 合成 Chrome×2 / Firefox archive），瀏覽器開 http://127.0.0.1:1420。

- [ ] **WORK-REFACTOR-INDEXING**（用戶 2026-06-21 指示，承巨檔審計）— `ai/indexing.rs` 拆分重構。詳見 `main` 上的 STATUS。_非當前 focus_

> `WORK-V03-PAPER-REDESIGN-A`（Paper 風格前端重建）被本次 redesign 取代，不再繼續。
