# Agent history：實作與維護交接

WORK-AH-EVIDENCE-FORMAT，2026-11-13。此處記錄目前尚未發布的格式；完整
schema、每個 index、量測、取捨與維護者審查在
[crate README](../../../../src-tauri/crates/agent-history/README.md#evidence-format-work-block-2026-11-13)。
本 block 依使用者限定只修改 crate 與本頁；core／feature／product-design 文件的
其他部分留給後續 block，不能把舊頁的全程加密、256 KiB、SQLite v1、source=session
或 legacy wrappers 當成目前契約。沒有舊格式遷移或相容路徑。

## 責任邊界

- `adapters/*` 回傳 native facts、conversation key 與 source retention；不開 reference
  指向的檔案。預設 origin 是第一筆原始 record 的 digest；Claude 以檔名／subagents
  下的相對位置辨認 native key；Codex 用 thread ID 與 native creation timestamp；
  DeepSeek 用 id／createdAt，跨 generation 不換身分。沒有 native ID 才退回 source key。
- `archive/{conversation,selection}` 把 key 與來源的投影一起發布；source membership
  是可重建資料，不另建立跨資料庫的 canonical mapping protocol。
- `query/{parents,conversation_sources,events}` 在讀取時解析原生連結。parent_native_id
  可先存在，parent_id 要等 parent 已在存檔中；Claude 優先同 root 的 sibling transcript。
  Codex history_base 保留 immutable rollout ID、exclusive ordinal 與 decompressed byte
  offset；不把 rollout ID 當 thread ID，不猜另一段對話的 hash。歧義仍是 unresolved。
- `archive/lineage` 在發布前驗證 boundary，結果 keyed by immutable raw revision；讀頁
  查這個結果，不每頁重掃 inherited prefix。事件仍有 physical source_id／ordinal。
- `archive/{revisions,reader,retention,maintenance}` 管理 staged→sealing→complete、
  manifest、prefix sharing 與只回收零引用 slices 的 queue。Parser 不能刪除 raw evidence。
- `archive/user_state` 只接受使用者的 mark。Canonical state 在 evidence DB；catalog 的
  indexed cache 用 generation 重播，查詢持 publication gate，cache 未追上就重播或失敗。
  Reparse、search rebuild 不改 canonical star／put-away／trash／title。
- `archive/observations` 保存每 run 的 source seen/missing 與變化的 folder facts；
  last_seen_at／missing_since 是這些觀察的查詢 cache。讀不到 root 不推論檔案消失。
- `capture/{observations,git}` 只從 adapter 的 structured cwd 觀察 folder；.git、HEAD、
  commondir、gitdir、config 各限 1 MiB，不執行 git、不讀 includes/index/objects/hooks。
  Git file 使用原 pathname 的 descriptor traversal，不能在驗證後 canonicalize 到另一個
  target 再讀。Remote URL 先去 userinfo 才持久化；`--no-git` 不讀任何 git file。
- `archive/mod` 繼續擁有 standalone CLI root 與 owner lock。加密選擇由 format marker
  識別；不加密沒有 envelope，持久化 DB／Tantivy 不加密，spill 始終由 process key 保護。
  與產品 Vault roles 的整合不在此 block。

## 保存與發布

唯一 slice size constant 是 4 MiB。Release encrypted Turso 的 1／4 MiB 合成 JSONL
壓縮成 337,783／1,358,842 bytes；cold median 0.543／1.869 ms，warm median
0.061／0.224 ms。Cold 是 synthetic files 做 F_NOCACHE sweep 後重開 Turso，沒有
全機 cache purge。First write 174.8／249.7 MiB/s、後續 222.8／280.4 MiB/s，按 stored
bytes 計。量測 source 在 crate examples；crate release 產物已依要求清理。

Slices 使用 zstd level 3，地址仍是未壓縮 bytes 的 SHA-256；native zstd transport 不重壓。
Appended source 的每個 revision 保留 digest／length／capture time；驗證新內容在舊長度
處的 running digest 後，舊 prefix 直接指到新 manifest，沒有 A→B→C chain。Manifest
有 byte offset，已開的 reader 也可跨不同切片邊界繼續，最後一塊裁到自己的 revision
長度。Retarget、manifest removal 與 garbage candidates 在同一 transaction 提交；
回收在短 publication gate 外，以固定批次且依引用判定。Content 掃描也在 gate 外，提交前
再確認 source head／publication 未變。

State source 僅最新 verified snapshot 保有 bytes，舊 record 用 bytes_retained=false 表明。
State A→B→A 不能重用已失去 bytes 的舊 handle，要有新的 verified manifest。
Document／write-once changed revisions 全保留。Codex thread_history_N.sqlite 不再發現。
SQLite logical evidence v2 用 JSON string 保存有效 UTF-8 TEXT；無效 TEXT／BLOB 繼續以
storage class＋hex 保存，integer／real bits／null／rowid／一致 read transaction 不變。

Projection API 是 prepare_projection → ProjectionAttempt → append_projection_events →
publish_projection_attempt；舊 begin_projection／append_events／publish_projection、
無版本 parse-report wrappers 與 DEFAULT_PARSER_VERSION 已刪除。Parser identity 是
agent-history-parser/2 加 record／decoded budgets。失敗 raw／projection attempt 都不取代
已發布資料；restart 把未完成 run 標成 interrupted，staging 清理由既有 bounded protocol
處理。內容相同但回到較早 revision 時，head pointer 仍是觀察到的版本，不取 MAX(id)。

## 查詢與限制

List ID 是 conversation ID，source membership 在 sources array。Put-away 預設不列出，
搜尋照常；trash 排除自己與可解析的 descendants，含 search total；raw/revisions/events
仍可按 ID 讀。Trash-only 只列自己有 trash flag 的對話。No permanent deletion。

Source identity 仍是 domain／harness／canonical root／relative path；conversation hash
的每個輸入有 u64 big-endian length prefix，不使用 capture clock、root 或發號 counter
（無 native ID 的 source-key fallback 是明定的例外）。分支看 native last activity，平手用
content digest；byte-equivalent copies 才以 source ID 做 deterministic tie。

逐事件 ah_sessions／ah_events 暫時保留。Search 仍是 top-k lexical AND、exact filters、
CJK scalar token；沒有 streaming／phrase／regex／usage dedup。它仍可能搜尋 native
mirror、branch 與 copy records，不能當 distinct human-message count。Normalized export
讀 current line，但仍不宣稱 native resume ready。Read-model replacement 是後續 block。

Turso 對 computed view 的 parent alias 在 nested query 回 no such column: parent_id；本地
重現形狀是 view SELECT s.*, correlated expression AS parent_id，再從另一個 view／CTE
讀 alias。實作改成直接 indexed SELECT 與 read-time parent lookup，不修依賴、不對外聯絡。

Native rollout 檔名以 checked UTF-8 slicing 擷取 ID，陌生 Unicode 檔名不能讓 raw-preservation 流程 panic。

本 block 只驗證 synthetic fixtures 的 macOS 執行；不把下面上一版的 Linux、真實樣本、
14.4M 數字當成新格式重測。未做 Windows／Linux runtime、實體斷電、每個 state-cache
replay／retention crash point、git race scheduling 或目標機多來源 profiling。已有 subprocess
capture/reparse crash tests 與新多 slice/open-reader/reassignment tests；這些不是全平台或
所有 crash windows 的證明。所有改動保持 uncommitted。

## 2026-11-11：上一版格式的驗證紀錄

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
