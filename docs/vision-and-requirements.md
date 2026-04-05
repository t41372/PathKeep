# Browser History Vault — Vision & Requirements

> **Status:** Living document · **Author:** Human + AI pair · **Created:** 2026-04-05

---

## 1. What Is This Product?

Browser History Vault 是一個**本地優先、開源、可信賴的瀏覽器歷史紀錄長期保存與智能分析工具**。

瀏覽器只保留很短時間窗口內的歷史紀錄 — Chrome 本地 90 天、Google 帳號同步最多 18 個月。一旦過期，這些紀錄就永久消失了。但對很多人來說，瀏覽紀錄是一種極其私密且有價值的**個人注意力日誌** — 它記錄了你什麼時候在研究什麼、怎麼做決策、怎麼學習、怎麼工作。

這個產品要做三件事：

1. **Archive（歸檔）**— 安全、可靠、可審計地把瀏覽紀錄保存下來，設計壽命 20 年以上。
2. **Recall（召回）**— 讓你在未來任何時候都能找回過去的紀錄，不只是精確搜尋，還有語義搜尋，agentic search 和深度研究。
3. **Intelligence（洞察）**— 基於長期累積的歷史紀錄，幫你理解自己的興趣演化、研究軌跡、工作模式。

核心價值主張是：**你的瀏覽紀錄不應該因為瀏覽器的存儲策略而消失，它是你的數據，應該由你永久保管，並且能從中獲得洞察。**

### 1.1 產品定位

這不是一個「帶 AI 的瀏覽器歷史工具」。
更精確地說，這是一個：

> **本地、開源、可搜尋、可回顧、可理解的個人瀏覽記憶系統。**

- **本地**：所有數據永遠只存在你的機器上。
- **記憶**：不只是存檔，還要理解你看過什麼、在做什麼。
- **可回顧**：20 年以後打開，數據依然完整且可讀。
- **可理解**：從原始紀錄中提煉出意義 — 主題、任務、趨勢、模式。
- **可信賴**：每一次操作都有審計紀錄，用戶能驗證數據的完整性。

### 1.2 技術棧

| 層面 | 選型 | 理由 |
|------|------|------|
| 桌面框架 | Tauri 2 | 跨平台、Rust 核心、輕量級 |
| 核心邏輯 | Rust workspace（vault-core, vault-worker, vault-platform） | 高性能、安全、跨平台 |
| 前端 | React 19 + TypeScript + Vite | 現代前端、型別安全 |
| 工具鏈 | Bun | JS 側的包管理與腳本 |
| 數據存儲 | SQLite（可選 SQLCipher 加密） | 本地優先、20 年持久性 |
| 審計 | Git（只管理 manifests 和審計工件） | 可追溯性 |
| AI | Embedding + LLM via 本地推理（Ollama / LM Studio）或雲端 API | 可選、可配置 |

### 1.3 目標平台

- macOS（主要開發和測試平台）
- Windows 和 Linux（第一天做好 platform adapter 設計，後續補齊完整實機驗證）

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

## 4. 功能架構

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

下面按功能域和 user story 展開。

---

## 5. ARCHIVE — 歸檔（核心）

### 5.1 增量備份

**作為**用戶，**我想要**定期自動備份我所有瀏覽器的歷史紀錄，**以便**我的歷史紀錄不會因為瀏覽器的過期策略而丟失。

#### 需求要點

- 支援自動發現本機安裝的瀏覽器和 profiles。
  - Chromium 系列（Chrome, Edge, Brave, Arc, Vivaldi 等）為主要支持對象。
  - Firefox 和 Safari 為次要支持對象。
- **Profile 選擇是備份的入口**：
  - 用戶在 UI 中看到所有被發現的瀏覽器和 profiles，以勾選的方式選擇要備份哪些。
  - 交互清晰：選中/未選中狀態有明顯視覺差異和動畫反饋。
  - **未被選中的 profile 不會被備份**，無論是手動備份還是排程自動備份。
  - 排程自動備份只會處理用戶明確勾選過的 profiles。
  - 用戶可以隨時調整選擇，新增或移除要備份的 profiles。
- 備份流程：
  1. 複製瀏覽器的 History / Favicons 數據庫及其 sidecar 檔（`-journal`, `-wal`, `-shm`）到 staging 區。
  2. 對 staging 副本做完整性檢查（`PRAGMA quick_check`）。
  3. 解析並寫入 archive 數據庫。
- 備份是**增量的**：只新增/更新有變化的記錄。
- Archive 是 **append-only** 的：即使瀏覽器端的紀錄已過期或被手動刪除，archive 中的歷史紀錄永不刪除。
- 對於會變動的記錄（如 URL metadata），採用 row versioning，保留所有歷史版本。
- 原始來源數據以 JSON payload 形式保留在 `raw_row_versions`，包含 table 名稱、source primary key、schema 指紋、瀏覽器版本、run ID。
- 備份不讀取正在運行的瀏覽器的 live 數據庫 — 一律先複製到 staging。

#### 重複處理

- 增量備份的核心挑戰是去重。
- 以 `(source_kind, profile_id, table_name, source_pk, payload_hash)` 為去重鍵。
- 多次導入相同數據不會產生重複記錄。
- 需要考慮的邊緣案例：
  - 同一 URL 在不同 profiles 下的 visit 是獨立的。
  - 同一 URL 在不同時間點的 visit 是獨立事件。
  - Chrome 的 `urls` 表中同一 URL 的 metadata 可能變化（如 title 更新），這些變化應作為新 version 保存。

### 5.2 排程

**作為**用戶，**我想要**設定自動備份的排程，**以便**不需要手動記得去備份。

#### 需求要點

- 使用 OS 原生排程機制，不依賴 app 常駐運行：
  - macOS：LaunchAgent（`RunAtLoad=true` + 固定週期檢查）
  - Windows：Task Scheduler
  - Linux：systemd user timer（支援 persistent timer）
