# Agent history：實作與維護交接

本文件給後續 Claude Code 與維護者使用。研究日期與實作基準是 2026-11-11；[核心架構](../../agent-history-core.md)、[功能矩陣](../../../features/agent-history.md) 與 [crate README](../../../../src-tauri/crates/agent-history/README.md) 分別負責決策、能力邊界與操作方式。此處集中記錄容易在整合時遺失的契約、取捨及未驗證事項。

## 模組責任與呼叫位置

| 模組                                                                 | 責任                                                                                            | 不應放入的工作                                           |
| -------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| `model.rs`、`adapters/mod.rs`                                        | Harness、source／event／reference／diagnostic contract，編譯期 adapter registry                 | UI 文案、雲端帳號、動態 plugin 安裝                      |
| `adapters/claude*`、`codex`、`pi`、`deepseek`                        | Native 格式與已知 artifact discovery                                                            | Archive transaction、讀取 reference 指向的檔案、原生修復 |
| `adapters/session_json*`                                             | Continue envelope、Cline／Roo 經典 array 的分片解析與 allowlist discovery                       | 新 Cline SDK 格式、套用 native compaction replay         |
| `adapters/common.rs`                                                 | Bounded JSONL、時間／已知文字抽取、Zstd transport、diagnostic／reference budgets                | 猜測未知欄位的語意                                       |
| `capture/{defaults,plan,files,writer,projection,references,reparse}` | 預設來源、預覽、來源權限與穩定性、分塊 capture、解析、reference 解析、retained-only reparse     | 排程、UI consent、網路下載                               |
| `archive/*`                                                          | Encryption owner、raw revision、projection publication、run reports、有限批次 recovery／cleanup | Native harness 格式判斷                                  |
| `sqlite.rs`                                                          | Read transaction 的 schema／typed-row evidence                                                  | 原始 DB/WAL byte copy、native restore                    |
| `query/*`、`search/*`                                                | 有界分頁、加密 Tantivy、明確同步與 freshness 檢查                                               | Branch replay、去重 usage、AI 判斷                       |
| `export.rs`、`cli/output.rs`                                         | Writer streaming、export 證據、目的地安全發布                                                   | 原始目錄遷移、執行 transcript／resume                    |
| `cli/*`                                                              | 參數驗證、PME 執行旗標、三語 error／help、JSON output                                           | 產品 onboarding／sidebar／typed IPC                      |

高階入口是 `capture::preview`、`Archive::create/open`、`Archive::backup`、query／search／export APIs。`BackupOptions` 帶 record／decoded-byte budgets、`CancelToken` 與可選 progress callback。Progress 描述 durable boundaries，不是逐 token 的 UI stream。Callbacks 在呼叫執行路徑上，不應阻塞；產品應把同步 API 放到 worker，自己將進度送回 typed client。

`Archive::open` 解鎖成功後會執行 schema／未完成工作恢復，不能當完全不修改 archive 的底層檔案檢查。錯誤密碼在資料庫開啟／恢復前被拒絕。來源唯讀與 archive 內部維護是不同契約。

Initialization 在 KDF 前取得 archive owner lock，持鎖重新檢查目的地為空，以 create-new 安裝 envelope，資料庫建立完成後才寫最終 format marker。第一次初始化在 marker 前崩潰，會留下未識別／未完成的新 root；核心不覆寫它，需要檢查後處理。已識別 archive 缺任一 canonical database 時 open 報錯，不建立一個看似正常的空 DB 來掩蓋缺失。

## 身分與資料層

Source ID 是 length-delimited SHA-256，domain 為 `pathkeep-agent-source-v1`，輸入包括 harness、normalized source root、relative artifact path。它不使用 native session UUID，也不靠 mtime／長度識別內容。不同 root 的同 UUID 保持不同；搬動 root／改相對檔名可能建立不同 source identity，沒有自動 project merge。

