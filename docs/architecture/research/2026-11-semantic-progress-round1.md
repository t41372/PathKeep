# 第一輪：索引進度與續跑範圍

2026-11-14。本輪只修原始進度問題；全部實作完成後才驗證。依賴、儲存引擎、generation 命名與來源快取不改。

## 根因與正常流程證據

正常路徑是完成備份 → `archive_flows` 的 after-backup admission → 選定模型的索引 job → worker 從持久化 cursor 呼叫 backfill → cursor/read model → Settings 與 Background work。

使用者已安裝 app 的正常任務 467 保存 4,111 次寫入，其中 856 新增、3,255 更新；最初缺列分母為 94。`856 = 94 + 762`，最後 next id 674515 也超過原 scan target 673752。原始程式每次 backfill 呼叫重讀最高 id，resume 卻保留舊 target，與真實任務紀錄吻合。這份證據不依賴故障注入、修改內部資料或自訂 benchmark。

`embedded_so_far` 計入所有 changed visits，`embed_target` 只數缺列；兩者不是同一集合。上限重新讀取則讓續跑的集合再擴張。UI 將它們直接相除，造成超量、無效進度條與消失的 ETA。後三項是同一資料契約錯誤的表現，不能用 clamp 解決。

新發現的映射、快照、命名與 snapshot 競態不構成這組數字的原因，列下一輪，不在這輪順帶修。

## 方案與取捨

- 持久化小型 `IndexScan`：版本、固定最高 id、規劃位置、實際列數、已核對列數、來源 watermark、階段。與現有 cursor 一起存 JSON，不新增 schema 或逐筆任務表。
- 進度單位為「已核對的造訪記錄」，包含最後因可見性／工作集而略過的項目。新增／更新統計獨立保留。只有核對階段提供精確比例，準備、來源讀取、搜尋詞、投影與發布使用階段文字。
- 用主鍵 keyset 分頁取得固定上限內的實際 id；規劃與執行使用同一集合，id 空洞不當筆數。每次只保留一小頁。
- 正常產品的撤銷／還原是軟標記；備份只新增 canonical visit。清空或替換資料庫會關閉 owner／終止舊工作。因此固定上限內的實體記錄集合不因正常可見性變更而縮放。若日後加入實體刪除，必須重新設計此契約，不能默認相容。
- 同一 scope 的 pause/retry 沿用界限及規劃／核對進度；規劃期間 cursor 尚為 0 也必須沿用 source identity，不能當成首次重建而清掉 scope。lineage 改變才重綁。舊 cursor 沒有新測量契約，先以未知總量顯示，再重新核對固定範圍；保留向量和既有結果，避免重新送模型。
- 保留本輪捕捉的 source watermark，不能用續跑時較新的全域 watermark 宣稱覆蓋尚未核對的新增記錄。
- 規劃也屬 durable progress；有實際前進才恢復重試額度，心跳或階段切換不算。

不採只扣 updated：仍可能在更新舊內容時長時間顯示 0，且不修續跑集合。也不採 id/max-id 近似比例。逐筆 work table 在目前 append/soft-delete 契約下不必要，會增加寫入、回收與相容性成本。來源變更日誌及大型快取屬後續效能議題，不是修這個進度契約的必要條件。

這次展示從「缺列建立量」改為「固定範圍的核對量」，使數字、百分比、速度同單位；舊 outcome counters 保持原義。它沿用先前已提出的分階段進度方向，移除超出本輪的儲存與快取改動。

## 實作及驗證邊界

先封存過度擴張的未提交草案，恢復基準，再只實作本方案及三語文案。不得將舊草案的通過結果當成本輪驗證。

整輪完成後，驗證正常備份引起的既有頁面更新、新造訪、暫停期間再次備份、繼續、兩個進度畫面，以及其後階段。最後執行完整 `bun run check`，通過才依 Conventional Commits 提交。新失敗先記錄、調查與規劃；本輪範圍外問題留下一輪。

## 驗證失敗複核：查詢分類

第一輪完整 gate 的第一個失敗來自讀取計畫檢查，其他 499 個核心案例通過。失敗 SQL 是捕捉界限的 scalar subquery（`ORDER BY id DESC LIMIT 1`），不是規劃的主鍵區間分頁；Turso 的計畫把這種端點讀取也標為 `SCAN visits`。

