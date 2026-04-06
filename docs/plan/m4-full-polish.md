# M4 — Full Intelligence & Polish

> 核心目標：完整洞察套件、Enrichment 插件、遠端備份、多平台完整驗證。  
> **前置條件**：M3 完成  
> **需求來源**：[features/intelligence.md](../features/intelligence.md) · [features/archive.md](../features/archive.md)

---

## M4.1 — 完整洞察套件

> 見 [features/intelligence.md § V1 洞察功能](../features/intelligence.md)（M3 未完成的部分）

- [ ] **M4.1.1** 層 2：Session 和 Task 構建
  - [ ] Session 切分：相鄰 visit 時間差 ≤30 分鐘歸為同一 session
  - [ ] Session 合併：語義相近的 session 合併為 thread
  - [ ] Reopen detection：停了幾天又恢復的研究線
  - [ ] 100% test coverage
- [ ] **M4.1.2** Task / Thread Detection（任務偵測）
  - [ ] 自動偵測進行中的研究線
  - [ ] 跨天持續的語義連貫的 visit series
  - [ ] 明確「任務重新打開」標記
  - [ ] Active Threads UI（hot/warm/cool 狀態指示）
  - [ ] 100% test coverage
- [ ] **M4.1.3** Open Loops（未完成任務）
  - [ ] 基於 thread 的 revisit 次數和收斂信號
  - [ ] 識別反覆在看但未完成的事情
  - [ ] 100% test coverage
- [ ] **M4.1.4** Important but Unsaved
  - [ ] 計算 `importance = revisit_count × estimated_dwell × semantic_centrality`
  - [ ] 識別高頻訪問但未 bookmark 的頁面
  - [ ] 100% test coverage
- [ ] **M4.1.5** Explore vs Exploit
  - [ ] 計算 domain Shannon entropy
  - [ ] 計算新 domain 佔比
  - [ ] 計算 revisit concentration
  - [ ] Explore/Exploit ratio KPI 卡片
  - [ ] 100% test coverage
- [ ] **M4.1.6** Source Role Map（資訊來源角色圖）
  - [ ] 根據使用方式分類常用網站（搜尋入口、社群、問題定位、學習等）
  - [ ] Workflow 可視化
  - [ ] 100% test coverage
- [ ] **M4.1.7** Query Reformulation Ladder（搜尋演化路徑）
  - [ ] 分析同一研究線中搜尋關鍵詞的演化方向
  - [ ] 只對 Chromium 系瀏覽器有效（需要 search_terms 數據）
  - [ ] 100% test coverage
- [ ] **M4.1.8** Contrastive Summary（對比式摘要）
  - [ ] 結構化比較（本週 vs 上週的研究重心差異）
  - [ ] 交給 LLM 寫成人話
  - [ ] 100% test coverage
- [ ] **M4.1.9** 月度 / 年度總結
  - [ ] 月度總結：主題分布、最深入的研究線、最常用的資訊來源
  - [ ] 年度總結：年度回顧、注意力分布、主要研究階段和轉折點
  - [ ] 100% test coverage

---

## M4.2 — Enrichment 插件系統

> 見 [features/archive.md § Enrichment](../features/archive.md)

- [ ] **M4.2.1** Enrichment 架構
  - [ ] 定義 Enrichment Plugin trait（URL pattern match, enrich function, output schema）
  - [ ] 統一的 enrichment 表（以 JSON 格式保存，以 history_id 關聯）
  - [ ] 插件版本和來源記錄
  - [ ] Enrichment 可隨時重跑
  - [ ] 用戶可在設定中啟用/禁用個別插件
  - [ ] 100% test coverage
- [ ] **M4.2.2** 層 1：立即 Enrichment（不需要外部請求）
  - [ ] URL 結構解析（domain, subdomain, path tokens, query params）
  - [ ] Domain 分類（docs / forum / video / news / social / shopping / code）
  - [ ] 搜尋引擎 query 提取
  - [ ] Transition / referrer 信息
  - [ ] 100% test coverage
- [ ] **M4.2.3** 層 2：背景 Refetch
  - [ ] 訪問 URL 抓取頁面內容
  - [ ] 提取 readable text、meta description、OG tags
  - [ ] 提取頁面語言
  - [ ] Best-effort，失敗不阻塞
  - [ ] Rate limiter
  - [ ] 100% test coverage
- [ ] **M4.2.4** 層 3：專屬 Enrichment 插件
  - [ ] arXiv 插件（匹配 `arxiv.org/abs/*`，API 獲取論文 metadata）
  - [ ] GitHub 插件（匹配 `github.com/*/*`，提取 repo info）
  - [ ] YouTube 插件（匹配 `youtube.com/watch*`，提取影片 metadata）
  - [ ] Wikipedia 插件（匹配 `*.wikipedia.org/wiki/*`，提取摘要）
  - [ ] Stack Overflow 插件（匹配 `stackoverflow.com/questions/*`）
  - [ ] HN 插件（匹配 `news.ycombinator.com/item*`）
  - [ ] 每個插件都內建 rate limiter
  - [ ] 每個插件 100% test coverage
