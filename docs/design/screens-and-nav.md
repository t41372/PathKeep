# 畫面與導航結構

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出。  
> designer prototype 匯出檔目前已在 repo：`reference/PathKeep — Desktop UI Design/`。  
> 這份 export 主要覆蓋 shell chrome 與 Dashboard 的視覺語言；對於 prototype 尚未畫出的畫面或狀態，這份文檔與 [design-tokens.md](design-tokens.md) 仍是現行 source of truth。
> production token source of truth 是 [design-tokens.md](design-tokens.md)；新增 token 時要同步更新文檔與 `src/styles/tokens.css`。
> 如果 prototype 缺少某個畫面或狀態，才用 Stitch / 補充設計決策補齊；補齊時仍需維持和 prototype 一致的視覺語言與導航結構。

---

## 畫面清單

| 畫面                   | 核心職責                                                                                                                                                                                  |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Onboarding / Setup** | 首次啟動引導：發現瀏覽器、選擇 profile、設定存儲、加密選擇                                                                                                                                |
| **Dashboard**          | 備份狀態總覽、最近 run 摘要、歷史上的今天、定期總結卡片、Job Queue 狀態、快速操作入口                                                                                                     |
| **History Explorer**   | 時間軸 + 全文搜尋 + 篩選 + 詳情 + 匯出                                                                                                                                                    |
| **Insights**           | 洞察卡片、topic timeline、threads、query ladders、profile facets、storage analytics                                                                                                       |
| **AI Assistant**       | 自然語言問答介面                                                                                                                                                                          |
| **Import**             | Takeout 導入 wizard + 瀏覽器直接導入（含 step-by-step UI）、recent batch review、`?batch=` deep-link、revert / restore                                                                    |
| **Audit Ledger**       | Run timeline、summary delta、import change preview、artifact / warning review、rollback / restore quick jump                                                                              |
| **Security**           | 加密設定、keyring、rekey、密碼警告                                                                                                                                                        |
| **App Lock**           | App 級鎖定畫面：啟動時與閒置逾時後出現；macOS 可用 Touch ID 解鎖當前 session，其餘平台維持 truthful capability / degradation；鎖定時所有資料存取完全阻斷                                  |
| **Schedule Setup**     | 排程預覽 → 手動安裝/自動安裝 → 狀態監控                                                                                                                                                   |
| **Settings**           | 通用設定、analytics consent、manual update check / install、AI provider 管理、remote backup PME、derived-state controls、MCP 開關、數據目錄、archive / audit path、版本與 git commit 信息 |

---

## Prototype Coverage Snapshot

### 目前 export 已覆蓋

- shell chrome：sidebar 分區、brand / version、archive status footer、topbar 搜尋、共享 profile scope switcher 與主 CTA
- Dashboard 視覺語言：stat cards、recent runs table、On This Day、storage breakdown、AI / queue summary 的資訊層級
- Dashboard 導航語法：從首頁快速跳到 Explorer、Assistant、Insights、Audit 等核心入口

### 目前 export 尚未明確覆蓋

- Onboarding wizard 的逐步狀態、empty / error / resume-later 細節
- Import / rollback / doctor repair / rekey / remote backup 的 PME step-by-step 畫面
- Audit run detail、Schedule verify / mismatch、Security recovery / warning 變體
- Explorer / Assistant / Insights 的 loading / empty / disabled / failed / explainability 狀態
- keyboard-only walkthrough、reduced-motion fallback、長字串 i18n wrapping 等非靜態視覺驗收

## Non-Prototype State Coverage

