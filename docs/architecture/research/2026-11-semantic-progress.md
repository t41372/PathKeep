# 語意索引「644 / 94」：調查與修復設計

2026-11-13。調查來源：`e999ca83`、已安裝 app 的唯讀 owner IPC、現有索引 sidecar，以及使用者截圖。owner PID 3674 執行 `/Applications/PathKeep.app/Contents/MacOS/pathkeep-desktop --background`，該 bundle 標記為 `0.5.0-dev`；不將它誤記為已安裝公開版 0.5.0。進程與 binary hash 見 [owner-process.json](../../../artifacts/investigations/2026-11-13-semantic-progress/owner-process.json)。

**狀態：原始進度根因已複核，第一輪實作及完整 `bun run check` 已通過。其他線索另輪複核，不宣稱本輪已修復。**

## 結論與證據邊界

這次超量進度有兩個獨立原因：分母只數沒有 provenance 的造訪，分子也數更新與修補；增量續跑重新取得掃描上限，卻保留舊分母。前端再把這組不相容的數字當成百分比。另有內容映射的機制反例，但正常使用流程的觸發條件尚待複核，不能將它列為這次超量進度的根因或本輪必要修復。

從已安裝 app 的 `ai-jobs` 唯讀方法取得：

| 任務                  | 最後處理量 | 原分母 |   新增 |   更新 | 最後游標／原掃描上限 |
| --------------------- | ---------: | -----: | -----: | -----: | -------------------: |
| 467（分母與截圖一致） |      4,111 |     94 |    856 |  3,255 |     674,515／673,752 |
| 466                   |    102,368 | 86,882 | 86,882 | 15,486 |     673,659／673,658 |

任務 467 已成功結束，並非調查時仍卡在 644。最後游標是「下一筆 id」；任務 466 的多 1 是正常的 exclusive cursor，467 的多 763 不是這個原因。現有 `.pkmap` 實際有 762 筆 id 大於 673,752 的紀錄，證明資料範圍確實擴張。這些新增工作本身可以合法建立索引，問題是它們不屬於原分母的範圍。

數字也完全對得上：`4,111 = 94（原先缺列）+ 3,255（更新既有列）+ 762（原上限之後的新增造訪）`，且 `856 = 94 + 762`。不是小數精度、單位格式化或純粹多 1 的游標錯誤。

資料來源與原始數值見 [owner-jobs.json](../../../artifacts/investigations/2026-11-13-semantic-progress/owner-jobs.json)、[sidecars.json](../../../artifacts/investigations/2026-11-13-semantic-progress/sidecars.json)。沒有開啟 canonical DB、讀取金鑰或呼叫 embedding 服務；沒有修改或重啟使用者的 app。

任務摘要足以確認 3,255 筆走了更新既有 provenance 的路徑，但不記每筆更新的原因。無法從這份紀錄再區分其中多少來自標題、摘要或缺失來源修補，也無法還原截圖那一刻 644 筆的逐筆構成。這些分類不影響已重現的錯誤條件；本調查不將它們寫成已知事實。

## 原因一：分子與分母的集合不同

`ai/indexing/backfill.rs::candidate_visit_count` 的條件是：

```text
可見造訪 AND（完整重建 OR 此 provider/model/lineage 沒有 provenance 列）
```

實際執行則由 `candidates.rs::collect_visit_chunk` 比內容雜湊，再由 `run_embedding_backfill` 檢查來源是否缺向量或映射，最後套用 WorkingSet。每批保存後，`embedded_total` 加的是 `changed.len()`，包含新增、內容更新與來源修補，也包含不需新向量、只需建立映射的造訪。

因此 `embedded_so_far` 實際是「這次處理的造訪數」，不是欄位註解所說的向量數。向量、造訪、掃描 id 位置是三種不同的量。新分頁大量共用一條向量也是正常情況。

最小案例：5 筆造訪，4 筆已有 provenance；其中一筆內容改變，一筆缺映射。分母是 1，執行要處理 3。以原碼 SQL 在 SQLite 上重現了分母；這只驗邏輯條件，不冒充 Turso 的查詢計畫或效能測試。見 [reproduction.json](../../../artifacts/investigations/2026-11-13-semantic-progress/reproduction.json)。

另外兩個同源的情境：

- 只有內容更新時，分母可為 0，工作卻不為空。`0` 同時代表「未知」及「已知沒有缺列」，持久化又用它表示「保留上次的分母」，無法表達三種不同狀態。
- WorkingSet 在計數後才過濾；計數沒有它的範圍，會包含這次不處理的造訪，進度可能在低於 100% 時結束。

這個反向 join 的語意早於本次效能修復，`git blame` 可追到 2026-06 的進度改動。11 月 13 日的索引提示與分頁修復解決了查詢複雜度，沒有使計數條件等於實際工作的條件。

## 原因二：續跑保留分母，卻沒有保留工作邊界

`embed_target_for` 與 `scan_target_for` 在 `start_history_id > 0` 時回報 0，由 `persist_index_cursor` 保留原值。但 `run_embedding_backfill` 在沒有 `captured_upper` 時，每次進入都讀目前的 `MAX(visits.id)`；`BuildGeneration::upper_history_id` 只有完整重建才提供固定上限。

所以同一增量工作的下一次嘗試可能接著讀入備份新增的造訪，分母仍是第一次計數的值。初次執行的掃描上限與計數上限也來自兩次讀取，沒有同一個 manifest 約束。完整重建雖固定 lineage 上限，其首次計數也另讀目前資料，並非共用同一個上限。

