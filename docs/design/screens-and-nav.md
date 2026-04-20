# 畫面與導航結構

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出。  
> designer prototype 匯出檔目前已在 repo：`reference/PathKeep — Desktop UI Design/`。  
> 這份 export 主要覆蓋 shell chrome 與 Dashboard 的視覺語言；對於 prototype 尚未畫出的畫面或狀態，這份文檔與 [design-tokens.md](design-tokens.md) 仍是現行 source of truth。
> production token source of truth 是 [design-tokens.md](design-tokens.md)；新增 token 時要同步更新文檔與 `src/styles/tokens.css`。
> 如果 prototype 缺少某個畫面或狀態，才用 Stitch / 補充設計決策補齊；補齊時仍需維持和 prototype 一致的視覺語言與導航結構。
> 長期 UI review / implementation 紅線另見 [ui-review-guardrails.md](ui-review-guardrails.md)；這份文檔負責 route 與 IA 規格，guardrails 負責哪些退化一律不能放行。

---

## 畫面清單

| 畫面                   | 核心職責                                                                                                                                                                                                                                                                                                    |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Onboarding / Setup** | 首次啟動引導：發現瀏覽器、選擇 profile、設定存儲、加密選擇                                                                                                                                                                                                                                                  |
| **Dashboard**          | 備份狀態總覽、最近 run 摘要、歷史上的今天、年度瀏覽節奏預覽、定期總結卡片、Job Queue 狀態、快速操作入口                                                                                                                                                                                                     |
| **History Explorer**   | 時間軸 + 全文搜尋 + 篩選 + 詳情 + 匯出                                                                                                                                                                                                                                                                      |
| **Intelligence**       | Core Intelligence 主頁：analysis snapshot、Insight Access、spotlight、research signals、evidence / health、runtime digest，以及 day / domain / query family / refind / session / trail / compare-set 的 shared insights routes                                                                              |
| **AI Assistant**       | 自然語言問答介面                                                                                                                                                                                                                                                                                            |
| **Import**             | Takeout 導入 wizard + 瀏覽器直接導入（含 step-by-step UI）、recent batch review、`?batch=` deep-link、revert / restore                                                                                                                                                                                      |
| **Audit Ledger**       | Run timeline、summary delta、import change preview、artifact / warning review、rollback / restore quick jump                                                                                                                                                                                                |
| **Security**           | 加密設定、keyring、rekey、密碼警告                                                                                                                                                                                                                                                                          |
| **App Lock**           | App 級鎖定畫面：啟動時與閒置逾時後出現；macOS 可用 Touch ID 解鎖當前 session，其餘平台維持 truthful capability / degradation；鎖定時所有資料存取完全阻斷                                                                                                                                                    |
| **Schedule Setup**     | 排程預覽 → 手動安裝/自動安裝 → 狀態監控                                                                                                                                                                                                                                                                     |
| **Settings**           | 通用設定、analytics consent、manual update check / install、AI provider 管理、remote backup PME、manual external-output review / copy-export、trusted local host install / verify、derived-state controls（含 search-engine rule editor）、MCP 開關、數據目錄、archive / audit path、版本與 git commit 信息 |

---

## Prototype Coverage Snapshot

### 目前 export 已覆蓋

- shell chrome：sidebar 分區、brand / version、archive status footer、background-work footer strip、topbar 搜尋、共享 profile scope switcher 與主 CTA
- Dashboard 視覺語言：stat cards、recent runs table、On This Day、yearly browsing rhythm preview、storage breakdown、AI / queue summary 的資訊層級
- Dashboard 導航語法：從首頁快速跳到 Explorer、Assistant、Intelligence、Audit 等核心入口

### 目前 export 尚未明確覆蓋

- Onboarding wizard 的逐步狀態、empty / error / resume-later 細節
- Import / rollback / doctor repair / rekey / remote backup 的 PME step-by-step 畫面
- Audit run detail、Schedule verify / mismatch、Security recovery / warning 變體
- Explorer / Assistant / Intelligence 的 loading / empty / disabled / failed / explainability 狀態
- keyboard-only walkthrough、reduced-motion fallback、長字串 i18n wrapping 等非靜態視覺驗收

