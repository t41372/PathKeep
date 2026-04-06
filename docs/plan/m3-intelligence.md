# M3 — Intelligence

> 核心目標：語義搜尋、AI 問答、洞察系統。  
> **前置條件**：M2 完成  
> **需求來源**：[features/intelligence.md](../features/intelligence.md) · [architecture/tech-stack.md](../architecture/tech-stack.md) · [database-selection-decision-2026-04-05.md](../database-selection-decision-2026-04-05.md)

---

## M3.1 — AI Provider 配置

> 見 [features/intelligence.md § AI Provider 配置](../features/intelligence.md)

- [ ] **M3.1.1** Provider 概念模型實現
  - [ ] Request Format 定義（OpenAI-compatible, Anthropic, Google, Ollama, LM Studio）
  - [ ] Provider = Request Format + Base URL + API Key + Model List + Config
  - [ ] 用戶可創建多個 Provider
  - [ ] Provider 可啟用/禁用
  - [ ] 100% test coverage
- [ ] **M3.1.2** rig.rs 整合
  - [ ] 整合 rig-core crate
  - [ ] 實現 Provider adapter 層（將我們的 config 轉換為 rig.rs 的 provider）
  - [ ] 支持 Ollama provider（本地推理）
  - [ ] 支持 OpenAI-compatible provider（雲端 API）
  - [ ] Embedding 和 LLM 分別配置不同的 Provider/Model
  - [ ] API Key 安全存儲（Stronghold）
  - [ ] Connection test 功能
  - [ ] 100% test coverage
- [ ] **M3.1.3** Preset Provider
  - [ ] Ollama preset（http://localhost:11434）
  - [ ] LM Studio preset
  - [ ] OpenAI preset
  - [ ] Anthropic preset
  - [ ] Google preset
  - [ ] 所有 preset 支持自定義 Base URL
  - [ ] 100% test coverage
- [ ] **M3.1.4** AI Provider Settings UI
  - [ ] Provider card 列表（按設計稿：active/disabled 狀態、config rows）
  - [ ] Provider 新增/編輯/刪除
  - [ ] API Key 設定（加密存儲）
  - [ ] Model 選擇（Embedding model / LLM model 分開）
  - [ ] Connection test 按鈕
  - [ ] 100% test coverage

---

## M3.2 — Embedding Pipeline

> 見 [features/intelligence.md § 語義搜尋](../features/intelligence.md) · [database-selection-decision-2026-04-05.md § LanceDB](../database-selection-decision-2026-04-05.md)

- [ ] **M3.2.1** LanceDB sidecar 整合
  - [ ] 整合 LanceDB Rust SDK
  - [ ] 定義 vector table schema（history_id, embedding, metadata）
  - [ ] Sidecar 數據目錄管理
  - [ ] Sidecar 完全獨立於主 archive（可刪除重建）
  - [ ] 100% test coverage
- [ ] **M3.2.2** Embedding 生成
  - [ ] 使用 rig.rs 調用 embedding provider
  - [ ] 為歷史紀錄生成 embedding（title + URL + search terms 組合文本）
  - [ ] 增量計算：只處理新增/未嵌入的記錄
  - [ ] 避免重算已處理的記錄
  - [ ] Batch 處理以提高效率
  - [ ] 100% test coverage
- [ ] **M3.2.3** Vector 索引
  - [ ] 使用 LanceDB IVF-PQ 索引
  - [ ] 索引建立和更新
  - [ ] ANN 近似最近鄰搜尋
  - [ ] 100% test coverage
- [ ] **M3.2.4** 語義搜尋 API
  - [ ] 接受自然語言查詢
  - [ ] 生成查詢的 embedding
  - [ ] 在 LanceDB 中做 ANN 搜尋
  - [ ] 回傳匹配結果（帶相似度分數）
  - [ ] 支持按 profile / domain 篩選
  - [ ] Hybrid search（FTS5 + 語義搜尋融合排序）
  - [ ] 100% test coverage

---

## M3.3 — AI 計算任務系統（Job Queue）

> 見 [features/intelligence.md § AI 計算任務系統](../features/intelligence.md)

- [ ] **M3.3.1** Job Queue 核心
  - [ ] Job Queue 數據表設計（job_id, type, status, priority, created_at, started_at, finished_at, error, retry_count）
  - [ ] 任務產生：備份完成後、導入完成後、手動觸發
  - [ ] 任務排隊：按優先級和時間排序
  - [ ] 背景異步執行，不阻塞 UI
  - [ ] 成功/失敗處理
  - [ ] 重試策略（可配置最大次數、指數退避）
  - [ ] 暫停/恢復所有任務
  - [ ] 100% test coverage
- [ ] **M3.3.2** Job Queue 任務類型
  - [ ] Embedding 計算任務
  - [ ] Enrichment refetch 任務（M4 完整實現，M3 預留接口）
  - [ ] Insight 計算任務
  - [ ] LLM 摘要生成任務
  - [ ] 100% test coverage
- [ ] **M3.3.3** Job Queue UI
  - [ ] Dashboard 上顯示 Job Queue 狀態面板（按設計稿：running/queued/completed jobs）
  - [ ] 顯示待處理任務數量
  - [ ] 顯示當前運行任務和進度
  - [ ] 顯示最近完成/失敗的任務
  - [ ] 暫停/恢復按鈕
  - [ ] 手動觸發「掃描 + 排隊」
  - [ ] 清理失敗任務 / 重新排隊
  - [ ] 100% test coverage

---

## M3.4 — 洞察系統（V1 基礎）

