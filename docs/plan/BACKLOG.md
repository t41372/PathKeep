# BACKLOG — 後續工作塊佇列

> 這裡只放 **half-milestone 粒度**的 work blocks。  
> `STATUS.md` 清空或完成當前 block 後，才從這裡把下一個未被阻塞的 block 搬上去。

---

## 工作塊佇列（按順序）

> 2026-10-25：清掉已完成或前提已不存在的 block（v0.3 Paper 前端、已移除的 coverage / mutation gate、已修好的 flaky 測試等，清單見 CHANGELOG 同日）。2026-04 到 2026-06 的收尾註記也一併移除，要看去 git 歷史或 CHANGELOG。

- [ ] **WORK-AH-APP-SURFACE** — 把 Agent 對話接進 app：指令、每種資料各自的備份、進度與取消、背景任務、設定、排程與選單列（v0.6.0；接在 `WORK-VAULT-STORE-ROLES` 與 `WORK-AH-READ-MODEL` 之後）
  - 讀先：`docs/features/data-spaces.md`、`docs/features/agent-history.md`；`docs/architecture/desktop-command-surface.md`、`ipc-performance.md`、`owner-process.md`。
  - 啟用流程的預覽（PME）、每個空間的「立即備份」與「全部立即備份」、排程與選單列的備份涵蓋所有開著的資料類型、結束前詢問認得新的工作種類。
  - 背景任務要一個共用的讀法：現在備份、匯入、維護、匯出、佇列、索引是五套各自的機制。這一塊同時解掉 `WORK-BACKGROUND-DASHBOARD`：設定裡的工作清單搬到背景任務頁，設定只留排程與限制（「設定 › 排程與背景工作」），每個空間的備份頁留一列「自動備份」連過去。
  - 每個新指令照現有的七個地方接好，dev bridge 一起；`WORK-COMMAND-CONTRACT-PARITY` 的比對測試先做，免得漏接。

- [ ] **WORK-AH-SCREENS** — Agent 空間的畫面：洞察、紀錄與閱讀器、專案、備份、設定、啟用流程、背景任務頁（v0.6.0；接在 `WORK-SPACES-SHELL` 與 `WORK-AH-APP-SURFACE` 之後）
  - 用戶 2026-11-14：v0.6.0 有了每個空間的顏色之後，預設要是不透明、顏色比較鮮明的樣子，接近設計稿的背景，不要半透明。視窗材質現在的預設已經是不透明（毛玻璃要自己在「設定 › 一般 › 視窗」開）；這一塊做 Agent 空間的色相時，把瀏覽紀錄空間的底色也拿出來一起看夠不夠鮮明，並確認毛玻璃下每個空間還分得出來。
  - 動工前先讀 `data-spaces.md` §0。搬位置的畫面搬現有的元件，不照設計稿重畫；每搬一個畫面，先從現在的元件和它的 E2E 列出所有控制項、狀態、規則，做完逐項對過。
  - 也在這一塊：設定側欄的「資料類型」分組與設定項目的搬移（`data-spaces.md` §8）、新手引導的「要保存什麼」與「來源」兩步（§3.3）、切換選單的「保存其他類型的資料」、導航列指示器的「打開背景任務」改到 `#/tasks`。這些是 `WORK-SPACES-SHELL` 留下的，用戶 2026-11-13 都同意了。
  - 讀先：`docs/features/agent-history.md` 第一部分、`docs/features/data-spaces.md`、`docs/design/data-spaces-tradeoff.md`；`docs/design/` 核心四份。
  - 版面與文案從設計原型搬進 `src/`，資料改成透過 typed client 向後端要：清單虛擬化、頁面先出骨架、重的計算不在主執行緒。每個畫面的空、載入、錯誤狀態照規格補齊，三語一起交付。
  - 示範資料與 E2E 用的假資料改成各工具原生格式的合成檔，走真的備份流程（`dev:demo` 與 E2E 都用它）。每個畫面至少一個中等以上難度的 E2E。
  - 收工條件：`docs/design/prototype-2026-11-agent-history/` 連同 `.prettierignore`、`eslint.config.js` 裡的排除項刪掉，這個分支才進 `main`。
  - 還原對話框（設定 › 儲存空間、存檔打不開時的恢復畫面）的說明要改：現在寫「存檔會回到副本保存時的狀態；密碼和加密方式維持現在的樣子」，有了 Agent 對話之後要加「Agent 對話保持現在的樣子」（用戶 2026-11-15 的決定），三語一起改。這一版還沒有存對話的介面，所以現在的文字仍然正確。
  - 還沒決定、動手前先問用戶：匯入搬家檔蓋過一份已經有 Agent 對話的存檔時，對話要整份換成搬家檔裡的（現在的做法，換下的在「匯入之前」那份安全副本裡），還是跟還原一樣留著。問的時候一併說明：照現在的做法，還原「匯入之前」那份副本只拿回瀏覽紀錄，原本的對話不會回來（還原不碰對話），要靠「從副本修復 Agent 對話」。

- [ ] **WORK-AH-REVIEW-FOLLOWUPS** — Agent 對話核心庫修正之後那一次審查留下的事（2026-11-13；審查沒找到會丟失或毀損已存資料的路徑）
  - 用戶 2026-11-13：跟 `WORK-AH-READ-MODEL` 一起做，不另外開一塊。那一塊開工時把這裡的項目併進去；「一個壞檔停掉整個存檔」那一項在讀取模型接上清單和搜尋之前修好。
  - 讀過程式碼確認的兩項：
    - 一條接續鏈繞成圈時（`query/line.rs` 回 `native lineage cycle`），錯誤一路傳到 `append_lineage_issues` 和 `normalization_complete`，整個存檔的備份、搜尋、`verify` 都會失敗。一個壞檔不該停掉整個存檔：那段對話標成不完整並回報，其他照常。先寫出能造出圈的合成檔（兩個 rollout 互指、邊界都是 0）。
    - `archive/comparison.rs` 用索引直接取切片清單，清單比版本短時會 panic，應該回報存檔損毀。
  - 還沒驗證、動手前先重現的：
    - 三個來源的對話裡，一個「複本」長大後改成另一個來源的前綴時，快速路徑只跟原本那一個比，可能把它當成獨立來源，甚至選成目前的那一份。
    - 重新解析時，目標已經依保存規則過期的附件被記成 `authority_unavailable`。
    - `mark` 在主要的寫入成功、鏡像失敗時回報失敗；備份在回收切片失敗時也一樣，其實都已經寫進去了。
    - 接續邊界的快取鍵沒有包含上一份繼承了多少筆。
  - 已知、這一塊沒要改的成本：檔案有變動時整份重讀重算雜湊（保存契約要求）；一直在長的檔案連續三次沒讀穩，這一次就不產生新版本。讀取模型那一塊量大檔時一起看。

- [ ] **WORK-AH-REPAIR-FROM-COPY** — 從安全副本修復 Agent 對話（v0.6.0 發布前；用戶 2026-11-15）
  - 用戶 2026-11-15 決定：還原安全副本只退回瀏覽紀錄，Agent 對話永遠不被還原退回；副本裡的對話只給一個另外、明確的「從副本修復 Agent 對話」動作用，在對話的資料庫本身壞掉時。還原這一半已經做好（`archive-compatibility.md`「What an installed archive is stamped with」）。
  - 為什麼發布前要有：存檔打開時七個資料庫一起開，對話的兩個資料庫有一個不見或讀不出來，整份存檔就拒絕打開，連瀏覽紀錄也看不到；還原又不碰對話，所以救不回來，使用者只能自己動資料夾。
  - 發布前兩件都要做到（2026-11-15 side agent 提醒後明確列出）：
    - 對話的資料庫壞掉時，瀏覽紀錄照樣打開，Agent 空間顯示「對話資料讀不出來」與修復的入口（`agent-history-product-design.md` 的原則：一種資料出事不拖垮另一種）。現在 `Vault::open` 一次開全部資料庫，要改成依資料種類分開開、分開回報。
    - 「從副本修復 Agent 對話」這個動作本身（做法見下一點）。
  - 動作本身走 PME：預覽寫出副本是哪一天的、會退回多少段對話、目前的對話放到哪裡（變成一份安全副本，不刪）。副本的金鑰和存檔不同時，跟還原一樣轉成存檔的金鑰。

- [ ] **WORK-AH-PERMANENT-DELETE** — 從存檔永久刪除單段 Agent 對話（用戶 2026-11-13 決定 v0.6.0 先不做，用封存和垃圾桶）
  - 現在垃圾桶裡的對話還在存檔裡；存檔不加密時是明文。要真的刪掉只能刪掉整個「Agent 對話」的存檔。
  - 為什麼不是刪幾列就好，寫在 `docs/architecture/agent-history-product-design.md` §4：資料庫要整個複製重寫、索引分段要合併、舊的安全副本要整份重做、工具還留著檔案時下次備份不能存回來、工具的共用檔（輸入歷史、索引、狀態資料庫）裡的片段、從它接續出去的對話。
  - 開工前先量：這台機器大小的存檔重寫一次要多久、寫多少；再決定是每次刪除都重寫，還是累積到清空垃圾桶時重寫一次。

- [ ] **WORK-AH-MORE-TOOLS** — Continue、Cline、Roo Code 開給產品（用戶 2026-11-13 決定 v0.6.0 先不做）
  - 核心庫讀得懂 Continue 的 session 檔，和 Cline、Roo 的「經典」存法（每個任務一個資料夾）。Cline 新版改用另一套儲存，現在的支援不涵蓋；三個工具在開發機上都沒有真實檔案可以對過。
  - 要做的：各自的預設位置（要有工具自己的原始碼或文件當證據，不能猜）、新版儲存的研究、拿真實檔案驗過、整份改寫的 JSON 檔的保留規則（被下一版完整包含的版本只留紀錄）。

- [ ] **WORK-AH-BULK-ACTIONS** — 一次封存或刪除多段對話（例如整個專案、某一天之前的全部）（2026-11-13 做封存與刪除時留下，v0.6.0 之後）
  - v0.6.0 只有一次一段。多選、整個專案、依條件的刪除都要各自的預覽（會刪掉幾段、多少版本、多大）。

- [ ] **WORK-BACKUP-INTEGRITY-SCALE** — 原生備份的新身分與讀法在 14.4M 和目標機上的量測（2026-11-01 PR #18 只量了 100 萬筆、M5 Max）
  - 要量：14.4M 的 v1 存檔更新（migration 2–21，含開啟者 backfill 與重建列號索引）的時間與峰值記憶體；更新後第一次備份（整個重讀、給每筆時間）；之後每小時與每週的備份。目標機 4 核 / 8 GB，冷快取。
  - 另外量一份 Sync 從新到舊寫的大檔（開啟者大多在子造訪之後）：讀完才找到的開啟者分批寫，量它的時間。
  - 工具：`archive::ingest::scale_measure`（`docs/architecture/research/2026-11-backup-integrity-1m/README.md`）。

- [ ] **WORK-INSIGHTS-LATE-OPENERS** — Insights 已經建過的造訪後來才有開啟者，站點流向與路徑要等下一次完整重建才看到（2026-11-01 PR #18 第五輪審查）
  - 讀先：`docs/architecture/backup-integrity.md` §Openers、`vault-core/src/intelligence/build_pipeline.rs`（`count == 0` 與 append 的判斷）、`build_append.rs`（只讀 `visit_id > old max`）、`build_rows.rs` 讀 `parent_visit_id` 的地方。
  - 現象：PR #18 讓備份可以更新已存的造訪：時長（查詢時才讀，不受影響）、開啟者（`parent_visit_id`，只在原本沒有時填上）。Navigation 跟著連結走，馬上對；Insights 的站點流向、trail、導航鏈深度是建置時算好的，append 只建比上次大的 id，`backup_changed_archive` 也只看新造訪，所以後來才填上的開啟者要等下一次完整重建（規則或階段版本變了、手動重建）。會碰到的：Sync 分批從新到舊寫，一批的子造訪先備份、它的開啟者下一批才進檔案；備份在寫開啟者之前被打斷（下一次備份會補上連結）。
  - 不採用：每次這樣就整個重建。`ipc-performance.md` 的完整建置在 14.4M 上是 40–100 分鐘（伺服器 CPU），目標機更久；為了少數連結這樣做不行。
  - 方向：備份在同一個交易裡記下「哪些已建過的造訪換了開啟者」（造訪 id，不是一個計數），建置只重算那些造訪所在的 session／trail 與它們加減的站點流向。先量 Sync 使用者一週會有多少這種造訪，再決定做法。

- [ ] **WORK-URL-TITLE-REFRESH** — 最後造訪時間沒變時，頁面標題不會更新（2026-11-01 PR #18 第五輪審查；規則早於 PR #18）
  - 讀先：`vault-core/src/archive/ingest/writes.rs` 的 `upsert_url`（`url.last_visit_ms > last` 才更新）、`browser-history-parser/src/url_pages.rs` 的 URL 讀取條件。
  - 現象：Chrome 在造訪開始時記下時間，標題載完才寫（單頁應用會改好幾次）；那之間跑的備份存到舊標題，之後的備份看到同一個最後造訪時間就不更新，要等下一次造訪那一頁。
  - 方向：不是直接改成 `>=`：重播舊的 checkpoint 也是同一個時間，會把舊標題寫回來。要分清楚「這次讀的是瀏覽器現在的檔案」與「重播」：前者在時間相同時也可以更新標題，後者不行（跟時長「不往回拿」同一個規則）。

- [ ] **WORK-WIPE-WAITS-FOR-BACKUP** — 刪除所有資料不等正在跑的備份（2026-11-01 PR #18 第五輪審查時看到；早於 PR #18）
  - 讀先：`vault-worker/src/data_wipe.rs`（`stop_background_work`）、`vault-core/src/archive/wipe.rs` 的檔頭（說「進行中的先做完」）、`Vault::begin_backup`。
  - 現象：刪除只停並等背景佇列與連結預覽，正在跑的備份照跑；存檔關掉之後它的寫入都失敗（`Closed`），不會把資料寫回來，但那次備份會以錯誤結束，檔頭的說法也不對。
  - 方向：刪除取得備份那把鎖（或讓備份走 `ArchiveOpGate`），等它結束或請它停；檔頭照實寫。

- [ ] **WORK-STAGING-BUSY-WAIT** — 瀏覽器開著時，每個 profile 每次備份先空等 10 秒（2026-11-01 PR #18 第五輪審查時從代碼與瀏覽器原始碼找到，本機用 SQLite 重現了鎖，沒在真的瀏覽器上量；早於 PR #18）
  - 現象：Chrome 與桌面版 Firefox 執行時一直鎖著歷史檔，線上備份每次都等滿 5 秒的 busy timeout 才改成直接複製，History 和 Favicons 各一次。Safari 不鎖，等待有用。
  - 方向：鎖是瀏覽器整段執行期間都在的（排他鎖），不是短暫的寫入；先用很短的 timeout 試一次，拿不到就直接複製，只有 Safari 這類一般鎖定的檔案才等。改之前先在真的 Chrome／Firefox 開著時量一次。