- Onboarding、shared empty / error / loading、locked / no-data、permission-denied 等 production state，現在以本頁、[ux-principles.md](ux-principles.md) 與對應 feature / milestone docs 為 source of truth；prototype 沒畫到不再代表 UX 未定義。
- long-running operation、generated artifact review、rollback confirmation、manual fallback 與 verify / rollback hint，全都遵循 PME grammar，而不是各頁自己發明流程。
- `On This Day` 與其他 evidence surface 以使用者目前系統 timezone 的本地日曆日判斷，不再用 raw UTC slice 假裝是「今天」。
- keyboard-only walkthrough、reduced-motion fallback、locale-length wrapping 已是 trust-critical acceptance contract；剩餘的全站 accessibility review 與 release-level polish 留在 M4。
- route metadata、sidebar section label、topbar title / subtitle、loading / skeleton label、empty / error / disabled state，以及 browser preview honesty copy 都屬於正式 i18n surface；不能因為 prototype 沒畫到文字細節就留下英文硬編碼。
- Settings 的 remote backup 現在以 `Preview / Manual / Execute / Verify` tabs 呈現：Preview 顯示 bundle path / object key / upload URL，Manual 保留 curl command 與 retention guidance，Execute 顯示 upload result，Verify 則列出 checksum / required-entry / restore-readiness checks。
- Settings 的 enrichment / derived-state panel 是正式 review surface，而不是 debug affordance。它必須顯示 plugin version、queue、freshness、derived tables、storage impact，以及 rebuild / clear controls。
- Settings 的 general diagnostics 現在是 support / release 文檔依賴的正式入口：至少要顯示 app data root、archive DB path、audit repo path、app version、git short SHA，並提供直接打開對應路徑的動作。
- Insights 現在除了既有 card / topic / thread surface 外，還要顯示 storage analytics 與 latest growth signal，並提供回到 Audit run 的 deep-link。
- M5-B 起，Insights 也正式包含 `query groups`、`reference pages`、`source effectiveness`、`template summaries` 與 deterministic module registry status；這些都屬 shipping review surface，不是 debug-only affordance。
- shared profile scope 是 production shell 的正式 viewer state：Topbar 可切換全域 viewing scope；Explorer 預設繼承、Assistant / Insights 直接沿用，Dashboard 則必須用 callout 清楚說明哪些區塊是 scoped、哪些 KPI 仍是 archive-wide。
- Settings 的 derived-state panel 現在除了 enrichment runtime review，還要顯示 deterministic module registry：module enable / disable、dependency、derived tables、last built time、stale reason，以及需要 manual rebuild 的 honesty copy。
- Assistant 的 empty / disabled state 要保留 seeded prompts、settings / queue 修復入口，以及 shared profile scope honesty；不能只剩「AI 尚未啟用」這種靜態段落。
- Audit run detail 應以 `Summary / Artifacts / Warnings` 分頁控制資訊密度，同時保留 open / copy path 動作在單次 review 內可達。
- Schedule 除了 Preview / Manual / Execute tabs 外，還要把 Verify 做成正式 surface：顯示 install state、detected files、warnings、latest audit artifact，並提供 PME quick-jump，而不是把驗證訊息藏在單一 badge。
- Onboarding 與 Dashboard 的 browser profile surface 現在也屬於 trust-critical review surface：除了 `history found / missing` 之外，還要顯示 browser-retention honesty，明講 browser-managed local history 可能在下一次 backup 前消失，而 PathKeep 只有在成功 backup 後才開始提供 append-only 保存。

---

## 導航

- 左側 sidebar 導航（可收合）。
- 頂部顯示當前頁面的 breadcrumb / title。
- 全局快速搜尋入口。

### M1 導航與 deep-link 規則

- Sidebar 依固定分區導航：`CORE`（Dashboard / Explorer / Insights / Assistant）、`OPERATIONS`（Import / Audit / Schedule）、`SYSTEM`（Security / Settings）；Onboarding 是 utility route，不常駐 sidebar。
- 頂部搜尋送到 `History Explorer`，直接寫入 `/explorer?q=...`，讓搜尋結果可以被複製、重整和重新打開。
- Explorer 的 day-one filter deep-link 使用 query string：`q`、`profileId`、`browserKind`、`domain`、`start`、`end`、`sort`、`regex`、`page`。
- Audit Ledger 的 run detail deep-link 使用 `/audit?run=<id>`；Dashboard recent runs 直接跳進這個 URL。
- Import recent batch review 允許 `/import?batch=<id>` deep-link；Audit / Dashboard 可以直接把使用者帶回指定 batch 的 review surface。
- Dashboard zero-state、Security、Topbar 都可以回到 Onboarding，確保 first-backup flow 永遠有明確入口。
- Onboarding shell header 必須有明確的 `Exit setup` 動作；離開後保留目前已選的 storage / profile / security 決策，避免把使用者困在 setup route。
- Schedule / Security 在 M1 起就是 review surface；M2 之後 Import、Audit、Dashboard、Settings 也要能透過 callout / quick action 直接跳回這些修復頁，而不是把排障資訊藏在單一路由裡。
- Sidebar 以視窗高度而不是頁面內容高度佈局；footer 的 archive 狀態與 theme toggle 在不捲動主內容區的情況下也要可見。
- Settings 擁有 day-one 語言切換與平台 troubleshooting；Schedule 擁有 platform-specific Preview / Manual / Execute / Verify story；Import 擁有 recent batch review、revert / restore 與 doctor repair 入口。
- 共享 profile scope 存在於 shell chrome，而不是散落在各頁各自記憶；Explorer 若未指定 page-specific `profileId`，必須明講自己目前沿用 shared scope。

