# 畫面與導航結構

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出。  
> **目前的具體視覺 source of truth 是 `reference/PathKeep — Desktop UI Design/` 的 designer prototype。**
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
