# Agent history 核心

狀態：獨立核心庫與 CLI；產品 UI 整合尚未開始。使用者要求先交付研究、可靠備份與可維護的底層，之後由 Claude 接手 UI／UX 和產品呈現。本文件不指定 sidebar、tabs、強調色或 onboarding 的最終方案。

核心在 `src-tauri/crates/agent-history`。它保存所選來源的歷史 revisions，提供按 harness／項目路徑篩選、建立／更新時間排序、事件分頁、全文搜尋及匯出。它不依賴模型、embedding、provider 帳號或雲端服務，也不使用 PathKeep 自己的 assistant conversation store。

## 能力與來源邊界

| Adapter          | 本次來源與正規化範圍                                                                                         | 保留但不應誤稱為完成的能力                                                                                                                                        |
| ---------------- | ------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Claude Code      | 原生 JSONL、subagents、已知工具／附件資料；本機 Desktop cards、Cowork transcript／audit 等 evidence          | Desktop cards 仍是 raw metadata，不完成 card-to-transcript enrichment；Cowork 有 native transcript 時 audit 不重複索引。一般 Claude chat／cloud Cowork 不自動抓取 |
| Codex            | Active／archived、plain／Zstd rollout，legacy 與 paginated native events；相關 state/history SQLite evidence | 不 replay inherited prefix，不去重鏡像 UI／model events；metadata DB 不轉成完整 Desktop UI 狀態。Remote media 只報告不可用                                        |
| Pi               | v1–v3 JSONL、原始樹、compaction、context edit、extension records；inline image 與已知 full-output reference  | 不選 active branch／replay model context；v1 衍生 ID 有診斷。Desktop／TUI／RPC 來源無足夠證據就 unknown                                                           |
| DeepSeek Harness | 目前 v4 JSONL／多 frame Zstd、數字 generations、附件 object；已知 native event 欄位                          | 舊 v0–v3 與未知新版本明確限制解析；舊 authoritative SQLite provider 不支援。Spill 文字 footer 不授權讀檔；desktop／CLI 共用來源不能推斷 origin                    |
| Continue         | Session JSON 的 `history` entries，保留 envelope metadata 與原始檔；index 獨立保存                           | 不以 index 代替資料發現；不補回來源 CLI 未保存的 system context，不從 filesystem 時間杜撰原生建立時間                                                             |
| Cline            | 經典 task bundle 的 API conversation array；UI/context/task metadata 保留                                    | API／UI 記錄不視為兩份新聊天；不宣稱覆蓋未知新儲存世代或全部 editor profile                                                                                       |
| Roo Code         | 經典 task bundle 的 API conversation array 與 legacy fallback evidence                                       | 不呼叫可能刪舊檔的 native loader；compaction／truncation evidence 保留，並非已重建每個原生 branch                                                                 |

這是格式能力矩陣；測試結果與平台證據見 [交接紀錄](../architecture/research/agent-history/implementation-notes.md)。其他 harness 的調研不代表已有 adapter。完整來源、版本與已知坑見 [研究入口](../architecture/research/agent-history/README.md)。

## 使用者可依賴的資料契約

來源保持唯讀，不啟動 harness，不執行來源 loader 的修復／遷移，不讀登入狀態來補 cloud history。只從已知資料位置與明確選取的根目錄收集；聊天或工具文字不能授權讀取任意路徑。可用的本機附件以來源與 reference 關係保存；缺失、越界、遠端或無法解析的內容回報診斷。

Raw revisions 不跟著來源刪除而消失；重寫、縮短、同大小內容變更和再次出現舊內容都有自己的觀察結果。Ordinary native files 保留原始 bytes，包括壓縮、未知欄位、壞行與尾端碎片。SQLite 是一致 transaction 的 logical schema／rows evidence，export 明示格式，不能改名成 `.sqlite` 便宣稱可直接還原。每個來源檔各自驗證，不宣稱整個 harness 的所有檔案同時停在同一時間點。