- 預設邏輯：短週期（如每 1 小時）喚醒檢查，但只有超過設定的備份間隔才真正執行備份。
- **預設備份間隔為每 12 小時**。用戶可以在設定中自定義間隔（從最短 1 小時到最長數天）。
- 「沒有一直開機也能補跑」：開機/登入後，如果距離上次備份已超時，自動補跑。
- 排程的設定走 Preview/Manual/Execute 流程：
  - 用戶能看到將要安裝的 plist / XML / service 文件的內容。
  - 用戶能選擇手動複製並安裝，或讓 app 代為安裝。
  - 安裝結果記入審計日誌。
- App 的「開機啟動」（autostart）和備份排程是兩件分開的事。

### 5.3 數據導入

**作為**用戶，**我想要**從 Google Takeout 導入歷史數據，**以便**能把 Google 帳號端保留的更早的歷史紀錄也納入 archive。

#### Google Takeout 導入

- GUI wizard 流程：
  1. 拖入 Takeout 的 zip 或解壓後的資料夾。
  2. 先做 dry-run：掃描文件、識別格式、產生報告。
  3. 已知格式（如 `BrowserHistory.json`）進入 importer。
  4. 未知文件進入 quarantine（隱離區），不會被導入，在 UI 中顯示原因和文件內容摘要，讓用戶決定如何處理。
  5. 用戶確認後，才正式寫入 archive。
- 導入前的預覽：用戶能看到將導入多少筆記錄、時間範圍、會不會與現有記錄重複。
- 導入後可回滾：用戶可以查看每次導入的記錄，如果發現導入的數據有問題（髒數據），可以回滾整次導入。
- 提供詳細的操作指南：怎麼從 Google Takeout 請求導出、怎麼下載、怎麼找到歷史紀錄文件。

#### 瀏覽器直接導入

- 同樣走 Preview/Manual/Execute 流程。
- Step-by-step UI：
  - 每個步驟都說明我們在做什麼、為什麼要做這件事。
  - 自動化模式下：逐步展示進度。
  - 手動模式下：每步有操作指南，能複製命令，做完再進下一步。

### 5.4 數據匯出

**作為**用戶，**我想要**把 archive 數據匯出成通用格式，**以便**能在其他工具中使用或做長期備份。

#### 本地匯出

- 支援格式：HTML、Markdown、純文本、JSONL。
- 匯出支援篩選：profile、時間範圍、domain、搜尋 query。
- 匯出報告記入審計日誌。

#### 遠端備份

- 支援備份到 S3 相容的對象存儲。
- 用戶在設定中配置 endpoint、bucket、credentials。
- 遠端備份是明確的用戶操作，不會自動上傳。

### 5.5 安全與加密

**作為**用戶，**我想要**可以選擇加密我的 archive 數據庫，**以便**就算有人拿到我的硬碟也讀不了我的瀏覽紀錄。

#### 需求要點

- 加密方案：
  - 使用隨機數據庫金鑰，由用戶主密碼經 Argon2id 導出的金鑰包裝。
  - 包裝後的 secret 存入 Stronghold vault（Tauri 提供的加密安全存儲，類似於 macOS Keychain 但跨平台）。
  - 可選擇把便利解鎖信息存入系統 keyring（macOS Keychain, Windows Credential Manager, Linux Secret Service/KWallet）。
- 不加密模式也可以使用，但 UI 必須明確標示「數據庫為明文」。
- Linux 沒有可用 keyring 時：仍允許加密模式，但每次啟動都需要輸入主密碼。不做弱保護 fallback。
- 提供完整的 rekey 流程：更改密碼、明文→加密、加密→明文。
- **密碼遺忘等於數據丟失**：UI 中必須有明確、醒目的警告，要求用戶把密碼妥善保存，並且告知風險。

### 5.6 審計與可信性

**作為**用戶，**我想要**能驗證我的 archive 數據完整性，**以便**確信數據沒有被篡改或丟失。

#### 需求要點

- 每次備份 run 產生不可變的 manifest：
  - 上一個 manifest 的 hash
  - 來源文件的雜湊
  - 來源 schema 指紋
  - 各表的記錄數、insert/update 統計
  - 工具版本
  - 失敗原因（如有）
- Manifest 形成 hash chain（可審計的 append-only ledger）。
- Git 只管理審計文字工件：manifests, schema snapshots, 排程設定草稿, 導入/匯出報告, 完整性報告。
- 主 SQLite archive、raw 快照、cache 和 staging 不進 git。
- Doctor / 健康檢查命令：重算 archive table hash 並出報告。
- 如果用戶已配置 GPG 簽名 commit，就沿用。

### 5.7 Schema 演化與容錯

**作為**開發者 / 長期用戶，**我想要** archive 能夠適應瀏覽器未來的格式變化，**以便** 10 年後依然能正常備份新版瀏覽器的紀錄，且舊數據不受影響。

#### 需求要點

- 兩層處理：
  - **Raw capture 層**：永遠動態讀取所有欄位並落盤。即使 Chrome 加了新欄位或改了結構，raw capture 只要 SQLite 能打開就能繼續工作。
  - **Derived normalizer 層**：把 raw data 映射到我們的統一 schema。新增未知欄位時只降級該欄位，不丟全部數據。
- Archive schema 獨立版本管理，用編號 SQL migration。每次升級前自動備份 archive DB。
- 關鍵時刻保存完整原生快照（壓縮保存 History/Favicons DB 原檔）：
  - 首次備份
  - 來源 schema 變更時
  - 每季 checkpoint
- 記錄每次備份的瀏覽器版本、schema 指紋、profile metadata。

### 5.8 統一時間格式