- [ ] **WORK-INSIGHTS-COLD-READS** — 目標機大小的機器上，Insights 有卡片第一次讀要等一分鐘以上（2026-10-25 14.4M 量測找到；和「目標機上流暢」的硬指標衝突，下一輪先做）
  - 讀先：`docs/architecture/ipc-performance.md` §11（各讀取的量法與 2026-10-09 的表）與 §16 最後一段、`vault-worker/tests/archive_scale_bench/turso_intelligence.rs`。
  - 現象（Modal microVM 4 vCPU / 8 GB，release、加密，14.4M 造訪，預設形狀；資料最大 22.5 GB）：app 剛起來的第一次讀，interrupted habits 30 天 66 s、一年 68 s，全部時間的 digest 單獨讀 67 s，一年的第一批卡片 29.5 s；同一個讀取第二次 1 s 以內到 0.5 s。
  - 對照：2026-10-09 在 Modal EPYC VM、`habits` 形狀上，interrupted habits 第一次 2.7 s、全部時間 digest 2.9 s；形狀和機器都不同，不是同條件比較。一年的第一批在預設形狀上當時是 39.1 s，所以那一條不是新退步，而是一直都慢。
  - 猜測（沒驗證）：資料進不了 8 GB 的頁快取，第一次讀要從磁碟拿；Modal 的磁碟和真機不同，實際可能沒這麼慢，也可能更慢。
  - 「最常回來搜的」（`get_search_effectiveness` 的 hardest topics，`2351ba22` 起跨瀏覽器與引擎合併同一句搜尋）每一頁仍讀完範圍內所有 query family，跟合併之前一樣（舊的 SQL 排序也讀完範圍）；2026-10-29 本機 debug build 同一份資料比較：10 萬個 family 新讀法每頁 0.3 s、舊 SQL 0.16 s，30 萬個新讀法 1.4–1.6 s、舊 SQL 2.3–3.0 s。這次 14.4M 也量它的第一頁與下一頁；太慢就在建置時存好合併用的 key，讓 SQL 分組、只排前 k 個。
  - 要做：(1) 先量每條讀了多少頁、多少列（同一台小機器，預設與 `habits` 兩種形狀各一次），分清是讀太多還是磁碟慢；(2) 讀太多就改讀法（例如 interrupted habits 在建置時算好存起來、all-time digest 走快照）；(3) 不管多快，卡片等的時候都要有 skeleton 與漸進載入，不能讓整頁空著；(4) 結果寫回 `ipc-performance.md`。

- [ ] **WORK-SNAPSHOT-WRITE-ORDER** — 快取的 app snapshot 被較舊的讀取蓋掉（2026-11-01 PR #16 最後一次審查找到）
  - 讀先：`src/lib/queries/app.ts`（`useSnapshot`，`staleTime: Infinity`）、`src/app/backup-runner.tsx` 的 `refreshAfterArchiveChange`、`src/lib/query.ts`。
  - 現象：十幾處（備份、匯入、索引工作事件、設定的各個對話框、onboarding）各自 `getSnapshot` 再 `setQueryData(queryKeys.snapshot, …)`，不管先後。兩個讀取同時在路上、先發的後回來，就把新的 snapshot 換回舊的；snapshot 不會自己過期，畫面上的存檔狀態要等下一次有東西改了存檔才對。存檔本身不受影響。
  - 2026-11-01 匯入改成回覆後就放開、重新整理放在後面（`fix(import): take the next file while the pages refresh after an import`），連續兩次匯入也會讓兩次重新整理重疊；之前要等前一次整理完，這條路碰不到，其他來源一直都會。
  - 方向：所有寫入走同一個地方，丟掉比已寫入的更舊的回答（請求開始時取號，或後端給 snapshot 一個遞增的版本）。不要只在 `refreshAfterArchiveChange` 裡擋：其他寫入照樣會蓋。

- [ ] **WORK-INSIGHTS-TITLE-QUERY** — Insights 開到 History 的連結把頁面標題原樣當 `q`，標題以 `Note:`、`Title:`、`site:` 這類字開頭時就變成篩選（2026-11-02 從 `WORK-SEARCH-QUOTED-OPERATORS` 分出來）
  - 讀先：Insights 組 History 連結的地方（`src/features/insights/` 裡帶 `q` 的 `Link`）、`docs/features/recall.md` §搜尋與篩選的引號規則。
  - 現象：整個 token 加引號已經是字面片語（2026-11-02），但這些連結不加引號，標題裡的字照查詢語法讀。
  - 方向：先決定這個連結要的是「這個標題的頁面」還是「跟這些字相關的頁面」：前者用片語或直接帶頁面，後者把看起來像運算子的字加引號（`words-query.ts` 的 `word()` 已經這樣做）。

- [ ] **WORK-SEMANTIC-INDEX-CLEAR-ENTRY** — app 裡沒有清除語意索引的入口：語意搜尋關掉之後，索引的檔案（14.4M 造訪時 4–59 GB）只有「刪除所有資料」刪得掉（2026-11-08 PR #16 第十輪審查時發現，2026-10-04 前端改版起就是這樣，不是 #16 帶進來的）
  - 讀先：`docs/features/intelligence.md`（semantic index 的三種明確操作：incremental catch-up、full rebuild、clear-only）、`docs/architecture/semantic-index-control.md`（清除的提交點與 GC）、`src/features/ai-setup/use-semantic-index.ts`、`src/features/settings/storage-section.tsx`。
  - 現象：舊前端的 Clear index（`onClearIndex`）在 2026-10-04 換成 shadcn 版時沒有帶過來，之後沒有任何地方送 `clearOnly: true`；後端的清除還在，Settings → 背景工作也還認得「清除搜尋索引」這種工作。關掉語意搜尋只改設定。移除嵌入服務也只改設定，它的索引要等下一次索引工作開始（`forget_unconfigured_indexes`）才刪，語意搜尋關著就沒有那次工作；內建模型的索引連這條路都沒有，只能換模型並建完新的。Storage 的「釋放空間」只列快照、匯出、暫存、隔離區。
  - 功能文件仍要求 clear-only 與清除入口，看起來不是刻意拿掉的；要不要加回來、放在哪裡由用戶決定。
  - 方向（待用戶決定）：Settings → AI 的「重建搜尋索引」旁，或 Storage「釋放空間」多一個類別；走 Preview → 確認 → 執行，先說會刪多少、之後語意搜尋要重建才能用。清除後的狀態已是 `empty`（2026-11-08 修過，不再說成要重建的損壞索引）。E2E：建好索引、清除、檔案與 Storage 的大小都不見、Settings 說還沒有建立任何索引。

- [ ] **WORK-SEMANTIC-REMAP-CHANGED-CONTENT** — 造訪的內容變了、重新 embedding 之後，語意搜尋仍拿舊內容比對（2026-11-03 PR #16 第七輪審查找到；main 就有）
  - 讀碼看到的：增量建置從頭掃，內容 hash 變了的造訪會重新 embedding、provenance 換成新的 key；但 `indexing/backfill.rs` 的 `DedupTracker::take_unmapped` 只看 `history_id` 有沒有映射過，已有的不再 append，`.pkmap` 仍是舊 key，新 generation 的投影從 `.pkmap` 建（`read_all` 是同一 `history_id` 後寫的算數）。
  - 要先做：寫測試確認。同一次造訪改 enrichment summary 再增量建置，看 forward 映射是不是換成新 key、舊 key 的 reverse 成員有沒有留著；再看中途當掉、續跑會不會把舊映射帶回來。
  - 方向：內容 key 變了的造訪 append 一筆新映射（`read_all` 本來就是後寫的算數），去重只擋同一個 key 的重複；不要只是拿掉去重，續跑會重複映射。

- [ ] **WORK-SAME-VISIT-UNDONE-STAND-IN** — 兩個匯入記了同一次造訪、都復原（代表所在的那個最後），先恢復另一個時，它的副本仍藏在那筆看不到的代表後面，那次造訪不算，要等另一個匯入也恢復（2026-11-02 PR #18 第六輪審查找到；早於 PR #18，2026-10-09 的跨來源同一次造訪就是這樣）
  - 讀先：`docs/architecture/same-visit-across-sources.md` §3、`archive/same_visit.rs`（`promote`、`link`、`FOLLOW_SQL`）、`takeout/visibility.rs` 的 `chunk`、`dedup_scenarios_baselines/firefox_sync.rs` 的復原／恢復場景。
  - 試過、不行：恢復時讓副本頂上、並把被復原的代表指向它。跨來源「一秒內算同一次」會遞移地合併組，這個指向之後被帶到同一個瀏覽器的另一次造訪或另一個設定檔的造訪，兩次不同的造訪恢復後只算一次；PR #18 第六輪連四輪讀碼都找到新的順序。
  - 方向：不靠跟著組走的指向。例如恢復時只用「這筆副本當初是跟誰配的」（寫入時另存、不跟著組換），代表已復原時它頂上、那筆回來時照順序比；先把一秒規則能不能遞移、要不要限制在同一對之內想清楚。每個順序寫成場景，每一步檢查各匯入的可見數、總數與每個來源的數。

- [ ] **WORK-WINDOWS-RAW-COPY-CHECK** — Windows 上，瀏覽器開著時的直接複製可能收下一份複製途中被寫過的檔案（2026-11-03 PR #18 第七輪審查指出；早於 PR #18：之前連這個檢查都沒有）
  - 根因：`chrome/staging.rs` 的 `copy_while_still` 用複製前後的大小、修改時間、建立時間判斷「期間沒人寫」。macOS 與 Linux 每次寫入都立刻更新修改時間；Windows 的 `WriteFile` 文件說寫入用的 handle 關掉之前 last-write time 不一定更新，而 Chrome 一直開著它。大小不變的原地寫入（rollback journal 模式最常見）於是看不出來。
  - 要先做：在 Windows 上量。Chrome 開著、持續寫入時連續複製，看修改時間何時變；用能判斷交易是否完整的資料（例如每筆交易同時改兩張表的計數）檢查收下的複製。只測 append 的假資料測不到。
  - 方向（量過再選）：Windows 上換一種接受條件，例如兩次複製逐位元組比對（每次備份多讀一兩次檔案，要量成本）；或拿不到一致的複製時照實回報這個來源這次讀不到。不要無限重試，也不要沒量過就讓所有平台多複製。

- [ ] **WORK-FLAKY-BLOCK-CACHE-RSS** — `vault-crypto` 的 `the_block_cache_bounds_memory` 量常駐記憶體（RSS），記憶體緊時不準（2026-10-25 gate 連兩次失敗，同一晚 03:31 的整套 Rust 測試過了；vault-crypto 當晚沒改）
  - 現象：上限 2 MiB 的快取 RSS 漲 8.3 / 6.9 MiB，沒有上限的漲 11.1 / 4.9 MiB（它的快取最多 9.9 MiB），斷言要前者比後者少 4 MiB 以上。
  - 根因：測試的索引只有 16.6 MiB，兩種快取的差距本來就只比門檻大一點；macOS 記憶體緊時會壓縮頁面，RSS 不跟著快取長（那時另一個 session 在跑 benchmark，壓縮器裡有約 126 萬頁）。
  - 方向：斷言快取自己回報的用量（child 已經印出 `max_cache_mib`），RSS 只當上限的佐證；或把索引做大到差距遠超雜訊。不要只是放寬門檻。
  - 2026-10-25 16:13 Modal 上閒著的 Linux 跑過了，和「記憶體緊時才失敗」一致；測試本身的弱點還在。

- [ ] **WORK-DOCS-DRIFT-2026-11** — 架構文件與程式不一致的地方（2026-11-13 盤點 v0.6 要接的地方時讀程式發現，都早於這次工作）
  - `docs/architecture/module-boundary-map.md` 列 5 個 crate，workspace 有 11 個；它列的點號指令名（`archive.backup.preview` 等）不存在。
  - `docs/architecture/tech-stack.md` 還在講 SQLite / SQLCipher / FTS5 與 `.sqlite` 檔，程式是 Turso 加 Tantivy；它說助理的對話不在搬家包裡，程式有匯出。
  - `vault-store/README.md` 寫 cipher 是 `aegis256`、Turso 0.8.1；程式是 `aes256gcm`、0.8.3。`store-schema.md` 每個資料庫都寫 version 1，Archive 已到 23、Intelligence 到 2。
  - `docs/architecture/owner-process.md` 的允許清單少了 `ai-jobs`、`intelligence-jobs`。`vault-worker/src/cli.rs` 預設指令是 `snapshot`，自己的 match 卻不接受。
  - AGENTS.md 說檔案行數的硬門檻在 `repo-baseline.md`，那裡只有「1000 行以上列冊」一個數字，TS 與 Rust 沒有各自的上限，也沒有工具在檢查。要嘛補上數字和檢查，要嘛改 AGENTS.md 的說法。
  - `quality-matrix.md` 的 E2E project 清單少了 `empty`。

- [ ] **WORK-COMMAND-CONTRACT-PARITY** — 後端指令要手動寫在七個地方，沒有測試確認它們一致（2026-11-13 盤點時發現）
  - 一個指令：`vault-worker` 的函式、`worker_bridge` 的 `*_impl`、`commands/` 的 Tauri command、`generate_handler!` 清單、dev bridge 的 dispatch 與 payload、TS client、TS 型別。E2E 走 dev bridge，所以真正的 Tauri invoke 那一段（command 與 handler 清單）沒有任何自動檢查。
  - 現在兩邊各 192 個、名稱一致（用腳本比過）。先加一個測試比對 handler 清單與 dispatch 表；要不要用程式產生其中幾份另外評估（會是新依賴）。

- [ ] **WORK-PROTOTYPE-2026-10-REMOVE** — 刪掉 `docs/design/prototype-2026-10/`，文件不再指向它（先問用戶）
  - 用戶 2026-11-13 說原型不留在倉庫裡、文件也不該指向原型。那句話是針對 v0.6 的原型說的；2026-10 的原型已經做進前端，照同一個原則也該刪，但這是我的推論，動手前先確認。
  - 指向它的地方：`screens-and-nav.md`、`ux-principles.md`、`design-tokens.md`、`ui-review-guardrails.md`（截圖路徑）、`docs/design/handoff/README.md`、`src/index.css` 的註解、`.prettierignore`。