Raw revision ID、source ID、native session／event ID、hot projection ID 與 search publication watermark 各有用途：

- Raw revision 指向已驗證的 retained evidence；同內容可重用已存在 revision，來源 head 也可能回到較舊的相同內容。讀最新觀察內容應使用 source 的 head pointer，不能取 `MAX(revision_id)`。
- Native ID 是來源證據。多檔、fork、profile、harness 之間不可拿它當全域主鍵。
- Hot projection 有獨立 publication identity，使同一份 retained raw evidence 也能重新解析而不覆寫讀者目前看見的投影。
- Session／event cursors 約束查詢 scope 與 publication；不能把舊 cursor 在新投影繼續使用。
- Search watermark 表示索引追上 canonical publication；不是最後一次掃描來源的時間。

`NativeEvent.payload` 是完整已解析 JSON object；raw revision 才保存 whitespace、CRLF、壞行、尾端與 compressed bytes。Pi v1 衍生 IDs 已標明，不能當成來源實際提供的 ID。Native stream 可能同時有鏡像事件、互斥 branches、compaction checkpoints、subagents 與 metadata。Event count 不等於 distinct message／request／billable response count。

`serde_json::arbitrary_precision` 保留超出機器整數／浮點精度的數值，但 `Value` 不是原始 JSON 語法樹：重複 keys、exponent 拼字及 serde_json 的保留 number key 都不能被當成逐位元組的保存契約。需要原生語法證據時讀 raw revision，不從 normalized export 反推來源。

Cold evidence 是原始版本的唯一權威；`ah_events` 是可替換的逐事件讀取模型，結構化 payload 服務分頁閱覽、工具訊息與 normalized export，避免每次查閱都重掃 native stream。這符合 ADR-010 不把 per-row raw JSON 當 hot archive contract 的邊界：投影能重建，永久原始版本留在 cold evidence。代價是文字與部分 native 資料重複。10 萬筆合成資料的 42,144,599 bytes 來源形成 124,038,305 bytes 整體 archive，約 2.94 倍；此數字包含 evidence、投影、索引等檔案，不能全部歸因於 payload。後續若大型工具／附件 body 的實測壓力需要按需讀取，仍須保持分頁、原始完整性與重解析契約。

重解析的 archive protocol 是 `prepare_projection(source_id, raw_revision_id, parser_version, force)` → `ProjectionAttempt` → `append_projection_events` → `publish_projection_attempt`。Attempt 有自己的 `id`；raw revision 不因重解析再複製。Versioned parse report 以 `(raw_revision, parser_version)` 儲存；`current_projection` 指 raw revision，`current_projection_id` 才是 hot publication。不要把這兩者在 cursor／cache 中互換。

`parser_version` 必須涵蓋 adapter 語意與 record／decode budgets，避免增加限制或修 parser 後仍錯用舊 report。目前 capture identity 在 `capture/projection.rs` 是 `agent-history-parser/1:record={}:decoded={}`；改變 parser 語意要提升 `/1` 的版本，不能只改 search schema。`force` 允許相同 identity 明確重解析；失敗 attempt 不應取代已發布事件。舊 begin／append／publish wrappers 保留給序列化 caller，使用 default parser version；新整合應使用完整 attempt API 或高階 backup 流程。`retire_projection` 可撤下不再是 current 的 native generation，併入 search tombstone，raw evidence 仍保留。

高階 API 是 `Archive::reparse_source(source_id: &str, options: &BackupOptions) -> Result<BackupReport>`；CLI 是 `reparse --source-id ID --execute`，搭配正常 `--archive`／`--password-stdin`，不接受 live `--source`。它只讀該 source 最新 retained native bytes 與 durable reference bindings；provenance path 不做 existence check／canonicalize／open。只可重解析 transcript，SQLite logical snapshot 會拒絕。Reference binding 記錄當時哪個已授權 target revision 提供 bytes，offline reparse 驗證該 retained revision，不讀原路徑。新增 parser 辨識出從未保存的 reference 時仍會報缺口，不能靠 reparse 補回不存在的附件。