## Non-Prototype State Coverage

- Onboarding、shared empty / error / loading、locked / no-data、permission-denied 等 production state，現在以本頁、[ux-principles.md](ux-principles.md) 與對應 feature / milestone docs 為 source of truth；prototype 沒畫到不再代表 UX 未定義。
- long-running operation、generated artifact review、rollback confirmation、manual fallback 與 verify / rollback hint，全都遵循 PME grammar，而不是各頁自己發明流程。
- `On This Day` 與其他 evidence surface 以使用者目前系統 timezone 的本地日曆日判斷，不再用 raw UTC slice 假裝是「今天」；`On This Day` 只回看過去幾年的同一天，不得把當前年份今天的紀錄混進去，而且現在只屬於 Dashboard，不再佔用 `/intelligence` 的 route chrome。
- keyboard-only walkthrough、reduced-motion fallback、locale-length wrapping 已是 trust-critical acceptance contract；剩餘的全站 accessibility review 與 release-level polish 留在 M4。
- route metadata、sidebar section label、topbar title / subtitle、loading / skeleton label、empty / error / disabled state，以及 browser preview honesty copy 都屬於正式 i18n surface；不能因為 prototype 沒畫到文字細節就留下英文硬編碼。
- Settings 的 remote backup 現在以 `Preview / Manual / Execute / Verify` tabs 呈現：Preview 顯示 bundle path / object key / upload URL，Manual 保留 curl command 與 retention guidance，Execute 顯示 upload result，Verify 則列出 checksum / required-entry / restore-readiness checks。
- Settings 現在也是 `embed cards` / `widget snapshot` / `public snapshot` 的第一個正式 consumer surface：使用者可在這裡手動 preview、切換 shared profile scope 與 local time window、複製 raw payload；同一塊 surface 也會 preview / build / verify 第一個 trusted local host `browser-snippet-v1`，固定寫到 `app_root/integrations/core-intelligence/browser-snippet-v1/` 的 `index.html` / `bundle.json`。目前仍不會直接安裝 OS widget，也不會發布 localhost/public host API。
- `/intelligence`、`/intelligence/day/:date` 與 `/intelligence/domain/:domain` 的 deterministic section 現在都會共用 compact evidence / freshness badge + floating review panel：顯示 generated-at、active scope / window、owning modules、source tables、是否包含 enrichment，以及 stale / disabled / degraded reason；這是 review chrome，不是 mutation surface。
- section title 與 compact evidence / freshness badge 必須共用同一個 header row。badge 是 inline-end review chrome，不得獨占一整行、也不得把 hover / focus 命中區擴成整張卡的空白 header。
- Settings 的 enrichment / derived-state panel 是正式 review surface，而不是 debug affordance。它必須顯示 queue、freshness、derived tables、storage impact，以及 rebuild / clear controls；plugin / module 的內部版本標記只留在 diagnostics / runtime trace，不佔主產品 review chrome。
- `refind` overview/day/detail route 現在也共用同一套 workbench shell：title / description / factor presentation / entity-first CTA grammar 不得再由各頁各自手寫。
- Explorer `session` / `trail` grouped view 與 promoted route member list 現在共用同一套 workbench row primitive；expand header 仍維持 browse-first canonical surface，不可偷渡成直接導頁。
- Settings external outputs 與 trusted local host review 現在共用同一套 review row / code preview / target-link grammar；後續若要再擴到 Jobs / Import / Audit，必須沿用這批 primitive，而不是再長一套新 shell。
- M11 之後，neutral review primitive 不再只屬於 intelligence subtree：Settings、Schedule、Audit 與 Jobs 也正式共用同一套 `review-surface`、PME tab、generated-artifact viewer 與 verify checklist grammar；剩餘 general diagnostics / support actions / Import follow-through 漂移改由 M12 追蹤。
- M12 之後，Settings general diagnostics / App Lock、Audit manifest / artifact review、Import selected-batch audit path、Schedule detected-file / audit quick jump、Security / Lock config path，以及 Explorer export path 現在都共用同一套 review-layer support-action / clipboard grammar；Jobs plugin / module summary rows與 dev-bridge / worker parity 則明確 deferred 到 M13，而不是在 M12 內硬抽成過度抽象的共用殼。
- Jobs 頁是正式 shipping route：顯示 background queue summary、recent AI jobs、recent derived-data jobs、pause / resume control、plugin / module runtime status，以及 crash / restart recovery note；它不是 hidden diagnostics page。
- Jobs 頁的閱讀順序必須先回答「現在在做什麼、什麼只是排隊或延後、哪裡需要我處理」，再展開 plugin / module / recent job 細節。`readable-content-refetch` 的大型 backlog 不能被排版誤導成「全部失敗」；頁面要先把 deterministic rebuild 優先、network fetch deferred、少量 failed/retry 的邊界講清楚。
- Settings 的 general diagnostics 現在是 support / release 文檔依賴的正式入口：至少要顯示 app data root、archive DB path、audit repo path、app version、git short SHA，並提供直接打開對應路徑的動作。
- Intelligence 現在除了既有 card / topic / thread surface 外，還要顯示 storage analytics 與 latest growth signal，並提供回到 Audit run 的 deep-link。storage analytics 的 top-level summary 先固定為 `core history` / `other data` 兩個 bucket，detail 再在卡內展開。
- Intelligence 頁的主閱讀順序必須是 `analysis snapshot -> spotlight -> research signals -> evidence / health`。完整 queue / retry / cancel review 留在 Jobs；Intelligence 只保留一個小型 runtime digest 與回到 Jobs 的入口，避免真正的洞察被 runtime chrome 擠到頁面下半部。
- Intelligence 首屏只有 **執行摘要 / 時段概覽 / 瀏覽節奏** 可以佔滿主內容欄寬；其餘卡片一律留在 half-width row 或 secondary grid，並遵守限高 + 卡片內滾動。
- Intelligence 的 `Browsing Rhythm` 主圖現在正式採用 **真實日期日曆熱力圖**；每個方格都必須對應一天真實日期。`/intelligence` overview 與 Dashboard 的這張卡現在採 **preview-first**：點某一天後，先在卡片下方 lazy-load 當天摘要 / 重點網站 / 24 小時分布；只有使用者再按明確的 `查看詳情` CTA，才進 `/intelligence/day/:date` 的完整 day insights route。這是 `Browsing Rhythm` 卡片的特例，不可外溢成其他 day entry surface 的通則。
- Dashboard 也正式共用同一套 `Browsing Rhythm` 真實日期日曆熱力圖，但固定以**單一 calendar year** 呈現。若 archive 內有多個年份，卡片必須顯示當前查看年份，並以 bounded pager 在 `getDiscoveryTrend(..., 'day').availableYears` 之間前後翻頁；不得翻到不存在的未來年份。這張卡不受 `/intelligence` route time scope 影響。
- `/intelligence` 頂部現在固定有一條精簡的 `Insight Access` strip：可直接用本地日曆日或 domain 打開完整 insights route。這條 strip 是 entity-first entry，不是另一套獨立 fetch surface。
- `/intelligence` 不再承擔 external-output full review。它只保留一個小型 CTA，把使用者帶到 Settings 的 manual review / trusted-local-host surface，避免主產品分析頁再次長出第二套 export / host-integration chrome。
- M5-B 起，Intelligence 也正式包含 `query groups`、`reference pages`、`source effectiveness`、`template summaries` 與 deterministic module registry status；這些都屬 shipping review surface，不是 debug-only affordance。
- shared profile scope 是 production shell 的正式 viewer state：Topbar 可切換全域 viewing scope；Explorer 預設繼承、Assistant / Intelligence 直接沿用，Dashboard 則必須用 callout 清楚說明哪些區塊是 scoped、哪些 KPI 仍是 archive-wide。
- Settings 的 derived-state panel 現在除了 enrichment runtime review，還要顯示 deterministic module registry：module enable / disable、dependency、derived tables、last built time、stale reason，以及 auto rebuild job / manual override 的 honesty copy。
- Assistant 的 empty / disabled state 要保留 seeded prompts、settings / queue 修復入口，以及 shared profile scope honesty；不能只剩「AI 尚未啟用」這種靜態段落。
- Audit run detail 應以 `Summary / Artifacts / Warnings` 分頁控制資訊密度，同時保留 open / copy path 動作在單次 review 內可達。
- Schedule 除了 Preview / Manual / Execute tabs 外，還要把 Verify 做成正式 surface：顯示 install state、detected files、warnings、latest audit artifact，並提供 PME quick-jump，而不是把驗證訊息藏在單一 badge。
- Onboarding 與 Dashboard 的 browser profile surface 現在也屬於 trust-critical review surface：除了 `history found / missing` 之外，還要顯示 browser-retention honesty，明講 browser-managed local history 可能在下一次 backup 前消失，而 PathKeep 只有在成功 backup 後才開始提供 append-only 保存。

