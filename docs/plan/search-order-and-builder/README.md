# 搜尋排序與進階搜尋面板：交接筆記（2026-10-25）

`STATUS.md` 的 `WORK-SEARCH-ORDER-AND-BUILDER` 做到一半換機器時留下的。`notes/` 裡是讀碼 agent 的報告，**沒有逐條驗證**，用之前對照程式碼；這個 block 做完就刪掉這個資料夾。

## notes/ 裡有什麼

| 檔案                    | 內容                                                                                 |
| ----------------------- | ------------------------------------------------------------------------------------ |
| `read-backend-order.md` | 後端串流搜尋怎麼排序（relevance / newest / oldest）、游標、加「最常造訪」排序的成本  |
| `read-semantic.md`      | 語意搜尋的排序與 `noteCodes`、其他用到搜尋排序的地方（⌘K、Ask、MCP、code mode）      |
| `read-frontend.md`      | History 工具列、shadcn 元件、i18n 寫法、設計文檔對新控制項的要求                     |
| `read-e2e.md`           | History 搜尋的 E2E 寫法、fixture、怎麼從測資算出預期順序                             |
| `read-decisions.md`     | 已定案的相關決策、v0.2 / v0.3 的前例（舊 Explorer 的 regex 切換與語法浮窗）          |
| `triage-*.json`         | BACKLOG 舊 block 對照現在程式碼的分類（obsolete / done / valid / partial / blocked） |

沒跑完、要重做的：查詢語法（`search_query.rs` 的每個運算子、值能不能有空白與引號、regex 比對哪些欄位）；BACKLOG 分類的 `WORK-REVIEW-0614/0612-FOLLOWUPS-A`、舊前端的幾個 block（shadcn sweep、schedule、runtime lite、og queue、virt、stars paginate）、加密的三個 block，以及 STATUS.md 裡 `[x]` block 內還沒做完的小項。

## 已確認

- 後端 `start_search` 的 `SearchRequest.sort` 已支援 `relevance` / `newest` / `oldest`（`vault-search/src/filter.rs` 的 `SortOrder`），但 History 沒讓使用者選：全文固定 relevance、regex 固定 newest（`src/features/history/queries.ts`）。
- 「?」語法速查（`src/features/history/search-help.tsx`）是一串點了就填進搜尋框的例子，regex 只有一個。
- 基線：`bun run check:js` 綠，`cargo test --workspace --all-targets --no-run` 編得過。

## 方向（還沒定案）

- 排序：搜尋時在結果數那一行右邊放排序選單；全文有相關度 / 最新 / 最舊（是否加「最常造訪」看 `read-backend-order.md` 的成本），regex 沒有分數，只有最新 / 最舊；語意搜尋先看 `read-semantic.md`（後端的時間排序會退回純關鍵字，不能直接接）。寫進 URL（`sort=`），預設值不寫。
- 進階搜尋：掛在搜尋框上的面板（像 Gmail 的搜尋選項），打開時把框裡的查詢解析回欄位，即時顯示會送出的語法，按「搜尋」才套用（Preview → Manual → Execute）。regex 那部分讓使用者用「文字 / 任意文字 / 一段數字 / 網址的一段」這類組件拼出 pattern，不必會寫 regex。語法速查留著給熟的人。
- 動手前先用三種方向各出一版設計、評審後再定。

## 其他發現

- `scripts/prepare-worker-sidecar.mjs` 一律帶 `--target <triple>` 建置 worker，即使就是本機的 triple，cargo 也會把所有依賴另編一份（Linux 上 `src-tauri/target/x86_64-unknown-linux-gnu` 3.1 GB）。目標等於本機時不帶 `--target`、從 `target/<profile>/` 複製，就能和主建置共用。
- E2E 的建置吃 `CARGO_TARGET_DIR`（`playwright.config.ts`）；設成 `src-tauri/target` 可以和 `cargo test` 共用依賴，磁碟緊時有用。