不同瀏覽器使用完全不同的時間格式來儲存歷史紀錄：

| 瀏覽器 | 原生時間格式 |
|--------|-------------|
| Chrome / Chromium | WebKit epoch — 自 1601-01-01 00:00:00 UTC 起的微秒數 |
| Firefox | Unix epoch 毫秒數 |
| Safari | Mac absolute time — 自 2001-01-01 00:00:00 UTC 起的秒數（浮點數） |
| Google Takeout | ISO 8601 字串（如 `2024-03-15T10:30:00.000Z`） |

**我們的 archive 必須統一所有時間為單一格式。**

#### 設計決策

- Archive 內部統一使用 **ISO 8601 UTC 字串**（如 `2024-03-15T10:30:00.000Z`）作為主要時間格式。
  - 人類可讀，方便調試和匯出。
  - SQLite 的日期函數原生支援 ISO 8601。
  - 時區一律存 UTC，前端根據用戶 locale 做顯示轉換。
- 同時保留一份 **Unix epoch 毫秒數**（`INTEGER`）用於高效排序和範圍查詢。
- Raw capture 層保留原始瀏覽器的時間值不做轉換（存在 `raw_row_versions` 的 JSON payload 中）。
- Schema 中需要明確註釋時間格式的選擇理由和轉換規則。

#### 時區處理

知道用戶在當地時間的幾點瀏覽網頁是有價值的（例如分析工作時段、夜間瀏覽模式）。我們不需要去猜測時區 — 因為我們是桌面應用，可以直接讀取系統時區。

- **每次備份 run 時，記錄當時的系統時區**（如 `America/Los_Angeles`），存在 run metadata 中。
- 該 run 中所有紀錄的「當地時間」就是 UTC 時間 + run 時的時區。這對最近的數據來說足夠準確（你不太可能 3 天內換時區）。
- 對歷史數據（Takeout 導入等），無法得知當時的時區，**預設使用用戶當前系統時區**。
- 用戶可以在設定中配置一個「回退時區」（fallback timezone），用於沒有時區信息的舊數據。
- 前端顯示時，統一用用戶當前的系統時區做轉換。用戶不需要手動管理時區 — 正常使用就夠了。
- 如果用戶經常旅行或換時區，這個近似會有一些誤差，但對趨勢分析和時段統計來說已經足夠。

#### 地理位置（可選）

既然每次 run 已經在記錄時區，順便記錄地理位置也很便宜，未來可以做地理相關的洞察（例如「你在東京出差那週主要在研究什麼」）。

- **完全可選，預設關閉**。需要用戶在設定中明確開啟。
- 使用 OS 的定位 API（macOS Core Location, Windows Location API, Linux GeoClue）獲取位置。
- **預設記錄完整精度的座標**。所有數據都只存在用戶本地，只有用戶自己能看到，所以完整精度沒有隱私風險。
- 用戶如果有顧慮，可以在設定中選擇降低精度（例如只保留城市級別）。
- 存在 run metadata 中，和時區一起。
- 如果獲取失敗（用戶未授權、無定位服務），靜默跳過，不影響備份。
- 未來可以用這些數據做「旅行時的瀏覽模式」、「不同城市的研究主題」等地理洞察。

### 5.9 可擴展的記錄增強系統（Enrichment）

**作為**用戶，**我想要**我的歷史紀錄不只有 URL 和標題，還能附帶更豐富的上下文信息，**以便**搜尋和洞察功能更準確、更有價值。

瀏覽器原生提供的歷史紀錄信息非常有限（URL + title + visit time），但 URL 本身暗含了大量可提取的信息。

#### 增強層級

**第一層：立即可用（不需要外部請求）**
- URL 結構解析：domain, subdomain, path tokens, query parameters
- Domain 分類：docs / forum / video / news / social / shopping / code 等
- 搜尋引擎 query 提取（從 URL 參數中解析 `q=`, `search_query=` 等）
- Transition / referrer 信息
- Favicon

**第二層：背景 refetch**
- 訪問 URL 抓取頁面內容，提取 readable text、meta description、OG tags
- 提取頁面語言
- Best-effort，失敗不阻塞

**第三層：基於 URL 的專屬 enrichment 插件**
- 設計為**可擴展的插件架構**：每個插件匹配一組 URL pattern，負責提取特定的結構化信息。
- 插件範例：
  - **arXiv 插件**：匹配 `arxiv.org/abs/*`，調用 arXiv API 獲取論文標題、作者、abstract、分類、發表日期
  - **GitHub 插件**：匹配 `github.com/*/*`，提取 repo description、language、stars、README 摘要
  - **YouTube 插件**：匹配 `youtube.com/watch*`，提取影片標題、頻道、時長、描述
  - **Wikipedia 插件**：匹配 `*.wikipedia.org/wiki/*`，提取文章摘要
  - **Stack Overflow 插件**：匹配 `stackoverflow.com/questions/*`，提取問題、最佳答案摘要
  - **HN 插件**：匹配 `news.ycombinator.com/item*`，提取討論主題和熱度
- 插件以 trait / interface 的方式定義，第三方可以貢獻新的插件。
- 用戶可以在設定中啟用/禁用個別插件。
- 插件的增強結果存入統一的 enrichment 表，以 JSON 格式保存，欄位隨插件不同而不同。

**第四層：未來擴展 — 瀏覽時即時捕獲**
- 未來可能透過瀏覽器擴充套件在瀏覽時即時抓取頁面內容。
- Schema 預留 `content_source` 欄位標記來源（`plugin`, `refetch`, `realtime_capture`），讓即時捕獲的數據可以取代 refetch 結果。

#### 設計原則