- [ ] **WORK-BACKGROUND-DASHBOARD** — 後台任務做成 dashboard，不是設置頁（用戶 2026-10-15，說之後再做）
  - 現況：Settings → 後台任務是一列列設置卡片加工作清單。它其實是給人看「現在在跑什麼、跑多快、哪裡卡住」的地方：正在跑的工作、進度與速度、排隊的、失敗的與原因、可以暫停 / 繼續 / 取消 / 重試。
  - `WORK-AI-INDEX-CONTROL` 已做完（2026-10-15）：暫停 / 繼續 / 取消、會動的進度與速度、剩餘時間、工作結束的事件都有了，這頁可以直接用（`use-index-pace.ts`、`use-background-work.ts` 的工作動作、分頁的 `JobList`）。先讀 `docs/design/` 核心四份再設計。

- [ ] **WORK-INDEX-SEARCH-WHILE-BUILDING** — 換模型時，新索引一邊建一邊就能搜；建置由新到舊（2026-10-16 寫內建 transformer 的 trade-off 時提出，用戶同日同意）
  - 現況：換模型（例如從內建靜態模型換成 LM Studio 跑的 Qwen3-Embedding）後，搜尋一直用舊的完整索引，直到新索引整份建完；14.4M 造訪、外部服務時可能好幾天都用不到新模型。建置照 history ID 由小到大走，最常被搜的近期頁面最後才進新索引。
  - 提案（[inapp-transformer-embedding-tradeoff.md](../architecture/inapp-transformer-embedding-tradeoff.md)「Searching while it builds」「Build order」）：建置中的索引與完整的舊索引一起搜，兩份語意結果和全文結果以 RRF 融合，不同模型的分數不互相比較；建置改成由新到舊，cursor 仍是一條水位線、往下走。代價：建置期間每次搜尋要用兩個模型各算一次查詢向量，外部服務多一次請求。
  - 推翻 `semantic-index-control.md` 與 `intelligence.md` §1（未完成的索引只在沒有完整索引時才搜）：用戶 2026-10-16 同意。做的時候改這兩份文檔。和 `WORK-INDEX-REBUILD-SWAP` 一起設計：同一個模型重建時也是「兩份索引、一邊建一邊搜」，查詢向量只要算一次；等 REBUILD-SWAP 合進 main 再開工（2026-10-25 在做）。
  - 不做：外部服務不在時退回靜態模型、兩份索引永久並存（用戶 2026-10-16：不要）。

- [ ] **WORK-INDEX-ROW-CLEANUP-OFF-JOB** — 完整重建成功後，刪舊 lineage 的 provenance 列在工作執行緒上同步跑，14.4M 時約 28k 個寫入交易（2026-10-30 PR #16 第三輪審查找到，讀程式碼確認，沒量）
  - 現況：`indexing/backfill.rs` 結尾的 `generations::collect(vault, true)` 在 `INDEX_EXECUTION` 裡刪完舊列才返回，所以 `pathkeep://ai-index-job-ended` 要等刪完才發，排隊的下一個工作也跟著等。搜尋分組的暖機已經移到刪除之前（同日）。如果刪除改由背景 reaper 做，下一個工作開頭的 `collect` 會在 `admit()` 等它，停在 0%。
  - 方向（先量 14.4M 刪一個 lineage 要多久）：工作結束只做決定與改名，列的刪除交給 reaper；工作開頭的清理在已有 collector 時不等它（它本來就在做同一件事），但要先確認 `recover_pending` 等開頭清理不是建置的前提。

- [ ] **WORK-INDEX-RESOLVE-RETRY-LIMIT** — 讀語意索引時，指標在讀的過程中連換四次就當作沒有索引（2026-11-01 PR #16 第四輪審查找到，讀程式碼確認，現在碰不到）
  - 現況：`generations::resolve` 釘住指標指的那一代，再讀一次指標確認沒換；換了就重來，最多四次，之後回 `None`：搜尋只比對字詞，建置當成沒有索引。
  - 為什麼現在碰不到：同一個 provider / model 的指標只有一個寫入者在動（佇列一次只跑一個索引工作；開啟時的部分復原等那個工作的 writer），一次發布之間隔著至少一個 embedding 塊，而 resolve 一輪只讀兩次指標。
  - 要做的時候（例如部分發布變得比一輪 resolve 還頻繁）：改成等發布的通知再讀，不設次數，跟等 GC 保留時一樣；測試用 hook 在每次讀指標之間發布一次。

- [ ] **WORK-INTELLIGENCE-MIGRATION-OPEN** — Intelligence 資料庫的 schema 遷移失敗（例如磁碟滿）會讓整個存檔打不開，雖然語意索引是可選的衍生資料（2026-10-30 PR #16 第三輪審查找到）
  - 現況：vault-store 開檔時依序跑每個資料庫的遷移，任何一個失敗 `Store::open` 就失敗。v2 遷移已改成每批搬移，峰值只多一批，但磁碟幾乎滿時仍可能失敗，而且每次啟動都在同一點失敗，直到用戶騰出空間。
  - 要決定（寫 trade-off 再改）：Intelligence 遷移失敗時是否先開 archive、把 AI 功能標成不可用並說明原因；或啟動前先檢查空間並給明確的錯誤。牽涉 vault-store 的開檔契約，不是這個 PR 的範圍。

- [ ] **WORK-TURSO-MINMAX-WRAPPER-AUDIT** — `COALESCE(MAX(id),0)` 這種包了一層的 MAX 在 Turso 是全表掃描（2026-10-30 PR #16 第三輪審查找到）
  - 根因：Turso 0.8.1 `translate/optimizer/mod.rs` 的 `detect_simple_aggregate` 只在結果欄「正好是」聚合式時走 MIN/MAX 捷徑（沿索引或 rowid 直接走到極值）；外面包 `COALESCE`、`+1` 都不算，就退回掃描。Intelligence v2 遷移因此每批掃一次越長越大的新表，1M / 4M 量到 26.5 s / 129 s；改成不包、在 Rust 裡處理 NULL 之後 16.4 s / 66.9 s（`semantic-index-control.md`）。
  - 要查（14.4M 時每次都掃一遍的優先）：`intelligence/build_pipeline.rs:24`（每次 intelligence 建置掃 `visits`）、`ai/indexing/backfill.rs:413` 與 `:539`、`ai/generations/build.rs:145`、`ai/similar_searches.rs:198`、`ai/indexing/store_rows.rs:91`、`archive/doctor_derived.rs:76`，以及 `intelligence_runtime/enqueue.rs`、`job_control.rs` 的 `MAX(list_order)+1`（小表，可能不用改）。
  - 做法：先用 `EXPLAIN` 確認每一條真的掃表，再改成不包的 `MAX(...)` 並在 Rust 裡處理 NULL；有 WHERE 的要確認存取路徑能直接走到極值。結果記進 `turso-review-ledger.md`。

- [ ] **WORK-SEMANTIC-VISIT-SIDECAR** — 語意搜尋每次都讀一遍範圍內的所有造訪：100 萬次 385 ms，14.4M 估 5.5 秒（M5 Max），目標機更慢（2026-10-17 做完 `WORK-SEMANTIC-PIPELINE` 量到）
  - 現況：一次語意搜尋 100 萬次造訪 500 ms，其中 385 ms 是一條 SQL 串流範圍內每次造訪（算頁面成員、最新時間與每頁最高分）。向量掃描只要 6 ms。
  - 串流是一次讀取，整段佔住 archive 兩條讀取連線中的一條；writer 做 WAL truncate checkpoint 前要等進行中的讀取結束（`vault-store/src/checkpoint.rs` 的 `wait_for_readers`，`worker.rs` 呼叫），這段時間進來的寫入也跟著等。14.4M 時一次語意搜尋可能讓匯入、備份的寫入延後數秒。以前的逐批回查是很多次短讀取，不會這樣；日期篩選的全文搜尋（`eligibility::load`）本來就是同一種長讀取。
  - 方向（先設計、先量）：建索引時把每次造訪的頁面與時間投影到索引旁（跟 `.pkpos` 同一份或並排），加上撤銷造訪的集合；沒有篩選或只有日期篩選時，頁面成員與每頁分數在記憶體裡算完，用到網站、標籤、筆記、星號、profile 等篩選才走 SQL 串流。要處理：索引之後才進來的造訪（成員與最新時間仍要算到它們）、撤銷與還原、App Lock 時放掉記憶體、`semantic_stage_bench` 量 1M / 4M。

- [!] **WORK-VECTOR-MEMORY-AT-SCALE** — 大語料時完整 int8 排序放不進 8 GB（等用戶在研究文件「What this means」的 A / B / C 中選）
  - 大語料時完整 int8 排序放不進 8 GB，要嘛從磁碟讀、要嘛用緊湊碼排全部再重算前段（後段順序是近似的），要嘛回到候選池（推翻 2026-10-07 的完整計分）。Turbovec 2-bit 當第一段（挑 2,000 筆再用 int8 重算）在 256 維與 1,024 維都不掉召回；sign bits 只在高維夠用。這只在 13 萬頁上量過（2,000 筆佔 1.5%）；到幾百萬頁時候選數多半要跟著放大，選 B 或 C 之前要先在幾百萬頁的真實語料上量。

- [!] **WORK-DECISION-MODELS** — 量決策模型（Jev、OpenAI Decisions、開源 pplx-decider 等）在 PathKeep 的用處（用戶 2026-10-17：磁碟沒空間，之後再說）
  - 讀先：[2026-10-decision-models.md](../architecture/research/2026-10-decision-models.md)。
  - 先量：在 LM Studio 下載一個決策模型（3–25 GB，先問用戶），量這台 Mac 與目標機的速度、在標題＋網址（中英混雜）上的準確度。候選用途：每月固定類別、Ask 的篩選工具與語意結果重排、送雲端前的本機敏感頁檢查。

- [ ] **WORK-QUIT-FOLLOWUPS** — 結束前詢問的收尾（2026-10-25 `WORK-QUIT-DURING-WORK` 留下）
  - 更新後重開（`relaunch_after_update` → `RESTART_EXIT_CODE`）不攔：匯出中按了更新重開，工作照樣中斷。方向：有工作在跑時，About 頁的更新先說正在做什麼，等做完或確認再重開。
  - E2E 只驗了匯出；匯入、備份、維護、復原匯入、兩種工作同時、問題開著時工作先做完，都還沒有 E2E。「現在結束」不先停下工作，匯出的暫存檔會留在目標旁。

- [ ] **WORK-SEARCH-OR-OPERATORS** — `OR` 也能連運算子：`site:a.com OR site:b.com`、`intitle:x OR inurl:y`（2026-10-25 把 `OR` 改成 Google 的讀法時留下）
  - 現況：`OR` 只連兩邊的字或引號片語；旁邊是運算子或減號時什麼都不連（`search.md`），所以 `site:a OR site:b` 還是兩個都要，什麼都找不到。Google 允許這樣寫。
  - 為什麼沒一起做：`site:`、`intitle:`、`inurl:`、`filetype:`、`tag:`、`note:` 是 canonical SQL 上的 AND 條件（`eligibility::Scope`），索引那層的候選也照 AND 算；要讓它們能 OR，條件要變成一棵 AND / OR 樹，SQL 與索引候選兩層都得照這棵樹算，還要保持完整覆蓋與現在的查詢計畫。
  - 要做：先設計（解析成樹、兩層怎麼各自求值、14.4M 的查詢計畫），再做；E2E 用 fixture 驗 `site:a OR site:b` 找到兩個網站的頁、`-site:` 與 OR 一起用的情形。

- [ ] **WORK-SEARCH-ACCENT-GRAMS** — 錯字搜尋的候選集在重音很多的頁面上會漏（2026-11-05 Codex 審查錯字回退時找到，不是這個 PR 帶進來的）
  - 現況：索引的 trigram 保留重音（`déjàvu`），頁面檢查把重音折掉（`dejavu`）。夠長的目標（12 個字元以上）只走共同 trigram，假設改動以外的字都一樣；重音不算改動，所以每三個字就有重音的頁面可能一個 trigram 都不共用，檢查明明會接受，候選集卻沒拿到它。短目標會連不共用的頁一起走，不受影響。
  - 要做：讓索引也存折過重音的 compact 文字，索引和檢查讀同一份；要重建索引，先量 14.4M 的重建時間與索引變大多少。

- [ ] **WORK-SEARCH-TYPO-WALK** — 搜尋沒有精確結果時，錯字回退要佔滿一個核心 1.5～2.5 分鐘才結束（2026-11-05 PR #16 第九輪量到；main 就這樣，這個 PR 沒有變慢）
  - 現況：14.4M 造訪、2.9M 頁、4 vCPU / 8 GB：`raresentinl` 92 s、`archive raresentinl` 139 s、拼對但沒結果的 `quartz lantern` 116 s（`docs/architecture/research/2026-11-typo-walk-benchmark.md`）。不擋別的事：取消 26 ms 內，小寫入的中位數不到 1 ms，記憶體 170 MiB 內；但使用者要等這麼久才知道真的沒有。8 個字母以下的字（10、11 個字母也是）改動次數足以毀掉所有 trigram，索引幫不上忙，只能走完所有頁。合成頁面的文字短，真實存檔可能更久。
  - 方向（先量）：索引加一份詞表，錯字先在詞表上比（詞表比頁小得多），再用比中的字查頁。不能用上限或抽樣換速度：搜不到的東西對用戶等於不見了。要重建索引，跟 `WORK-SEARCH-ACCENT-GRAMS` 一起想。

- [ ] **WORK-TAG-SAVE-FAILURE-STALE** — 標籤存失敗時退回的可能是舊清單，下一次編輯會把另一個編輯器存好的標籤蓋掉（2026-11-05 PR #16 第九輪審查提到，讀程式碼確認，沒重現；main 就有）
  - 現況：`tag-editor.tsx` 有存檔在路上時，收到別處存好的新 `initial` 會整段跳過，連 `saved.current` 也不更新；這時排隊的存檔失敗，`catch` 退回的是更舊的清單，`applied.current` 又已經記下新值，畫面不會再改回來。
  - 方向：`saved.current` 記的是已存下的內容，收到 `initial` 就更新，只有 `setTags` 等存檔結束。要先有能讓這一次寫入失敗的 E2E。

- [ ] **WORK-SCRAPER-LATEST** — `scraper` 0.24 → 0.29，順便拿掉 `fxhash` 的 allowlist（2026-11-05 核對 RustSec allowlist 時找到）
  - 現況：`fxhash` 現在只經 `selectors` 0.31 ← `scraper` 0.24 進來（Tauri 2.12 已經不帶它）；`scraper` 0.29 用 `selectors` 0.41，不再需要 `fxhash`。`scraper` 是 0.x，宣告鎖在 0.24，要手動升。
  - 為什麼沒一起做：CI 不靠這條，而升級會動到讀網頁內容的 selector API，要看 enrichment 與連結預覽的行為有沒有變。
  - 要做：升級、跑 enrichment 與 link-preview 的 E2E、把 `scripts/check-rust-security.mjs` 與 `deny.toml` 的 `RUSTSEC-2025-0057` 拿掉。