Binding observation 追加保存 target revision，不改已保存 raw；同一 source 被先當 reference metadata 收集、後來確認為 transcript 時可提升角色，不改 source identity，也不降級既有 transcript。Publication 的 parser／reference coverage 與 metadata 同一 transaction 發布，stale source head／projection attempt 的修補會被拒絕。

## Raw capture 的可靠性與成本

一般檔案先從受約束的 handle 完整讀取並 hash；新內容再讀一次寫入 encrypted chunks，驗證 identity／metadata／digest 後發布。對 source change 最多重試三次。未改變檔案仍需 hash 整份：**增量是 retained content／storage deduplication，不是只讀新增 bytes 的增量游標**。這是避免同大小、同時間戳改寫漏資料的明確取捨；不能宣稱大 corpus 的空備份成本是 O(新增資料)。

每塊 256 KiB，用 SHA-256 content address；manifest 封存後再驗證全部 chunk 與完整 digest。未完成 revision 不可被 raw reader／export 當有效版本讀取。來源移除不刪 archive；清理只針對 abandoned／duplicate staging 與沒有 retained reference 的 chunks。Raw retention policy／使用者刪除歷史還不是此核心的自動行為。

來源有不穩定、失蹤、permission／parse 問題時，保留之前 evidence 並回報 partial；destination write／archive integrity 錯誤中止工作，不能包成單一來源不可用。Durable backup run 有 running、complete、partial、failed、interrupted；重開會將未完成 running 記為 interrupted。成功回傳 report 仍需檢查其 status。

每個檔案有自己的 capture 邊界，沒有跨 transcript／card／DB／附件的全域原子快照。Unix 以 directory descriptors／`O_NOFOLLOW` 逐 component 開啟來源；Windows 做 reparse-point 檢查，但標準庫沒有等價的 descriptor-relative traversal。敵意的並發 directory replacement 在 Windows 仍是已知限制，不能寫成已證明消除的 race。

## SQLite evidence 的不同承諾

`pathkeep-sqlite-evidence-v1` 在一個 read transaction 裡讀 schema、table／column metadata 與 rows，包含 committed WAL-visible 資料；不將 main DB、WAL、SHM 各自複製後拼成「一致快照」。也不產生 plaintext staging database。

Integer、real bits、text bytes、blob bytes、null 分開編碼；保存可透過 SQL 定址的 rowid。TEXT 使用 SQLite UTF-8 API，不是原始 UTF-16 cell encoding；所有 rowid alias 被遮蔽且無可用 integer PK 時，會報告 hidden rowid 不可取得。SQL connection 雖為唯讀，SQLite 仍可能透過 WAL shared-memory sidecar 協調，不能承諾 SHM 位元組完全不變；不授權更改 conversation rows。

`raw-export` 在這種 revision 輸出的是 logical evidence JSONL，report 的 `source_capture_format` 會標明。它不是 `.sqlite` 檔，沒有 native restore utility；不能只看 raw 一詞就宣稱 byte-exact DB recovery。Codex state/history DB 目前作補充 evidence，沒有從中建立完整 Desktop sidebar／pin／title 投影。

## 解析、附件與 publication

Native bytes 先保存，parser 再從 archive 的 authenticated reader 讀取。Zstd 解碼、record 過大、unknown version、缺 metadata 等問題不會刪掉 raw。解析的 staging rows 和發布 generation 分開；讀者持續看到已發布版本，沒有把一半舊、一半新拼成同一份成功輸出。

