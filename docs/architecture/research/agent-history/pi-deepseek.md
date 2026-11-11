# Pi 與 DeepSeek Harness：儲存機制研究

研究日期：2026-11-11。本文只記錄來源證據、備份契約與後續維護事項；不是產品 UI 設計，也不代表本文列出的能力都已實作。實際支援範圍以核心庫 capability matrix 與驗證紀錄為準。

## 證據與方法

以官方原始碼為主要依據，讀取寫入、讀取、遷移及匯出路徑；issue 用於辨識已發生的失敗方式，不把 issue 的舊行為當作目前版本的事實。沒有執行外部專案的程式、測試或安裝腳本。沒有上傳本機聊天內容。

| 專案                                                        | 本次固定版本                                                                                                                                      | 用途                                |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------- |
| Pi，原 `badlogic/pi-mono`，目前重新導向 `earendil-works/pi` | [`49fe0a237450eb003434d4020b0bfdf53bc6449c`](https://github.com/earendil-works/pi/commit/49fe0a237450eb003434d4020b0bfdf53bc6449c)                | 來源格式、寫入與官方匯出實作        |
| DeepSeek Harness                                            | [`fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159`](https://github.com/deepseek-ai/deepseek-harness/commit/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159)     | dsh `0.2.2-rc.1`，來源儲存實作      |
| Agent Session Browser                                       | [`834a16792a63b8b3c4802b43d0a0d33d995e060a`](https://github.com/gautamgpt1/agent-session-browser/commit/834a16792a63b8b3c4802b43d0a0d33d995e060a) | Pi 解析、來源唯讀與顯示模型的比較   |
| resume-from                                                 | [`3cdc8a089157a015702648e2e4177b64096850dc`](https://github.com/alexei-led/resume-from/commit/3cdc8a089157a015702648e2e4177b64096850dc)           | 跨 harness 轉移與安全發布新檔的比較 |

本機只做唯讀結構驗證：Pi 預設 sessions 根目錄有 37 份 JSONL、14 個項目目錄；抽查 20 份的 header 都是 v3，每份最多讀前 1,001 行，只輸出欄位集合與分類計數。觀察到 `context_edit`、`custom_message`、`system`、`bashExecution`，不能把 Pi 簡化成 user／assistant／toolResult 三種訊息。本機安裝目錄包含 1.3.0、1.4.0；這不是「所有檔案都由該版本產生」的證據。未讀 `auth.json`。未記錄私人項目路徑或對話內容。

## Pi：來源位置與身分

預設路徑是 `~/.pi/agent/sessions/--<encoded-cwd>--/<timestamp>_<session-id>.jsonl`。三平台均以 Node 的 `homedir()` 為起點；不是 macOS Application Support／Windows AppData 專屬資料夾。目錄編碼會去掉開頭的分隔符，將 `/`、`\\`、`:` 換成 `-`。**此編碼不可逆，也不保證不碰撞**；項目路徑必須讀 header 的 `cwd`，不能從資料夾猜回去。

來源發現還要涵蓋：

- `PI_CODING_AGENT_DIR` 改變 agent 根目錄；預設 session 根為其下的 `sessions`。
- `--session-dir` > `PI_CODING_AGENT_SESSION_DIR` > `sessionDir` 設定，指定另一個 session 目錄。
- `--session <path>` 可以開啟上述目錄之外的 JSONL。無法僅靠掃預設根目錄發現所有這類檔案；核心庫需要使用者明確提供的額外來源。
- SDK 可指定自訂 session ID／路徑，或使用 `SessionManager.inMemory()`；`--no-session` 不持久化，結束後沒有可備份的本機 transcript。

證據：[config.ts](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/coding-agent/src/config.ts#L602-L635)、[main.ts 的 session 選擇與優先序](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/coding-agent/src/main.ts#L734-L744)、[SessionManager 的路徑編碼與建立](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/coding-agent/src/core/session-manager.ts#L529-L545)。

Session header 的 `id` 是 native session identity，預設由 UUIDv7 產生，但允許自訂字串；不能把 UUID 格式驗證當必要條件。`parentSession` 是來源 session 的**路徑**，不是 ID，也不足以證明是 subagent。相同 ID 在兩個來源根目錄出現時，要保留來源來源域及內容差異，不可只用全域 `id` 去重。

## Pi：JSONL 與樹形歷史

第一筆有效資料應為 session header：`type: "session"`、`version`、`id`、ISO `timestamp`、`cwd`，可帶 `parentSession`。目前格式 v3。後續 entry 有 `type`、`id`、`parentId`、ISO `timestamp`；entry ID 通常是 8 位十六進位字串，但也可能是完整 UUID。nested message 的時間是 epoch **毫秒**，與 entry 的 ISO 時間不同。

| Entry                                   | 應保存的語意                                                                                |
| --------------------------------------- | ------------------------------------------------------------------------------------------- |
| `message`                               | 完整 `message`；包含 user、assistant、toolResult、system、bashExecution 及 extension role   |
| `model_change`／`thinking_level_change` | 分支上的 provider／model／thinking 設定變更                                                 |
| `usage`                                 | 額外 usage category；不能與 assistant usage 盲目加總                                        |
| `compaction`                            | summary、`firstKeptEntryId`、`tokensBefore`、details／usage／systemMessage；原始 entry 仍在 |
| `branch_summary`                        | `fromId` 及 summary，記錄離開分支時的資訊                                                   |
| `context_edit`                          | `targetId`、replacement 或 null；只改未來 model context 的投影，不覆蓋歷史原文              |
| `custom`／`custom_message`              | extension 狀態與內容；保留未知 data、details、display                                       |
| `label`／`session_info`                 | bookmark 與顯示名稱變更，不能當一般聊天訊息計數                                             |

`parentId` 構成樹，而不是「上一行就是上一輪」。`/tree` 在同一份檔案建立不同路徑；`/fork`／`/clone` 可產生新 session 檔。Compaction 與 context edit 都不表示舊原文已刪除。備份應保存全部 physical entries，再把目前分支／model context 當可重建的投影。按檔案順序攤平成一段對話會混合互斥分支。`branch()` 只變更記憶體 leaf，直到新增 entry 才留下新分支；「目前 UI 選取哪個 leaf」不一定能從磁碟重建。

版本差異：v1 沒有 `version`，是線性序列；v2 加 `id`／`parentId`；v3 將舊 `hookMessage` role 改成 `custom`。PathKeep 若正規化 v1，衍生 ID 必須可重現且標明 synthetic；不能照抄上游遷移時產生隨機 ID 的做法，否則每次備份都成為新事件。未來版本應照原樣保留並給出解析限制，不可丟棄未知 event。

證據：[entry definitions 與 migrations](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/coding-agent/src/core/session-manager.ts#L40-L316)、[branch／context projection](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/coding-agent/src/core/session-manager.ts#L334-L523)、[branching 實作](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/coding-agent/src/core/session-manager.ts#L1592-L1769)。

## Pi：內容、附件與 analytics

user content 可以是字串或 blocks。assistant 有 text、thinking、toolCall；toolResult 以 `toolCallId` 對應呼叫，並有 `toolName`、`isError`。Images 是 `{type: "image", data: <base64>, mimeType}`，**嵌在 JSONL 裡**。保留原始 bytes 已保存這些圖像；正文搜尋不要索引 base64。Provider 的 reasoning／text／tool signature 是不透明 continuity data，保留但不能解讀成可顯示的推理內容。

外部附件例外：`bashExecution.fullOutputPath` 與工具結果的 `details.fullOutputPath` 可以指向被截斷輸出的完整檔案，常在臨時目錄。來源檔可能已被清理；缺失時只能說 transcript 中仍有截斷版本，不能宣稱完整輸出已備份。應只辨識這些已知欄位並受允許來源根目錄約束，不能將聊天裡任意 `path` 欄位當成讀取指令。read 工具的 `path` 是工作檔案，不代表整個工作目錄是 transcript 附件。

assistant 可能有 `provider`、`model`、`responseModel`、`responseId`、`usage`、`stopReason`、`timestamp`、`durationMs`。usage 有 input／output／cacheRead／cacheWrite 與 cost；缺失是未知，不是零。模型可隨分支變更；單一「session model」只能是摘要，不能取代逐事件資料。延遲、費用、tokens 等 analytics 需帶來源欄位與計算規則，尤其不要將已保留的 fork 內容當成新模型消耗。

證據：[message content types 與 usage](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/ai/src/types.ts#L430-L480)、[message metadata](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/ai/src/types.ts#L560-L625)、[bashExecution](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/coding-agent/src/core/messages.ts#L26-L40)、[bash 截斷輸出](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/coding-agent/src/core/tools/bash.ts#L367-L380)、[read 圖像處理](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/coding-agent/src/core/tools/read.ts#L274-L295)。

Pi 本身不保證 extension 的 subagent 儲存方式。可辨識標準 SessionManager header 的額外來源仍可讀取，但不能憑 `parentSession` 或資料夾名稱杜撰 parent-child 關係。桌面包裝、RPC、SDK、TUI 使用相同 SessionManager 格式時，header 不提供 launch surface；應回報 unknown，而非硬寫 CLI／TUI。

## Pi：一致性與已知陷阱

目前 `_persist` 在首次 user／assistant message 時以 `wx` 建檔並寫出尚未持久化的 entries，之後 `appendFileSync`。設定、system 等 entry 單獨存在時可能還在記憶體，沒有檔案可備份。上游 `_rewriteFile` 以 `openSync(..., "w")` 截斷重寫，沒有暫存檔 rename，因此不能永遠假設 append-only。

**不要以 Pi SDK 開啟原始檔來做唯讀備份。** `loadEntriesFromFile` 在 EOF 有內容但沒換行時會對來源追加 newline；`SessionManager.open` 還會遷移／重寫 v1、v2。官方 standalone HTML export 會呼叫 `SessionManager.open`，因此也不能在原始來源上當作唯讀轉換器。PathKeep 要用自己的唯讀解析器，或只在隔離副本上做上游驗證。

備份需處理寫入中的最後一行、UTF-8 跨 buffer、來源被截斷／重寫、同 ID 不同內容、分支孤兒與 cycle。原始 bytes 仍可保存，解析狀態另行回報；不要像上游 loader 一樣悄悄 skip malformed JSON 然後宣稱 transcript 完整。當檔案正在變動時，保留可驗證的快照與觀察到的完整範圍，不把暫時的尾端碎片當永久損壞，也不讓 incomplete tail 推進增量游標。

歷史問題提供必測情境：

- [#6002](https://github.com/earendil-works/pi/issues/6002)：曾經 open 非 session 檔會覆寫；本次固定 source 已拒絕非空 invalid session。備份不可呼叫 mutating opener。
- [#8345](https://github.com/earendil-works/pi/issues/8345)：無換行尾端與部分寫入後，下一次 append 可黏在壞尾巴，導致 parent chain 缺失。本次 source 的讀取修補 newline 是另一個唯讀陷阱。
- [#1717](https://github.com/earendil-works/pi/issues/1717)：tool result 與 assistant message 的持久化順序曾造成損壞。不能用「看起來孤立」就刪除事件。
- [#8852](https://github.com/earendil-works/pi/issues/8852)、[#6242](https://github.com/earendil-works/pi/issues/6242)：另一套 `JsonlSessionStorage` 的 duplicate sequence／ID 與並發問題。Issue 自己指出 coding-agent 尚使用 v3 SessionManager；不要把 durable runtime 的格式混為 CLI v3。

寫入／讀取證據：[reader](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/coding-agent/src/core/session-manager.ts#L628-L670)、[open／migration／rewrite](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/coding-agent/src/core/session-manager.ts#L1037-L1142)、[首次 flush 與 append](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/coding-agent/src/core/session-manager.ts#L1168-L1210)。

## Pi：resume／遷移研究，非本次寫入能力

`pi --session <absolute-jsonl-path>` 可直接選擇 session；完整 ID 比 partial ID 安全，後者可能選到不同匹配。選 session 的工作目錄會影響工具、project settings 與 context；只在新目錄執行 `pi --session` 不等於重綁。

目前官方對不存在的 cwd，在互動模式提供 continue（只在本次使用目前目錄）、fork（新 ID、新 header cwd、保留來源）或 cancel；非互動模式報錯。`SessionManager.forkFrom` 複製來源 entries、建立新 header 並記錄 parentSession；這是比直接搜尋取代舊路徑更有證據的後續方案，但仍須另外處理外部附件及舊路徑引用。不能全檔替換絕對路徑，因為訊息文字、工具參數可能只是歷史事實。

官方 HTML export 包含全部 entries、header、leaf；TUI 可額外提供 runtime system prompt／tools，自檔案匯出則沒有該 runtime 狀態。可攜式 raw JSONL 保存 native 資料；閱讀用 Markdown／HTML 是衍生結果。PathKeep export 應表明是否包含全部分支、工具內容及缺失附件，不模仿 `/share` 上傳。

證據：[CLI session path/ID 解決](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/coding-agent/src/main.ts#L299-L329)、[跨項目 fork](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/coding-agent/src/main.ts#L438-L454)、[不存在 cwd 的處理](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/coding-agent/src/main.ts#L745-L761)、[官方 HTML export](https://github.com/earendil-works/pi/blob/49fe0a237450eb003434d4020b0bfdf53bc6449c/packages/coding-agent/src/core/export-html/index.ts#L235-L316)。

## Pi 相關工具：具體取捨

**Agent Session Browser** 的 [pi-parser.ts](https://github.com/gautamgpt1/agent-session-browser/blob/834a16792a63b8b3c4802b43d0a0d33d995e060a/src/server/pi-parser.ts) 逐行讀 JSONL、保存 native／parent IDs、將未知 entry 顯示為 provider event，並用有上限的 collectors 避免摘要階段保留全部內容。這是有用的分層參考，但其 catalog 保存路徑／摘要，開啟 transcript 時仍讀原始檔；不是資料備份。其 Pi originator 固定為 `pi-tui`，`parentSession` 存在就記 source=fork，不能作為 PathKeep 的來源辨識證據。所有 entry 設成 `turnId: "main"` 也不能取代分支投影；image 顯示 marker 不等於附件備份。

**resume-from** 的 [parse.ts](https://github.com/alexei-led/resume-from/blob/3cdc8a089157a015702648e2e4177b64096850dc/src/adapters/pi/parse.ts) 驗證 duplicate ID、cycle、missing parent，選 active branch 和 compaction 後的 context，刻意移除 tool result body／reasoning 並做 redaction；未知類型會計數。這是為跨模型轉移而選擇的有損模型，不能當 archive canonical representation。

它的 [serialize.ts](https://github.com/alexei-led/resume-from/blob/3cdc8a089157a015702648e2e4177b64096850dc/src/adapters/pi/serialize.ts) 建新 session ID、將 imported tool call 寫成 outcome 文字以免產生缺 toolResult 的原生協定、將 summary 當普通文字以免 native compaction 隱藏前文，並加 provenance custom entry。[file-committer.ts](https://github.com/alexei-led/resume-from/blob/3cdc8a089157a015702648e2e4177b64096850dc/src/platform/store/file-committer.ts) 限定一次發布一個檔案，先以 `wx`／0600 staging，再 hardlink 到不存在的目的地，並檢查 root／symlink／inode ownership；可以借鑑不覆寫與中斷安全的契約，不能直接當多檔 archive transaction 的解法。

## DeepSeek Harness

DeepSeek 的格式與 Pi 完全不同，且預設使用壓縮 session log。以下補充由固定 commit 的 storage、codec、附件及 export 原始碼交叉確認；不要套用「所有 harness 都是直接掃 JSONL」的假設。

### 根目錄、generation 與身分

預設 home 是三平台的 `~/.dsh`，優先序為 explicit configured home > 非空白 `DSH_HOME` > `homedir()/.dsh`。`~`、`~/`、`~\\` 可展開；相對 override 相對於啟動 cwd。base profile 的 JSONL persistence root 指向 `dshHomePath('sessions')`，但 plugin 與 root 都能替換。不要為了找路徑執行設定檔裡的 YAML `!!js`；無法靜態判讀的自訂設定由使用者提供來源根目錄。

```text
$DSH_HOME/sessions/
  --<encoded-cwd>--/               # 或 _no-cwd
    <escaped-session-id>/
      session.jsonl.zstd           # v0
      session.v1.jsonl.zstd
      session.v2.jsonl.zstd
      session.v3.jsonl.zstd
      session.v4.jsonl.zstd        # 此次 source 的 current format
```

`compression: none` 才是 `.jsonl`。project slug 將分隔符序列換為 `-`，不安全 UTF-16 code unit 編成 `~XXXX`，截短並加 `--`；這是有意採用的有損顯示路徑，不能逆推 cwd。session ID 另有可逆／無碰撞的 escaping，不能與 project slug 算法混用。

Runtime 選擇**數字最大的 canonical generation**，就算它是目前不支援的新版本也不退回舊檔。暫存檔、uppercase、帶 leading zero、`v0` 拼法不算 canonical generation。備份應保存所有原始 generations，但目前的 normalized view 只選該 session 的最新 generation，避免一次遷移變成多份對話。未支援的新 generation 必須報告解析不支援；舊 generation 不是安全 downgrade 副本。

**文件漂移：**此次 README 的 layout 仍稱 v3 current，程式的 `SESSION_FORMAT_VERSION` 與 generated catalog 已是 v4，codec 覆蓋 v0–v4。維護 adapter 時，先驗證 catalog／寫入程式，不僅看 README。

證據：[home paths](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/util/home-paths/src/index.ts#L135)、[base profile](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/bundle/base/cordis.patch.yml)、[ID 與 project 編碼](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/session/session-persistence-jsonl/src/format.ts#L199)、[provider](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/session/session-persistence-jsonl/src/index.ts)、[format catalog](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/session/session-format-catalog/src/generated.ts#L17)。

### v4 格式與解析位置

Header 必要資料包括 `type: "session"`、`version: 4`、`id`、`createdAt`（epoch 毫秒）、`isSeeded`、`delegationDepth`；可有 `cwd`、`parentSession`、`origin: "subagent"`、`agentPreset`。它**沒有 desktop／CLI launch-origin 欄位**。

Event envelope 包含 `type`、從零連續增加的 `seq`、epoch 毫秒 `time`、`data`，以及適用時的 `ignorable: true`、`surfaceOp`、`sourceEventSeqs`。`sourceEventSeqs` 在磁碟上會將長度至少三的連續整數壓成 `[start,end]`，不能將這種 pair 當兩個獨立 seq。

| Event／欄位                                                               | 語意與固定位置                                                                                     |
| ------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `user/message`                                                            | message 直接在 `data`，不是 `data.message`                                                         |
| `system/message`、`developer/message`、`assistant/message`、`tool/result` | message 在 `data.message`                                                                          |
| Message                                                                   | `id`、`role`、`source`、`content`；`source.kind == "user"` 才能證明是真正的人類輸入                |
| Content                                                                   | text、reasoning、image、file、tool-call、developer tools 變更與 plugin extension；未知 blocks 保留 |
| `session/title`                                                           | 最新 `data.title` 是 session 顯示名稱                                                              |
| assistant provider／model                                                 | `data.message.source.provider`／`.model`                                                           |
| assistant usage                                                           | `data.usage`；`inputTokens` 指未命中 cache 的 input，與 cacheRead／cacheWrite 分開，不可再扣一次   |

未知 required event（未標 `ignorable`）可能改變 replay，應保留並回報無法完整重建；未知 ignorable event 也要保存。只拿「含 text 的物件」遞迴搜尋不能保證正確解析，可能把工具參數、重複 stream 或注入 context 當成獨立聊天。

證據：[session types](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/core/session/src/types.ts)、[message types](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/llm/llm/src/message.ts)、[blocks／tokens](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/llm/llm/src/types.ts)。

### 分支、subagent、compaction 與舊版本

`surfaceOp` 可以 append 或 replace model-visible surface；compaction 取代的是當前 surface，原始 append history 仍存在。全歷史與當前 model context 是不同投影，備份不可只保留後者。

`origin: "subagent"` 才是 subagent 的直接證據。歷史普通 fork 也可能帶 `parentSession`，不能僅據此分類。`isSeeded` 表示繼承歷史；精確界線是**最後一筆** `session/end-seed` 且 `data.inherited: true` 的 marker，其 seq 等於 inherited event count。Analytics 不可將繼承的 assistant usage 當成子 session 新花費。

| 格式變化 | 對備份／parser 的影響                                                                                                             |
| -------- | --------------------------------------------------------------------------------------------------------------------------------- |
| v0／v1   | physical assistant delta rows 可能 packed，不能只逐行當 current event                                                             |
| v1 → v2  | stream chunks 放入 assistant/message 或 log-only assistant/attempt，event references 重新連續編號                                 |
| v2 → v3  | 加 system message、canonical surface replacement endpoints                                                                        |
| v3 → v4  | user-role 包裝的 tool-result 改成 first-class tool-role message；producer source kinds 改變；child evidence 補足 subagent catalog |

因此 v4 的簡單解析不能宣稱同時完整支援 v0–v3。沒有實作該 codec 時仍可保存原始 generation，回報 raw-only／partial normalization。執行中的 unsettled assistant stream 可能只存在記憶體；純 filesystem backup 無法保存尚未落盤的 tokens。

遷移證據：[v0→v1](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/session/session-format-v0-to-v1/README.md)、[v1→v2](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/session/session-format-v1-to-v2/README.md)、[v2→v3](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/session/session-format-v2-to-v3/README.md)、[v3→v4](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/session/session-format-v3-to-v4/README.md)。

### Zstandard、live capture 與 durability

預設壓縮檔不是一個永不結束的壓縮 stream，而是**串接的獨立 Zstandard frames**：第一個 frame 只含 header，之後每批 append 是一個 checksummed frame。必須能解碼所有 concatenated frames，不能只讀第一個 frame 就說完成。

上游每批 fsync；捕獲到 append／sync error 時回滾到先前長度。第一次 materialization 不覆寫既有檔。寫入擁有權使用 POSIX flock／Windows named kernel semaphore。Read observer 不會阻止 writer，所以 capture 仍可能停在新 frame 中間。

備份應分清：完整且 checksum 正確的 frame、最後一個尚未完成的 frame／JSONL row、已提交 frame 的 checksum／格式損壞。上游能從 torn last frame 保留完整 rows，之後 write-open 會裁切／修補來源；PathKeep 不可呼叫這種路徑。備份 exact captured bytes，另記可解析範圍與尾端狀態；pre/post metadata 變動時重試，不能以「能解壓前半段」當完整成功。

上游 read 會在記憶體遷移舊格式，write-open 才發布新 generation。純備份不能替使用者升級原始記錄。來源：[provider contract 與實作](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/session/session-persistence-jsonl/README.md)。

### 附件、spill 與完整性範圍

| Artifact             | 位置與規則                                                                                   |
| -------------------- | -------------------------------------------------------------------------------------------- |
| Image object         | `$DSH_HOME/attachments/v1/objects/<digest前2字元>/<sha256>`                                  |
| Generic file object  | `$DSH_HOME/attachments/v1/file-objects/<prefix>/<digest>`                                    |
| Generic file alias   | `$DSH_HOME/attachments/v1/files/<prefix>/<digest>/<name>`，通常是 hardlink                   |
| Request image cache  | `$DSH_HOME/cache/attachments/request-images`，可重新產生                                     |
| Oversized text spill | 預設 OS temp 下 `dsh-spill-*` 私有根目錄、`session-<sha256(sessionId)>` 子目錄，可自訂根目錄 |

Image reference 的 `attachmentId` 是 `sha256:<64 lowercase hex>`，還有 media type、bytes、dimensions、可選 name／原始尺寸。保存的是上游正規化後的 image bytes，原始動畫、metadata 或解析度可能早已消失；不能承諾還原最初上傳原圖。Generic file reference 有 hash、name、bytes；去重 payload 可以依 content address，仍要保留每個 reference 與 alias metadata。

Spill 啟動清理預設 30 天，fork 後仍引用原路徑、不會自動複製 ownership。Transcript 存在不表示完整工具輸出仍在。附件只抓已知 content slot 且位於允許根目錄內的 object，驗證 size／hash，缺失明確記錄，不任意遍歷 JSON 中的路徑。附件 bytes 與 transcript 必須建立可追溯關係，不能僅備份資料夾卻不知哪些 reference 缺失。

**Spill 與 attachment reference 的信任邊界不同：**再次讀取 [spill-policy](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/spill/spill-policy/src/index.ts) 與 [notice](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/spill/spill-policy/src/notice.ts) 後確認，persisted locator 是工具輸出裡的 `Full formatted result stored at:` 文字 footer，並非受驗證的結構化 reference。上游也明示這是 convention，不是 authenticated tool-output origin。PathKeep 不得根據該文字自動開檔；在已知工具事件偵測 marker 時可保守報告 spill coverage unknown／partial，或由明確提供的 spill 根目錄做獨立備份，不能假裝已保存完整輸出。Marker 也可能出現在一般工具文字中，因此診斷是可能缺失，而不是已證明檔案存在。

證據：[image object path](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/attachment/attachment-local/src/store.ts#L51)、[attachment contract](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/attachment/attachment/src/types.ts)、[local store](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/attachment/attachment-local/README.md)、[spill retention](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/spill/spill-local/src/index.ts#L88)。

### Desktop、resume 與遷移限制

官方 Electron desktop 與 npm CLI 共用 `DSH_HOME` 下的 sessions、settings、workspaces；安裝套件與 activation 不同。Desktop 使用保留 profile `desktop`，但目前 header 不能證明是哪個 surface 建立對話。應回報 shared-storage／unknown，不把安裝位置或 profile 名稱硬寫成逐對話 provenance。

原生 resume 經 `ctx.agents.resume({resumeSessionId})`，會取得 writer ownership，並可能 append interrupted-turn repair events；cold read 可只在記憶體產生 closers。能讀 archive 不等於可直接 resume：還受 format、plugins／preset、外部 resources 及既有 writer 影響。本次僅研究，不啟動 native session。

Cwd 是 creation metadata，也參與目錄布局；runtime 改工作目錄不代表改 session 身分。未來遷移需一併處理 header cwd、project slug、外部路徑、附件、spill locator、preset 與 ownership。僅搬檔或替換字串不足以保證原生可用。

證據：[desktop 共用資料](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/README.md)、[persistence 與 resume](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/docs/subsystems/persistence.md)。

### DeepSeek 已有 export 實作

官方 [session-log-export/archive.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/packages/session-query/session-log-export/src/archive.ts) 會 flush live session，透過 persistence read handle 讀取，輸出 canonical header 與完整 logical event fields；以 seen-set 遍歷 descendants；只從定義好的 content slot／completed stream blocks 收集附件，避免任意遞迴讀路徑。ZIP response queue 上限 64 KiB，但每個 session 的 JSONL 仍整份組成字串。它適合 native logical export，不保留 filesystem-exact compressed generations。官方 Markdown 有意省略 reasoning、system、injected content，也不是無損備份。

**歷史 SQLite 不能漏報，但也不能誤稱為目前預設。** 官方在 [`4553c9d957ec09c1e92660ca4d549cfcef84eda9`](https://github.com/deepseek-ai/deepseek-harness/commit/4553c9d957ec09c1e92660ca4d549cfcef84eda9) 移除 `session-persistence-sqlite`；[保留決策](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/.agents/notes/implemented/simplification/2026-08-30-jsonl-only-session-persistence.md) 明確說舊 DB 不自動開啟／遷移，需要用較舊 build 匯出。目前第一方 authoritative log 是 JSONL；`session-query-sqlite` 是可丟棄的搜尋索引，`storage-sqlite` 是其他 domain 的 KV，不是聊天 authority。

最後的舊 provider 檢查版本為 [`2c6ff296af12aebad578aaedf7154041f8b67e28`](https://github.com/deepseek-ai/deepseek-harness/tree/2c6ff296af12aebad578aaedf7154041f8b67e28/packages/session/session-persistence-sqlite)。`path` 必填、沒有預設位置、相對於 launch cwd，允許 `:memory:`。預設 WAL、synchronous FULL、busy timeout 5000。Physical schema v20、`application_id=0x44534850`；`sessions` 保存 session_key、version、created_at、cwd、parent_session、seed_length、origin、delegation_depth、agent_preset、incarnation、revision；`events` 以 `(session_id,seq)` 為主鍵，保存 time、type、data、source_event_seqs、surface_op、ignorable。data 可為使用指定 dictionary 的 Zstd blob，packed row 也可能含多個 logical chunks。舊 `loadStored` 用 read transaction 讀 header+events 再 decode，不能當普通 JSON 欄位讀。未實作該舊 provider 時必須列為 legacy unsupported，不能宣稱支援所有歷史 DeepSeek 儲存。

第三方 [`kittimzhe/dsh-session-export`](https://github.com/kittimzhe/dsh-session-export/commit/d0e7d8328597ba19dffae13935a9d629a9db6462) 的 [src/archive.ts](https://github.com/kittimzhe/dsh-session-export/blob/d0e7d8328597ba19dffae13935a9d629a9db6462/src/archive.ts) 有以下具體限制：

- 經 `sessionQuery.readSession` 讀 logical events，每個 session 產 ZIP／manifest，隔離單 session error。
- `eventToJsonlLine` 只挑選既知欄位，遺失 `ignorable` 與未來 envelope 欄位。
- JSONL 沒有 native header，manifest 沒 format／isSeeded／agentPreset；不能當 native-restorable archive。
- 不保存附件；`--all` 只過濾目前 cwd，`--since` 使用建立時間而不是最近互動。
- 檔名只用 session ID 前 8 字元與日期，可能碰撞；完整 session 與 ZIP 都放記憶體。
- 部分 session 失敗時 command result 仍是 success，只在文字內寫失敗。
- 其註解以「官方 exporter 不支援 SQLite」作比較，不能沿用為目前限制：官方 exporter 已依 read handle 抽象，但 authoritative SQLite provider 本身已被移除。

這些比較支持將 raw preservation、normalized index、閱讀匯出、完整性診斷與 native restore 分成不同契約。沒有任何閱讀用 export 可以自動被視為完整備份。

## 給 parser 與 E2E 的失敗清單

在實作前把以下場景寫入測試規格；實際測試最後集中執行。本次 source 檢查不是執行驗證。

1. 來源被 append、truncate、整份 rewrite；完整但沒 newline 的最後一筆；半筆 UTF-8／JSON；備份前後原始 hash 不變。
2. Pi 多分支、compaction、context_edit、unknown extension、v1 無 native entry ID；不得混淆原始歷史與 context 投影。
3. Pi inline image、存在／已刪除／越界的 `fullOutputPath`；不能掃工具參數指向的任意工作檔。
4. DeepSeek 多個 concatenated checksummed Zstd frames、最後 frame 只寫一半、checksum 壞掉；檢查 raw 保存與 normalization 結果分開。
5. DeepSeek v3／v4 兩份 generation 同時存在，之後新增 v5；應選最新，對未知 v5 明確報告，不悄悄退回 v4。
6. DeepSeek 真實完整 lifecycle（turn／step／request／message／結束事件）、unknown ignorable／required event、sourceEventSeqs 壓縮範圍、surface replacement。
7. Header identity 與 filename 不一致、相同 native ID 來自不同根目錄、project slug 碰撞、移動來源目錄；不可合併不相關 session。
8. DeepSeek inherited history／subagent 與普通 fork；原始事件不丟、analytics 不把繼承 usage 當新花費。
9. 附件 canonical object／hardlink alias、hash／size 不符、缺失 spill、跨根 symlink；缺失不得變成 complete。
10. 無限長 line、超大 base64／tool output、數百萬 events；streaming 邊界有上限，不能將全部 transcript、ZIP 或分支圖常駐記憶體。
11. Reader 不可透過上游 loader 對來源追加 newline／執行 migration／修補；所有 native 驗證僅在隔離副本進行。
12. Export 完全離線、保存 unknown raw records，檔案存在即失敗或使用明確新目的地，不覆寫使用者原有檔案。

## 本次 adapter 實作備註

`adapters/pi.rs` 的公開入口只接受來源根目錄、已解壓的 `BufRead` 與逐事件 callback。每個 JSON object（包括 header）都會交付，payload 不改寫；未知 extension entry 仍保留。文字投影抽取已知 content／summary／bash 輸出／tool-call 參數，不索引 inline image bytes。Pi v1 的衍生 ID 使用來源 physical ordinal，並附 `pi-legacy-derived-identity`；它不是上游 native ID。v2／v3 保留 native ID／parentId，未知版本明確 partial。

Pi latest `session_info` 的 empty／missing name 表示清除名稱，不能保留前一次 title；實作與測試遵循上游 `getSessionName`。`SessionMetadata.model` 是 physical log 最後觀察到的 model，**不是已重播全部分支後證實的目前 model context**。`parentSession` 保存在 lineage reference，不假裝是 parent session ID。沒有圖形 replay、cycle 檢查或 native resume 保證；圖形證據在 payload 與 parent fields，後續 reader 要對壞鏈與 cycle 防禦。

Pi discovery 接受 `.pi`、agent 目錄、sessions 目錄或明確 JSONL 根目錄；不逆解 cwd slug。選取巢狀預設路徑前檢查 root／agent symlink，否則從 `agent/sessions` 開始 walk 會漏看中間 symlink。Capture 仍須重新確認 source containment／identity，inventory 不是寫入或讀取其他檔案的授權。

Pi references 依 kind／target 去重，上限 4,096 個、單 target 64 KiB、合計 target 4 MiB；超限只減少 reference projection，原始 record 仍交付並明確 partial。Count limit 不能取代 byte limit：若每個 path 接近 record budget，光 4,096 個就會耗盡記憶體。這個失敗方式已先寫測試，再修正實作，並同步給共用 helper。

本節記錄程式契約；最終執行結果、實際平台、14.4M 與 E2E artifacts 見 [驗證紀錄](validation/README.md)，不以 rustfmt 或讀碼代替驗證。