### M3 Intelligence deep-link 規則

- Explorer 以同一個 `/explorer` route 承接 `keyword`、`semantic`、`hybrid` 三種 recall mode；`mode` 走 query string，避免 intelligence 結果和 canonical evidence 被拆成兩套路由。
- semantic result、assistant citation、insight evidence 都要能 deep-link 回 `/explorer`，至少可帶 `q`、`profileId`、`domain` 等 canonical filters 讓使用者回看原始記錄。
- Assistant 的 seeded follow-up 使用 `/assistant?question=...`；Explorer、Insights、Dashboard 都可以透過這個 deep-link 把 scoped 問題帶進 assistant composer。
- Dashboard 的 intelligence quick actions 必須直接通往 Explorer、Assistant、Insights；錯誤或 disabled 狀態下還要能跳到 Settings / queue controls，而不是只剩靜態說明。
- Explorer 的 `semantic` / `hybrid` surface，以及 Assistant、Insights 的 AI status panel，都必須顯示 provider / model、queue counts、index state，並提供 test provider、refresh queue、rebuild / clear index、open settings 這類 controls；keyword-first Explorer 不應被 optional AI 面板壓過主工作流。
- Settings 是 M4-A 起的 remote backup 與 derived-state 控制塔：從這裡可以完成 remote upload 的 PME、credential review、bundle verification、plugin enable / disable、derived rebuild / clear，並回鏈到 Audit run 驗證最新 growth signal。

### App Lock 畫面與導航規則

- App Lock 是獨立的 utility route（`/lock`），不常駐 sidebar。
- 啟動時若 App Lock 已啟用，PathKeep 先顯示 lock screen，通過驗證後才載入主 shell。
- 閒置逾時（idle timeout）觸發時，自動導向 `/lock`，主 shell chrome 完全不渲染。
- Topbar 在 App Lock 已啟用時提供 `Lock now` 動作；手動鎖定也走同一個 `/lock` route。
- Lock screen 顯示 PathKeep branding、鎖定原因、config path、上次解鎖時間、passcode input、recovery hint callout，以及打開 config path 的 recovery 動作。
- 若平台是 macOS 且 Touch ID 目前不可用，lock screen 仍顯示 Touch ID CTA / note，但按鈕必須 disabled，並明講會回退到 passcode。
- 若使用者已在 Settings 關閉 biometric unlock，lock screen 不得再顯示 Touch ID / biometric CTA；capability 可用不代表可以繞過設定直接解鎖。
- 若平台不是 macOS，lock screen 繼續顯示 generic biometric honesty copy，不假裝有 native parity。
- 鎖定狀態下不僅 UI 隱藏，後端 query 與 MCP history query 也必須被阻擋 — 避免透過 dev tools / MCP 繞過。
- Settings 的 App Lock panel：enable / disable toggle、idle timeout duration、biometric toggle、passcode set / update / clear、recovery hint、`Lock now`、config path、last unlocked timestamp。
- Settings 另外新增兩個正式 review surface：analytics consent（explicit opt-in、payload boundary、endpoint honesty）與 manual update check / install（release availability、notes、install progress、restart CTA）。
- App Lock 與 archive encryption 是**獨立的兩層保護**：App Lock 保護 UI session，encryption 保護資料庫檔案。兩者可獨立啟用。
- 設計規格 → `docs/features/archive.md` §8

### Profile-Scoped Insights 導航規則

- Insights 頁面支援透過 shell chrome 的共享 profile scope 篩選 insight 資料。
- 當使用者在 topbar 選擇特定 profile 時，Insights 的 cards、topic timeline、threads 等 surface 都切換為該 profile 的 scoped view。
- Dashboard 的 aggregate KPIs 仍維持 archive-wide；Insights 頁面在 scoped 模式下必須以 callout 或 badge 明確標示「目前為 profile-scoped view」。
- scoped vs all-profile 切換不得產生新的 route；以 query string `profileId` 或沿用 shared scope 處理，保持與 Explorer 的 scope 語法一致。