核心庫回報 raw integrity、normalization coverage、search freshness 與診斷；未解析到的資料不等於空對話。已發布的歷史不因一次失敗的新 capture 被覆寫成空資料。Archive、衍生資料與搜尋索引沿用 repo 的加密儲存契約；只有使用者明確匯出的檔案是交付給其指定 writer／destination 的明文。

Parser 更新後可用 `reparse --source-id ID --execute` 從已保存的 native revision 重新建立投影；即使來源已消失，也只讀 archive 與先前保存的 attachment bindings，不重新掃描本機或補抓新檔案。Raw bytes 不變，發布成功才切換新投影；未保存的 reference 仍顯示缺口。這個操作不適用於把 SQLite logical evidence 當 transcript 解析。

## 查詢與匯出

`sources` 分頁列出保留的原始來源，包含附件、metadata 與無法解析的 transcript；不依賴 live source 或 parsed session。來源刪除後，仍可從這個清單找到 source／revision ID 並 raw-export。`plan` 的 256 筆預覽上限不限制已備份清單。

Session list 可以以 harness、**exact recorded project path** 篩選，按建立或更新時間排序；不同 harness 記錄相同 path 時可以一起查。它沒有 project alias、pin、路徑合併或 repo/worktree 語意判斷。Session／event 游標約束 publication，投影更換後舊游標需重新開始；raw revision list 則以 source 與 revision ID 向較舊資料分頁，新增 capture 不破壞已開始的歷史遍歷。

搜尋是區分 exact harness／project filter 的 lexical AND term search；不區分字母大小寫，CJK 以單字元 token 處理。沒有自然語言推理、語意相似、模糊／regex／phrase query。查詢回傳 top-k（上限 100）與 matching event 總數，**不提供搜尋結果逐頁翻完**；不要把保留的 `next_cursor: null` 當成分頁功能。索引落後時回錯，不以空結果掩蓋；`sync-search` 同步衍生資料，損壞索引另用明確的 `sync-search --rebuild` 重建。

匯出有 raw revision、normalized JSONL、Markdown。Normalized 格式仍含 native payload、ordinal、parent evidence；它保留事件順序，可能包含互斥分支、mirror records 與 metadata，不能當已去重、可直接 resume 的 native conversation。沒有 HTML／PDF、跨 harness native import 或帳號遷移。Raw export 的「完整」僅指該 captured revision 的 bytes，不是整個帳號歷史或附件全齊。

## 這次沒有做的產品功能

沒有 UI／typed IPC／設定／onboarding／排程整合，沒有執行 native resume 或遷移，沒有可安裝 plugin runtime。Adapter trait 與分層已提供可維護的擴展位置，但新 adapter 仍須納入程式與驗證。

原始 usage、model 與 timing evidence 會保留；token／cost／response 去重 analytics 尚未交付。不能直接加總各 event 的 usage，因為 streaming blocks、mirror events、fork inherited history 與 compaction 都可能重複。同樣沒有完成 AI 摘要、語意搜尋或 project pinning；未來功能應建立在可重建投影上，而非改寫原始 archive。

## UI／UX 接手條件

後續產品整合需在背景 worker 使用同步 Rust API，保持主線程可響應；自行提供 PME 選取／預覽／執行、取消與進度，明確顯示來源／解析／附件差異。不得因有一個 complete raw revision 就把整個來源標成完全成功。User-visible copy、error／empty／loading／disabled 與 accessibility 都仍須交付三語。

Resume、項目移動與跨 harness 轉移需先按 [Claude](../architecture/research/agent-history/claude.md#resume-and-migration-research-no-execution)、[Codex](../architecture/research/agent-history/codex.md#resume-and-project-migration-research-only)、[Pi／DeepSeek](../architecture/research/agent-history/pi-deepseek.md) 的證據重新定義 per-version 能力；目前 `native_resume_ready` 一律 false。平台、效能、未知格式與限制必須從交接紀錄延續，不因換了一層 UI 就消失。
