# PathKeep — Vision & Requirements

> **Status:** Living document · **Author:** Human + AI pair · **Created:** 2026-04-05

---

## 1. What Is This Product?

PathKeep 是一個**本地優先、開源、可信賴的瀏覽器歷史紀錄長期保存與智能分析工具**。

瀏覽器只保留很短時間窗口內的歷史紀錄 — Chrome 本地 90 天、Google 帳號同步最多 18 個月。一旦過期，這些紀錄就永久消失了。但對很多人來說，瀏覽紀錄是一種極其私密且有價值的**個人注意力日誌** — 它記錄了你什麼時候在研究什麼、怎麼做決策、怎麼學習、怎麼工作。

這個產品要做三件事：

1. **Archive（歸檔）**— 安全、可靠、可審計地把瀏覽紀錄保存下來，設計壽命 20 年以上。
2. **Recall（召回）**— 讓你在未來任何時候都能找回過去的紀錄，不只是精確搜尋，還有語義搜尋，agentic search 和深度研究。
3. **Intelligence（洞察）**— 基於長期累積的歷史紀錄，幫你理解自己的興趣演化、研究軌跡、工作模式。

核心價值主張是：**你的瀏覽紀錄不應該因為瀏覽器的存儲策略而消失，它是你的數據，應該由你永久保管，並且能從中獲得洞察。**

### 產品定位

這不是一個「帶 AI 的瀏覽器歷史工具」。
更精確地說，這是一個：

> **本地、開源、可搜尋、可回顧、可理解的個人瀏覽記憶系統。**

- **本地**：所有數據永遠只存在你的機器上。
- **記憶**：不只是存檔，還要理解你看過什麼、在做什麼。
- **可回顧**：20 年以後打開，數據依然完整且可讀。
- **可理解**：從原始紀錄中提煉出意義 — 主題、任務、趨勢、模式。
- **可信賴**：每一次操作都有審計紀錄，用戶能驗證數據的完整性。

### 功能架構

產品分三大功能域，按重要性排序：

```
┌─────────────────────────────────────────────────────────┐
│                    INTELLIGENCE                         │
│     語義搜尋 · 問答 · 趨勢分析 · 個人洞察卡片          │
├─────────────────────────────────────────────────────────┤
│                    RECALL                               │
│     全文搜尋 · 時間軸瀏覽 · 篩選 · 匯出                │
├─────────────────────────────────────────────────────────┤
│                    ARCHIVE                              │
│     增量備份 · 排程 · 安全 · 審計 · 導入                │
└─────────────────────────────────────────────────────────┘
```

---

## 2. 用戶是誰？

### 主要用戶

- **知識工作者**：開發者、研究者、分析師、記者 — 每天花大量時間在瀏覽器裡做研究、比較、學習。
- **數據意識強的個人用戶**：重視個人數據所有權，想要長期保留自己的所有數位足跡。
- **安全與隱私偏好者**：不信任雲端服務保管自己的瀏覽紀錄。

### 用戶特徵

- 能理解基本的系統概念（目錄結構、定時任務、數據庫）。
- 願意為數據安全做一些設定。
- 不一定願意安裝 GPU 或複雜的 AI 工具鏈 — 所以 AI 功能必須是可選的、按需的。

---

## 3. 核心原則

### 3.1 Trust & Transparency（可信與透明）

**用戶必須能理解和驗證這個工具的一切行為。**

- 所有涉及系統層面的操作（安裝定時任務、訪問瀏覽器數據、修改文件系統）都走 **Preview → Manual → Execute** 三段式：
  - **Preview**：展示將要做什麼、為什麼做、會產生什麼文件/命令、如何回滾。
  - **Manual**：用戶可以複製命令、下載設定檔，自己手動操作。提供每一步的操作指南和背後原因。當然，也可以選擇跳過 Manual 步驟，直接讓 app 執行操作。
  - **Execute**：只有用戶明確確認後，app 才代為執行，並將結果寫入審計日誌。
- 每次備份生成不可變的 manifest，串接成 hash chain，形成可審計的 ledger。
- 用戶可以隨時查看所有 manifest、diff、schema 變化記錄。

### 3.2 Data Sovereignty（數據主權）

- 所有數據永遠只存在本地。
- 用戶完全擁有自己的數據，知道數據存在哪，能直接訪問底層文件。
- 遠端備份（如 S3）是用戶主動配置的，app 本身不會偷偷上傳任何東西。
- 開源。用戶可以審計所有代碼。

