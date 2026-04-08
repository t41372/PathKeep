# 畫面與導航結構

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出。  
> designer prototype 匯出檔目前已在 repo：`reference/PathKeep — Desktop UI Design/`。  
> 這份 export 主要覆蓋 shell chrome 與 Dashboard 的視覺語言；對於 prototype 尚未畫出的畫面或狀態，這份文檔與 [design-tokens.md](design-tokens.md) 仍是現行 source of truth。
> production token source of truth 是 [design-tokens.md](design-tokens.md)；新增 token 時要同步更新文檔與 `src/styles/tokens.css`。
> 如果 prototype 缺少某個畫面或狀態，才用 Stitch / 補充設計決策補齊；補齊時仍需維持和 prototype 一致的視覺語言與導航結構。

---

## 畫面清單

| 畫面                   | 核心職責                                                                                                  |
| ---------------------- | --------------------------------------------------------------------------------------------------------- |
| **Onboarding / Setup** | 首次啟動引導：發現瀏覽器、選擇 profile、設定存儲、加密選擇                                                |
| **Dashboard**          | 備份狀態總覽、最近 run 摘要、歷史上的今天、定期總結卡片、Job Queue 狀態、快速操作入口                     |
| **History Explorer**   | 時間軸 + 全文搜尋 + 篩選 + 詳情 + 匯出                                                                    |
| **Insights**           | 洞察卡片、topic timeline、threads、query ladders、profile facets、storage analytics                       |
| **AI Assistant**       | 自然語言問答介面                                                                                          |
| **Import**             | Takeout 導入 wizard + 瀏覽器直接導入（含 step-by-step UI）                                                |
| **Audit Ledger**       | Manifest chain、run 歷史、diff 視圖、schema 變化紀錄                                                      |
| **Security**           | 加密設定、keyring、rekey、密碼警告                                                                        |
| **Schedule Setup**     | 排程預覽 → 手動安裝/自動安裝 → 狀態監控                                                                   |
| **Settings**           | 通用設定、語言、AI provider 管理、remote backup PME、derived-state controls、MCP 開關、數據目錄、版本信息 |

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
- Settings 的 remote backup 現在以 `Preview / Manual / Execute / Verify` tabs 呈現：Preview 顯示 bundle path / object key / upload URL，Manual 保留 curl command 與 retention guidance，Execute 顯示 upload result，Verify 則列出 checksum / required-entry / restore-readiness checks。
- Settings 的 enrichment / derived-state panel 是正式 review surface，而不是 debug affordance。它必須顯示 plugin version、queue、freshness、derived tables、storage impact，以及 rebuild / clear controls。
- Insights 現在除了既有 card / topic / thread surface 外，還要顯示 storage analytics 與 latest growth signal，並提供回到 Audit run 的 deep-link。
- shared profile scope 是 production shell 的正式 viewer state：Topbar 可切換全域 viewing scope；Explorer 預設繼承、Assistant / Insights 直接沿用，Dashboard 則必須用 callout 清楚說明哪些區塊是 scoped、哪些 KPI 仍是 archive-wide。

---

## 導航

- 左側 sidebar 導航（可收合）。
- 頂部顯示當前頁面的 breadcrumb / title。
- 全局快速搜尋入口。

### M1 導航與 deep-link 規則

- Sidebar 依固定分區導航：`CORE`（Dashboard / Explorer / Insights / Assistant）、`OPERATIONS`（Import / Audit / Schedule）、`SYSTEM`（Security / Settings）；Onboarding 是 utility route，不常駐 sidebar。
- 頂部搜尋送到 `History Explorer`，直接寫入 `/explorer?q=...`，讓搜尋結果可以被複製、重整和重新打開。
- Explorer 的 day-one filter deep-link 使用 query string：`q`、`profileId`、`browserKind`、`domain`、`start`、`end`、`sort`、`regex`。
- Audit Ledger 的 run detail deep-link 使用 `/audit?run=<id>`；Dashboard recent runs 直接跳進這個 URL。
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