- [ ] **WORK-SQLCIPHER-SQLITE-354** — 內建的 SQLCipher 還是 SQLite 3.53，RUSTSEC-2026-0459 修不掉，暫時在允許清單裡（2026-11-09 PR #18 第十二輪 gate 的 `audit:rust` 找到）
  - 現象：SQLite 3.54 之前，資料庫檔和它的 `-wal` 頁大小不同時，查詢可能讀回未初始化的記憶體（只有打開別人做出來的檔案對才會碰到）。我們用 `rusqlite` 的 `bundled-sqlcipher`。修補版 `libsqlite3-sys` 0.39（`rusqlite` 0.41）的 SQLCipher 來自 `sqlcipher-src =419.0.6`，包的是 SQLite 3.53.4，公告自己寫仍受影響；升上去只會讓稽核閉嘴。
  - `sqlcipher-src` 是新依賴：個人維護、repo 2026-09 才建、2 顆星，不到自動放行的門檻，要風險評估和用戶同意。它的 420.0.0（SQLCipher 4.20.0、SQLite 3.54.1）已經發布，等 `libsqlite3-sys` 改用它。
  - 現在：用戶同意（2026-11-09）把它加進 `scripts/check-rust-security.mjs` 的允許清單，理由寫在條目裡；之前 CI 的供應鏈稽核一直紅，下一條新公告會看不到。
  - 方向：那一版出來時寫風險評估，用戶同意後升 `rusqlite` 與 `libsqlite3-sys`，跑 v0.4.0 存檔的轉換 E2E（SQLCipher 的加密後端可能從 OpenSSL 換成 libtomcrypt，檔案格式相同，要量），**同時刪掉允許清單裡的 RUSTSEC-2026-0459**。升級後公告不再命中時，稽核腳本會因為條目過期而失敗，不會漏刪。
  - 另一條路：轉換器改用 Turso 的 `PageCodec`（`turso_core` 0.8.3 公開）自己解 SQLCipher 4，app 就不再需要 SQLCipher，`rusqlite` 改回一般的 `bundled`，升 0.41 就是修好的 SQLite 3.54.1，vendored OpenSSL 也拿掉（只有 SQLCipher 用它）。要先驗證 Turso 打得開每個 v0.4.0 檔案的 schema、WAL 裡已提交的頁面讀得到、能唯讀打開；會推翻 ADR 012 的 SQLite library note，需要取捨文件與用戶同意。2026-11-09 用戶決定先不做。

- [ ] **WORK-SEARCH-QUOTED-LITERALS** — 引號裡看起來像運算子的字照字面找，補 E2E（2026-10-29 審查 PR 16 時找到；解析在 2026-11-02 第六輪做掉，當時記在 `WORK-SEARCH-QUOTED-OPERATORS`）
  - 現況：從引號開始的整個 token 是字面片語，SQL 篩選與索引查詢兩個 parser 一起改了；進階搜尋的表單會替看起來像運算子的字加引號。有 Rust 的表格測試（`search/query_tests.rs`）與表單的 TS 測試（`words-query.test.ts`），History 裡還沒有 E2E 走過。
  - 要做：fixture 加一個標題含 `site:` 字樣的頁、一個標題有以 `-` 開頭的字的頁（現在都沒有）；E2E 在 History 搜 `"site:…"` 找到那一頁而不是變成網站篩選，搜 `"-…"` 找到另一頁而不是排除它，再從進階搜尋的表單各送一次。

- [ ] **WORK-SEARCH-NEAR-DUPLICATES** — 搜尋結果合併近似重複：同一篇文章的不同網址、AMP、鏡像（`WORK-EMBED-USES` 剩下的候選，2026-10-17）
  - 2026-10-25 審查：先做研究，不先寫碼。在真實資料的測試副本上（只看數字）分類一批查詢前段結果裡的近似重複（同網站同標題、AMP 與行動版主機、跨站鏡像、只差追蹤參數；先查 `canonical_url` 已經合併了哪些），抽樣標錯合的比例；再寫 trade-off（合成一列「另有 N 個網址」並能展開，讓每個網址都找得到；計數算頁還是算文章；在排名時還是顯示時合併）。它改的是「一頁一列」的結果契約，要用戶決定哪幾類能合。
  - 相關頁面在真實歷史上看到約三成的近鄰是同一頁換了網址；`ai/related_pages.rs` 的折疊規則（可註冊網域＋標題主體，去掉計數括號）可以沿用。搜尋結果要合併的範圍更大（跨網站的鏡像、AMP），先在真實資料上量哪些情況值得合、合錯的代價。
  - 讀先：[2026-10-related-pages.md](../architecture/research/2026-10-related-pages.md)、`docs/architecture/search.md`。

- [ ] **WORK-HISTORY-SEARCH-NOTES-E2E** — History 的語意搜尋提示還沒有 E2E（2026-10-25 做完 `WORK-HISTORY-SEARCH-NOTES` 的前後端時留下）
  - 已做：History 在結果上方照語意搜尋回的提示說一句（上一個模型、索引是空的、篩掉了、設定變了、連不上服務），連到 Settings → AI；服務算不出查詢向量時改用字詞搜尋並帶 `providerResolutionFailed`。
  - 要做：`semantic-index.spec`（restart 專案）在換模型、新索引還沒建完時，History 的語意搜尋寫「所選模型的索引還沒建完，所以先用上一個模型的索引」；讓假的嵌入服務對查詢回錯，History 寫「連不上嵌入服務…」並列出字詞結果。

- [!] **WORK-SEED-TURSO-ASSERT** — debug 版的 14.4M 種子資料觸發 Turso 的 reserved-bytes 斷言（2026-10-15 Codex 量 Takeout 匯入時看到，release 版沒事，根因沒查）[!blocked: 要 14.4M 的 debug 種子存檔，約 40 GB，這台 Mac 放不下；Modal 上 5 小時種不完（2026-10-25），要一次更長的執行。斷言的原文當時沒記下]
  - 先在 debug 版重現，看是我們寫入的方式還是 Turso 的 bug；是 Turso 的就照 AGENTS.md「不對外」在我們這邊繞過並記下重現。
  - 2026-10-25 在 Modal 試了一次（8 vCPU / 32 GB，磁碟 495 GB，上限 5 小時）：debug 版的 14.4M 種子沒有在 5 小時內結束（工作沒寫下結束碼：停在上限，或 sandbox 中途沒了，分不出來）；輸出寫在 artifact 裡，當時的 offload 只在工作結束時打包 artifact，所以沒有留下（offload 之後改成每 10 分鐘打包一次）。沒有重現，也沒有排除。下次給更長的上限，或只種到斷言出現為止。

- [ ] **WORK-SEARCH-FAMILY-ACROSS-PROFILES** — 同一族搜尋在不同設定檔各算一族，「所有瀏覽器」的清單仍會列兩行（2026-10-25 `WORK-SEARCH-FAMILY-DUPLICATE` 做了一半留下）
  - 已做（2026-10-25）：Insights「最常回來再搜的搜尋」在所有瀏覽器時，同一句（`search_family_key`）在不同設定檔、不同搜尋引擎合成一行，次數相加。原本 `台北天氣` 在兩個設定檔各一行、文字一模一樣的情形不會再有。
  - 還沒做：族是照設定檔與搜尋引擎建的（`QueryFamilyAccumulator::scope`，`family:{profile}:{n}`）。不同設定檔的同一族如果第一句不同（fixture 裡哪一句先搜依它的 `now` 每天不同，兩個設定檔可能不一樣），合併只看第一句，就還是兩行。「你回頭再看的事」（Insights）與 Home 的常回來的主題也照設定檔各列一行（`oneThreadPerAnchor` 的 anchor 是各設定檔的 family id）。
  - 要做：先決定跨設定檔的族怎麼認同一族（兩族有共同的寫法？照族的成員比對？），再改這三處；E2E 用 fixture 的兩個設定檔驗「台北天氣」與「台北天氣預報」在所有瀏覽器只列一行。

- [ ] **WORK-DEPS-LATEST** — 依賴跟到最新版（用戶 2026-10-15：之前的 agent 過度 pin）
  - 0.x 寫 `"0.34.0"` 等於鎖在 0.34.x，要手動升（rig 已在 `WORK-AI-AGENT-TURNS` 換掉）。
  - 列出所有落後的 Rust 與 JS 依賴：先升範圍內的（`cargo update`、`bun update`），major 一個一個升（tokenizers、getrandom、lucide-react 的圖示改名、TypeScript / ESLint、tauri 與它的 plugins），合在一次 gate 裡跑，失敗才二分；要鎖舊版的在宣告旁寫理由（AGENTS.md 供應鏈）。最後跑 `check:supply-chain`。
  - 2026-10-18 發布 v0.4.0 前為了清 RustSec 先升了 rustls、wasmtime、zip（到 9）、rmcp（到 3）、tar、serde_with、openssl（CHANGELOG）。其餘的還沒掃。

- [!] **WORK-WINDOWS-SNAP-LAYOUTS** — Windows 11 的貼齊版面與自畫視窗按鈕的實機確認（2026-10-13 `WORK-UI-POLISH-2026-10` 留下）[!blocked: 要一台 Windows 11 與 GNOME / KDE 的 Linux 桌面來建置與實看]
  - 現況：Windows、Linux 已關掉系統標題列，最小化 / 最大化 / 關閉由 app 畫在右上角（`window-controls.tsx`），從沒在兩個平台的實機上看過。滑鼠停在最大化上時 Windows 11 的貼齊版面不會出現。
  - 為什麼還沒做：要讓系統把我們畫的按鈕當成最大化按鈕，得在 Rust 裡回應 `WM_NCHITTEST`（`windows` crate），但 WebView2 的子視窗蓋住整個內容區，訊息可能到不了頂層視窗；這台 Mac 也編不了 Windows 版（`ring`、`aws-lc-sys` 要 mingw），寫了無法建置也無法試。
  - 要做：在 Windows 上建置，先確認縮放邊、拖到螢幕邊緣（Aero Snap）、雙擊最大化、深淺色、按鈕 hover 與鍵盤都對；再試 `WM_NCHITTEST` 子類化或 WebView2 的 non-client region；Linux 在 GNOME 與 KDE 各看一次。

- [ ] **WORK-RECLASSIFY-STALL** — 14.4M 上改一個網站的類別，建置卡在第一頁之前；修好後仍要 16 分鐘（2026-10-13 `WORK-SCALE-BENCH-RERUN` 找到）
  - 現象：`turso_intelligence` 的 `categories` 階段（覆寫 `site1.test` 的類別後做一次增量建置）在 visit-derived-facts 結束後，Intelligence 寫入執行緒單核跑滿 30 分鐘以上，沒有任何 reclassify 進度；之前「全部頁重新分類」的版本是 62 s。使用者在網站頁改類別就是這條路。
  - 已排除：reclassify 每個網址的兩條讀取（`archive.visits` 依 `idx_visits_visible_url_time` 找、再接 facts / assignments）都走索引，有計畫測試守著。
  - 根本原因（同日找到並修好，未 gate、未提交）：重寫一筆造訪 facts 的 `UPDATE visit_derived_facts_rows … WHERE generation=?1 AND visit_id=?2`，planner 選了 `idx_vdf_generation_canonical_visit` 只用 `generation=?` 找，每筆造訪都讀過這一代全部 14.4M 筆 facts。小資料看不出來。改成 `INDEXED BY idx_vdf_visit_generation`，計畫變成 `(visit_id=? AND generation=?)`；`reclassify.rs` 的計畫測試要求 `visit_id=?`（原本只擋 SCAN，沒擋到這種只用前綴的 SEARCH）。reclassify 4 條測試（含與從頭建一次相等）過。
  - 2026-10-14 在 14.4M 上重量（修正已提交 `f4301c1f`）：跑得完了，958 s，峰值 RSS 1.5 GB。reclassify 只走 site1.test 的 144 個網址（21 s），daily rollups 845 s（1,669 天，site1.test 每天都有造訪，每一天整天重算），概覽快照約 90 s。卡住的問題解決；但「改一個網站的類別」在這台 Mac 上要 16 分鐘，目標機（4 核）更久。
  - 2026-10-25 在 4 vCPU / 8 GB 上量：1,220 s、峰值 1.12 GiB；重分 144 個網址 35 s，daily rollups 1,013 s（1,669 天）。比這台 Mac 慢約 27%，還是整天重算。
  - 還要做：(1) 改了類別時只重算類別那一種彙總（`category_daily_rollups` 與受影響的 summary），用移動的造訪算增減，不要整天重讀（網域、引擎那幾種跟類別無關）；(2) 144 個網址、約 720 筆造訪的 reclassify 要 21 s，每筆改寫約 29 ms，查清楚；(3) 把其他「依鍵改一列」的 UPDATE / DELETE 也用計畫測試掃一遍（同一種 planner 選錯索引）；(4) 結果寫回 `ipc-performance.md`。

- [ ] **WORK-SCALE-BENCH-RERUN** — 補量 14.4M：改回本機跑之後沒量的數字（2026-10-11；用戶指示「先把所有任務做完，再處理這個真實測速」）
  - 讀先：`docs/architecture/ipc-performance.md`、`vault-worker/tests/archive_scale_bench/`（`turso_intelligence.rs`、`turso_reads.rs`）。
  - 前提：14.4M 的測試存檔最多約 40 GB，Modal 已停用，在這台 Mac 上跑。2026-10-11 用戶騰出空間後剩約 77 GB，放得下（另留 5 GB 安全線）；開跑前再看一次，量完刪掉測試存檔。
  - 待量：改一個網站的類別到洞察照新類別建好要多久（`WORK-SITE-CATEGORY-REVIEW`：只重分那個網站的頁，成本照那個網站的網址數與日子數，沒在 14.4M 上量過）；`WORK-EVIDENCE-REIMPORT` 的重新匯入在 14.4M 上多花多少時間（2026-10-12 已改成不再寫重複的證據，20 萬筆的量測見 `ipc-performance.md` §15：第一次匯入的時間不變、再匯入 0 列；14.4M 上要量第一次匯入的唯一索引與再匯入的耗時）。之後的 block 沒量到的 14.4M 數字也記在這裡。
  - 做法：照 `ipc-performance.md` 各節的量法在 14.4M 上跑，結果寫回各節與原 block 的 CHANGELOG 附註。
  - 2026-10-13 第一次在這台 Mac 上跑（`turso_intelligence::encrypted_intelligence_at_scale`，release，加密，14.4M 造訪、2.88M 頁；Apple Silicon 18 核 64 GB，比目標機快得多）：完整建置 4,333 s（visit-derived-facts 1,443 s、daily rollups 863 s、sessions 774 s、refind pages 1,146 s），峰值 RSS 462 MiB，建置中的互動寫入等 0.3 ms；概覽 cold 1.67 s / warm 122 ms，網站頁 32 / 6 ms，sessions 1 ms；全部匯出 5.0 GB、289 s，**峰值 RSS 2.07 GB**（8 GB 目標機上偏高，要查是匯出本身還是 turso 的頁快取）；增量（新增 1 萬筆造訪）140 s，**峰值 RSS 1.26 GB**（比完整建置還高，要查）。
  - 「改網站類別」卡住：`categories` 階段在 visit-derived-facts（11 s）之後，Intelligence 的寫入執行緒單核跑滿 30 分鐘以上，一次 reclassify 進度都沒回報，停掉。之前的量法（全部頁重新分類）是 62 s。查過：每個網址的兩條讀取都走索引（`intelligence/tests/reclassify.rs` 的計畫測試），不是它們。下一步見 `WORK-RECLASSIFY-STALL`。
  - 2026-10-14 重跑一次（含 `f4301c1f`）：完整建置 4,288 s、峰值 RSS 410 MiB；增量 134 s、1.26 GB；改網站類別 958 s、1.5 GB（見 `WORK-RECLASSIFY-STALL`）。兩次的完整建置與增量相差不到 5%。
  - 2026-10-14 補量 `WORK-EVIDENCE-REIMPORT`：14.4M 的存檔上匯入 100 萬筆 256.8 s，同一份再匯入 24.8 s、0 列（`ipc-performance.md` §15）。改網站類別見 §16。這一項只剩兩個記憶體峰值要查：增量 1.26 GB、匯出 2.07 GB。
  - 2026-10-25 第一次在目標機大小的機器上跑（Modal microVM，4 vCPU / 8 GB，load average 1.0，資料最大 22.5 GB；數字在 `ipc-performance.md` §16 最後）：完整建置 5,179 s、462 MiB；增量 213 s、646 MiB；改網站類別 1,220 s；匯出 394 s、993 MiB。兩個記憶體峰值在 8 GB 上都在 1 GB 上下，是隨可用記憶體長的快取，不是工作本身，這兩項結案。
  - 同一次找到的慢讀取：見 `WORK-INSIGHTS-COLD-READS`。