- 所有 enrichment 都是**非阻塞的** — 核心備份流程不依賴 enrichment 成功。
- Enrichment 結果和原始歷史紀錄鬆耦合 — 存在獨立的表中，以 history_id 關聯。
- Enrichment 可以隨時重跑 — 插件升級後可以重新增強舊記錄。
- Enrichment 的版本和來源有記錄 — 知道每條增強是哪個插件、什麼時候產生的。
- 調用外部 API 的插件必須內建 **rate limiter** — 避免觸發 API 速率限制。重試策略由 Job Queue（7.6）統一管理。

---

## 6. RECALL — 召回

### 6.1 歷史紀錄瀏覽器

**作為**用戶，**我想要**用直覺的方式瀏覽和搜尋我的所有歷史紀錄，**以便**能快速找到過去看過的內容。

#### 互動式時間軸

這是 Explorer 的核心導航元素。用戶面對的可能是跨越多年的海量歷史紀錄，需要一種直覺、流暢的方式在時間中穿梭。

- **可拖動的時間軸控件**：
  - 水平或垂直的時間軸 rail，用戶可以拖動、滾動、或點擊來快速定位。
  - 支援多個縮放級別：年 → 月 → 週 → 天。
  - 拖動時有即時的視覺反饋 — 顯示當前指向的日期和該時段的記錄密度。
  - 記錄密度的高低應該在時間軸上有視覺表達（例如色彩深淺、柱狀高度），讓用戶一眼看出哪些時間段比較活躍。
- **快速跳轉**：
  - 點擊年份 → 展開該年的月份 → 點擊月份 → 展開天數。
  - 也可以直接輸入日期跳轉。
- **回到今天**：一鍵回到最新的紀錄。
- 時間軸應當有流暢的動畫和過渡效果，拖動手感要好。

#### 搜尋與篩選

- **全文搜尋**（基於 FTS5）：搜尋 URL、標題、搜尋關鍵詞。
- **複合篩選**，可疊加使用：
  - 按瀏覽器 / Profile
  - 按 Domain（支援子域名匹配）
  - 按時間範圍（可與時間軸聯動）
  - 按頁面類型（如果有分類數據：docs, forum, video, news 等）
  - 按來源途徑（typed, link, redirect, bookmark 等）
  - 按 run ID / 導入批次
- 篩選狀態在 UI 上有清晰的標籤式展示，可逐個移除或一鍵清除。

#### 單條記錄顯示

用戶瀏覽歷史紀錄時，每條記錄默認顯示的信息以及可選顯示的信息，**用戶可以在設定中自定義**。

**預設顯示：**
- Favicon + 頁面標題
- URL（可展開/摺疊長 URL）
- 訪問時間
- 來源瀏覽器 / Profile 標識

**可選顯示（用戶在設定中開關）：**
- 訪問次數
- 來源途徑（typed, link, redirect 等）
- Domain 分類標籤
- Provenance 信息（哪次 run 寫入的）
- 所有 metadata versions（如果該 URL 的 title 等信息有過變化）
- 搜尋關鍵詞（如果這次訪問來自搜尋）
- Transition / referrer 信息

用戶的顯示偏好持久保存，跨 session 保留。

#### 記錄詳情面板

點擊任一條記錄，展開詳情面板，顯示該條記錄的完整信息（無論用戶的顯示偏好設定如何）：
- 完整 URL
- 頁面標題（所有歷史版本）
- 所有訪問時間（如果同一 URL 被多次訪問）
- 訪問次數、typed count
- 來源途徑和 referrer
- Favicon
- 來源瀏覽器 / Profile
- Provenance：寫入的 run ID、run 時間、run 來源
- 如果有 metadata 變化歷史，顯示 version diff

#### 通用

- 支援 keyboard shortcut 和快速導航（上下鍵切換記錄、Enter 展開詳情、Esc 關閉）。
- 支援從 Explorer 直接匯出當前篩選結果。
- 大數據量下保持流暢（虛擬滾動 / 分頁加載）。

### 6.2 版本管理與回滾

**作為**用戶，**我想要**在發現任何誤操作後能回滾到之前的狀態，**以便**不用擔心「試一下會不會搞壞數據」。

這是數據完整性之外的另一個核心要點：**用戶必須有信心操作這個工具，知道任何操作都是可撤銷的。**

#### Run 級別的回滾

- 每次寫入操作（定時備份 run、手動備份 run、Takeout 導入、瀏覽器直接導入）都有唯一 run ID。
- 用戶能在 Audit Ledger 中檢視每次 run 的：
  - 執行時間、來源類型、來源 profile
  - 寫入記錄數量（新增 / 更新 / 跳過 / 失敗）
  - 當前狀態（completed / reverted / partial）
- 用戶能展開某次 run，預覽它寫入的所有記錄。
- 用戶能**回滾整次 run**：
  - 該 run 寫入的所有記錄標記為 reverted（軟刪除，不物理刪除）。
  - Reverted 的記錄從正常搜尋和瀏覽中隱藏，但保留在底層以備審計。
  - 回滾操作本身記入審計日誌，產生新的 manifest。
  - 回滾是可逆的 — 用戶可以「取消回滾」，重新恢復那次 run 的記錄。

#### 典型誤操作場景

| 場景 | 恢復方式 |
|------|----------|
| 導入了格式錯誤的 Takeout 檔案 | 回滾該次 import run |
| 錯誤的 profile 被選中並備份了 | 回滾該次 backup run |
| 同一份數據被重複導入 | 去重機制自動處理；如有異常可回滾 |
| 導入了別人的歷史紀錄 | 回滾該次 import run |
| Schema migration 後發現問題 | Archive DB 自動備份可恢復 |

#### Archive 快照（Safety Net）

- 除了 run-level 的回滾，archive 本身在以下時機自動保存完整快照（SQLite DB 的完整備份）：
  - Archive schema migration 前
  - 大型導入（超過設定閾值的記錄數）前
  - 用戶手動觸發
