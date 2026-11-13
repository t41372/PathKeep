# Agent history：研究與交接入口

研究日期：2026-11-11。目標是為本機 agent 記錄建立可獨立使用、來源唯讀的備份核心；產品 UI／UX、onboarding、排程及 IPC 整合留待下一階段。研究結論與已實作能力分開記錄，原始碼是 API 的依據，最終驗證紀錄是執行結果的依據。

## 建議閱讀順序

1. [功能邊界與支援矩陣](../../../features/agent-history.md)：交付什麼、哪些能力還沒有。
2. [核心庫使用說明](../../../../src-tauri/crates/agent-history/README.md)：CLI、公開 API 與驗證入口。
3. [架構契約](../../agent-history-core.md) 與 [實作／維護備註](implementation-notes.md)：儲存、身分、完整性、擴展方式與待驗證項目。
4. 需要改某個 adapter 時，再讀下面對應的來源研究，不以另一個 harness 的形狀推測。

| 研究                                           | 重要結論                                                                                                                                     | 主要原始碼 pin                                                                                                                                                                                                            |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Claude Code／Desktop／Cowork](claude-code.md) | Desktop card 不等於 transcript；平行工具、subagent、tool-results 與本機 Cowork audit 都要保留。一般 chat／新 cloud Cowork 不是完整本機資料集 | [Anthropic Python SDK `3cf3446b`](https://github.com/anthropics/claude-agent-sdk-python/tree/3cf3446b0fa6c97042e85b245bba07459f22f946)，另有當日官方文件與去識別的本機結構觀察                                            |
| [Codex](codex.md)                              | Active／archived、`.jsonl.zst`、paginated history 與 inherited prefix 必須分清；SQLite 補充 metadata，rollout 仍獨立保留                     | [OpenAI Codex `3a89580f`](https://github.com/openai/codex/tree/3a89580f08c985e99b9cc99df53c67fb8482ef93)                                                                                                                  |
| [Pi／DeepSeek Harness](pi-deepseek.md)         | Pi 的原生 opener 可能改來源；DeepSeek 預設是多個 Zstd frames、數字版本 generations 與獨立附件。兩者不能當普通線性聊天                        | [Pi `49fe0a23`](https://github.com/earendil-works/pi/tree/49fe0a237450eb003434d4020b0bfdf53bc6449c)、[DeepSeek `fcc0ad46`](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159) |
| [生態系與其他 harness](ecosystem.md)           | 備份、讀來源的 catalog、閱讀匯出、跨 harness 有損轉移是不同契約；Continue／Cline／Roo 經典格式可獨立讀取，其他格式須按版本擴展               | 各專案 pin 與實作連結在該文件的 inventory／來源章節                                                                                                                                                                       |

## 研究範圍如何計數

[生態系 inventory](ecosystem.md#inventory-and-pinned-revisions) 固定了 **28 個相關專案**的 revision；其中 **9 個**在該報告直接深入讀取 parser／archive／writer 等核心實作，另有 companion research 和僅 inventory 的項目。Claude、Codex、Pi／DeepSeek 報告又包含各自的實作讀取與發現清單，與這 28 個有重疊。因此不能把 28／9 說成全網總數，或把 inventory 當成已驗證支援。

這 9 個直接實作研究是 CASS、txcript、codex-claude-transfer、agent-session-pack、session-porter、ah、agent-session-browser、claude-code-transcripts、ccusage。它們的 pin、程式位置、優點與具體缺口都在 [ecosystem.md](ecosystem.md)。Pi／DeepSeek 報告另外記錄 resume-from、官方 DeepSeek exporter 與 dsh-session-export。未安裝或執行這些第三方工具，也沒有複製它們的實作進核心庫；CASS 的非標準授權條款已明確記錄，不能當一般 MIT dependency。

本機研究只輸出格式、欄位、數量等必要證據，不保存私人對話到 Git 或送遠端研究服務。未取得的閉源內部行為、cloud-only 記錄、尚未讀取的專案都保留為未知；不以猜測補齊。

## 對後續設計最重要的區別

- **保存的 bytes、解析出的 native events、使用者看到的對話不是同一層。** 未知事件、分支、compaction、鏡像記錄、工具輸出都有原始證據；目前 normalized export 是事件流，不是完成 native replay 的單一對話。
- **完整性有不同維度。** Raw revision 可完整且驗證 hash，但解析／附件／lineage 仍有缺口。CLI 成功返回 JSON 也不代表 report 裡的 status 是 complete。
- **原始檔來源與 native ID 不可混用。** 同一 UUID 可能出現在不同來源、複本、fork 或 generation；路徑重新命名不是自動合併授權。
- **一般檔案與 SQLite 的保存格式不同。** 一般檔保留 exact captured bytes；SQLite 保留一致 read transaction 的 typed logical evidence，不是 `.sqlite`／WAL byte copy。
- **搜尋不需要 LLM。** 現有 lexical search、分頁、匯出可離線工作；token／cost analytics、語意搜尋與 AI 功能尚未實作。
- **Resume／遷移只是研究。** 沒有啟動 harness、寫入其原生目錄、修復 native index 或重綁 cwd 的功能。

## 驗證狀態

測試與失敗場景先寫，程式後寫，實際執行集中在最後。macOS 與 Linux 核心測試各 220 項通過，完整 `bun run check` 通過（135 項桌面 E2E）。七個真實本機樣本的原始匯出一致，來源未改動；抽樣未包含的 lineage 仍如實標記 partial。

Linux release CLI 在真正的 4 CPU／8 GiB 硬限額下完成 1,440 萬筆 Pi 合成資料驗證，全部事件可搜尋、原始 SHA-256 一致、重開冪等與完整性檢查通過。Windows、多 session 大量探索、精確 3 GHz 時脈及產品 UI responsiveness 未在此驗收。完整範圍、測量與去識別 evidence 見 [驗證紀錄](validation/README.md) 及 [實作注意事項](implementation-notes.md#驗證紀錄)。