> 見 [features/intelligence.md § 洞察系統](../features/intelligence.md)

- [ ] **M3.4.1** 洞察模塊架構
  - [ ] 定義 Insight Module trait（名稱、依賴、計算邏輯、觸發條件、UI 組件）
  - [ ] 模塊註冊和管理
  - [ ] 用戶可開啟/關閉個別模塊
  - [ ] 100% test coverage
- [ ] **M3.4.2** 層 1：結構特徵提取
  - [ ] URL 結構解析（domain, subdomain, path tokens, query parameters）
  - [ ] Domain 分類（docs / forum / video / news / social / shopping / code 等）
  - [ ] 搜尋引擎 query 提取（解析 URL 中的 `q=`, `search_query=` 等）
  - [ ] Transition / referrer 信息提取
  - [ ] 估計停留時長（基於相鄰 visit 時間差）
  - [ ] 100% test coverage
- [ ] **M3.4.3** On This Day（歷史上的今天）
  - [ ] 拉出歷年同一天（±1 天容差）的歷史紀錄
  - [ ] 按年份分組展示
  - [ ] 不需要 AI，純數據庫查詢
  - [ ] 有 LLM 時可生成一句話摘要
  - [ ] Dashboard 上的 On This Day 卡片（按設計稿）
  - [ ] 100% test coverage
- [ ] **M3.4.4** Site Analytics（網站統計）
  - [ ] 按 domain 統計訪問次數
  - [ ] 估計 session 時長
  - [ ] Top Domains 排行（按設計稿：rank + domain + bar + count）
  - [ ] 純數據庫查詢，不需要 AI
  - [ ] 100% test coverage
- [ ] **M3.4.5** Periodic Summaries（定期總結）
  - [ ] 日度總結（今天主要在研究什麼？多少頁面？最活躍 domain？）
  - [ ] 週度總結（和上週對比、新主題）
  - [ ] 統計部分不需要 AI
  - [ ] 主題歸納和對比描述使用 LLM
  - [ ] Weekly Summary 卡片（按設計稿）
  - [ ] 100% test coverage
- [ ] **M3.4.6** Topic Timeline（主題時間軸）
  - [ ] 對歷史紀錄做 embedding + 增量聚類（topic clusters）
  - [ ] LLM 為每個 cluster 起名
  - [ ] 可視化主題隨時間的變化
  - [ ] 30D / 90D / 1Y 切換
  - [ ] 點擊主題看具體頁面
  - [ ] Topic Timeline 面板（按設計稿：topic rows + bars + count）
  - [ ] 100% test coverage

---

## M3.5 — AI Assistant（Ask My History）

> 見 [features/intelligence.md § AI 助手](../features/intelligence.md)

- [ ] **M3.5.1** Agentic RAG 引擎
  - [ ] 使用 rig.rs 實現 agentic RAG
  - [ ] 多步檢索：先語義搜尋，再精製查詢，再深入
  - [ ] Context 組裝（從 archive 拉取相關記錄組成 LLM context）
  - [ ] 回答必須附帶 evidence（來源歷史紀錄）
  - [ ] 100% test coverage
- [ ] **M3.5.2** AI Assistant UI
  - [ ] 對話式介面（按設計稿）
  - [ ] User message + AI message 卡片
  - [ ] Evidence panel（顯示支持結論的歷史紀錄）
  - [ ] 輸入框 + 發送按鈕
  - [ ] Provider info hint（底部顯示使用的 model 和隱私聲明）
  - [ ] 只在用戶配置了 AI 且有 embedding index 時可用
  - [ ] 100% test coverage

---

## M3.6 — MCP Server + AI IDE Skill

> 見 [features/intelligence.md § 外部 AI 工具整合](../features/intelligence.md)

- [ ] **M3.6.1** MCP Server
  - [ ] 在設定中手動開啟
  - [ ] App 啟動本地 MCP server（localhost only）
  - [ ] 提供搜尋、檢索歷史紀錄的 MCP tools
  - [ ] 使用 rmcp crate
  - [ ] 100% test coverage
- [ ] **M3.6.2** AI IDE Skill
  - [ ] 撰寫 skill 定義檔（markdown 格式）
  - [ ] 描述可用的 MCP tools、典型查詢、回傳格式
  - [ ] Settings 中可查看/複製 skill 文件路徑
  - [ ] 100% test coverage

---

## M3.7 — Insights 頁面

- [ ] **M3.7.1** Insights 頁面完整實現
  - [ ] Summary KPI 卡片（This Week pages, Top Domain, Explore/Exploit ratio, Active Threads）
  - [ ] Topic Timeline 面板
  - [ ] Weekly Summary 面板
  - [ ] Active Threads 面板（thread items with hot/warm/cool status）
  - [ ] Site Analytics（Top Domains）面板
  - [ ] 100% test coverage

---

## M3.8 — Dashboard 增強

- [ ] **M3.8.1** On This Day 面板（Dashboard）
  - [ ] 按設計稿實現
  - [ ] AI Summary（如有 LLM）
  - [ ] 100% test coverage
- [ ] **M3.8.2** Job Queue 面板（Dashboard）
  - [ ] 按設計稿實現
  - [ ] 100% test coverage

---

## M3.9 — 驗收

- [ ] 所有質量門通過
- [ ] AI Provider 配置 → Embedding → 語義搜尋端到端驗收
- [ ] AI Assistant 問答驗收
- [ ] 洞察生成驗收（On This Day, Site Analytics, Weekly Summary, Topic Timeline）
- [ ] MCP Server 連接驗收
- [ ] 無 AI 配置時，系統正常工作（graceful degradation）