> 2026-07-26 coherence-sweep closeout note：使用者插單的全庫「逆天點」審查（`WORK-REVIEW-2026-07-26-COHERENCE-SWEEP`，見 CHANGELOG）已完成並全 gate 綠。結構化 `CommandError` envelope、十餘條 stable-code copy 通道、六個影子翻譯表剷除、posture/boundary-map 文檔如實化都已落地。待使用者拍板：`?layout=paper` QA mounts 去留、og:image 預設是否翻 `OnDemand`、onboarding 一次性 egress 披露。

- [ ] **WORK-PERF-OG-QUEUE-A** — Background OG Image Refetch Queue
  - 2026-10-25 審查：原本卡住的「前端進度契約」已經不需要（連結預覽在 Settings → 後台任務有自己的一列）。剩下的範圍照審查是「備份之後的連結預覽補抓不擋在備份的路徑上」；開工時先確認這個前提。
  - 讀先：
    `docs/features/explorer-browse.md`
    `docs/architecture/data-model.md`
    `src-tauri/crates/vault-core/src/archive/history/og_images.rs`
    `src-tauri/crates/vault-core/src/archive/history/og_images_fetch.rs`
    `src-tauri/crates/vault-worker/src/archive_flows.rs`
    `src-tauri/src/commands/archive.rs`
    `src-tauri/src/dev_ipc_bridge/dispatch.rs`
    `src/pages/explorer/hooks/use-explorer-og-images.ts`
  - 觀察（2026-05-21）：`trigger_og_image_refetch` 是 fire-and-forget 從 FE 角度看是不阻塞 UI（dev bridge 也已包了 spawn_blocking），但 backend 端單次 batch 20 URL 仍會花 ~18s 把一條 dispatch 連線占住做網路+HTML 解析，背景成本不透明。
  - 目標：把 refetch 拆成 enqueue + worker：command 立即返回 `{ enqueued, alreadyQueued }`，新 `og_image_fetch_jobs` 表（或 reuse 既有 intelligence queue）紀錄 pending URL 狀態；新 worker thread 處理 fetch，per-host rate limit 與 retry policy 集中在 worker；FE 改成 long-poll 或 SSE 觀察進度（沿用 import progress event 模式），UI 上 og:image cell 顯示骨架直到 worker 寫回。
  - 契約：失敗仍要產 negative-cache row（既有合約不變）；UI 不得因 og:image 失敗轉成全頁錯誤；新 worker 要 honor user-agent / rate limit / timeout 既有設定；不得引入新依賴除非經審核。
  - 驗收：batch 100+ URL 時 UI 維持 60fps；`bun run check`；新增 worker + FE polling 測試；docs/features/explorer-browse.md 更新 fetch lifecycle 圖。

- [ ] **WORK-REVIEW-0612-FOLLOWUPS-A** — Deferred findings from the 2026-06-12 deep code review
  - 來源：`WORK-REVIEW-0612-HARDENING-A`（見 `CHANGELOG.md`）。當時已修掉可驗證且範圍受控的安全/正確性 bug；下列是已確認為真、但屬「大型重構 / 需 Accepted-doc 決策 / 需實機視覺驗證 / 低優先」而**刻意延後**的項目。**不要**在沒有對應 slice 與覆蓋計畫下硬塞進別的 block。
  - **Intelligence 規模化效能（最高優先，需先做審查階段）**：以下都在 14.4M-visit 目標機上違反「禁止全量加載」。多數位於 >1000 行的 intelligence owner，必須先走大檔重構兩階段（審查→執行）：
    - 語意 index 全量 build 把整張 visits 表收進 `Vec<IndexedVisit>`（`ai/indexing.rs`，no-limit 分支），且在 v0.3 bail 之前就分配。
    - structural rebuild accumulator 的 `visit_ids: Vec<i64>` 與 evidence_json 的 `visitIds` 無上限（`intelligence/intelligence_structural_aggregates.rs`）。
    - query-family 分組 O(events × families) 且每次比較重新 tokenize（`intelligence/intelligence_structural_build.rs`）。
    - scoped-debug rebuild 的 `limit` 在全量 materialize + 分類之後才裁剪（`intelligence/intelligence_visit_records.rs`）。
    - 增量 / fallback daily-rollup 與 phase_three/phase_four read models 把無上限 `Vec` 收進記憶體（`phase_three.rs`、`phase_four.rs`；2026-10-12 刪掉的 `intelligence_daily_rollups.rs` 只剩測試在用）；另含增量 vs fallback 對 `domain_category` 的「first-seen vs most-frequent」不一致與雙 local-day 實作。
  - **Intelligence job lease / recovery 強化**：`recover_expired_intelligence_jobs` 無 attempt cap 地把過期 `running` job requeue（300s lease + 每 profile 才一次 heartbeat），可造成重跑 / 雙執行；enrichment job 在 claim 後失敗（`store_enrichment` 等）會被 `let _ =` 吞掉而永遠 `running`。修法：背景 heartbeat 續租或每 batch 進度、recovery 加 attempt cap、worker 端 on-Err `mark_intelligence_job_failed`。需與 lease 設計一起做並補測。
  - **Parser（需 Accepted-doc 決策 / 語義變更）**：~~Chromium `visit_duration`（µs）以 `_ms` 命名 verbatim 存~~（2026-10-09 已修：寫入時換成毫秒，升級轉換時把舊資料除以 1000，見 `docs/architecture/visit-duration-unit-tradeoff.md`）；`PRAGMA quick_check`（無界）每次 Browser-Direct import/preview 全庫掃描（`takeout/browser_history/staging.rs`）；orphan visit 因 INNER JOIN 靜默丟棄（chromium/firefox/safari），需 warn 或 LEFT JOIN。
  - **History regex recall 無界**：`list_history_with_regex` 綁 `:pageLimit = -1` 後全量 materialize 再過濾（`archive/history.rs`）。修法需 bounded 候選掃描 + 三語誠實提示「已掃描前 N 筆，請縮小範圍」(en/zh-CN/zh-TW) 與 response contract 欄位。
  - **App-lock KDF 強度（需新增依賴 + 遷移）**：passcode 走 120k 輪 SHA-256，建議改 argon2id（RustCrypto，符合 trust gate 但需依賴授權）+ 提高最小熵 + 線上 attempt 鎖定；同時 rekey 前的 plaintext snapshot（`archive/maintenance.rs`）在 Plaintext→Encrypted 後仍留明文，需加密或誠實提示並清理；`UnlockAppSessionRequest`/`SetAppLockPasscodeRequest` 的 `Debug` 應 redact passcode。
  - **og:image SSRF 殘留**：目前 pre-flight 守住「page URL / og:image 直接指向私網」與最終 host；但 `reqwest` 仍會自動跟隨 redirect 到私網（中間 hop 仍會發 GET）。徹底封堵需自管 redirect-follow + 逐 hop 驗證（hermetic 覆蓋需可注入 resolver/transport）。
  - **Frontend（需實機視覺驗證 / 中等）**：~~Explorer sticky day-header 的 `top` offset 由 ResizeObserver→setState 驅動，filter chips 換行時有 1 幀延遲導致短暫脫離 sticky~~ — **已修（2026-06-13，見 CHANGELOG `WORK-REVIEW-0612-FOLLOWUPS-A`）**：改用 `--pk-toolbar-h` CSS custom property，由 `useLayoutEffect` 內的 ResizeObserver 直接寫入(無 React render 中介);剩餘 interactive chip-wrap/scroll pixel 驗收待在實機 app 補眼。其餘未做：`renderedTimeResults`→`groupEntriesByDay` 在每次 favicon/og cache mutation 對整個累積集 O(N log N) 重排+重建物件（建議把 icon hydration 與 grouped data 解耦）；`useBrowseDayInsightsCache.resolve` 在 render 期間發 IPC；IntersectionObserver `root` 假設 viewport 是 scroller 但實際是 `<main>`；link-previews rebuild 在 `refreshStats()` 完成前就顯示成功。
  - **Frontend lib（中等）**：`readRequestCache` / `overviewCache` 無界增長（建議 LRU）；`invokeCachedRead` 的 `force` 在有非 forced in-flight 時被忽略；`flattenDictionary` bare-alias 跨 namespace 葉鍵碰撞（目前無 live hit，建議加 catalog 測試斷言無碰撞，或移除 bare alias）。
  - **Build/tooling（低）**：desktop-bridge truth gate 用 `Math.random` 端口偏移 + `retries:2`，跨 run / 殘留進程可能撞埠造成偽紅（建議 OS ephemeral port）；native-deps cache 的 broad `restore-keys` 可能在 baseline bump 後重用舊樹；rusqlite `bundled-sqlcipher-vendored-openssl` 的 vendored C compile 與 native-dependency ADR 措辭需補一句一致性說明。
  - 契約：每一項落地時都要先確認屬於哪個 owner / slice、>1000 行檔案先走兩階段、保持 100% JS/Rust coverage 與 mutation gate；Accepted-doc 項目（visit_duration 單位、KDF/snapshot 安全模型）必須先產出 trade-off 文檔並徵得使用者同意。

- [ ] **WORK-AI-MAPREDUCE-A** — Sub-agent time-chunk map-reduce for whole-history analysis（需先做設計階段）
  - 來源：2026-06-24 使用者驗收。使用者要「讓 agent 在知道 what's ahead 的狀況下拿到完整數據」。當時已做 **option A**（`run_code` 在沙箱內分頁聚合完整數據——rows 不進 context，只回 distilled 結果；見 CHANGELOG 的 sandbox 完整聚合條目）。本 block 是 **option C**：當單一 context 裝不下、或需要「逐塊的 LLM 質性分析」（不只是 JS 統計聚合）時，用 sub-agent map-reduce 跨時間塊分析整段歷史。
  - 目標（先設計，再實作）：
    - 一個確定性 orchestrator（worker 端，非讓弱模型自己 spawn）把時間軸切成塊（依時間窗或資料量），每塊派一個**全新 context 的 sub-agent**，給它該塊的完整數據存取（search/run_code/intelligence_report 限定該塊範圍），各自回一個**結構化摘要**，主 agent 再 synthesize 成最終答案。對應「分析我這一年/整段歷史」這種單一 context 裝不下的質性問題。
    - 形狀參考 Claude 的 workflow/sub-agent map-reduce，但落在 PathKeep 的 local-first worker 上：要能 spawn 多個 `drive_agent_run`（或一個 map-reduce harness）、chunk 規劃、結果 schema + 聚合、跨 N 個 agent 的 token budget / 取消 / journaling、以及進度可觀測（使用者看得到 fan-out，呼應 [[WORK-AI-INDEX-OBS-A]] 的 observability 精神）。
    - 與 option A 的邊界要寫清楚：純統計聚合 → A（一個 run_code 沙箱內搞定，便宜）；需要每塊 LLM 質性判讀或資料超過單 context → C。
  - 讀先：
    `docs/architecture/ai-security-posture.md`
    `docs/features/intelligence-and-ai.md`（AI/assistant 規格）
    `src-tauri/crates/vault-core/src/ai/agent_harness.rs`（`drive_agent_run` 迴圈 / budget / journal / cancel）
    `src-tauri/crates/vault-worker/src/intelligence/chat.rs`（`run_agent_stream` / `ai_chat_send` 的 thread + runtime）
    `src-tauri/crates/vault-core/src/ai/agent_tools.rs`（工具表面，sub-agent 也要用）
  - 契約：本機跑 N×LLM 呼叫成本高，要 bound + 進度回報 + 可取消；前端流暢度硬指標（不阻塞主線程）；AI 可選、無 provider 安靜不出現；i18n×3；100% JS/Rust coverage + lethal tests；誠實態（部分失敗的塊要誠實標示，不靜默吞）。先產出 design 文檔（chunk 策略、result schema、budget 模型、failure/部分結果語義、UI）再動代碼。
  - 驗收：對「總結我這一整年的瀏覽」這類問題，orchestrator 正確切塊、各 sub-agent 跑完回結構化結果、主 agent 合成出涵蓋全段、誠實標示任何缺漏的答案；全程進度可見、可取消；`bun run check`；更新 `docs/features/` 與 `docs/architecture/`。