- 用戶可以在 UI 中查看所有可用的快照，查看快照的時間和大小。
- 用戶可以從快照恢復整個 archive（相當於全局回滾到某個時間點）。
- 快照的保留策略可配置（保留最近 N 個，或按時間保留）。

#### 設計約束

- 回滾不依賴 Git — Git 只管理審計工件（manifests），不管理主 archive DB。
- 回滾是在 archive DB 層面實現的（軟刪除 + 快照），不是 Git revert。
- 回滾操作要足夠快 — run-level 的 revert 應該是 O(records in run)，不需要重建整個 archive。
- UI 中回滾必須有明確的確認步驟和影響預覽（將會隱藏多少筆記錄）。

---

## 7. INTELLIGENCE — 洞察

> Intelligence 是建立在 Archive 夠好的基礎上的增值層。  
> 所有 AI 功能預設關閉，可以在設定中開啟。

### 7.1 語義搜尋（Semantic Search）

**作為**用戶，**我想要**用自然語言搜尋我的歷史紀錄，**以便**能找到「我記得看過某個講 local-first 的文章但記不得具體名字」這類模糊記憶。

#### 需求要點

- 基於 embedding 的向量相似度搜尋。
- Embedding 增量計算、本地索引、避免重算。
- 搜尋不只找頁面，還要支持找 session、task 和 topic level 的語義匹配。

### 7.2 AI 助手（Ask My History）

**作為**用戶，**我想要**用自然語言問我的歷史紀錄問題，**以便**能在不手動翻閱的情況下回顧過去的研究。

#### 需求要點

- 基於 LLM 的問答，context 來自 archive 的 agentic RAG 檢索。
- 問的不是網際網路，而是「我的過去」：
  - 「我什麼時候開始研究 MCP 的？」
  - 「我上次比較 vectorDB 方案時看了哪些東西？」
  - 「我最近三個月在哪些領域花了最多時間？」
- Agentic search：LLM 可以多步檢索、比較、歸納。
- 回答必須附帶 evidence（哪些歷史紀錄支持了這個結論）。
- 這是**顯式觸發**的功能，不是背景常駐。

### 7.3 MCP 搜尋介面

**作為**用戶，**我想要**能透過 MCP (Model Context Protocol) 存取我的瀏覽歷史，**以便**在其他 AI 工具中也能搜尋我的歷史紀錄。

#### 需求要點

- 在設定中手動開啟。
- App 啟動本地 MCP server。
- 提供搜尋、檢索歷史紀錄的 MCP tools。
- 安全考量：只綁定 localhost，不對外暴露。

### 7.4 洞察系統（Insights）

**作為**用戶，**我想要**看到基於我的長期歷史紀錄生成的洞察，**以便**理解自己的興趣演化、工作模式、和注意力走向。

#### 架構原則

洞察系統採用**模塊化設計**：每個洞察功能是一個獨立的模塊，有自己的數據需求、計算邏輯和 UI 呈現。這樣做的目的是：

- 新的洞察可以獨立開發和添加，不影響現有功能。
- 用戶可以選擇開啟/關閉個別洞察模塊。
- 不同模塊可以有不同的計算頻率和觸發條件。
- 第三方未來可以貢獻新的洞察插件。

每個洞察模塊定義：
- **名稱和描述**
- **數據依賴**：需要哪些表、哪些 enrichment、是否需要 embedding
- **計算邏輯**：怎麼從原始數據算出洞察結果
- **觸發條件**：什麼時候重算（每次備份後、每天、每週、手動）
- **UI 組件**：怎麼展示

#### 分析管線

```
層 1：結構特徵提取
  從每條歷史紀錄中提取可計算的特徵：
  URL 結構、domain 類型、搜尋關鍵詞、transition/referrer、
  訪問時間/星期/時段、訪問頻率、估計停留時長

層 2：Session 和 Task 構建
  把零散的頁面訪問聚合成有意義的單元：
  相鄰訪問 → session → 語義相近的 session 合併為 thread → 偵測 reopen

層 3：Topic 聚類和時間序列
  對 visit/session 的 embedding 做聚類，形成 topic →
  追蹤每個 topic 隨時間的變化 → 偵測趨勢、爆發、轉折

層 4：LLM 增強
  為 topic 和 thread 起人類可讀的名字 →
  生成對比式摘要 → 解釋為什麼某個洞察值得關注
```

#### 具體洞察功能

##### 🕐 歷史上的今天（On This Day）

讓用戶看到過去幾年的同一天，自己在瀏覽什麼。

- 拉出歷年同一天（±1 天容差）的歷史紀錄，按年份分組展示。
- 如果有足夠數據，用 LLM 生成一句話摘要：「2024 年的今天，你在深入研究 Rust async runtime；2023 年的今天，你在比較幾個 CSS 框架。」
- 適合放在 Dashboard 上作為每日亮點。
- **實現**：純數據庫查詢（按月日篩選），不需要 embedding。有 LLM 時可生成摘要，沒有也能用。

##### 📊 定期總結（Periodic Summaries）

自動生成日度、週度、月度、年度的瀏覽總結。

- **日度總結**：今天你主要在研究什麼？訪問了多少個頁面？最活躍的 domain 是什麼？
- **週度總結**：和上週相比，你的研究重心有什麼變化？本週新出現了什麼主題？
- **月度總結**：本月的主題分布、最深入的研究線、最常用的資訊來源。
- **年度總結**：年度回顧 — 你這一年的注意力分布、主要的研究階段和轉折點、最重要的發現。
- **實現**：
  - 統計部分（訪問量、domain ranking、時段分布）不需要 AI。
  - 主題歸納和對比式描述使用 LLM：先把時間窗內的 topic 分布和上一期做 delta 計算，把結構化的差異交給 LLM 寫成人話。
  - 年度總結可能需要比較大的 context window，可以分段處理。

