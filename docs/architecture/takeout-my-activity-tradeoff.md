# 匯入 Google Takeout 的「我的活動 → Chrome」 — Trade-off

> **狀態：用戶 2026-10-09 決定匯入 JSON（HTML 只認出來、請改選 JSON）。本文推翻 `archive.md`「Google Takeout 導入」的 Chrome-first（只讀 Chrome 產品的 `History.json`，「我的活動」標 `needs-review` 不匯入）。跨來源重複怎麼處理（§3.3）要在真實資料上量過再定。**
> **範圍：** `browser-history-parser` 的 Takeout 辨識與解析、`vault-core/src/takeout/`、匯入卡的文案、`archive.md`。

## 1. 為什麼

Takeout 有兩份 Chrome 的瀏覽紀錄：

- **Chrome 產品**的 `Chrome/History.json`（舊稱 `BrowserHistory.json`）：Chrome 同步到帳號的歷史，觀察到只回溯約一年。PathKeep 現在只讀這份。
- **我的活動 → Chrome**：跟著帳號的「網路和應用程式活動」保留（3 / 18 / 36 個月自動刪除，或一直保留），可能比上一份久得多；只有開了同步並勾「包含 Chrome 記錄和其他網站活動」才有。格式可選 JSON 或 HTML，HTML 是預設。

用戶的樣本就是後者：一個 `我的活動/Chrome/我的活動.json`，109,644 筆，涵蓋一年（只看結構，見下）。不匯入的話，用戶拿到的唯一一份 Takeout 就整份被略過。v0.3 的 parser 曾讀 `titleUrl`，但時間只認 Chrome 產品的欄位、不認 `time`，也不解 Google 的轉址，所以那時匯入的結果要另查（§3.4）。

## 2. 格式（用戶的樣本，2026-10-09，只看欄位與格式）

一個 JSON 陣列，每筆：

| 欄位               | 內容                                                                                         |
| ------------------ | -------------------------------------------------------------------------------------------- |
| `header`           | 網站的網域，或 `Chrome`                                                                      |
| `title`            | `Visited <頁面標題>`；沒有網址的是 `Used Chrome`（238 筆）。中文帳號的匯出前綴仍是英文       |
| `titleUrl`         | 造訪的網址；109,406 筆有。87,139 筆包在 `https://www.google.com/url?q=<網址>&usg=…` 的轉址裡 |
| `time`             | UTC 的 RFC 3339（`…T…Z`，多數帶毫秒）                                                        |
| `products`         | `["Chrome"]`                                                                                 |
| `activityControls` | 例如「網路和應用程式活動」的名稱；部分有                                                     |

沒有造訪 id、轉場類型、開啟它的頁面（opener）、停留時間。

## 3. 做法

### 3.1 辨識

- 依內容辨識，不靠檔名：頂層是陣列，筆都有 `time`、`products` 含 `Chrome`、多數有 `titleUrl`，就是「我的活動 → Chrome」的 JSON。檔名只當快路（各語言的「我的活動」資料夾名不必列舉）。
- HTML（`我的活動.html` / `MyActivity.html`）照現在：認出來、不讀，匯入卡寫明請在 Takeout 改選 JSON。
- 同一份匯出裡同時有 Chrome 產品的 `History.json` 與「我的活動」：兩份都匯入（各自一個來源），重複的處理見 §3.3。

### 3.2 每一筆

- 網址：`titleUrl`；是 `google.com/url?q=` 的轉址就取 `q` 參數解碼後的網址（只解這一層 Google 轉址，其他網址原樣）。沒有網址的（`Used Chrome`）略過、計入「略過」。
- 時間：`time` 解析成 UTC 毫秒（ADR-002）。
- 標題：去掉開頭固定的 `Visited `；其他語言的前綴見到再加（樣本只有英文）。去不掉就原樣存，不猜。
- 來源：獨立的 source profile（例如 `takeout:my-activity:chrome`），瀏覽器種類 Chrome。重匯同一份不重複（現有的 `source_visit_id` 規則：網址、時間、在檔案中的順序）。
- 沒有的欄位留空：opener 存成「沒記錄」（`from_visit` NULL，跟 Safari、Chrome 產品的 Takeout 一樣，洞察的「怎麼來到這裡」說無法知道）。

### 3.3 跨來源重複（待量）

「我的活動」記的多半是本機 Chrome 也記了的同一次造訪；Chrome 產品的 `History.json` 也是。現在每個來源各自存、各自去重，洞察把所有來源加起來，所以同一次造訪會算兩、三次。這不是「我的活動」才有的問題，但它讓問題變大。

選項：

- **A. 照現在，各來源分開存、分開算。** 最簡單、證據完整；洞察的總數高估。
- **B. 匯入時，跟 archive 裡任何來源同一個網址、時間差在 N 秒內的造訪視為重複，不寫入、計入「已在存檔」。** 洞察不重算；要先量「我的活動」的 `time` 跟 Chrome 自己記的造訪時間差多少（N 取多少才不會把真的兩次造訪合成一次）。
- **C. 都寫入，但記下它是哪一筆的重複，洞察只算一筆。** 證據完整、洞察不重算；資料模型要多一層。

建議先在用戶的真實資料副本上量（只數，不看內容）：「我的活動」的每筆，在本機 Chrome 來源裡有沒有同網址、時間差 1 / 5 / 60 秒內的造訪，各占幾成。量完再在 B 與 C 之間定，A 只當暫時的現況。

### 3.4 用戶已匯入過的那份

用戶說舊版匯入過這份樣本。要在真實資料副本上數：archive 裡網址以 `https://www.google.com/url?q=` 開頭的造訪有多少、標題以 `Visited ` 開頭的有多少、屬於哪個來源。如果舊版把轉址或前綴原樣存進去了，升級轉換時要把它們改成解開後的網址與標題（Turso 格式未發佈，轉換時做一次），否則新的匯入會跟它們變成兩份。

### 3.5 證明

- `takeout-import.spec`：Takeout 資料夾放一份照樣本結構造的「我的活動」JSON（含轉址、`Used Chrome`、中文資料夾名），驗預覽的筆數、匯入後搜尋找得到解開後的網址與去掉前綴的標題、重匯不重複；HTML 仍只提示改選 JSON。
- 跨來源重複定案後，再加一個 E2E：本機 Chrome 已有的造訪，匯入「我的活動」後洞察不重算。