- [ ] **WORK-AI-CTX-BUDGET-A** — Context-budget hardening for the agent loop（從 ai-redesign 審查 defer）
  - 2026-10-25：第 3 項（模型沒寫出回答就停時要說）已做（`noAnswer` 提示）。審查認為第 2 項（視窗）已做、第 4 項已做，tokenizer 的部分被 `intelligence.md` 取代；剩下第 1 項：每一次送出的請求都要在預算內，不只最後的整理回合。開工時先確認。
  - 來源：2026-06-24 移除 8-step cap + 強制 final-synthesis turn 後的審查。當時已修：64-step backstop、live-context gate（量「最後一輪 prompt」而非 cumulative）、無 usage 時 fallback 估算（`estimate_context_tokens`）、ceiling → 強制 tool-free synthesis turn。以下是已知殘留邊界，本 block 把它們做成長期最優解：
    - **synthesis turn 自身可能 overflow**：gate 量的是「剛跑完那輪」的 prompt（`last_prompt_tokens`），但 synthesis turn 還會把「該輪新 threaded 的 tool results」+ directive 再送一次。若最後一輪 `run_code` 回了接近 256KB 的結果且 context 已逼近 budget，這個「救援回合」反而可能撞 context-length（→ 誠實的 provider error / Failed，note 已先發，不是靜默）。要：gate 改用「含本輪新結果的投影大小」，或組 synthesis request 時丟掉/截斷最肥的那筆剛 threaded 的結果（模型本就被要求「用已收集到的證據」作答）。
    - **budget 是寫死的 110k，不知道 provider 真實 window**：`capabilities().max_context_tokens` 目前是 `None`。小 context 模型（如 32k）會在 budget gate 觸發前就被 provider 報 context-length 錯。要把 provider 回報的真實 window 接進來推導 budget（無回報時 fallback 110k），並把 `estimate_context_tokens` 換成真 tokenizer 估算（CJK 比例 char/token 與英文差很多，現在的 bytes/4 只是粗估）。
    - **synthesis turn 回空字串無 fallback**：模型若硬是在 tool-free turn 只吐一個（無效的）tool call、不吐 text，使用者會拿到 note + 證據 chips 但沒有 prose——等於那個 case 又「靜默」了。要：偵測 `accumulated.text` 為空時，串一條 deterministic 的最小摘要（localized），讓「never silent」契約對不聽話的模型也成立。
    - **synthesis directive 未進 journal**：trace replay 重建得到一段沒有可見成因的 assistant 答案。要把 directive（或一個「final-synthesis」marker）寫進 journal payload，維持「trace = 忠實重放」。
  - 讀先：
    `src-tauri/crates/vault-core/src/ai/agent_harness.rs`（`drive_agent_run` 迴圈、`run_final_synthesis_turn`、`estimate_context_tokens`、`DEFAULT_TOKEN_BUDGET` / `DEFAULT_MAX_ITERATIONS`）
    `src-tauri/crates/vault-core/src/ai/llm.rs`（`capabilities()` / `to_llm_usage` / streaming usage）
    `src-tauri/crates/vault-core/src/ai/traits.rs`（`LlmCapabilities::max_context_tokens`）
    `docs/architecture/ai-security-posture.md`
  - 契約：誠實態（寧可誠實 provider error 也不靜默吞；部分結果要標示）；前端流暢度不受影響；AI 可選；i18n×3；100% JS/Rust coverage + lethal tests（含 no-usage、小 window、超大最後一筆結果、空答 fallback 的案例）；`bun run check`。
  - 驗收：構造「context 逼近 budget 時最後一輪回超大結果」的測試，synthesis turn 不再 overflow（或誠實降級且有測試證明）；小 window provider 走 graceful 路徑；no-usage provider 用真 tokenizer 估算；空答有 fallback 摘要；更新 `docs/architecture/`。

- [ ] **WORK-ENCRYPT-AT-REST-ALL** — 評估並（評估後）實作「全量加密」衍生資料庫（需先做設計/性能評估階段）
  - 2026-11-14 更正：加密模式下仍是明文的不只語意向量。讀程式確認：存下的網頁文字（`sidecars/intelligence-blobs/`，`intelligence_blobs.rs` 直接 `fs::write`）、每次備份留的瀏覽器檔案副本（`raw-snapshots/<設定檔>/…/History`、`Favicons`，`archive/ingest/checkpoint.rs` 直接 `fs::copy`，是瀏覽器歷史檔的完整複本）。盤點另外列了 `audit/`、`integrations/`、`schedule/`，還沒逐一讀過。瀏覽器自己刪掉舊歷史之後，這些副本裡還有。
  - 2026-10 狀態：Turso 版每個原生資料庫（archive、source evidence、intelligence、agent、scratch）都用同一把資料金鑰加密，搜尋索引用加密的 Tantivy Directory。下面提到的 SQLCipher 原語（`at_rest.rs` reconcile、`open_archive_connection`、`search_projection.rs`、`intelligence_projection.rs`）已刪除。剩下要評估的只有語意搜尋的平面檔（`.pkvec` / `.pkmap` / `.pkbin` / `.pki8`）是否加密。
  - 來源：2026-06-28 使用者指示。修 backup OOM 時確認：開啟加密只加密 canonical encrypted-tier（`history-vault.sqlite` + `source-evidence.sqlite`）；衍生 sidecar（`derived/history-search.sqlite` FTS、`derived/history-intelligence.sqlite`、`derived/agent.sqlite` 對話）目前**明文**落盤，內含 URL / 標題 / enrichment 摘要 / 聊天內容。使用者選擇當時先只修 source-evidence，並要求把「encrypt EVERYTHING 的性能與實現設計評估」記成待辦，評估後再決定是否實作。
  - 讀先：`docs/architecture/`（at-rest posture）；`src-tauri/crates/vault-core/src/archive/at_rest.rs`（既有的 reconcile/rewrite 原語，可推廣到 sidecar）；`schema.rs`（`open_archive_connection` 的 `ATTACH … KEY ''` 慣例）、`search_projection.rs`（`open_search_connection` 明文開啟）、`intelligence_projection.rs`（明文 + attach archive）；W-AI-3 `agent.sqlite` migration；`scripts/verify-rust-coverage.mjs`。
  - 設計問題（先評估再下結論，量測為準）：(1) **性能**——SQLCipher 對 14.4M 級 FTS5 / 向量 sidecar 的查詢/重建吞吐與 RAM 影響（目標機 4 核 / 8GB）；semantic plane（`.pkbin`/`.pki8`/`.pkvec`/`.pkmap` flat sidecar）本身不是 SQLite，是否一併加密、怎麼加密（檔案層 vs 應用層）。(2) **實現模型**——衍生資料可丟棄/可重建，加密它們 vs「鎖時不可讀、解鎖才解密」的取捨；rekey 時是否要連 sidecar 一起 lockstep（推廣 `migrate_*_for_rekey`）；明文→加密的遷移/重建路徑與 blast radius。(3) **威脅模型**——衍生明文到底洩漏多少（URL/標題/摘要/對話），是否值得這個性能與複雜度成本。
  - 契約：先交 trade-off 設計文檔 + 性能量測（env-gated bench），使用者確認後才動代碼；不破壞非加密路徑；100% JS/Rust coverage；前端流暢度硬指標。
  - 驗收：設計+量測文檔成立並獲使用者批准 → 若實作：開啟加密後 sidecar 不再明文落盤、查詢/備份/重建在目標機仍流暢、rekey lockstep、`bun run check` 綠。

- [ ] **WORK-TEST-REAL-IO-A** — 真實 I/O 測試化：embedding transport seam + fake-HTTP 整合測試 + dispatch 契約斷言（承 2026-06-28 behavioral-assertion 鐵律）
  - 來源：2026-06-28，note 搜尋與 embedding 一碰就炸但「100% coverage」沒擋。根因＝真實 I/O 引擎被 `#[cfg(not(any(test,coverage)))]` 整段換 stub、行為從不被斷言（見 `quality-matrix.md`「覆蓋率 ≠ 行為」鐵律）。2026-06-28 已把 static engine 的真實 compute 放回 coverage binary + 補 note/embedding 行為斷言；剩兩塊較大的 reform 延後：
  - **(1) external embedding transport 可注入 seam**：把 `ai/embedding_external.rs` 的真實 `reqwest` send/decode/timeout 從「整段 cfg-out 換 stub」改成把 HTTP **endpoint** 抽成可注入（本地 fake server / injected client），讓真實 decode / timeout / 空 `data[]` / mid-batch 500 的邏輯留在被量測 build。加 fake-HTTP `/v1/embeddings` 整合測試斷言「誠實失敗，絕不成功但 0 向量」。
  - ~~**(2) `dispatch_for_coverage` → 契約斷言**~~：2026-10-12 `WORK-TEST-CUT-2026-10` 把那個 walk 整個刪了；IPC 契約由 E2E 經 dev bridge 打真的命令證明。
  - 驗收：external 真實失敗模式有測試覆蓋；`quality-matrix.md` 鐵律全達成；gate 全綠。

- [!] **WORK-RELEASE-SIGNING-A** — macOS Developer ID signing + notarization in CI（需使用者決定 Apple Developer 帳號）[!blocked: 需 Apple Developer Program 帳號（$99/yr）+ 使用者授權]
  - 來源：2026-06-25 備份失效調查。Root cause 不是代碼，而是 **macOS TCC / Full Disk Access**：備份要讀 `~/Library/Application Support/Google/Chrome`，OS 拒絕 → `Operation not permitted` → 備份失敗。FDA 授權綁在 app 的**簽章身份**上；目前 debug/release 都是 **ad-hoc 簽章（無穩定身份）**，所以每次 rebuild / 每次發版更新，macOS 都視為「新 app」→ FDA 授權失效 → 使用者更新後備份再次靜默失效。這是發版的硬阻斷：不能讓使用者每次更新都重新授權 FDA。
  - 目標（release 工程）：
    - 取得 **Apple Developer ID Application** 憑證，存成 GitHub Actions 加密 secrets，在 CI 對每個 release 做 **codesign（hardened runtime）+ notarize + staple**。所有 release 共用同一個穩定身份 → app 的 TCC designated requirement（team + bundle id）跨版本不變 → 使用者授一次 FDA 後**跨更新持續有效**。
    - 設計 CI 簽章流程（憑證/私鑰注入、`tauri build` 的 signing identity、notarytool、更新器 artifact 簽章與 `TAURI_SIGNING_PRIVATE_KEY`）。
    - 文檔化 first-run FDA 授權引導（即使簽章穩定，第一次仍需使用者授權；配合 in-app FDA 引導，見備份透明度修復）。
  - 開發期 interim（已/將做，非本 block）：本機用 **自簽憑證**（stable self-signed）簽 debug build，讓開發測試時 FDA 不會每次 rebuild 失效；自簽 ≠ release（自簽會被 Gatekeeper 擋、無法 notarize），只用於本機 dev。
  - 讀先：`src-tauri/tauri.conf.json`（bundle / identifier / updater）；`.github/workflows/`（CI）；Tauri v2 macOS code-signing + updater 文檔。
  - 契約：憑證/私鑰只進加密 secrets，永不入 repo；reproducible build 原則不破；release rehearsal（`bun run verify`）通過。
  - 驗收：CI 產出 signed + notarized + stapled 的 `.app`/`.dmg`；裝在乾淨機器上 Gatekeeper 不擋；授一次 FDA 後，安裝下一個版本仍保有 FDA（備份不再靜默失效）。

---

## 維護規則

1. `STATUS.md` 清空時，從這裡頂部取最多 2 個未被阻塞的 work blocks
2. 新工作一律先放進這裡，除非使用者明確要求立刻調整 `STATUS.md`
3. 如果某個 block 需要再拆，只在 milestone 文檔的 WBS 裡拆，不把 `STATUS.md` / `BACKLOG.md` 再退回原子 task
4. 依賴解除後，直接更新行尾的 `[!blocked: ...]`

- [ ] **WORK-V05-SIDE-FINDINGS-2026-11** — 盤點 v0.6 設計時讀現在的程式看到的（2026-11-13；第一項讀程式確認了，其餘還沒重現）
  - Windows、Linux 上，主畫面以外的全螢幕畫面都沒有最小化、最大化、關閉按鈕：新手引導、鎖定、啟動與啟動失敗、復原、存檔升級。讀程式確認的（沒在 Windows 或 Linux 上跑過）：`desktop_integration/window.rs` 的 `frame` 在 macOS 以外關掉系統標題列；`WindowControls` 只在 `app/shell/app-frame.tsx` 畫，而 `AppFrame` 只在 `ready` 狀態掛上。v0.5.0 已經是這樣。修法是把視窗按鈕畫在所有狀態共用的那一層。
  - 「找不到瀏覽器」或掃描失敗的說明上按 `重新檢查` 失敗時，什麼都不顯示；「自動備份」卡的 `再檢查一次` 失敗時也一樣。
  - 「自動備份」卡讀排程狀態失敗時，一直停在骨架。
  - 毛玻璃視窗下，視窗那一層沒有變淡：`src/index.css` 裡毛玻璃的 `--window`／`--panel` 寫在 `@layer base` 裡，而 `:root` 的值不在任何 layer，沒有 layer 的規則贏過有 layer 的。做外框時在瀏覽器裡量到毛玻璃下 `--window` 仍是不透明時的值（`rgb(250 250 252 / 0.62)`）；用戶 2026-11-14 說 macOS 上選了毛玻璃確實有變透明，但「非常，非常不明顯」，跟量到的一致：桌布那一層有拿掉，視窗那一層沒有照設計降到 14%。修的時候把那兩條規則移出 layer，並在桌面視窗上看過深淺兩種主題。
  - 新手引導的排程預覽只在每個間隔第一次被選時寫入設定；選過別的再選回來、然後關掉，下次可能從錯的間隔開始。
  - `vault-crypto/tests/search_index.rs` 的 `the_block_cache_bounds_memory` 在機器記憶體吃緊時會假失敗：它比較兩個子行程的常駐記憶體成長量，而系統在壓力下會把記憶體壓縮或換出，量到的成長變成負數（2026-11-14 同時有別的編譯在跑時看到「2 MiB 的快取 -0.5 MiB、不設上限的 -3.9 MiB」，單獨重跑通過）。快取大小本身的斷言（`max_cache_mib`）是可靠的；常駐記憶體那一條要換成不受系統壓力影響的量法。