Diagnostics 保存 code、來源位置、ordinal 與必要 detail；parser 不把整個 conversation body 或可能含內容的 decoder error message 塞進診斷。未解決 reference 的原始 locator 可能含 URL credentials 或不合法文字，因此 resolver 不把它回填到 diagnostic path／detail；精確值仍在 encrypted native payload／parse report。一般來源 path 仍可能含本機項目名，不能把所有診斷當成可直接送 telemetry 的無敏感資料。採樣有上限，省略數以 `diagnostics_omitted` 記錄。不能只因診斷清單未列出每個壞行，就當成其他行都沒有錯。

References 是不受信任的資料。Resolver 只接受被選取 roots 內的已知本機 artifact；拒絕 traversal／symlink／reparse、敏感設定／credentials、foreign／remote file URI，不抓 URL。選一個檔案只授權那個檔案，不能藉相對 reference 擴大成整個 parent directory。額外的本機 tool-output／image root 需明確提供。

Pi `parentSession` 是 provenance path，fork 通常已複製內容；缺 Pi lineage 會保留 provenance-only 診斷，不視為子檔原文缺失。Codex `history_base` 則可依賴另一 rollout 的 bounded prefix，屬必要歷史依賴。不能讓同名 `lineage` 掩蓋兩者差異。現階段沒有完整 materialization／cycle／boundary replay；Codex inherited history 因此明確 partial。DeepSeek spill locator 是 unauthenticated text footer，只回報可能缺失，不按文字開檔。

## 搜尋與匯出

Tantivy index 透過 encrypted directory 保存。Canonical publication 與 outbox 在同一 transaction 更新；search 同步明確執行，讀取前後檢查 watermark，落後時回 `IndexBehind`。一般查詢不隱藏重建；`sync-search` 同步索引，缺失／版本不相容的 metadata 可觸發重建，但損壞索引需明確 `sync-search --rebuild`／`Archive::rebuild_search`，不能把 integrity failure 當成空索引忽略。`SearchPage.complete` 表示已知 normalization coverage，不是「此結果已包含所有 remote history」。

搜尋將 Unicode 字母／數字／underscore 組成 word，lowercase；Han／kana／Hangul 每個 scalar 各自 token。Distinct tokens 以 AND 結合，harness／project exact filter；長 word 用固定雜湊 term 避免索引引擎默默丟棄。這不是 phrase／substring／fuzzy／regex／semantic search。CJK AND 也不要求字元相鄰或順序。

回傳 top-k≤100、matching event total、bounded plain-text excerpt；搜尋 `next_cursor` 保留，但 Rust 值為 `None`、JSON 為 `null`。Source／session／event／revision list 才有 pagination。Cursor 不是授權 token，不應放進遠端 API 後就視為驗證身分。

Normalized JSONL／Markdown 保存 session provenance 與 native payload，`native_resume_ready=false`。Markdown 將內容當 literal data 寫入，不能執行其中 code／links。Raw export 驗證 chunks 與整體 digest；通過只證明 captured revision bytes。CLI 在目的地私有 staging 後才發布，拒絕覆寫與 source/archive 內輸出；library writer caller 必須自行做到同樣的 publication discipline。

## Resource budgets

下面是程式目前的預設或硬界線，目的在於讓超限可觀察，**不是已量測的性能保證**。Persistent parse／run report JSON 上限 16 MiB；source ID 與 parser identity 上限 256 bytes。

| 邊界              | 值與行為                                                                                |
| ----------------- | --------------------------------------------------------------------------------------- |
| Preview artifacts | 256 筆 sample；總數另計、明示 truncated                                                 |
| Native record     | 預設 16 MiB；backup 可調至最高 64 MiB；超限 drain、保留 raw、診斷 partial               |
| Decoded source    | 預設每個 artifact 8 GiB；可透過 `BackupOptions` 明確調整                                |
| Zstd window       | 64 MiB；decode 全部 frames，錯誤不當 EOF 成功                                           |
| References        | 4,096 個、單 target 64 KiB、合計 target 4 MiB；超限 partial                             |
| Diagnostics       | 128 samples 加 omitted counter                                                          |
| Projection batch  | Capture 以 128 events／4 MiB flush；storage 另有 batch 與 single-event 上限             |
| SQLite            | 預設 row 64 MiB、deadline 15 分鐘、busy timeout 2 秒；`SqliteCaptureOptions` 可明確設定 |
| Query／search     | List 1–500；search 1–100、query≤4,096 bytes、1–64 distinct terms                        |
| Search writer     | 32 MiB writer budget；bounded canonical page 與 encrypted cache                         |

