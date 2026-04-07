# UX 設計原則

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出。

---

## 1. 視覺設計方向

- 追求現代、精緻、有科技感的視覺風格。
- Dashboard 和洞察頁面可以做得炫酷和花哨 — 用漂亮的數據視覺化、流暢的動畫、動態圖表讓用戶覺得好看好用。
- 暗色模式優先，但也要支持淺色模式。
- 信息密度適中，不要太空也不要太擠。
- 桌面優先，但保留窄視窗可用性。
- 需要操作透明性的地方（audit ledger、排程設定、導入流程）保持清晰嚴謹，但不需要整個 app 都像帳本。

---

## 2. 操作透明性

- 每一個涉及系統/數據的操作，都要讓用戶看到：
  - 我們在做什麼
  - 為什麼做這件事
  - 具體執行了什麼命令/步驟
  - 已經做了哪些、還剩哪些
  - 如何撤銷
- 自動模式和手動模式都是 step-by-step 的 UI。
- 對於自動模式：逐步顯示進度，每步可展開查看詳情。
- 對於手動模式：每步有指南、理由、可複製的命令、完成後的確認。

### PME 共用 grammar

- **Preview**：先顯示邊界、影響範圍、generated artifact / visible query / profile scope，再出現真正的 execute CTA。
- **Manual**：所有需要碰檔案系統、排程或匯出物的流程，都要有可檢視的 artifact viewer 與 open / copy path 動作，不要求使用者自行去資料夾猜位置。
- **Execute**：執行按鈕文案必須直接說明會做什麼，例如 first backup、run backup、copy path、open path；不要把高風險操作藏在模糊 CTA 裡。
- **Verify**：完成後要在原頁面留下可見的結果訊號，例如 recent run、latest export path、artifact list、warning / no-warning 狀態。
- **Rollback hint**：凡是會寫入 archive 的流程，都要讓使用者知道之後去哪裡檢查或回滾，而不是只回報「成功」。

### Trust warning grammar

- **Info**：能力存在但仍需閱讀說明，例如 platform summary、可選便利功能、手動安裝入口。
- **Needs attention**：mismatch、legacy install、部分權限缺失等需要人工確認的狀態。
- **Blocked / degraded**：Full Disk Access 未授予、keyring unavailable、native schedule 無法安全 apply 之類會改變能力邊界的情境。
- **Success**：已驗證、可追蹤、可回滾的狀態，而不是單純「看起來沒錯」。
- warning / callout 不能只是說明文字；要直接附上下一步或修復入口，至少能跳到 Import、Schedule、Security 或 Audit 的對應頁面。

---

## 3. 狀態清晰

- 所有可交互元素的狀態必須清晰可辨：
  - 勾選/未勾選有明顯視覺差異和動畫反饋。
  - 操作進行中有明確的 loading / progress 狀態。
  - 錯誤有明確的錯誤信息和修復建議。
- 空狀態友好：沒有數據時告訴用戶該怎麼開始。
- 降級狀態友好：AI 未配置時明確提示，但不阻礙核心備份功能。