---

## 導航

- 左側 sidebar 導航（可收合）。
- 頂部顯示當前頁面的 breadcrumb / title。
- 頂部左側提供全局上一頁 / 下一頁按鈕，語意與瀏覽器一致：只能在本次 app 內已走過的 route history 間返回或前進。
- 全局快速搜尋入口。

### M1 導航與 deep-link 規則

- Sidebar 依固定分區導航：`CORE`（Dashboard / Explorer / Intelligence / Assistant）、`OPERATIONS`（Import / Audit / Jobs / Schedule）、`SYSTEM`（Security / Settings）；Onboarding 是 utility route，不常駐 sidebar。
- 頂部搜尋送到 `History Explorer`，直接寫入 `/explorer?q=...`，讓搜尋結果可以被複製、重整和重新打開。
- Explorer 的 day-one filter deep-link 使用 query string：`q`、`profileId`、`browserKind`、`domain`、`start`、`end`、`sort`、`regex`、`page`、`pageSize`。
- Audit Ledger 的 run detail deep-link 使用 `/audit?run=<id>`；Dashboard recent runs 直接跳進這個 URL。
- Import recent batch review 允許 `/import?batch=<id>` deep-link；Audit / Dashboard 可以直接把使用者帶回指定 batch 的 review surface。
- Dashboard zero-state、Security、Topbar 都可以回到 Onboarding，確保 first-backup flow 永遠有明確入口。
- Onboarding shell header 必須有明確的 `Exit setup` 動作；離開後保留目前已選的 storage / profile / security 決策，避免把使用者困在 setup route。
- Schedule / Security 在 M1 起就是 review surface；M2 之後 Import、Audit、Dashboard、Settings 也要能透過 callout / quick action 直接跳回這些修復頁，而不是把排障資訊藏在單一路由裡。
- Sidebar 以視窗高度而不是頁面內容高度佈局；footer 的 archive 狀態、background-work strip 與 theme toggle 在不捲動主內容區的情況下也要可見。
- Settings 擁有 day-one 語言切換與平台 troubleshooting；Schedule 擁有 platform-specific Preview / Manual / Execute / Verify story；Import 擁有 recent batch review、revert / restore 與 doctor repair 入口。
- 共享 profile scope 存在於 shell chrome，而不是散落在各頁各自記憶；Explorer 若未指定 page-specific `profileId`，必須明講自己目前沿用 shared scope。