- [ ] **WORK-VAULT-FOLLOWUPS-2026-11** — 規劃 `WORK-VAULT-STORE-ROLES` 時讀程式找到、跟 Agent 對話無關的現有行為（2026-11-14；兩份獨立的盤點都指出，但都只讀了程式，動手前先重現）
  - 搬到另一台電腦會丟掉使用者改過的網站分類與搜尋引擎規則：它們存在 `derived/intelligence.db`（`site_dictionary_overrides`、`search_engine_rules`），而搬移檔把這個資料庫當成可以重算、不帶走，匯入預覽還寫「洞察會重建」。同一個原因，已存的網頁文字檔會被帶過去，但記錄「哪個網址對應哪個檔」的那幾列（`page_content_enrichments`）不會，所以在新電腦上這些文字找不到（`vault_lifecycle` 的搬家測試重現了這兩件事）。修法要改 `data-model.md` 已接受的那一句，需要用戶同意。
  - 還原快照會把當時的設定與助理對話一起還原回來（加密方式、密碼、App Lock 與鑰匙圈設定 2026-11-15 起改成維持現在的樣子；其餘設定與助理對話要不要也留著，沒決定，動手前問用戶）。改密碼只重包目前的信封，舊快照仍是舊密碼，所以還原舊快照要輸入當時的密碼。
  - `maintenance.bak-*` 資料夾只有「刪除所有資料」會清，儲存空間用量也不算它們。還原與搬移匯入換下的存檔 2026-11-15 起改成安全副本（列在還原清單、算進用量、可釋放）；剩下舊格式轉換留的那一個，裡面只有轉換前的設定檔。
  - 儲存空間用量少算 `models/`、`audit/` 的其餘部分、`schedule/`、`integrations/`。
  - 搬家檔可以匯出到存檔資料夾的根目錄（檔名不是保留字就行），而「刪除所有資料」只刪它認得的資料夾和檔名含 `.bak-` 的東西，所以那個搬家檔會留下來，裡面是整份瀏覽紀錄。
  - 各個操作對資料庫伴隨檔的處理不一致：複製只複製主檔，安裝處理 `-wal`／`-shm`／`-tshm`，重設處理 `-wal`／`-shm`，用量算主檔和 `-wal`。現在都明列在 `vault-layout` 的表裡，逐一確認哪些是有意的。
  - 安裝步驟與刪除所有資料在當機時的幾個邊界、存檔打不開時原生格式到不了復原畫面、`SchemaTooNew` 沒有錯誤碼也沒有畫面。
  - 快照格式要能記錄「這份快照含哪幾種資料」，之後只刪一種類型、只還原一種類型時要用。

- [ ] **WORK-ARCHIVE-INVARIANTS** — 用「永遠該成立的事」來測存檔，而不是等使用者踩到（用戶 2026-11-14：「為什麼問題這麼多啊？我們之後要怎麼檢查才能查出這些問題？等用戶一個一個踩坑然後開 issue 來噴我們嗎？」）
  - 為什麼需要：2026-11 讀程式盤點出的問題（開了加密仍有明文檔、搬移會丟使用者改過的設定、清單之間對不上、舊版 app 會弄壞新存檔）有一個共同點：每個功能單獨測都是好的，壞的是「好幾個功能之間都該成立的事」。現在的測試一個功能一個情境，沒有一條在檢查這種事，所以只有整個讀過一遍才看得到。
  - 做法是少數幾條性質測試，每一條擋一整類，而且新增資料庫、檔案或使用者資料時不登記就會失敗：
    - **存檔資料夾裡沒有不認得的檔案**：跑過所有會產生檔案的操作之後走一遍資料夾（`WORK-VAULT-STORE-ROLES` 做）。
    - **加密模式下磁碟上沒有明文**：讓一個獨特的字串經過每一條輸入路徑（造訪、標題、網頁文字、筆記、助理對話、Agent 對話…），再掃資料夾裡每一個檔案，哪裡都不該找得到它。這一條今天就會紅（網頁文字、瀏覽器檔案副本）。
    - **使用者輸入的東西經過任何操作都還在**：一份「使用者自己的資料」登記表（星號、筆記、標籤、網站分類、搜尋引擎規則、助理對話、設定、Agent 對話的星號與封存…），對每一項跑快照→還原、搬移匯出→匯入、開關加密、改密碼、升級，之後都要讀得回來。這一條今天也會紅（搬移丟掉網站分類）。
    - **刪除所有資料之後什麼都不剩**（除了明列保留的記錄檔）。
    - **存到磁碟的結構不會無聲地變**：`WORK-VAULT-STORE-ROLES` 第四步做了設定檔、安裝日誌、刪除標記、App Lock、快照清單、搬家檔清單、排程執行紀錄（`vault-core/tests/persisted_schema/`）。還沒納入、要補進同一份快照的：資料庫欄位裡存的 JSON、搜尋索引與向量索引自己的清單檔、搬移時記下的來源位置、舊格式轉換的進度檔、匯入來源清單、當機報告、獨立 Agent 對話存檔的標記檔。
    - **不同版本互相打開**：留著每個已發布版本的執行檔或存檔當測試資料，跑「誰寫的、誰來讀、做什麼操作」的表；新的要能讀舊的，舊的遇到新的要乾淨地拒絕。做法照跨版本相容的設計。
    - **多步驟操作在每一步當機**之後，下次打開是完整的舊狀態或完整的新狀態。
    - **文件裡每一句保證都對得到一個測試**（「加密後沒有密碼就打不開」這類句子列成表，各指到驗證它的測試；沒有測試的句子要嘛補測試、要嘛改句子）。
  - 另外每次發布前做一次今天這種盤點：兩個不同的模型各自從程式碼列出存檔的組成和每個操作涵蓋什麼，互相比對、再跟文件比對。列進發布檢查。
  - 先做前三條和跨版本那一條；會紅的那幾條連同它們揭露的問題一起排（`WORK-ENCRYPT-AT-REST-ALL`、`WORK-VAULT-FOLLOWUPS-2026-11`）。

- [ ] **WORK-RELEASE-REHEARSAL-V06** — 第一次用新的發布把關之前，把發布流程實跑一次（v0.6.0 發布前）
  - 為什麼需要：`release:build` 會解開每個安裝包、執行裡面的桌面程式與 worker 來確認是正式發布版（`docs/architecture/build-identity.md`）。這段只對合成的安裝包跑過。解不開或跑不起來時它會擋下發布而不是放行，所以不會發出錯的版本，但第一次真的發布時才發現解不開 NSIS 就太晚了。
  - 做法：從這條分支用 `unsigned_preview` 手動跑 `Release` workflow（三個平台），產物留在草稿、不公開。要動到 GitHub 上的東西，開跑前問用戶。
  - 要看的：Windows 上 7-Zip 解得開 NSIS 與 MSI、讀得到 MSI 的檔名表；Linux runner 上有 `rpm2cpio`、`dpkg-deb`，AppImage 解得開；macOS 上 DMG 掛得起來、Intel 版在 Apple Silicon runner 上靠 Rosetta 跑得起來；上傳的檔案名稱和 v0.5.0 的一致（產品名稱現在由 `tauri.distribution.conf.json` 蓋回去）；故意拿掉標記跑一次，確認會停在上傳之前。
  - 順便做：這次產出的 Windows 與 Linux 安裝包是正式發布版身分，拿來測「裝在 v0.5.0 上面升級」。Windows 測試包（`Windows Test Binary`）現在是開發版，測不了升級。

- [ ] **WORK-WRITE-LOCK-WINDOWS** — 跨行程寫入鎖在 Windows 上是會直接報錯的空殼，「刪除所有資料」在 Windows 上很可能一律失敗（2026-11-15，ChatGPT 審查 5a 時順帶指出，未在 Windows 上驗證）
  - `vault-core/src/archive/write_lock.rs` 的 `try_lock_exclusive` 在非 unix 平台直接回傳「this platform is unsupported」。「刪除所有資料」、中斷刪除的接續、v0.3 的啟動修復與還原都會取這把鎖，所以在 Windows 上這些操作應該都會失敗。大部分刪除測試沒有標成只在 unix 跑，CI 也沒有在 Windows 上跑 `vault-core` 的測試，所以一直沒被看見。
  - 先在 Windows 上實際跑一次「刪除所有資料」確認；成立的話，用 Windows 的檔案鎖（`LockFileEx`）實作同樣的語意，並讓這些測試在 Windows CI 跑。
  - 同一份檔案的開頭說明還在講「排程備份是另一個行程」的舊架構。現在只有一個 owner 行程能打開存檔（owner lock），排程備份在 owner 裡跑，改名前 Store 已暫停並關閉；說明要跟著改成現況，並寫清楚這把鎖現在擋的是什麼。
  - 0.5 的「刪除所有資料」標記（`.pk-wipe-in-progress.json`）只在沒有設定檔時照 0.5 的方式就地刪完。GPT-6 Pro 指出（2026-11-15，屬實）：沒有設定檔本身不能證明標記屬於這個資料夾；被帶進來的標記會讓一個本來該拒絕打開（有存檔資料、沒有設定檔）的資料夾被刪掉。0.5 的標記什麼都沒記，分不出來。拒絕畫面（`WORK-ARCHIVE-REFUSAL-SCREEN`）有了之後，改成在畫面上問使用者「上次的刪除沒有做完，要做完嗎」，不再自動刪。
  - 存檔升級時，已發布格式的五個資料庫裡少了一個（例如主資料庫被刪掉）：現在照 0.5 的做法建一個空的，再蓋版本宣告；蓋了版本宣告的存檔少一個資料庫則拒絕打開（`archive-store-missing`）。升級時要不要也拒絕、讓使用者知道資料不見了，待決定。

- [ ] **WORK-FAT-DATA-FOLDER-NOTICE** — 資料資料夾放在 FAT32／exFAT 的磁碟上時，讓使用者知道斷電可能丟檔（2026-11-15，GPT-6 Pro 第三輪程式審查 M8）
  - 原因：有日誌的檔案系統（APFS、ext4、NTFS）上，改名是一步完成的，斷電後檔案不是在原處就是在新處。FAT32／exFAT 上不是：系統可能先寫來源資料夾，寫到一半的配置表可能讓任何檔案消失，呼叫的順序怎麼排都保證不了。PathKeep 在這種磁碟上會留下所有可能共用資料的名字、認不出來就停下（`archive-compatibility.md`「Moves are made durable destination first…」），但斷電弄丟的檔案救不回來。
  - 要做：偵測資料資料夾所在的檔案系統（macOS `statfs` 的 `f_fstypename`、Linux `statfs` 的 `f_type`、Windows `GetVolumeInformationW`），是 FAT 家族時在設定的儲存位置與新手引導的資料夾那一步說明風險、建議換到內建磁碟或備份前先接好電源；三語。建議只提醒、不擋（不少人用隨身碟放資料），待用戶決定。
  - 不做：為了 FAT 改存檔的檔案配置（每個原檔留在自己的名字下、新世代放別處再切換）。那樣會改掉 0.5 讀得懂的配置，而檔案系統本身仍保證不了。

- [ ] **WORK-FLAKY-CACHE-RSS-TEST** — `vault-crypto/tests/search_index.rs` 的 `the_block_cache_bounds_memory` 在機器記憶體吃緊時會失敗（2026-11-15）
  - 現象：完整檢查同時跑很多測試時失敗一次，上限 2 MiB 的快取確實守住（`max_cache_mib=2.00`），但「無上限」那一輪的常駐記憶體只多 7.8 MiB（單獨跑是 18.7 MiB），斷言 `small + 4 < unbounded` 不成立。單獨重跑兩次都通過。
  - 原因：它用行程的常駐記憶體當量尺；系統在壓力下回收或壓縮分頁時，無上限那一輪看起來就沒有變大。測的東西沒有錯，量尺會被別的行程影響。
  - 要做的：讓這條測試不受系統記憶體壓力影響（例如在獨立、閒置的行程裡量，或改比較快取本身記錄的位元組加上一個與分頁回收無關的指標），不能只是放寬門檻。

- [ ] **WORK-ARCHIVE-REFUSAL-SCREEN** — PathKeep 拒絕打開一份存檔時，使用者要有下一步可走（v0.6.0 發布前必須完成；2026-11-14）
  - 現況：啟動時先判定存檔能不能用（`docs/architecture/archive-compatibility.md`）。不能用時不開視窗、不碰資料夾，只跳一個系統對話框說明原因（三語），按掉就結束。
  - 缺的兩件事：
    - **設定檔損壞或不見、但資料庫還在**：必須能從安全快照還原。後端仍會列出快照、也允許在設定檔損壞的資料夾上還原，但現在沒有視窗，使用者按不到。資料沒有危險，但人卡住，只能自己去動資料夾。沒有快照可用時，還要有「這份存檔的資料庫還在，重新建立設定」這條路（v0.5 以前在這種情況會直接進新手引導、重新接上原本的資料庫；現在這條路被擋掉了）。
    - **存檔需要較新版本**：畫面要能直接更新，而不只是請使用者自己去下載。
  - 需要一個「不碰資料夾也能開視窗」的啟動模式：不取鎖、不寫日誌、不啟動背景服務，只有這個畫面用得到的指令可以用；還原由使用者按下之後才動資料夾。
  - 後端要給的資料（原因、這一版的格式等級、存檔宣告的版本、資料夾位置）在 `WORK-VAULT-STORE-ROLES` 第六步；畫面與三語文案在這一塊。
  - 安裝紀錄認不出原位的檔案（`archive-control-invalid: … is not a file this install moved`）：紀錄被帶進別的存檔，或安裝中斷後原位的檔案被換過。這不是存檔損壞，資料都在；畫面要說明，並提供從安全副本還原。不提供「丟掉紀錄直接打開」：安裝中途的資料夾是新舊兩份混在一起的。
  - 「刪除所有資料」的紀錄或升級的「建立中」紀錄解不開、或指向它自己的資料夾以外的地方（`archive-control-invalid`）：同樣不是損壞，什麼都沒動；畫面說明，讓使用者把那個紀錄檔移走再打開。
  - 第二輪新增的拒絕（2026-11-15）：安裝回滾發現備份裡的原檔內容不對或不見（拷貝到一半的副本），撤銷刪除發現紀錄裡的項目兩邊都不在；兩者都是這份資料夾不完整，畫面要指向原本那份資料夾或安全副本。v0.5 的存檔少了自己的一個資料庫（`archive-store-missing`，升級時）：要說少了哪一個（Ask 對話、洞察），提供從安全副本還原，以及使用者明確同意後建一個空的再升級。
  - 第三輪新增的拒絕（2026-11-15）：發布後的安裝在原位找到不是它放的檔案（`archive-control-invalid: … is not what the install published`），是發布前後各拷一半、又缺了東西的資料夾副本，畫面指向原本那份資料夾或安全副本。刪除所有資料做完後、trash 外面還有像存檔的東西（中斷期間使用者放回來的），資料夾讀成設定檔不見的存檔：畫面要說明這些是刪除之後才出現的，提供從安全副本還原或重新建立設定。沒有設定檔的資料夾裡有被帶進來的刪除紀錄（沒有它記的 trash）或升級紀錄（沒有它建資料庫的資料夾）：資料夾讀成設定檔不見，紀錄原地不動，畫面要能讓使用者把紀錄移走。
  - 從安全副本還原時遇到 `archive-current-password-required`（存檔打不開、而且是加密的；還原保留存檔自己的加密與密碼，所以要它的金鑰）：畫面要問目前的存檔密碼，用它解出金鑰交給還原（後端的 `restore_snapshot_to` 已經收 `live_key`），不能就此卡住。金鑰檔本身壞了、怎樣都解不出金鑰時，這份存檔的資料（含 Agent 對話）本來就讀不回來；畫面要提供「照副本的加密還原」的明確選項，並講清楚對話救不回來，後端到時再加這個選項。