來源檔／語料總量沒有以 whole-corpus Vec 收集。仍有每個 record 的 JSON object／文字配置、hash 全檔 I/O 與 SQLite row 編碼成本；本次單一 1,440 萬筆 Pi transcript 的驗證見下。多 session 大量探索、其他 harness 的同量級資料和 Windows 不在此量測範圍。

分頁的 byte budget 允許單一大型 record 自成一頁，不會截斷原始內容。這些 archival DTO 不能不加處理地當成前端 IPC payload：產品整合須提供摘要／按需 body 或串流，把大型 JSON 解析與渲染移出主執行緒；目前沒有 UI／IPC，不能由核心量測推論前端已符合 60fps。

## 擴展與版本維護

1. 先固定 upstream revision，讀 writer／reader／migration／export，不只看 README。更新研究文件的來源路徑、格式範例、origin／identity／attachment 邊界。
2. 先寫失敗場景與有來源證據的 synthetic fixture。包含真實 envelope／lifecycle／sidecar／compressed frames，避免只造 parser 喜歡的 happy path；不提交私人 session。
3. 在 `Harness`／adapter registry 加讀取器；將 discovery、transport、native projection 保持獨立。不要從 adapter 讀 reference、執行 native loader 或寫原始資料。
4. 更新 default source rules、CLI identifier／help、能力矩陣與跨平台缺口。額外格式可選明確 root，不能靠遍歷整個 home 尋找所有 `.json`。
5. Archive schema 變動在 `archive/schema.rs` **追加** migration，不改已發布 migration 的意義；改 raw evidence encoding、export shape、cursor semantics 或 search tokenizer/schema 時，各自更新其 format/version 與相容性測試。
6. Parser 語意改變必須使已保存 raw 可以重新產生新 hot projection，不能只更新程式後沿用舊 parse report。新 projection 完成後才切換；測試同一 raw revision 的重解析與舊 cursor 失效，再同步 search。
7. 執行核心 E2E／integrity 與 repo gate，保留可重複 artifact。新平台宣稱需要真實執行證據；fixture pass 不等於真實 harness／平台驗證。

這次依使用者要求把測試執行集中到實作末尾；這不改變 repo 的權威 gate，也不是未來維護可以省略測試的理由。

## 後續工作與不能默認成立的事

- Native transcript replay／branch view、Codex inherited prefix materialization、Desktop card enrichment、project alias／pinning 都未完成。
- Native resume、原生目錄重綁、account re-home、跨 harness import 只有研究；沒有執行 API。各 harness 的限制連到 [研究入口](README.md)。
- Token／cost／response dedup analytics、AI 摘要與 embedding／semantic search 未實作。保留 usage payload 不等於已正確計算費用。
- 一般 Claude chat／新 cloud Cowork／remote-only Codex 和 provider media 不是完整可自動抓取的本機來源；沒有登入／私有 web API。
- DeepSeek legacy SQLite／v0–v3 完整 codec、新 Cline SDK／其他 storage generations、Gemini／Goose／OpenCode 等 research-only 格式不能被 adapter 名稱掩蓋。
- `reparse` 已有獨立 CLI／library 操作，產品 UI 尚未整合。它只重建 retained native transcript，不是一般格式轉換器，也不觸碰 live source。
- 沒有 raw retention 刪除策略或 archive portability/import bundle。單檔 raw export 不是整個可直接恢復的 harness home。

