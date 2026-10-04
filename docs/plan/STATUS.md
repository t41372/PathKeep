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
    - [x] 5. 刪舊前端殘留和沒人用的後端命令（`5edf5645`；32 個 intelligence 讀取命令依用戶決定保留給 Insights）
    - [x] 6. E2E：`bun run test:e2e`，在真 Rust 後端上全綠（first-run → read → change → wipe），產物在 `artifacts/e2e/`；2026-10-04 Security 分支跑過時是 10 條場景
    - [ ] 7. 文檔同步（features / design / architecture）、CHANGELOG、`bun run check` 全綠
  - 已知缺口（2026-10-02 盤點）：
    - ~~前端呼叫但 Rust 沒註冊的命令~~：已補（`load_source_stats`、`get_url_detail`、`preview_wipe_all_data` / `wipe_all_data`），Tauri 與 dev bridge 都有。
    - ~~開機時啟動、選單列圖示~~：已做（用戶 2026-10-02 決定補做）。macOS 實機看過；**Windows / Linux 的 tray、登入啟動、語言偵測從沒編譯過**。2026-10-02 已推上 `origin/redesign/v0.4` 並手動觸發 CI 與 Platform Native workflow；編譯過了之後仍要在實機上點過一次。
    - ~~IPC 性能問題~~：已修並量測（14.4M：History 列表第一頁約 26 s → 1.9 ms；加密 `app_snapshot` 237 → 97 ms），見 `docs/architecture/ipc-performance.md`。剩下：常見詞的關鍵字搜尋第一頁 14.4M 約 1.1 s；`app_snapshot` 每次查 keychain 未量測。
    - 「刪除所有資料」會保留已安裝的自動備份排程。用戶 2026-10-02 決定：刪除時一併移除排程。已做（`d0755426`）。
    - 前端 `HistoryQueryResponse` 型別（`src/lib/types/archive.ts`）缺 `totalExact`，`history/queries.ts` 目前用交集型別補。
    - 30 多個前端未用的 intelligence 讀取命令：用戶 2026-10-02 決定保留，留給 Insights 之後擴充（清單見 `desktop-command-surface.md` §2026-10）。`vercel.json`（browser-only 預覽站）已刪。
    - 打包時 Recharts 被併成一個名叫 `heatmap` 的 546 kB chunk（Home 和 Insights 共用、lazy load），不算問題。
  - 2026-10-02 試跑時記下、還沒修的：
    - History 搜尋把同一頁的每一次訪問都列出來，重訪多的頁面會把其他結果擠掉。用戶 2026-10-02 決定：搜尋結果按網址收成一行。已做（`groupByUrl`，`22f11a5e`）；「topic」這種每頁都中的詞第一頁在 14.4M 仍要約 10 s（精確總數約 25 s）。2026-10-04 決定 release 前要修：排序 / 分組只在最新的有限窗口內做、總數超過上限顯示「N+」並在 UI 說明。進行中。
    - Regex 搜尋只掃最新的 50,000 筆訪問，計數卻像是精確的（14.4M 時只覆蓋約 0.35%）。決定：regex 要涵蓋整個 archive——按時間往回分段掃、每段有時間上限、捲動或「繼續搜尋」接著掃，全部掃完前計數標成「N+（目前為止）」並寫明掃到哪天。進行中。
    - ~~Insights「常搜尋」的次數是整個 query family 的歷史總數，不受日期範圍限制~~：已修。新命令 `get_frequent_searches` 只數範圍內的關鍵字搜尋（intelligence migration 9 給 `search_events` 加上訪問時間和索引），14.4M 下 30 天約 10 ms、一年約 145 ms，見 `ipc-performance.md` §8。
    - ~~排程器沒有沙盒~~：已補 `PATHKEEP_PLATFORM_TEST_SANDBOX_DIR`（只在 debug build 生效），dev:demo 與 E2E 都會設。E2E 的 onboarding 仍選 Manual；`schedule.spec`（2026-10-04）在沙盒裡裝自訂間隔、核對檔案、移除。改了 project root 卻沒設沙盒的 debug run 會被排程器拒絕。
    - ~~`export_history` 先把整個結果集載入記憶體；`doctor_report` / `repair_health` 用 `NOT IN`~~：已修。匯出改成逐頁寫入暫存檔再原子改名（1M：heap 1.2 GB → 0.6 MB）；doctor 改成 `NOT EXISTS` 按主鍵查（14.4M：SQLite 暫存 321 MB → 0）。見 `ipc-performance.md` §7、§9。14.4M 全量匯出量過：heap +0.6 MiB、常駐約 92 MiB、4.7 GB 檔。剩下：冷快取下要 66 分鐘，Settings 沒有進度和取消。
    - ~~App Lock 面板與已接受文檔不一致~~：用戶 2026-10-04 選 trade-off 選項 B，已做（Touch ID 開關與解鎖、passcode hint、鎖屏「忘記密碼？」；不做 config 路徑列與上次解鎖時間），文檔已改。
    - ~~更改 archive 密碼 / 關閉加密不需要目前的密碼~~：已修（2026-10-04），後端用目前的密碼實際打開 archive 驗證，dev bridge 也繞不過。
    - E2E 偶發失敗（2026-10-04 見過一次，重跑通過）：`history.spec`「a starred page keeps its note after a reload」輸入搜尋後立刻點結果，那一刻列表有兩列同標題（之後只剩一列），strict mode 報錯。疑似舊查詢的結果還在畫面上，屬 History 範圍，未修。
    - ~~合成 Firefox profile 被 Floorp / LibreWolf / Waterfox 各認一次~~：已修，override 只套用在 Firefox。
    - Home「一年的瀏覽」載入中會先顯示「0 次瀏覽」。
  - 2026-10-04 用戶：「UI 本身就缺很多東西，prototype 做得很寬泛，很多細節沒做出來，你補上就行。」連結預覽：繼續抓、在 UI 顯示。依此分五條平行補齊（進行中）：Security（App Lock B + 改密碼驗證）、History（預覽圖、標籤、搜尋語法說明、Rust regex 方言、語意狀態）、Backup（自訂排程間隔、安裝前顯示檔案、Verify、Browser Direct 匯入、匯入批次回滾、執行詳情、排程沙盒防呆）、Settings（背景工作、derived state、搜尋 tuning、診斷）、Insights（day / site / 搜尋家族 / 重找頁面 drill-in 與 V1 洞察卡）。每條都要附 E2E。
  - 2026-10-04 Backup 補完時記下、還沒修的：
    - `revert_import_batch` / `restore_import_batch` 每次都同步 `rebuild_search_projection` 整個搜尋投影；1440 萬條時一次復原要重建全部索引。應改成只刷新該批影響的 URL（匯入時已經這樣做）。
    - `vite.config.ts` 的 `server.watch.ignored` 含 `**/.claude/**`，在 `.claude/worktrees/` 裡開的 dev server 看不到任何檔案變更（要重開才生效）。
    - 自訂排程間隔上限 30 天是新加的限制（archive.md §2 原文只寫最短 1 分鐘）。
  - 試跑：`bun run dev:demo`（真後端 + 合成 Chrome×2 / Firefox archive），瀏覽器開 http://127.0.0.1:1420。

- [ ] **WORK-REFACTOR-INDEXING**（用戶 2026-06-21 指示，承巨檔審計）— `ai/indexing.rs` 拆分重構。詳見 `main` 上的 STATUS。_非當前 focus_

> `WORK-V03-PAPER-REDESIGN-A`（Paper 風格前端重建）被本次 redesign 取代，不再繼續。