- [ ] **WORK-WIPE-UNDONE-NOTICE** — 「刪除所有資料」沒做完、被撤銷或紀錄被丟掉時，要告訴使用者（2026-11-15，A2/A3 第三版留下）
  - 現況：還沒把東西全部搬進 trash 就當機時，下次啟動把東西搬回原位、放回原本的設定檔；紀錄對不上資料夾的設定檔時只丟掉紀錄。兩者都只寫 log（`SettledWipe::RolledBack` / `Discarded`）。使用者看到存檔還在，會以為刪除壞了。
  - 要做的：啟動後說一次「上次的刪除沒有做完，存檔維持原樣；要刪除可以再做一次」（三語）；紀錄被丟掉時也說，並提到留下的 `wipe-trash-*` 會在下一次刪除時一起刪掉。
  - 同一次呼叫裡失敗並撤銷時，存檔已經被關上、工作階段的密碼被清掉（`close_store`），使用者要重新解鎖。可以改成撤銷後用原本的密碼直接重新打開。
  - 一併要說的（2026-11-15，A2/A3 第二輪留下）：撤銷時原位的東西被搬進 trash 的 `displaced/`、trash 因此留著；安裝回滾或升級把一個檔案可能的第二個名字移進 `quarantine/`（exFAT／FAT32 斷電後的狀況，建議先檢查磁碟再到「釋放空間」刪掉，否則可能釋放到存檔還在用的空間）。目前都只寫 log。
  - 留著的 `wipe-trash-*` 不算進儲存空間的用量（版面表裡沒有用量分類），只在下一次「刪除所有資料」的預覽裡看得到；要和「釋放空間」的副本清單一起列出來。

- [ ] **WORK-SEMANTIC-SWITCH-WARMING** — 換了嵌入模型、還沒按「建立索引」，背景可能已經用新模型送出過去的搜尋詞（2026-11-15，完整檢查時一個端對端測試失敗一次查到；未證實）
  - 現象：`semantic-index.spec.ts:331` 的「changing the model builds nothing」一次收到 41 個嵌入請求、預期 40；同一份程式重跑全部通過。
  - 推測的原因（還沒證實）：洞察重建完成後 `warm_search_groups`（`vault-worker/src/semantic_reads.rs`）會用「當下選的」嵌入模型整理搜尋分組、送出過去的搜尋詞；模型剛換成新的就送給新的。設定畫面寫的是「Nothing starts before that」，若推測屬實，這是承諾沒做到，不只是測試不穩。
  - 要做的：失敗時把切換後收到的請求內容印出來，確認是不是搜尋詞；是的話，選的模型還沒有完整索引時不要整理搜尋分組（或沿用正在服務的舊模型），測試只認頁面以外的請求也不行，承諾是「什麼都不送」。

- [ ] **WORK-FAT-TWO-NAMES-EXPORT-SWEEP** — 匯出資料夾清舊暫存檔時，可能刪到 exFAT 斷電後和匯出檔共用資料的那個名字（2026-11-15，A2/A3 第二輪順帶查到）
  - `durable_io::sweep_temps_older_than` 清匯出資料夾裡超過時限的 `.pk-durable-*`。exFAT／FAT32 上暫存檔改名成匯出檔時斷電，兩個名字可能指向同一份資料；刪掉暫存檔的名字會釋放匯出檔還在用的空間。要做的：刪之前比對它和同資料夾的匯出檔內容，一樣就留著（或移到別處等使用者處理）。只影響匯出檔，不影響存檔。

- [ ] **WORK-TURSO-COMPOSITE-KEY-PLANS** — Turso 0.8.3 不走複合主鍵的索引：從磁碟打開的存檔裡，用 (kind, key) 查收藏會掃過同一類的所有收藏（2026-11-15，A2/A3 第三版的完整檢查失敗時查到）
  - 重現（`star` 表：`PRIMARY KEY (entity_kind, entity_key)`，另有 `idx_star_kind_starred_at ON star(entity_kind, starred_at DESC)`）：
    - 同一個行程剛建好的 schema：`WHERE entity_kind=?1 AND entity_key>?2 ORDER BY entity_kind,entity_key` 走 `sqlite_autoindex_star_1 (entity_kind=? AND entity_key>?)`。
    - 把 store 關掉再打開（每次啟動都是這樣）：同一句變成 `idx_star_kind_starred_at (entity_kind=?)` 加 `USE SORTER FOR ORDER BY`。
    - `WHERE entity_kind=?1 AND entity_key=?2` 的單筆查詢，剛建好或重開都走 `idx_star_kind_starred_at (entity_kind=?)`。
    - 加 `INDEXED BY sqlite_autoindex_star_1` 兩種情況都走主鍵。
  - 已繞過：舊格式轉換驗證收藏時的分頁（`vault-convert-sqlcipher/src/transfer/stars.rs`）。之前的測試是在剛建好的 schema 上看計畫，所以一直通過；存檔升級改成先在 `staging/` 建好再搬進來之後，新存檔的 store 也是從磁碟打開的，測試才看到真實情況。
  - 要做的：在從磁碟打開的 store 上檢查所有熱查詢的計畫（剛建好的 schema 上看到的計畫不準），特別是其他「複合主鍵 + 以第一欄開頭的另一個索引」的表；需要的地方加 `INDEXED BY` 或改寫。查 turso_core 選索引的邏輯，弄清楚為什麼載入 schema 後選擇會變；穩定版出了新版（目前只有 0.9 的預覽版）就重測。

- [ ] **WORK-UPGRADE-ROLLBACK** — 某一版出了嚴重問題時，使用者要有路可退（v0.6.0 發布前；用戶 2026-11-14 同意要做，並否決「升級前整份複製」：「我們備份數據不可避免毫無疑問 20 gb 起跳，之後可能會更大，悄悄漲一倍的容量，不太行」，要的是記錄改動、允許回滾的做法）
  - 為什麼需要：跨版本相容的做法定為「改了儲存格式之後，舊版拒絕打開那個存檔」。儲存格式包含設定欄位、資料庫欄位、資料庫裡存的 JSON，所以有新功能的版本大多會改到，等於多數升級之後都裝不回上一版。這在資料庫這一層本來就是現況（v0.4.0 打不開 v0.5.0 更新過的存檔），新做法只是把它講明、並把舊版繞過檢查弄壞存檔的路堵上。但「出事只能等修正版」對一個備份工具不夠。
  - 做法：由新版執行「準備給上一版使用」。它不是把存檔倒回升級那一天，而是把現在的存檔轉成上一版讀得懂的樣子：上一版看得懂的資料（包含升級之後才備份進來的）留在原地，上一版看不懂的資料收到存檔資料夾旁邊、一份能獨立打開的檔案裡，重新升級時再收回來。
  - 2026-11-14 第一次寫這一項時有三句話說得太滿，GPT 讀完每一個既有的升級步驟後查出來，在此更正：
    - 「一次升級只加不刪」當不了硬規則。過去的升級裡，瀏覽紀錄資料庫的第 4、6、7 步拿掉了唯一性限制（同一個瀏覽器列號可能被重用，舊限制會把有效的資料擋在外面），洞察資料庫的第 2 步把整張表改名、搬資料、刪舊表。這些都是對的修正，只加不刪做不到。可行的規則是：**升級不能毀掉「轉回上一版所需要的資訊」**；要改寫或刪掉舊資料時，先把轉回去需要的舊值留下來。
    - 「不佔額外空間」不是每次都成立。資料庫拿掉一個欄位要把整張表重寫一遍；需要保留舊值的改動，佔的空間就是那些舊值的大小。每一次改格式要說清楚它轉回去的真實代價。
    - 「升級步驟寫錯也救得回來」不成立。轉回去只能保住還在的、或事先特意留下的資訊；一個寫錯的升級把不相干的資料蓋掉，轉回去救不了，只有另外留的一份才救得了。
  - v0.6.0 的情況比一般情況簡單：瀏覽紀錄的四個資料庫在 v0.6.0 沒有新的升級步驟（已比對 v0.5.0 與現在的結構定義，沒有差異），所以轉回去不用動它們，不佔額外空間，上面三個限制在這一版都碰不到。要做的是：
    - 把 Agent 對話的資料庫移到存檔資料夾旁邊的一個資料夾，成為一份帶自己的金鑰信封、能獨立打開的存檔（移動，不複製）。不能留在原地：舊版不認得它，之後舊版一換加密金鑰它就再也解不開。
    - 存檔資料夾裡 v0.6 格式的快照、還原時留下的舊一代、搬家檔也一起移過去。舊版去還原這些會把自己讀不懂的設定檔裝進來。
    - 把設定檔寫回舊格式，這是最後一步。中途當機要能接著做完，紀錄檔放在存檔資料夾外面（舊版會清掉資料夾裡它不認得的東西）。
    - 之後新版再打開這個資料夾時不能自動又升級，要先問；確認後把移出去的 Agent 對話收回來。
    - 使用者要知道的三件事：移出去的 Agent 對話在舊版裡看不到；舊版的「刪除所有資料」刪不到它；它的密碼停在移出去那一刻的密碼，之後在舊版改密碼不影響它。
  - 守門測試：真正的上一版執行檔寫的存檔，升級、再備份一些新資料、準備給上一版使用，然後用真正的上一版執行檔打開，新舊瀏覽紀錄都在，還能繼續備份；移出去的 Agent 對話能獨立打開；重新升級後三段資料都在。另外在每一步當機、在舊版裡改密碼與開關加密之後再升級，各一條。
  - 之後每一個改格式的版本都要附「準備給上一版使用」和它的測試，否則不能發布；修正版（0.6.x）不改格式，所以裝回前一個修正版永遠可行。
  - v0.6.0 不做、留到第一個會改寫既有資料的版本再做（那時才有東西要保護，而且那段程式是跑在新版裡，不需要 v0.6.0 預先帶著）：
    - 升級前用檔案系統的免複製功能留一份（macOS 的 APFS、Linux 的 btrfs 與 XFS、Windows 的 ReFS 才有；NTFS 和 ext4 沒有，不退回整份複製；之後原檔被改多少它就佔多少，最多可以佔到一整份）。
    - 限制升級步驟只能做哪些事的檢查、一千四百萬筆規模的轉回測試。
  - 用戶 2026-11-14 決定：v0.6.0 不做「背景程式（worker）不經過桌面 app 直接執行」這條路（它會推翻已接受的 `owner-process.md`：worker 只當客戶端、不直接開資料庫）。代價講明：出問題的那一版如果連桌面程式都啟動不了，使用者當下沒有任何一版能把存檔轉回去，要等 0.6.x 修正版；修正版不改格式，一定開得了，也能帶這條路。為了把這個缺口縮到最小，「準備給上一版使用」要能在存檔還沒正常打開時執行：桌面程式啟動後、跑升級步驟與背景佇列之前就提供這幾個指令，所以「程式開得起來但存檔打不開」「卡在升級途中」都還有路；只有程式本身起不來才需要修正版。
  - 方案審過一輪後定稿（2026-11-14，交接資料夾 `PLAN-upgrade-rollback.rev2.md`），實作順序：
    - 先修安裝紀錄檔的一個既有問題：寫入「已完成」失敗時，程式只把記憶體裡的旗標改回去就開始回滾，但磁碟上的紀錄可能已經寫成「已完成」；之後再當機，下次啟動會照磁碟上的紀錄清掉不該清的那一代。改成一律以磁碟上的紀錄為準。新增資料庫與收回 Agent 對話都要用這份紀錄檔，所以這個修正排在它們之前，不留到之後的耐久性那一塊。
    - 轉換完成後，存檔資料夾旁邊留一份固定格式的交接紀錄。之後新版（包含排程在背景啟動的）看到它就不自動升級：背景啟動直接回報「需要使用者選擇」、什麼都不做；前景視窗讓使用者選「把 Agent 對話收回來」「從空的 Agent 對話開始」或維持現狀給舊版用。這份紀錄的格式在 v0.6.0 就固定，之後的版本要退回 v0.6 時寫同一種。
    - v0.6 寫出的快照、還原時留下的舊一代、留在存檔資料夾裡的搬家檔各有固定的子資料夾，轉換時整個移出去；這些新快照不再登記進瀏覽紀錄資料庫的快照清單（否則移走之後舊版的健康檢查會看到一堆找不到的項目）。移出去的這些重新升級後不會自動搬回來，仍可從那個資料夾指定使用。
    - 獨立的 Agent 對話存檔格式從來沒發布過，直接改成和產品裡同一套檔名與金鑰標籤，移出去就只是改檔名位置。
    - 收回來一律用「複製並驗證」：移出去的那份在新的一份裝好並驗過之前保持完整；需要多一份 Agent 對話大小的空間，預覽時量出來顯示。目的地已經有 Agent 對話時不合併，兩份都保留並回報。
  - 自己做一層「記錄每一頁改動」的機制、讓舊版自己去開或降級新存檔，都不做。

- [ ] **WORK-V051-READER-CHECK** — 如果發 v0.5.1：帶上「讀到比自己新的存檔就乾淨地拒絕」的那一半（用戶 2026-11-14 提出，還沒決定發不發 v0.5.1）
  - 現在沒有任何 v0.6 的存檔，所以今天沒有人有風險；風險從 v0.6 發布那天開始，而且只發生在降版的人身上。v0.5.1 帶上這個不是救急，是讓那天到來時有更多人手上的舊版懂得拒絕，並且看得到「這個存檔是較新的 PathKeep 寫的，請更新」的畫面，而不是 app 直接起不來。
  - 只做讀取端：認得存檔上的格式宣告、在每一條路徑（打開、還原、搬移匯入、刪除、健康檢查、排程與 MCP）都檢查、拒絕時不寫入任何東西，加上那一個畫面。不寫新格式；寫新格式是 v0.6 的事。
  - 前提：跨版本相容的設計先審完。在設計定下來之前把一個格式宣告發布出去，等於把今天的錯再犯一次。設計要把 v0.5.1 當成一個已知的讀者算進去：v0.6 寫的東西要同時讓 v0.5.0 起不來（它改不了）、讓 v0.5.1 讀得懂到足以顯示那個畫面。
  - v0.5.0 的人仍然存在，所以 v0.6 那個靠設定檔擋 v0.5.0 的辦法不能因為有了 v0.5.1 就拿掉。
  - 從 `main` 開分支做，不在 v0.6 的分支上。
  - 設計審完了（2026-11-14），v0.5.1 要做的事定為：一決定資料夾位置就先讀設定檔，早於取得 owner 鎖、建資料夾、往資料夾寫記錄、接續未完成的刪除或安裝、鑰匙圈、App Lock。`archiveMode` 是物件（不論裡面的版本號認不認得）就回報「這個存檔已由較新的 PathKeep 更新」並顯示那一個畫面：不問密碼、不提供修復或重設、不改寫設定、不跑任何復原。有資料庫但設定檔不見或讀不了算「需要復原」，不算第一次啟動。設定檔是舊格式就照原本的流程啟動。交接內容在 `pathkeep-v06-handoff/DESIGN-archive-compatibility.rev2.md` 的 R6。
