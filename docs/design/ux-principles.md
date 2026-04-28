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
- **Execute paint-first**：凡是會觸發大量 SQLite / Rust / derived-state 工作的流程，前端必須先讓 busy overlay、skeleton 或 route shell 成功 repaint，再開始真正的重工作業；不能讓動畫或 route transition 因 foreground work 而卡死。
- **Verify**：完成後要在原頁面留下可見的結果訊號，例如 recent run、latest export path、artifact list、warning / no-warning 狀態。
- **Rollback hint**：凡是會寫入 archive 的流程，都要讓使用者知道之後去哪裡檢查或回滾，而不是只回報「成功」。
- 高風險流程至少要能完成 keyboard-only walkthrough，current step / selected filter / status chip 要有可朗讀的 label，而不是只靠顏色或位置辨識。
- 高風險流程在 reduced motion 模式下要降低動畫和 loading shimmer，避免把「透明」做成另一種視覺負擔。
- 長時間流程若超過單次短暫 spinner 的合理範圍，必須持續回報 phase、current/total、percent 與近期 log lines；「正在處理」不是可接受的最終 honesty copy。

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

### 國際化是 UX 契約

- 所有新的 user-visible copy 都先假設要同時支援 `en` / `zh-CN` / `zh-TW`，而不是先寫英文再回頭補翻譯。
- i18n 範圍不只正文，也包含 route title / subtitle、sidebar label、button 文案、error / warning / success callout、placeholder、aria-label、loading / skeleton label、busy overlay detail，以及 browser preview honesty copy。
- sample / preview / disabled / empty state 不是例外；如果畫面會顯示給使用者看，它就屬於產品文案。
- 設計和實作都必須預留 locale-length wrapping；至少要能承受 pseudo-locale、中文長句與較長的錯誤說明，不可因翻譯後溢出就回退成英文硬編碼。
- `html[lang]` 必須跟著目前 locale 同步，讓瀏覽器能選到正確的 glyph variant 與 fallback font；不能只改 React 文案而讓 DOM 還停在預設語言。
- UI typography 以可讀的 sans 為預設，monospace 只用於 path、ID、command、純 evidence values；不要把 terminal flavor 直接套進整個 shell chrome。

---

## 4. Loading States & Skeleton Screens

所有頁面必須提供有意義的載入狀態 — 永遠不允許出現純系統轉圈或空白畫面。

### 各頁面 Loading 規範

| 頁面             | Loading 表現                                                                                                                                                                                                                                                        |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Dashboard**    | Skeleton stat cards 匹配最終卡片佈局，加上 skeleton recent runs table 與 On This Day placeholder                                                                                                                                                                    |
| **Explorer**     | Skeleton timeline rail + skeleton list items（約 5–8 行），保持列表區域寬度與排版一致                                                                                                                                                                               |
| **Intelligence** | 先顯示 runtime digest + primary overview skeleton；digest / top summary / first-visible cards 先到，secondary grid 與較低優先 detail 在 first paint / idle 後分段載入。secondary cards 若已有 warm cache 要逐張顯示，不等整批 secondary overview 全部完成才一起跳出 |
| **Insights**     | Skeleton KPI cards + insight panel placeholders，配合 badge 顯示 "Loading insights..."                                                                                                                                                                              |
| **Import**       | 導入 scan / import / onboarding finalize / manual backup 過程中以 progress overlay 顯示：至少包含 phase、已處理筆數、百分比、近期 log line；未知總量時保留 indeterminate progress，且 follow-up rebuild / refresh 不得阻塞 overlay repaint                          |
| **AI 操作**      | AI assistant / insights refresh 使用 pulsing status indicator + 描述文字（例如 "正在檢索相關記錄..."、"正在生成洞察..."）                                                                                                                                           |

### 視覺規範

- Skeleton block 使用 `var(--border)` 色彩，搭配 1.5s ease-in-out 的 pulse 動畫。
- Skeleton 區塊必須匹配最終內容的佈局尺寸，避免載入完成後 layout shift。
- `prefers-reduced-motion`：skeleton pulse 應變為靜態或極微微的透明度變化，不得使用持續循環動畫。
- Progress overlay 必須包含進度數字（百分比或筆數）和可讀的狀態說明，不能只有轉圈。
- 對於重型 route（尤其 `/intelligence`），初始 render 不得一次 fan-out 十多個 foreground requests；應先有 route-level skeleton，再以優先順序把 section 資料分批填回。
- `/intelligence` 的 same-scope revisit（例如從 day/domain/entity route 返回 overview）必須優先用 warm cache 立即還原已看過的卡片，再在背景 revalidate；不能每次返回都把整頁打回 cold skeleton。
- tabbed secondary content 若屬於目前可見的重型分析頁面，也必須在首屏穩定後自動 prewarm；不得把第一次 request 綁死在「使用者必須先點 tab」這個互動上。
- AI 操作的 pulsing indicator 必須同時顯示目前階段說明文字，讓用戶知道系統在做什麼。
- 同一份 runtime / queue truth 只能由 shell 層共享 polling source 提供；sidebar、dashboard 與 intelligence digest 不得各自重複輪詢同一組背景狀態。