## 驗證紀錄

本機量測曾發現：Turso 0.8.3 對索引建立的 joined event page 雖使用 composite seek，仍為每頁執行 `ORDER BY` sorter，10 萬筆時反覆排序剩餘內容並溢出到加密暫存。取樣在 `artifacts/agent-history/scale-stack.txt`，修正前後 query plan 在 `search-plan-before.log`／`search-plan-after.log`。現在每個來源讀一次已發布 metadata，再以三個 bound parameters 直接 seek event index；回歸同時驗證完整索引條件與沒有 sorter，不能只看 index 名稱。

2026-11-11 已完成集中驗證；去識別、可版本化的結果在 [validation/](validation/README.md)。完整 logs、query plans、crash checkpoints 與 synthetic E2E reports 留在 `artifacts/agent-history/`，桌面證據在 `artifacts/e2e/`。原先工作目錄的未提交變更保持原樣；repo gate 是對這個共有工作目錄執行，本次提交只納入 agent-history 的變更。

| 驗證               | 實際結果                                                                                                              |
| ------------------ | --------------------------------------------------------------------------------------------------------------------- |
| 核心 crate         | macOS 與 Linux 各 220 passed、0 failed；strict Clippy 通過                                                            |
| Verifier           | 6 個 guards 通過；先證明缺 Git provenance、漏掉中間事件、重複尾端與 partial count 會被拒絕                            |
| Production CLI E2E | 包含四個必做 harness、壞壓縮、raw-only 257 檔超出 preview 上限、來源刪除、分頁、精確匯出、crash／重解析／附件邊界     |
| 真實本機樣本       | 四個必做 harness 共 7 檔；raw SHA 一致、原始 bytes／mtime 未變、重開冪等；抽樣缺 lineage 明示 partial，私人副本已清理 |
| `bun run check`    | exit 0；135 個桌面 E2E 通過（125 main＋9 healthy backup integrity＋1 damaged）                                        |
| 14.4M              | Linux release、真正 4 CPU／8 GiB 硬限額、90 分鐘上限；全量搜尋精確計數、raw SHA、重開冪等與 integrity 全通過          |
| 平台與範圍缺口     | Windows 未執行；沒有多 session 同量級探索或前端流暢度驗收；CPU 時脈未校準為 3 GHz                                     |

14.4M 結果：6,185,244,596 bytes 的來源，17,964,192,516 bytes 的整體存檔。首次備份 473.605 秒，未變更重跑 4.551 秒，中文搜尋／精確計數 0.840 秒，raw export 16.001 秒，完整性檢查 74.121 秒。最高程序 RSS 275,132,416 bytes（262.39 MiB，含 256 MiB KDF）；CLI 時間包含程序啟動與解鎖。全部 14,400,000 個 native events 皆命中共同詞，唯一尾端標記只命中一次，header 不計入事件總數。這是單一大型 Pi fixture，不代表所有 harness／磁碟／CPU 都有相同時間。

本機 100k debug CLI 量測首次備份 16.488 秒、未變更重跑 1.194 秒、峰值 331.16 MiB。曾實際取樣到 joined event query 的反覆排序；修改前的 query-plan regression 失敗，直接 composite seek 後通過，才重做全量驗證。沒有以「查詢用了 index」代替「沒有 sorter」的證據。

Modal 的 scalar CPU／memory 參數只是 reservation，不能當硬上限。第一個嘗試缺 GNU time 而退出，第二個在發現可 burst 後停止；成功的一輪以 API tuple 設定 CPU `(4,4)`、memory `(8192,8192)`，guest 確認 4 CPUs／affinity 4。只有合成資料上雲，三個 sandbox 全部停止，沒有保存新 cache snapshot，沒有改已安裝 skill 或 repo Modal config。詳細生命週期與兩次前置失敗均保留，沒有將它們記為完整驗收。
