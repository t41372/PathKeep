# 設計原型：v0.6 的資料類型空間與 Agent 對話

用 app 自己的元件、設計 token、字型與圖表畫的可操作原型，資料全是用固定種子生成的假資料（約 300 個專案、1,900 多段對話、上萬個 subagent）。它不進 build，也不接後端。

**它是暫時的。** 設計決定都寫在 `docs/` 裡，以那些文件為準，不以這裡的程式為準：

- 外框與顏色：`docs/design/data-spaces-tradeoff.md`
- 資料類型、啟用、備份、背景任務：`docs/features/data-spaces.md`
- Agent 對話的每個畫面：`docs/features/agent-history.md`
- 存檔格式與讀取模型：`docs/architecture/agent-history-product-design.md`

畫面做進正式前端（`src/`）之後，這個資料夾連同 `.prettierignore` 與 `eslint.config.js` 裡的排除項一起刪掉，不會進 `main`。在那之前只把它當參考看，不在這裡重構。

## 怎麼開

在 repo 根目錄：

```sh
./node_modules/.bin/vite --config docs/design/prototype-2026-11-agent-history/vite.config.ts
```

然後打開 <http://127.0.0.1:1431/>。右下角「原型選項」可以切換開了幾種資料類型、升級後還沒開 Agent、背景有沒有任務在跑、深色、玻璃材質，也能直接跳到幾個特殊狀態。

型別檢查：`./node_modules/.bin/tsc -p docs/design/prototype-2026-11-agent-history/tsconfig.json --noEmit`。

## 跟規格不一樣的地方

原型畫到一半就停了，規格比它新。規格定了、原型沒跟上的有：範圍的定義與分桶、按天切開、你在場的時間的算法、照每一輪的模型算 token 與費用、專案只在確定時自動歸類、垃圾桶裡的對話不該算進洞察、搜尋的片語與正規表示式、備份頁那則提醒（規格不讀工具的設定檔）。各頁寫的上次、下次備份時間也彼此對不上。另外原型裡很多操作只是示意（立即備份、匯出、改名、在 Finder 顯示、設定裡大部分的開關）。

## 檔案

- `src/data/`：假資料生成器、對話內容、搜尋、共用查詢、背景任務、格式化。
- `src/kinds.tsx`、`src/shell/`：資料類型清單、導航列、外框、⌘K、原型選項。
- `src/assets/harness-icons/`：各工具的圖示（來源見該資料夾的 README）。
- `src/pages/`：各畫面。
