# 語意索引「644 / 94」：調查與修復設計

2026-11-13。調查來源：`e999ca83`、已安裝 app 的唯讀 owner IPC、現有索引 sidecar，以及使用者截圖。owner PID 3674 執行 `/Applications/PathKeep.app/Contents/MacOS/pathkeep-desktop --background`，該 bundle 標記為 `0.5.0-dev`；不將它誤記為已安裝公開版 0.5.0。進程與 binary hash 見 [owner-process.json](../../../artifacts/investigations/2026-11-13-semantic-progress/owner-process.json)。

**狀態：原因已重現；下文是修復提案，尚未實作或變更已接受的索引契約。**

## 結論與證據邊界

這次超量進度有兩個獨立原因：分母只數沒有 provenance 的造訪，分子也數更新與修補；增量續跑重新取得掃描上限，卻保留舊分母。前端再把這組不相容的數字當成百分比。調查另確認既有造訪的內容映射不會隨內容更新，影響搜尋結果，不能只改進度顯示。

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

## 原因三：完成的批次沒有等於新的搜尋內容

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

## 原因四：read model 沒有階段與有效性，UI 直接當百分比

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

## 建議的最終設計

### 1. 每個工作持有持久化的範圍與進度契約

新增有版本的 `IndexWorkManifest`：provider/model、lineage、scope、來源變更邊界、掃描上限、各階段的工作單位與游標。首次建立後與工作綁定，claim/retry/resume 不重新延伸它。後來的新造訪與內容變更留在下一個工作；相同來源 lineage 的身份仍需驗證。

完整建置／明確重新檢查以固定 id 範圍分頁，不為 14.4M 筆造訪複製一份逐筆任務表。掃描進度數真正檢查過的造訪，不能把稀疏 id 當筆數。若總量尚未精確得到，提供已檢查筆數與未知總量；精確總量也必須依相同 scope、邊界與可見性規則取得。來源改動可能讓工作項目不再適用，將其記為已核對後略過，不當成索引遺失。

任何可變資料都不能假稱完整資料庫快照。manifest 固定的是工作集合與覆蓋邊界；讀到比邊界更新的內容時可以使用它，但較新的變更仍須保留待處理。發布只能確認已捕捉並核對的範圍，不能順手確認當下的全域 watermark。

### 2. 備份後更新由來源變更驅動

整合 BACKLOG 的 `WORK-INDEX-UPDATE-WALKS-ARCHIVE`。新增造訪、頁面標題變更、enrichment 發布、撤銷與還原，都在來源交易內留下持久化變更。按造訪範圍／頁面表示，避免一個熱門頁面一次展開百萬筆記憶體物件。

各來源維持自己的遞增邊界；跨 Archive／Intelligence 的交接使用 durable outbox，加上可重播的來源事件 id。寫完來源後才臨時呼叫 enqueue 不是可靠交接。不能拿會被全文索引器刪掉的現有 outbox 當作另一個消費者的永久紀錄。

增量 planner 捕捉各來源的邊界，分批合併變更為封存的工作描述，只展開受影響頁面的造訪；規劃完成才宣稱精確處理總數。之後的變更進下一份描述。每個 configured index 有自己的已發布確認位置，只有 generation 與成功狀態提交後才確認；中斷不會丟掉待處理變更。來源事件可在不再有工作／消費者引用時收斂與清理，AI 關閉時也必須有容量界限；沒有連續變更證據的模型重新啟用時，由明確的完整核對補基線。

這使候選發現與 stale-row 檢查隨變更量工作，不再每次備份全庫比雜湊。現行 immutable generation 的投影仍有全量成本，這項設計不把整個更新宣稱為 O(變更量)。投影先依下方驗收量測，保留既有 publication／lease 契約；分段投影若有必要，另立架構取捨，不能偷偷在這次修復中更換。

### 3. 將向量去重與映射更新分開

向量依既有 content identity 去重；映射則比較 `current_key(history_id)` 與 `desired_key`，不同就 append。以 `.pkmap` 最後一筆為準，利用既有 last-writer-wins 格式，不修改已發布 generation。讀取狀態必須包含實際 key，不能再降成 `HashSet<history_id>`。

14.4M 時避免把整張 Rust HashMap 再加進常駐記憶體：將現有 `.pkfwd` 作為唯讀基線，當次變動使用磁碟上的有序 overlay／批次查找；續跑從持久化 checkpoint 與 append tail 恢復。以查詢計畫與 RSS 驗收具體實作，不能用全量集合掩蓋查找設計。

保存順序延續現有耐久契約：向量與映射 append → fsync → 同一個 Intelligence 交易提交 provenance、各種計數與 cursor。重播相同 desired key 不重寫；不同 key 是合法更新。發布前核對「有效 provenance 的 key = 最後來源映射的 key = generation 投影的 key，且向量存在」。可邊流式投影邊核對，不另外載入全庫。

### 4. 修正已留下的資料，避免修完後繼續錯

新程式不能只改未來的 `take_unmapped`。現有 provenance 可能已是 K2，而 `.pkmap` 還是 K1；必須有一次可續跑的本機核對與修復工作，按來源 id 分頁比對，從 provenance 的完整 hash 推得預期 key，再核對來源向量。需要核對的原因與數量先預覽，再由正常操作入口執行。

有正確 K2 向量時只補映射，不再呼叫模型；缺向量或來源身份無法信任時，分列需要重新建立的內容，走既有模型與網路授權流程，不暗中全量重送。核對期間保留舊 serving generation，修復完才發布新 generation，留下修復數量與驗證結果。

此處不改既有 dedup hash／頁面文字規則。`embeddedText = null` 是這台存檔另有的舊文字格式標記；它已由既有 stale／重建契約處理，不是 644 / 94 的成因，修映射也不能把它標成已換新文字。