更一般地說，固定最高 id 只能界定新增造訪，不能界定既有低 id 的標題或 enrichment 變化。單一 id 游標無法代表所有內容變更；它前面的內容後來改了，要由下一次可追蹤的更新補上，不能只靠「此次成功」就認定最新。

## 獨立線索：完成的批次可能沒有等於新的搜尋內容

本節保留原方法與 sidecar 的調查結果。它不構成「644 / 94」的原因；正常瀏覽器操作的端到端因果鏈尚未確認，依證據稽核另輪處理。

`DedupTracker` 將已映射狀態存在 `HashSet<history_id>`。`take_unmapped` 只在 id 首次插入集合時 append `(history_id, content_key)`，沒有比較該造訪目前指向的 key。

具體因果鏈：

1. 造訪 H 原本指向 K1。
2. 標題或摘要改變，candidate 的 key 成為 K2，產生或重用 K2 的向量。
3. H 已在集合裡，映射沒有 append K2。
4. provenance 與計數照樣更新為 K2／已處理；projection 從 `.pkmap` 重建，仍將 H 指向 K1。
5. 下一次掃描看到新雜湊、已有 H、磁碟也有 K2，三項檢查都通過，於是繼續略過。一般增量重跑不會自行修復它。

這是把「相同工作重播」與「同一造訪有新版本」混為一談。正確的冪等條件是目標映射已等於這次要寫的映射，不能只看造訪存在。

直接擷取目前 `take_unmapped` 方法，僅去掉 standalone 編譯不適用的 `pub(super)`，以 rustc 執行：已有 H=2/K1=11，更新為 K2=22，實際 append 0 筆，最終仍為 11。新造訪與同 key 重播的對照檢查通過。這是原方法的最小重現，沒有跑完整建置或假裝是 E2E。

真實 sidecar 有 464,931 筆映射，全都不同 id，沒有一次 key remap；214,062 個向量 key 裡有 2,770 個沒有被目前來源映射引用。後一個數字也可能包含歷史殘留，不能直接宣稱全是這個缺陷造成的孤立向量，更不能拿它當「受影響造訪數」。映射裡沒有缺失來源向量的 key，這項檢查不能證明內容是最新的。

`take_unmapped` 的 id-only 條件可追到 2026-06-21，不是 generation publication 新引入的。immutable generation 能保證讀者拿到同一代檔案，但無法修正那一代從錯誤來源映射投影出的內容。

## 同源表現：read model 沒有階段與有效性，UI 直接當百分比

`semantic_index.rs` 將兩個 cursor 數字直接回成 `done/total`。Settings 與 Background work 都拿它們算百分比；`useIndexPace` 在 `done >= total` 時停止提供速度／剩餘時間，沒有說明數字已矛盾。

此外，頁面回填之後還有 keyword-search 向量回填、projection、fsync 與發布。這些工作不更新同一個頁面進度。即使集合計數剛好正確，頁面處理完也不等於整個索引工作完成。

使用目前安裝的 React／Radix 依賴重現 `644 / 94`：

- Settings 傳入的值是 685，超過 Radix 的 0–100 範圍；Radix 警告並將無障礙狀態改為 indeterminate。
- indicator 的 transform 字串是 `translateX(--585%)`，不是有效的位移表示式。

見 [ui-progress.json](../../../artifacts/investigations/2026-11-13-semantic-progress/ui-progress.json)。這項驗證是 server render 與字串核對，沒有宣稱已跑桌面 CSS 計算。前端的數字平滑不會產生這個超量：`useSmoothCount` 只往後端已回報的值靠近。

## 為什麼既有測試沒攔下

- `indexing/start.rs` 的計數案例驗的是「沒有這個模型的列」，沒有放入既有內容變更，也沒有證明它等於後續實際處理量。
- `semantic-index.spec.ts` 在首次填滿時斷言一次 `done < total`；後續更新沒有逐 checkpoint 核對進度。
- 改標題的 E2E 同時替該頁加了一次新造訪。新造訪能映射到新向量，頁面聚合搜尋因而可以通過；沒有把日期限在修改前的舊造訪，無法證明舊造訪已改指向新內容。
- enrichment 的 Rust 測試只證明 candidate 變成 `needs_embedding`，沒有繼續驗證 published forward/reverse/position map 與語意搜尋。
- 多數 resume 斷言守的是不重送與 id 完整，沒有在續跑前加入超出原上限的新資料，再核對整個 manifest 與分母。

## 本輪範圍及修復設計

本輪方案以 [第一輪：索引進度與續跑範圍](2026-11-semantic-progress-round1.md) 為準。原始證據保留；映射與其他發現依 [證據稽核](2026-11-semantic-progress-audit.md) 另輪處理。

先前未提交的大範圍草案已封存並撤回。新增快取、來源變更日誌、快照保護、generation 命名及 snapshot race 不在本輪實作內。不得將草案的測試結果作為本輪驗證。

## 既有大型測試檔的改動審查

`semantic-index.spec.ts` 涵蓋初次建立、暫停、取消、重啟、備份更新與換模型。本輪不重構其流程或檔案格式 helper，只修正一項測量契約：核對進度的總數包含隱藏記錄，不能再當成最後已索引造訪數。終態改由正常完成後的實際來源映射不同 id 數核對。新增的正常備份／更新／暫停／繼續驗收放在獨立 spec，沿用既有順序與隔離環境。