原因是本輪調整檢查時，用「包含 SELECT id 與 LIMIT」篩選規劃查詢，誤把界限捕捉也納入「不得 SCAN」斷言。這不是已證實的產品全表掃描。修正方案是只選定主鍵區間分頁 SQL，同時要求它確實出現在計畫中，並保留原本對 lineage 全掃及未分頁查詢的禁止。不得刪掉檢查或放寬實際規劃查詢的斷言。

## 驗證失敗複核：瀏覽器測資前提

第二次完整 gate 的原生檢查通過；新增 E2E 等待第二個模型請求逾時，尚未到達進度斷言。trace 證明兩次備份分別匯入 160 與 4 筆，索引任務 21、22 都成功；第二次摘要是 `4 new / 0 updated`，不是預期的 160 筆更新。`autoIndexAfterBackup=true`，因此不是備份未入列或索引停住。

複核既有合成瀏覽器 helper 後確認：`appendChromeVisits` 只在新 URL 插入 title；同 URL 的新造訪只更新次數及最後造訪時間，不改 title。失敗測資的四個 URL 仍各有 41 次造訪及 Original title。案例錯把傳給 helper 的新 title 當成已保存的瀏覽器觀測，實際沒有製造任何內容更新。另對照 typed contract 確認摘要集合叫 `recentJobs`，不是案例自定義的 `jobs`。

修正方案只改本輪案例：使用 repo 既有的 `retitleChromePage` 加上新造訪，完整表示合成 Chrome 的新 title／新造訪時間；使用正式 `AiQueueStatus` 型別，避免自定義回傳 shape 掩蓋欄位錯誤。保存備份回傳及實際送出的 Updated title，讓案例能指出前提是否成立。這是已確認根因的回歸測資，不把合成瀏覽器操作當成新產品缺陷的真實環境證據；原始因果證據仍是已安裝 app 的正常任務紀錄。產品程式與通用 fixture helper 不改，亦不以延長逾時、減少更新量或跳過案例放行。完成這組修正後再跑完整 gate。

## 驗證失敗複核：跨案例測資記帳

第四次完整 gate 中，新進度案例已通過全部檢查點，其他 121 個 E2E 也通過。最後 wipe 預覽的預期為 147,510，實際為 147,675，差額正好是新增案例保留的 160＋4＋1 筆。`wipe.spec.ts` 的既有契約是從各案例的 fixture helper 計算完整測資量；本輪新增案例沒有把自己的資料加入該總數。這是本輪測資整合遺漏，尚無證據顯示刪除預覽本身有錯，不能去改產品計數。

修正方案沿用 repo 慣例：將新增案例的三組造訪資料移到共用的 `progress-scope` fixture helper，提供由這些資料算出的筆數，並納入 wipe 的預期。保留預覽的精確相等斷言，不改成容許誤差、不讀產品計數反推預期、不跳過 wipe，也不修改刪除流程。整組改完再跑完整 gate。

## 最終驗證

全部本輪修改完成後，本機完整命令退出碼為 0：

```sh
RUST_TEST_THREADS=4 CARGO_TARGET_DIR=var/playwright/desktop-bridge/cargo-target bun run check
```

- 151 個前端測試、格式、lint、三語、型別與 build 通過。
- Rust workspace、desktop bridge、clippy 與 release 檢查通過；vault-core 為 500 passed／12 個既有 ignored。
- 主 E2E 126、健康存檔完整性 9、損壞存檔修補 1，共 136 passed，無略過、失敗或 flaky。
- 新進度案例驗證正常產品入口的備份／索引操作、4 新增／160 更新、雙畫面進度、暫停後的固定範圍，以及下一次處理較晚造訪。wipe 的精確預覽與後續空存檔、舊版升級均通過。

一次受限沙箱的執行因本機測試服務 `listen EPERM 127.0.0.1` 中止；以允許啟動本機測試服務的權限重跑，未修改程式繞過限制。最終完整 gate 的來源與結果見 [verification.json](../../../artifacts/investigations/2026-11-14-semantic-progress-round1/verification.json)，包含來源檔案 hash、三組結果 hash、固定測資時間及重現命令；[檢查點](../../../artifacts/investigations/2026-11-14-semantic-progress-round1/progress-checkpoints.json) 與 [畫面](../../../artifacts/investigations/2026-11-14-semantic-progress-round1/updated-progress.png) 一併保存。

兩份依賴鎖檔及依賴宣告未變，沒有修改依賴原始碼。本輪沒有新增 14.4M 量測，也不宣稱整條更新管線已變成增量或已達到新的速度。映射、快照、命名及容量線索持續留下一輪正常流程複核。