舊 cursor 沒有足夠資訊還原原工作集合。遷移不得用 `max(done,total)` 補造分母，也不應清掉已完成的來源。保留 durable cursor、舊摘要與累計處理量，將舊進度明確標為總量未知；完成 manifest 綁定與必要核對後，才使用新階段的計數。

### 5. 對外進度是帶單位的階段，不是一個通用分數

read model 改用明確型別：`phase`、`unit`、`completed`、`total: null | number`、獨立的新增／更新造訪及新向量數、checkpoint revision。`null` 是未知；`0` 是已知沒有工作；不再靠 0 表示「保持舊值」。

階段對應規劃／檢查、頁面索引、搜尋詞索引、建立投影、發布。沒有可靠總量的階段顯示已完成量及階段文字，進度條為不確定；有總量時只在該階段內算比例。100% 是該階段完成，工作成功仍只來自原子的 generation 發布。不得發明各階段權重拼成總百分比。

Settings 與 Background work 使用同一個進度 DTO 與 formatter。速度按同一工作、同一階段的 durable checkpoint 計算，階段／manifest 改變時重置；完成量不能代表向量速度或跨階段 ETA。平滑動畫沿用 reduced-motion 行為，不越過真實 checkpoint。

後端驗證 `0 <= completed <= total`；若壞掉的舊資料越界，保留原始診斷值、記結構化錯誤，UI 顯示總量未知與階段，停止錯誤 ETA。傳入 Radix 的值仍需合法範圍驗證，不能以視覺 clamp 代替資料修復。en／zh-CN／zh-TW 的文字、aria 與空／錯／暫停狀態同時交付。

## 取捨

| 做法                                              | 判斷                                                                                                                 |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| 分子 clamp 到分母、或分母提升成 `max(done,total)` | 拒絕：會隱藏錯誤，不能說還剩多少，也不修復搜尋內容。                                                                 |
| 只在 COUNT 加上更多條件                           | 拒絕作為最終方案：要把內容建構、來源檔案狀態及 WorkingSet 再算一次，仍有並行變動與續跑邊界問題；每次備份仍走完整庫。 |
| 所有工作只顯示 spinner                            | 拒絕：已有可靠階段進度時應呈現；未知總量才是不確定進度。                                                             |
| 每次更新全部重新嵌入                              | 拒絕：模型成本、隱私與磁碟寫入不必要，也沒有解決錯誤的重播身份。                                                     |
| 固定 manifest、分階段計數、來源變更交接、映射核對 | 建議：工作邊界、耐久進度與搜尋內容由同一份可驗證工作描述約束。                                                       |

其中穩定工作契約、phase DTO 與變更交接是本提案，須在落地時同步修訂 `semantic-index-control.md` 與 schema／功能文件；本調查沒有先將它們寫成已接受或已交付。既有「使用者要真實百分比」的要求仍成立：顯示有證據的階段百分比，不保留不成立的分數。

## 實作前寫定的驗收失敗模式

主驗收是一條真後端 E2E：先建完 → 只改既有頁面的標題／enrichment，不加新造訪 → 再加少量新頁 → 更新一半暫停 → 備份加入更多頁及改動游標前的舊頁 → 重啟續跑 → 發布 → 下一個更新處理較晚事件。

全程保存每個 durable checkpoint、manifest／phase、provider 收到的文件、generation pointer，以及語意查詢結果。以人可核對的 JSON、Playwright trace 與畫面作結，不能只留 pass 計數。

必須證明：

1. 每個階段的單位與工作集合一致，`done <= total`；全是既有內容更新、全是重複造訪、只有來源修補、WorkingSet、零工作、未知總量都成立。
2. 續跑不伸長舊 manifest；較晚新增／更新／撤銷事件不丟失，下一次確實處理。稀疏 id 和 exclusive cursor 不被誤當筆數。
3. 把搜尋日期限在修改前的舊造訪，只有新內容的語意才能找到它，且 `matchReason` 包含 semantic。檢查 forward/reverse/position 三份投影，不靠新造訪或全文搜尋替舊映射遮錯。
4. 對已經 provenance=K2、map=K1 的舊狀態，核對工作能補 map，已有向量不重送；修復再次執行是冪等。缺向量不得假裝修好。
5. 在來源提交／outbox 交接／向量 append／map append／fsync／provenance+cursor 交易／pointer 發布之間逐點 crash，重啟後不漏內容、不重算已耐久的相同工作、不混代、不越過停止請求。
6. 頁面完成後刻意阻住搜尋詞或 projection，UI 正確顯示所在階段，沒有「已完成」或假的剩餘時間。Settings、Background work、aria 都一致；超量舊資料不產生非法 Radix 值。
7. AI 關閉、模型切換、來源缺失、事件保留／清理、舊 cursor、舊文字格式與 legacy lineage 都保留既有 consent、partial serving 與 generation lease 契約。

必要的 isolated Rust 測試先驗上述 crash 與冪等失敗模式；E2E 負責整體功能，不再只驗 candidate 或首次填滿。這份調查的 predicate／純方法重現不替代它們。

14.4M／4 核／8GB 驗收分開量候選發現、受影響造訪展開、key 查找、投影、fsync 與發布：記錄掃描列數、峰值 RSS、讀寫量及各階段延遲。新資料很少時不得走完整候選庫，投影的現有全量成本單獨揭露；UI 以真後端更新期間的長任務／互動 profile 驗證。這一輪沒有新增效能數據，也沒有重跑整個大型存檔 gate。

實作完成後須通過 `bun run check`，並保存中等以上難度的上述 E2E artifact；不能以本次唯讀調查或舊版本 gate 宣稱修復完成。