### M3 Intelligence deep-link 規則

- Explorer 以同一個 `/explorer` route 承接 `keyword`、`semantic`、`hybrid` 三種 recall mode；`mode` 走 query string，避免 intelligence 結果和 canonical evidence 被拆成兩套路由。
- Explorer 的 deterministic 分組視角也固定留在同一個 `/explorer` route；`view=time|session|trail` 走 query string，而不是額外拆子路由。`session` / `trail` 視角必須帶著 `start` / `end` window，避免 grouped view 偷偷退回不誠實的全庫視角。
- semantic result、assistant citation、insight evidence 都要能 deep-link 回 `/explorer`，至少可帶 `q`、`profileId`、`domain` 等 canonical filters 讓使用者回看原始記錄。
- Assistant 的 seeded follow-up 使用 `/assistant?question=...`；若目前 intelligence / explorer surface 已經處於特定 `profileId`，deep-link 也必須一併帶上該 `profileId`，讓頁面級 scope 優先於 shared scope。
- Day Insights 現在是正式 route：`/intelligence/day/:date`。path 只使用本地日曆日 `YYYY-MM-DD`；query 只保留 `profileId`。從 day page 回 Explorer evidence 時，必須固定帶 `start=end=<date>` 的 exact-day window。
- Domain Deep Dive 現在是正式 route：`/intelligence/domain/:domain`，user-facing IA 視為 `Domain Insights`。它必須沿用 `/intelligence` 的 `range`、`start`、`end`、`profileId` query contract，讓使用者重新整理、複製 URL、或從 Top Sites / Stable Sources / Search Effectiveness / Explorer domain chip drill down 時都能回到同一個 scoped view；route 頂部的 archive-wide / scoped honesty 也要收斂成 compact inline strip，而不是 full-height callout。
- `query family`、`refind page`、`session`、`trail`、`compare set` 現在也有正式 shared insights route：`/intelligence/query-family/:familyId`、`/intelligence/refind/:canonicalUrl`、`/intelligence/session/:sessionId`、`/intelligence/trail/:trailId`、`/intelligence/compare-set/:compareSetId`。除了 `day` 之外，這些 route 一律沿用 `range`、`start`、`end`、`profileId` query grammar。
- non-overview shared insights routes 現在 additive 支援受限的 `focusType` / `focusId` query grammar，用來承接 aggregate context highlight；M8 只正式開放 `compare-set` 與 `path-flow`。`/intelligence` overview 本身不承接 focus state，回 overview 時必須清掉 focus。
- `/intelligence` 的 `Search Activity` 現在固定有 `engines / concepts / search keywords / families` 四個 tab。`Top Concepts` 是 aggregate summary，必須用 horizontal bar chart 而不是詞雲；`Search Keywords` 則是 bounded browser surface，支援 text / engine / nested date subrange / sort / pagination / page size。row 若已有 `familyId`，primary CTA 必須走 shared `query-family insights` route；其餘 drilldown 只允許 `trail insights` 與 Explorer evidence，不能藉機重開 Explorer `queries` view 或另一套 URL grammar。
- `Domain Insights` 對 search-engine domains 也要重用同一套 `Search Keywords` browser，但 request 需額外帶 `domain` filter；若當前 domain 沒有 keyword-eligible search rows，就誠實隱藏這個 section。
- `refind` route 的 path identity 直接使用 encoded canonical URL；shared focus contract 只能走 additive `focusType` / `focusId`，不得讓 consumer-local state 再次分裂 route grammar。
- Explorer 的 detail rail 與 grouped views 如果已經握有 visit 的本地日曆日 / registrable domain，就必須優先提供 `Open day insights` / `Open domain insights`；原始 evidence / visit record 仍可保留，但不再是唯一入口。
- Explorer 的 `session` / `trail` grouped view 仍是 browse-first canonical surface，但現在必須額外提供明確的 `Open session insights` / `Open trail insights` CTA；expand header 本身不應直接變成導頁。
- promoted route files、front-end Core Intelligence API，以及 Tauri command / worker bridge intelligence facade 現在都已按 ownership split；後續若要繼續拆 mixed helper / dev mirror / worker pass-through，必須走 M11 inventory，而不是回頭改 public contract。
- `reopened investigation`、`habit`、`stable source`、`friction`、`multi-browser diff` 這類 active entity 不再允許各自決定 destination：domain-based surface 一律走 `domain insights`；`compare set` 改為自己的 first-class insights route；path-flow 只有在 step 可穩定解析為 registrable domain 時才提供 shared CTA，且要帶上 `focusType=path-flow` / `focusId=<flowId>`。
- Dashboard 的 intelligence quick actions 必須直接通往 Explorer、Assistant、Intelligence；錯誤或 disabled 狀態下還要能跳到 Settings / queue controls，而不是只剩靜態說明。
- shell footer 與 Jobs 頁要形成同一套 queue grammar：footer 負責小型摘要與入口，Jobs 頁負責完整 progress / log / recovery；不能讓兩處各自發明不同的狀態名稱。
- 對長時間 deterministic rebuild，footer 與 Jobs 頁都必須優先顯示 phase / heartbeat / coarse percent，而不是永遠只給一條無信息的 indeterminate bar；使用者需要知道工作仍在前進，還是停在某個 phase 沒有 heartbeat。
- Intelligence 的 top-of-page runtime digest 與 Jobs / footer 必須使用同一套 queue grammar，但只保留摘要與 deep-link；不可在 Intelligence 重新長出一個第二套 full queue review wall。
- Intelligence section cards、day insights 與 domain deep dive 的 evidence / freshness badge / floating panel 必須沿用同一套 scope/window/module/source-table grammar；如果要做 rebuild / clear / retry，仍然導回 Settings / Jobs，而不是在分析頁面就地長出 mutation controls。
- route-level metric strip、`query-family-card`、compare-set page rows、以及 trusted output structured target label 現在也屬 shared composition contract；這些 UI 不應再由 overview / promoted route / Settings 各自手寫。
- Explorer 的 `semantic` / `hybrid` surface，以及 Assistant、Intelligence 的 AI status panel，都必須顯示 provider / model、queue counts、index state，並提供 test provider、refresh queue、rebuild / clear index、open settings 這類 controls；keyword-first Explorer 不應被 optional AI 面板壓過主工作流。
- Explorer 的 time view 必須同時在上方 timeline / summary 與底部分頁列明確顯示「當前頁 / 總頁數」；底部分頁列還要承接跳頁與每頁筆數控制，避免使用者只看到 loaded count 卻不知道自己在整個結果集的哪裡。
- Explorer 的 time-view detail rail 必須 sticky 在可視區內，而不是跟著左側長列表一起被拉成整列高度；使用者在頁面底部選到某筆記錄時，不應再為了看 detail 被迫捲回頁首。
- Explorer 的翻頁與每頁筆數切換只能刷新結果，不得把 `workspace-scroll` 強制拉回頁面頂端；用戶在列表底部操作分頁後，視角要留在原位。
- Settings 是 M4-A 起的 remote backup、manual external-output review、與 derived-state 控制塔：從這裡可以完成 remote upload 的 PME、credential review、bundle verification、`embed/widget/public snapshot` 的手動 preview / copy-export、`browser-snippet-v1` 本地宿主的 preview / execute / verify、plugin enable / disable、derived rebuild / clear、以及 search-engine rule editor（built-in read-only + custom CRUD），並回鏈到 Audit run 驗證最新 growth signal。
- Settings external outputs 現在優先根據 trusted payload 內的 structured entity targets 產生 `Open insights` links；`public snapshot` 則必須維持 redacted，不得帶出 internal reusable IDs。

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

### Profile-Scoped Intelligence 導航規則

- Intelligence 頁面支援透過 shell chrome 的共享 profile scope 篩選 deterministic analysis 資料。
- 當使用者在 topbar 選擇特定 profile 時，Intelligence 的 cards、topic timeline、threads 等 surface 都切換為該 profile 的 scoped view。
- 若 `profileId` 已經出現在 `/intelligence`、`/intelligence/day/:date` 或 `/intelligence/domain/:domain` 的 query string，頁面級 scope 優先於 shared profile scope；route 重新整理後仍必須保持這個 explicit scope。
- Dashboard 的 aggregate KPIs 仍維持 archive-wide；Intelligence 頁面在 scoped 模式下必須以 callout 或 badge 明確標示「目前為 profile-scoped view」。
- scoped vs all-profile 切換不得產生新的 route；以 query string `profileId` 或沿用 shared scope 處理，保持與 Explorer 的 scope 語法一致。