- [ ] **M4.2.5** Enrichment Settings UI
  - [ ] 插件列表（啟用/禁用開關）
  - [ ] 各插件的 enriched 記錄數統計
  - [ ] 100% test coverage

---

## M4.3 — S3 遠端備份

> 見 [features/archive.md § 遠端備份](../features/archive.md)

- [ ] **M4.3.1** S3 上傳
  - [ ] 用戶配置 S3 endpoint, bucket, credentials
  - [ ] Archive DB bundle 上傳
  - [ ] 明確的手動觸發（不自動上傳）
  - [ ] Upload progress 顯示
  - [ ] 100% test coverage
- [ ] **M4.3.2** S3 Settings UI
  - [ ] S3 配置表單（endpoint, bucket, region, prefix）
  - [ ] Credentials 安全存儲
  - [ ] Connection test
  - [ ] Last upload 狀態
  - [ ] 100% test coverage

---

## M4.4 — 地理位置記錄（實驗性）

> 見 [architecture/data-model.md § 地理位置](../architecture/data-model.md)

- [ ] **M4.4.1** macOS Core Location 整合
  - [ ] 使用 OS 定位 API 獲取位置
  - [ ] 完全可選，預設關閉
  - [ ] 預設完整精度，用戶可選降低精度
  - [ ] 存入 run metadata
  - [ ] 獲取失敗時靜默跳過
  - [ ] Permission 處理
  - [ ] 100% test coverage

---

## M4.5 — V1.5+ 洞察功能

> 見 [features/intelligence.md § V1.5+](../features/intelligence.md)

- [ ] **M4.5.1** Burst Detection（短期爆發偵測）
  - [ ] 短期內某主題訪問量突然暴增
  - [ ] 100% test coverage
- [ ] **M4.5.2** Learning Trajectory（學習軌跡）
  - [ ] 對某主題的瀏覽是入門、工具比較、還是實作
  - [ ] 100% test coverage
- [ ] **M4.5.3** Curiosity Graph（概念跳轉圖）
  - [ ] 概念間的跳轉關係
  - [ ] 橋樑節點識別
  - [ ] 100% test coverage
- [ ] **M4.5.4** Rediscovery Pain（重新搜尋痛點）
  - [ ] 識別以前找過又重新搜尋的內容
  - [ ] 100% test coverage
- [ ] **M4.5.5** Session Archetypes（Session 類型分類）
  - [ ] learn, debug, compare, buy, monitor, entertain
  - [ ] 100% test coverage
- [ ] **M4.5.6** Faceted Profile（資訊使用者側寫）
  - [ ] docs-first vs forum-first 等使用風格
  - [ ] 100% test coverage

---

## M4.6 — 多平台完整驗證

- [ ] **M4.6.1** Windows 完整驗證
  - [ ] Profile discovery 路徑驗證
  - [ ] Backup 端到端測試
  - [ ] Task Scheduler 排程驗證
  - [ ] Keyring 整合驗證
  - [ ] 安裝包構建驗證
- [ ] **M4.6.2** Linux 完整驗證
  - [ ] Profile discovery 路徑驗證（含 flatpak/snap）
  - [ ] Backup 端到端測試
  - [ ] systemd timer 排程驗證
  - [ ] Secret Service / KWallet keyring 驗證
  - [ ] 安裝包構建驗證

---

## M4.7 — CI/CD 完善

> 見 [standards.md § CI/CD](../standards.md)

- [ ] **M4.7.1** GitHub Actions 完善
  - [ ] PR 檢查：lint + test + coverage + build
  - [ ] Multi-platform build matrix（macOS, Windows, Linux）
  - [ ] Release pipeline（自動產出安裝檔）
  - [ ] README badges（CI 狀態、coverage）
- [ ] **M4.7.2** README 完善
  - [ ] 完整功能介紹
  - [ ] 構建指南
  - [ ] 從源碼運行指南
  - [ ] Contributing guide 完善

---

## M4.8 — 磁碟佔用顯示

> 見 [architecture/data-model.md § 長期容量](../architecture/data-model.md)

- [ ] **M4.8.1** Storage Breakdown（設定頁面 / Dashboard）
  - [ ] Core Archive 大小
  - [ ] FTS5 Index 大小
  - [ ] Embeddings (LanceDB) 大小
  - [ ] Snapshots 大小（含保留數量）
  - [ ] 增長趨勢
  - [ ] 按設計稿的 Storage Breakdown 面板
  - [ ] 100% test coverage

---

## M4.9 — 驗收

- [ ] 所有質量門通過
- [ ] 完整洞察套件端到端驗收
- [ ] Enrichment 插件驗收（至少 3 個插件完整工作）
- [ ] S3 遠端備份驗收
- [ ] 三個平台的安裝和使用驗收
- [ ] README 和 CONTRIBUTING 完整性確認
- [ ] 準備 v0.1.0 release