##### 🌊 Topic Timeline（主題時間軸）

你最近在關注什麼？哪些主題在升溫？哪些在冷卻？有沒有新冒出來的興趣？

- 以視覺化的方式展示主題隨時間的變化：升溫中（越來越多相關訪問）、穩定（持續出現）、降溫中（越來越少）、全新（最近才開始）。
- 用戶可以點擊任何一個主題，看到它下面的具體頁面和時間分布。
- **實現**：對所有歷史紀錄做 embedding，用增量聚類算法（如 nearest-centroid）分成 topic cluster。每個 cluster 在不同時間窗口（7 天、30 天、90 天）統計訪問量，算 trend slope。用 LLM 給每個 cluster 起一個人類可讀的名字。

##### 🧵 Task / Thread Detection（任務和研究線偵測）

你不是在「看網頁」，你是在「做事情」— 比較工具、查 bug、研究方案、學一個新技術。這個功能自動把你的歷史紀錄切成一個個有意義的任務。

- 自動偵測進行中的研究線：跨天持續的、語義上連貫的一系列訪問。
- 顯示每個任務的：開始時間、最近活躍時間、包含的頁面數、涉及的 domain。
- 偵測「任務重新打開」：停了幾天又回來繼續的研究線。
- **實現**：先用 `from_visit` chain 和時間間隔（≤30 分鐘都算同一 session）切出 session；再用 embedding 相似度把語義連貫、時間間隔 ≤14 天的 session 合併成 thread；間隔 ≥24 小時後又出現的算 reopen。

##### 🔄 Open Loops（未完成的任務）

你有沒有什麼事情反覆在看，但一直沒完成？

- 找出那些反覆打開、間隔性回來、但沒有明顯收斂的任務線。比如你連續幾天都在比較某些工具，但始終沒有做出決定。
- **實現**：基於 thread 的 revisit 次數、時間間隔分布、以及是否出現「收斂信號」（例如從比較頁轉向購買頁、從教程轉向實作代碼、從搜尋轉向寫文檔）。如果 revisit 多但沒有收斂信號，標記為 open loop。

##### 💎 Important but Unsaved（重要但沒保存的頁面）

你看了很多次，顯然覺得重要，但從來沒有 bookmark 或做筆記的頁面。也許你應該把它們整理一下。

- 根據 revisit 頻率、估計停留時長、在語義網絡中的中心度，計算每個頁面的「隱含重要性」分數。
- 找出分數高但沒有任何保存行為的頁面，提醒用戶。
- **實現**：`importance = revisit_count × estimated_dwell × semantic_centrality`。Semantic centrality 是該頁面的 embedding 在其所屬 topic cluster 中與 centroid 的距離（越近越重要）。

##### 📈 Explore vs Exploit（探索 vs 深挖）

你最近是在到處看新東西（explore），還是在集中精力鑽研某個問題（exploit）？

- 量化你某段時間的注意力模式：domain entropy 高 = 廣泛探索，domain entropy 低 + revisit 集中 = 深度鑽研。
- 追蹤這個比例隨時間的變化。
- **實現**：計算每個時間窗口的 domain Shannon entropy、新 domain 佔比、revisit concentration。可以做成類似「注意力模式波形圖」的視覺化。

##### 🗺️ Source Role Map（資訊來源角色圖）

你用不同的網站扮演什麼角色？Google 是搜尋入口、Reddit 是看口碑、Docs 是查答案、YouTube 是快速理解、GitHub 是看真相...

- 根據你實際的使用方式，把常用網站分成角色：搜尋入口、社群探索、官方確認、問題定位、學習消費、娛樂休息。
- 顯示你的「研究工作流」：你通常怎麼從搜尋走到答案。
- **實現**：結合 domain heuristics（`*.google.com/search*` = 搜尋）和 URL pattern 規則。更精細的分類可以用 LLM 對不確定的做標記。

##### 🔍 Query Reformulation Ladder（搜尋問題的演化路徑）

你是怎麼一步步把問題問清楚的？從模糊的大概念，到精確的技術問題。

- 把同一研究線中的搜尋關鍵詞按時間排列，分析演化方向：broadening（擴大範圍）、narrowing（縮小範圍）、compare（開始比較）、error-driven（查錯誤信息）、site-restrict（加 `reddit`、`site:github.com`）。
- **實現**：從 Chromium 的 search_terms 表提取搜尋詞，按 thread 分組，計算相鄰 query 的語義距離和結構變化。目前只有 Chromium 系瀏覽器能提供這個數據。

##### 🌐 Site Analytics（網站統計）

你在某個網站花了多少時間？訪問了多少次？

- 按 domain 或具體 URL 統計訪問次數和估計 session 時長。
- 支持不同時間窗口：今天、最近 7 天、最近 30 天、最近一年、全部時間。
- 可以選擇單個或多個 domain 做對比。
- 統計維度：訪問次數、唯一頁面數、估計總時長、平均 session 時長、最活躍的時段。
- **估計停留時長的方法**：瀏覽器歷史紀錄不直接記錄停留時間，但可以間接推算 — 相鄰兩次訪問的時間差即為前一個頁面的估計停留時長。超過一定閾值（如 30 分鐘）的視為 session 結束，不計入停留時長。這個方法不完美，但足夠用於趨勢分析。
- **實現**：純數據庫查詢和統計計算，不需要 AI。可以做成可視化的圖表（柱狀圖、折線圖、熱力圖）。

##### 🎯 Contrastive Summary（對比式摘要）

「這週 vs 上週，你的研究重心變了什麼？」

