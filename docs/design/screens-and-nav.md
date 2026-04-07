# 畫面與導航結構

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出。  
> **designer prototype 仍然是目標視覺語言，但目前 repo 內沒有同步帶上 `reference/PathKeep — Desktop UI Design/` 匯出檔。**
> 在 prototype 重新補回 repo 之前，這份文檔與 [design-tokens.md](design-tokens.md) 視為現行 source of truth。
> production token source of truth 是 [design-tokens.md](design-tokens.md)；新增 token 時要同步更新文檔與 `src/styles/tokens.css`。
> 如果 prototype 缺少某個畫面或狀態，才用 Stitch / 補充設計決策補齊；補齊時仍需維持和 prototype 一致的視覺語言與導航結構。

---

## 畫面清單

| 畫面                   | 核心職責                                                                              |
| ---------------------- | ------------------------------------------------------------------------------------- |
| **Onboarding / Setup** | 首次啟動引導：發現瀏覽器、選擇 profile、設定存儲、加密選擇                            |
| **Dashboard**          | 備份狀態總覽、最近 run 摘要、歷史上的今天、定期總結卡片、Job Queue 狀態、快速操作入口 |
| **History Explorer**   | 時間軸 + 全文搜尋 + 篩選 + 詳情 + 匯出                                                |
| **Insights**           | 洞察卡片、topic timeline、threads、query ladders、profile facets                      |
| **AI Assistant**       | 自然語言問答介面                                                                      |
| **Import**             | Takeout 導入 wizard + 瀏覽器直接導入（含 step-by-step UI）                            |
| **Audit Ledger**       | Manifest chain、run 歷史、diff 視圖、schema 變化紀錄                                  |
| **Security**           | 加密設定、keyring、rekey、密碼警告                                                    |
| **Schedule Setup**     | 排程預覽 → 手動安裝/自動安裝 → 狀態監控                                               |
| **Settings**           | 通用設定、語言、AI provider 管理、MCP 開關、數據目錄、版本信息                        |

---

## 導航

- 左側 sidebar 導航（可收合）。
- 頂部顯示當前頁面的 breadcrumb / title。
- 全局快速搜尋入口。

### M1 導航與 deep-link 規則

- Sidebar 依固定分區導航：`CORE`（Dashboard / Explorer / Insights / Assistant）、`OPERATIONS`（Import / Audit / Schedule）、`SYSTEM`（Security / Settings）；Onboarding 是 utility route，不常駐 sidebar。
- 頂部搜尋送到 `History Explorer`，直接寫入 `/explorer?q=...`，讓搜尋結果可以被複製、重整和重新打開。
- Explorer 的 day-one filter deep-link 使用 query string：`q`、`profileId`、`browserKind`、`domain`、`start`、`end`、`sort`。
- Audit Ledger 的 run detail deep-link 使用 `/audit?run=<id>`；Dashboard recent runs 直接跳進這個 URL。
- Dashboard zero-state、Security、Topbar 都可以回到 Onboarding，確保 first-backup flow 永遠有明確入口。
- Onboarding shell header 必須有明確的 `Exit setup` 動作；離開後保留目前已選的 storage / profile / security 決策，避免把使用者困在 setup route。
- Schedule / Security 在 M1 起就是 review surface；M2 之後 Import、Audit、Dashboard、Settings 也要能透過 callout / quick action 直接跳回這些修復頁，而不是把排障資訊藏在單一路由裡。
- Sidebar 以視窗高度而不是頁面內容高度佈局；footer 的 archive 狀態與 theme toggle 在不捲動主內容區的情況下也要可見。
- Settings 擁有 day-one 語言切換與平台 troubleshooting；Schedule 擁有 platform-specific Preview / Manual / Execute / Verify story；Import 擁有 recent batch review、revert / restore 與 doctor repair 入口。