### 3.3 Longevity（長期可用性）

- 數據存儲設計壽命 20 年以上。
- 使用 SQLite — 地球上存活最久的文件格式之一。
- 原始數據以超集格式保存 — 意即我們的 archive schema 包含瀏覽器原始欄位的所有信息且可能更多，即使未來瀏覽器 schema 變了，舊數據依然完整可讀。
- Archive schema 有自己的版本管理和 migration 機制。
- 原生快照在關鍵時刻（首次備份、schema 變更、季度 checkpoint）壓縮保存。

### 3.4 Intelligence Is Optional（智能功能可選）

- 所有 AI / 分析功能預設關閉。
- 核心備份功能在沒有任何 AI 配置的情況下必須完全正常工作。
- AI 功能是建立在「歸檔已經夠好」的基礎上的增值層。

### 3.5 Recoverability（可恢復性）

**用戶的誤操作不應該造成不可逆的傷害。**

- 用戶導入了垃圾數據？可以回滾。
- 用戶不小心跑了一次錯誤的備份？可以回滾。
- 用戶改了設定發現改壞了？可以恢復。
- 唯一不可恢復的是加密密碼丟失 — 但這是設計上的刻意決定，且有充分警告。
- Archive 的設計必須讓用戶有信心去「試」— 不需要在每次操作前擔心「做了這個會不會搞壞我的數據」。
- 所有寫入操作都是可識別的（run ID）、可追溯的（audit log）、可回滾的（revert）。

---

## 4. 文檔目錄

本文檔是 PathKeep 的 vision hub。詳細的需求和設計拆分在以下子頁面中：

### 架構與數據

- [技術棧與平台](architecture/tech-stack.md) — 技術選型、數據庫分層、AI 框架（rig.rs + LanceDB）
- [數據模型與長期設計](architecture/data-model.md) — 統一時間格式、Schema 演化、長期容量原則
- [數據庫選型決策](database-selection-decision-2026-04-05.md) — SQLite-first layered architecture 的完整論證

### 功能需求

- [Archive — 歸檔](features/archive.md) — 增量備份、排程、導入匯出、安全加密、審計、Enrichment
- [Recall — 召回](features/recall.md) — 歷史紀錄瀏覽器、時間軸、搜尋篩選、版本管理與回滾
- [Intelligence — 洞察](features/intelligence.md) — 語義搜尋、AI 助手、洞察系統、Job Queue、AI Provider

### 設計

- [UX 設計原則](design/ux-principles.md) — 視覺方向、操作透明性、狀態清晰
- [畫面與導航結構](design/screens-and-nav.md) — 畫面清單、導航結構

### 計劃與標準

- [里程碑](milestones.md) — M1 Solid Archive → M2 Recall & Trust → M3 Intelligence → M4 Full Intelligence
- [品質標準與社區](standards.md) — 測試覆蓋、代碼品質、CI/CD、i18n、開源、Non-Goals
- [**工作計劃與進度追蹤**](plan/README.md) — 先看這裡掌握實作順序、當前進度、阻塞與 WBS
  - [Program 基線與決策待辦](plan/program/README.md) — 先回答「repo 現在在哪裡」「還有哪些決策沒定」
  - [M0 — 重構基礎](plan/m0-foundation/README.md) — 舊 UI 拆除、新骨架建立、核心邊界重組
  - [M1 — Solid Archive](plan/m1-solid-archive/README.md) — migration、backup、audit、schedule、security、Explorer v1
  - [M2 — Recall & Trust](plan/m2-recall-and-trust/README.md) — import、rollback、Doctor、多瀏覽器、i18n、PME
  - [M3 — Intelligence](plan/m3-intelligence/README.md) — provider、index、assistant、insights、MCP
  - [M4 — Full Intelligence & Polish](plan/m4-full-polish/README.md) — enrichment、advanced insights、remote backup、release polish
  - [M5 — Deterministic Intelligence](plan/m5-deterministic-intelligence/README.md) — evidence-first analytics、taxonomy v2、query groups、reference pages、long-horizon no-AI baseline（proposal）

> 想知道某個功能「應該做成什麼樣」看 `features/` / `design/` / `architecture/`；想知道「接下來怎麼做、做到哪裡、卡在哪裡」就看 `docs/plan/`。

### 參考

- [瀏覽器支持參考](reference-review.md) — 與 1History/browserexport 的對比