- 把兩個時間窗口的瀏覽數據做對比：哪些主題新出現了、哪些消退了、哪些加速了。
- 不是告訴你「這週你看了 A B C」，而是「和上週相比，你明顯從 X 轉向了 Y」。
- **實現**：分別計算兩個時間窗口的 topic 分布，做 delta 差異；把結構化的差異（新增 topic、消失 topic、volume 變化最大的 topic）交給 LLM 寫成一段人話。LLM 只需要吃一小張差異表，不需要讀全部歷史。

---

**第二梯隊（V1.5+）**

以下功能放在 V1 之後迭代，但架構上第一天就預留位置。

##### Learning Trajectory（學習階段追蹤）

你對某個主題的瀏覽，是在入門探索、工具比較、還是已經進入實作？對頁面做類型判斷（overview / tutorial / docs / issue / benchmark / pricing），然後看時間上的遷移。例如：「你對 Rust 的瀏覽已從教程階段進入 issue debugging 階段。」

##### Burst Detection（興趣爆發偵測）

短期內某個主題的訪問量突然暴增 — 可能是一個新事件、一個突發問題、或一次 rabbit hole。用時間序列上的 spike detection（如 Kleinberg burst model）自動標出。

##### Curiosity Graph（好奇心跳轉圖）

把你的瀏覽跳轉做成一張概念圖：你怎麼從一個念頭跳到另一個念頭？哪些概念是你的「橋樑節點」？哪些 topic 容易把你帶進 rabbit hole？基於 session 內相鄰訪問的語義跳轉建圖。

##### Rediscovery Pain（重複搜尋疼痛指數）

你明明以前找過這個東西，結果又得重新搜一遍。找出那些被重複搜尋或重複訪問的知識點，提醒你把它們整理起來。

##### Session Archetypes（瀏覽模式分類）

把你的 session 分成幾種原型：learn（學習）、debug（排查問題）、compare（比較選型）、buy（購物決策）、monitor（追蹤動態）、entertain（娛樂放鬆）。基於 URL pattern、domain 類型、query 動詞等特徵做分類。

##### Faceted Profile（工作流畫像）

你是一個什麼樣的資訊使用者？docs-first 還是 forum-first？深挖型還是掃描型？偏好文字還是影片？從長期統計數據中構建多維度畫像，用証據支撐而非空洞標籤。

##### Narrative Arc（敘事弧線）

你最近在「講什麼故事」？把最近一段時間的瀏覽濃縮成一段敘事：你的主線是什麼、有哪些支線、走到了哪一步。有點像個人版的「Previously on...」。

#### 約束

- 不做人格評判型分析（你是什麼人格、你有什麼偏見）。
- 不對健康/政治/宗教/性向等敏感維度做推斷。
- 背景分析不能讓 app 變慢或風扇狂轉。Embedding 增量處理，LLM 按需觸發。

### 7.5 AI Provider 配置

**作為**用戶，**我想要**靈活配置 AI 模型的來源，**以便**用 Ollama、LM Studio、或雲端 API 來驅動 embedding 和 LLM 功能。

#### 需求要點

- 概念模型：
  - **請求格式**（Request Format）= API 協議，如 OpenAI-compatible, Anthropic, Google
  - **Provider** = 請求格式 + Base URL + API Key + 可用模型列表 + 模型配置
  - 用戶可以創建多個 Provider（例如 6 個 OpenAI 格式的、3 個 Anthropic 格式的）
  - Provider 可以被啟用/禁用
- 預設 Provider preset：Ollama, LM Studio, OpenAI, Anthropic, Google
- 所有 preset 都支援自定義 Base URL（方便用代理或自建服務）
- Embedding 和 LLM 分別配置：兩者可以用不同的 Provider 和模型。

### 7.6 AI 計算任務系統（Job Queue）

**作為**用戶，**我想要** AI 相關的計算（embedding、LLM 摘要、洞察分析）在背景自動運行，不阻塞我的操作，並且能控制它的行為。

所有需要 AI 模型推理的操作（embedding 計算、LLM 摘要生成、洞察模塊計算）都不是即時完成的，需要一個異步任務系統來管理。

#### 任務生命週期

- **任務產生**：備份完成後、導入完成後、用戶手動觸發、定時掃描發現未處理記錄。
- **任務排隊**：任務加入隊列，按優先級和產生時間排序。
- **執行**：在背景異步執行，不阻塞 UI。
- **成功**：結果寫入對應的表，標記任務完成。
- **失敗**：記錄錯誤原因，按可配置的策略自動重試（最多 N 次，指數退避）。
- **暫停**：用戶可以隨時暫停所有計算任務，之後恢復。

#### 任務類型

- **Embedding 計算**：對新的或未處理的歷史紀錄生成 embedding vector。
- **Enrichment refetch**：背景抓取頁面內容做內容增強。
- **Insight 計算**：計算各洞察模塊的結果（topic 聚類、thread 構建、統計指標等）。
- **LLM 摘要生成**：生成 topic 命名、對比式摘要、定期總結等。

#### 用戶控制

- 在 UI 中可以看到：
  - 當前隊列中有多少待處理任務
  - 正在運行的任務和進度
  - 最近完成/失敗的任務
- 可以調整**同時執行的併發任務數量**（預設保守，避免吃滿用戶 CPU/GPU）。
- 可以一鍵暫停 / 恢復所有計算。
- 可以手動觸發「掃描數據庫，把所有需要處理但還沒處理的記錄加入隊列」。
- 可以清理失敗的任務或重新排隊。

#### 設計原則

- 計算任務系統完全獨立於核心備份流程 — 備份不等待 AI 計算完成。
- 計算結果存入獨立的表 — 即使清空所有計算結果，重跑一遍就能恢復。
- 沒有配置 AI provider 的用戶完全看不到這個系統。

---

## 8. UX 設計原則

### 8.1 視覺設計方向

