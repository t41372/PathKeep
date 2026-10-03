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
    - [ ] 2. 後端：補齊前端已在呼叫的命令、IPC 性能修復、刪死命令，結論寫進 `docs/architecture/`
    - [x] 3. 新前端地基：tokens、窗口外殼 + nav rail、i18n、主題、shadcn 組件、數據層
    - [ ] 4. 頁面：Home ✓、History ✓、Insights ✓、Ask ✓、Backup ✓、Lock ✓、Command palette ✓、**Settings ✗、Onboarding ✗**
    - [ ] 5. 刪舊前端殘留和沒人用的後端命令
    - [ ] 6. E2E：在 desktop bridge（真 Rust 後端）上跑中高難度場景，產出 artifact（現有 `tests/e2e/*` 仍針對舊 UI，要重寫）
    - [ ] 7. 文檔同步（features / design / architecture）、CHANGELOG、`bun run check` 全綠
  - 已知缺口（2026-10-02 盤點）：
    - 前端呼叫但 Rust 沒註冊的命令：`load_source_stats`、`get_url_detail`（`vault-core` 已有實作）、`preview_wipe_all_data` / `wipe_all_data`（沒有實作）。
    - Settings 的「開機時啟動」「選單列圖示」在後端沒有對應（沒有 tray、沒有 autostart）。用戶 2026-10-02 決定：直接補做（`tauri-plugin-autostart` + Tauri 內建 tray，命令 `get_desktop_integration` / `set_launch_at_login` / `set_menu_bar_icon`，前端契約在 `src/lib/backend-client/desktop.ts`）。
    - IPC 性能問題（尚未修）：每次開 archive 都對 search DB 做一次寫交易 + `COUNT(*)`（search DB 無 WAL）；每個命令都重讀 config；`query_history` 每頁一次精確 `COUNT(*)`（已加 `includeTotal`）；`app_snapshot` 一次開約 5 次 DB。已修：keychain 查詢快取 60 秒。
    - 打包時 Recharts 被併成一個名叫 `heatmap` 的 546 kB chunk（Home 和 Insights 共用、lazy load），不算問題。
  - 2026-10-02 試跑時記下、還沒修的：
    - History 搜尋把同一頁的每一次訪問都列出來，重訪多的頁面會把其他結果擠掉（搜 "tokio"：前 2,000 多條全是同一頁）。要後端給 `query_history` 加「同一網址收成一行」的選項，前端顯示「N 次訪問」。
    - Insights「常搜尋」的次數是整個 query family 的歷史總數，不受日期範圍限制（會大於同範圍的搜尋 KPI）。要後端給一個按範圍統計的命令。
    - **排程器沒有沙盒**：debug build 的 keyring 會被 `CHB_TEST_KEYRING_DIR` 導到檔案，但 launchd / Task Scheduler 沒有對應的導向，dev:demo 和 E2E 會讀到（apply 時會改到）使用者真實的 LaunchAgent。E2E 動排程之前要先補一個只在 debug 生效的沙盒（label + LaunchAgents 目錄 + 不呼叫真的 launchctl）。
    - 合成 Firefox profile 被 Floorp / LibreWolf / Waterfox 也各認了一次（都顯示 `default-release`），疑似 Gecko 系 adapter 共用 `CHB_FIREFOX_PROFILES_DIR`。
  - 試跑：`bun run dev:demo`（真後端 + 合成 Chrome×2 / Firefox archive），瀏覽器開 http://127.0.0.1:1420。

- [ ] **WORK-REFACTOR-INDEXING**（用戶 2026-06-21 指示，承巨檔審計）— `ai/indexing.rs` 拆分重構。詳見 `main` 上的 STATUS。_非當前 focus_

> `WORK-V03-PAPER-REDESIGN-A`（Paper 風格前端重建）被本次 redesign 取代，不再繼續。