- 追求現代、精緻、有科技感的視覺風格。
- Dashboard 和洞察頁面可以做得炫酷和花哨 — 用漂亮的數據視覺化、流暢的動畫、動態圖表讓用戶覺得好看好用。
- 暗色模式優先，但也要支持淺色模式。
- 信息密度適中，不要太空也不要太擠。
- 桌面優先，但保留窄視窗可用性。
- 需要操作透明性的地方（audit ledger、排程設定、導入流程）保持清晰嚴謹，但不需要整個 app 都像帳本。

### 8.2 操作透明性

- 每一個涉及系統/數據的操作，都要讓用戶看到：
  - 我們在做什麼
  - 為什麼做這件事
  - 具體執行了什麼命令/步驟
  - 已經做了哪些、還剩哪些
  - 如何撤銷
- 自動模式和手動模式都是 step-by-step 的 UI。
- 對於自動模式：逐步顯示進度，每步可展開查看詳情。
- 對於手動模式：每步有指南、理由、可複製的命令、完成後的確認。

### 8.3 狀態清晰

- 所有可交互元素的狀態必須清晰可辨：
  - 勾選/未勾選有明顯視覺差異和動畫反饋。
  - 操作進行中有明確的 loading / progress 狀態。
  - 錯誤有明確的錯誤信息和修復建議。
- 空狀態友好：沒有數據時告訴用戶該怎麼開始。
- 降級狀態友好：AI 未配置時明確提示，但不阻礙核心備份功能。

---

## 9. 畫面與導航結構

以下是主要的畫面和導航結構建議。具體視覺設計應使用 Stitch 產出設計稿後再定。

### 9.1 畫面清單

| 畫面 | 核心職責 |
|------|----------|
| **Onboarding / Setup** | 首次啟動引導：發現瀏覽器、選擇 profile、設定存儲、加密選擇 |
| **Dashboard** | 備份狀態總覽、最近 run 摘要、歷史上的今天、定期總結卡片、Job Queue 狀態、快速操作入口 |
| **History Explorer** | 時間軸 + 全文搜尋 + 篩選 + 詳情 + 匯出 |
| **Insights** | 洞察卡片、topic timeline、threads、query ladders、profile facets |
| **AI Assistant** | 自然語言問答介面 |
| **Import** | Takeout 導入 wizard + 瀏覽器直接導入（含 step-by-step UI） |
| **Audit Ledger** | Manifest chain、run 歷史、diff 視圖、schema 變化紀錄 |
| **Security** | 加密設定、keyring、rekey、密碼警告 |
| **Schedule Setup** | 排程預覽 → 手動安裝/自動安裝 → 狀態監控 |
| **Settings** | 通用設定、語言、AI provider 管理、MCP 開關、數據目錄、版本信息 |

### 9.2 導航

- 左側 sidebar 導航（可收合）。
- 頂部顯示當前頁面的 breadcrumb / title。
- 全局快速搜尋入口。

---

## 10. 國際化（i18n）

- 支援語言：英文、簡體中文、繁體中文。
- 自動檢測用戶設備語言，預設選擇匹配的語言。
- 可在設定中手動切換。
- 所有用戶可見的文字都走 i18n，包括錯誤信息和通知。

---

## 11. 系統信息

- 前端顯示當前版本號和 git commit short SHA。
- 設定頁面中顯示數據存儲目錄，並提供「在文件管理器中打開」的按鈕。

---

## 12. 品質標準

### 測試覆蓋

- Rust 側：100% test coverage + mutation test + integration test。
- JS/TS 側：100% statement/branch/function/line coverage + mutation test。
- E2E：Playwright spec 覆蓋關鍵用戶流程。

### 代碼品質

- Rust：clippy + cargo fmt + cargo deny（supply chain audit）。
- JS：ESLint + Prettier + TypeScript strict mode。
- Pre-commit hooks 執行所有 linter 和 formatter。

### CI/CD

- GitHub Actions：
  - PR 檢查：lint + test + coverage + build。
  - Release pipeline：多平台構建 + 自動產出安裝檔。
- README badges 顯示 CI 狀態、coverage。

---

## 13. 開源與社區

- 協議：GPL v3。
- README：完整的功能介紹、構建指南、從源碼運行指南。
- CONTRIBUTING.md：開發環境設定、測試方式、commit 規範、PR 流程。
- Conventional Commits 規範。

---

## 14. 里程碑建議

### M1 — Solid Archive

- 增量備份（Chromium）完全可用
- 排程設定（macOS LaunchAgent）
- 基本加密/不加密選擇
- 審計 manifest + hash chain
- 歷史紀錄瀏覽和搜尋
- HTML/JSONL 匯出

### M2 — Recall & Trust

- Google Takeout 導入（含 dry-run, quarantine, 回滾）
- 多瀏覽器支持（Firefox）
- Doctor 完整性檢查
- Run 歷史與回滾 UI
- Preview/Manual/Execute 全面落地
- i18n（en, zh-CN, zh-TW）

### M3 — Intelligence

- AI provider 配置 UI
- AI 計算任務系統（Job Queue）
- Embedding pipeline + 語義搜尋
- 基礎洞察：Topic timeline, On This Day, Site Analytics, 定期總結
- Ask My History（AI 問答）

### M4 — Full Intelligence & Polish

- 完整洞察套件：Thread detection, Open Loops, Contrastive Summary, Explore/Exploit 等
- Enrichment 插件系統（arXiv, GitHub, YouTube 等）
- MCP server
- S3 遠端備份
- 多平台完整驗證（Windows, Linux）

---

## 15. 不做的事情（Explicit Non-Goals）

- 不做雲端同步或雲端存儲（除非用戶主動配置 S3）。
- 不做人格心理分析或敏感維度推斷。
- 不寫回瀏覽器的 live 數據庫。
- 不做背景常駐的 autonomous agent。
- 不做 SaaS 或 subscription model。
- 不收集用戶數據或 telemetry。
