# M1 — Solid Archive

> 核心目標：把「長期保存與可恢復」做對。  
> **前置條件**：M0 完成  
> **需求來源**：[features/archive.md](../features/archive.md) · [features/recall.md](../features/recall.md) · [architecture/data-model.md](../architecture/data-model.md)  
> **設計來源**：`reference/PathKeep — Desktop UI Design/`

---

## M1.1 — Schema Migration 系統

> 見 [data-model.md § Schema 演化](../architecture/data-model.md)

- [ ] **M1.1.1** 實現 migration 引擎
  - [ ] 建立 `schema_migrations(version INTEGER PK, applied_at TEXT, checksum TEXT, backup_path TEXT)` 表
  - [ ] 實現 migration runner：掃描 migrations/ 目錄下的編號 SQL 檔，依序執行
  - [ ] Migration 前自動做 archive DB snapshot
  - [ ] Migration report 寫入審計工件
  - [ ] Migration 執行結果記入 `schema_migrations` 表
  - [ ] 100% test coverage + mutation test
- [ ] **M1.1.2** 撰寫初始 migration
  - [ ] `0001_initial_schema.sql` — 建立所有基礎表（與現有 schema 對齊）
  - [ ] `0002_add_fts5.sql` — 建立 FTS5 虛擬表（`url`, `title`, `search_terms`）
  - [ ] `0003_add_run_type.sql` — 為 `backup_runs` 增加 run type 欄位
  - [ ] `0004_add_soft_delete.sql` — 增加軟刪除相關欄位
  - [ ] `0005_add_aggregates.sql` — 建立預聚合統計表
  - [ ] `0006_unify_timestamps.sql` — 統一時間欄位格式
  - [ ] 每個 migration 都有對應的回滾 SQL（或標記為不可回滾）
- [ ] **M1.1.3** 整合到啟動流程
  - [ ] App 啟動時自動檢查並執行 pending migrations
  - [ ] 首次啟動時自動執行所有 migrations
  - [ ] Migration 失敗時的錯誤處理和提示

---

## M1.2 — 增量備份核心

> 見 [features/archive.md § 增量備份](../features/archive.md)

- [ ] **M1.2.1** Profile Discovery（移到 browser-history-parser）
  - [ ] Chromium 系列 profile 發現（Chrome, Edge, Brave, Arc, Vivaldi, Opera, Opera GX）
  - [ ] 多平台路徑支持（macOS 為主，Windows/Linux 預留）
  - [ ] Profile metadata 提取（profile name, user name, browser version）
  - [ ] History DB 存在性檢查
  - [ ] Favicons DB 發現
  - [ ] 100% test coverage
- [ ] **M1.2.2** Staging 安全複製
  - [ ] 複製 History DB + sidecar 檔（`-journal`, `-wal`, `-shm`）到 staging 區
  - [ ] 對 staging 副本做完整性檢查（`PRAGMA quick_check`）
  - [ ] 處理瀏覽器正在運行時的檔案鎖定情況
  - [ ] 100% test coverage
- [ ] **M1.2.3** Chromium History 解析器
  - [ ] 解析 `urls` 表 → `url_versions`
  - [ ] 解析 `visits` 表 → `visit_events`
  - [ ] 解析 `downloads` 表 → `download_versions`
  - [ ] 解析 `keyword_search_terms` 表 → `search_terms`
  - [ ] **深度解析 Chrome 新功能**：
    - [ ] 解析 `segments` + `segment_usage` 表（最常訪問統計）
    - [ ] 解析 `content_annotations` 表（頁面內容註解）
    - [ ] 解析 `context_annotations` 表（上下文註解）
    - [ ] 解析 `clusters` + `clusters_and_visits` + `cluster_visit_duplicates` 表（History Clusters / Journeys）
    - [ ] 深入研究並記錄每個表的欄位含義和用途
  - [ ] Schema 指紋計算
  - [ ] 向後兼容：舊版 Chrome 缺少某些表/欄位時 gracefully degrade
  - [ ] 100% test coverage + mutation test
- [ ] **M1.2.4** Favicon 解析器
  - [ ] 解析 Chromium Favicons DB
  - [ ] 提取 icon URLs、圖標數據、尺寸
  - [ ] 避免重複存儲（payload_hash 去重）
  - [ ] 100% test coverage
- [ ] **M1.2.5** Raw Capture 層
  - [ ] 動態讀取所有表的所有欄位
  - [ ] 以 JSON payload 形式存入 `raw_row_versions`
  - [ ] 記錄 source table name, source PK, schema hash, browser version
  - [ ] 即使 parser 不認識新欄位，raw capture 仍然保存
  - [ ] 100% test coverage
- [ ] **M1.2.6** 增量寫入邏輯
  - [ ] 以 `(source_kind, profile_id, table_name, source_pk, payload_hash)` 為去重鍵
  - [ ] 重複記錄 skip，不寫入
  - [ ] URL metadata 變化（如 title 更新）作為新 version 保存
  - [ ] 所有寫入關聯 run_id
  - [ ] 增量 watermarks 更新（`profile_watermarks`）
  - [ ] 100% test coverage + mutation test
- [ ] **M1.2.7** Backup Run 管理
  - [ ] 建立 run 記錄：started_at, status, profiles, run_type
  - [ ] 記錄 run 的 summary（insert / update / skip / fail 統計）
  - [ ] Run 完成後更新 status
  - [ ] Manual run vs Scheduled run 的區分
  - [ ] 100% test coverage

---

## M1.3 — 審計 Manifest + Hash Chain

> 見 [features/archive.md § 審計與可信性](../features/archive.md)

- [ ] **M1.3.1** Manifest 生成
  - [ ] 每次 run 生成 JSON manifest
    - [ ] 上一個 manifest 的 hash
    - [ ] 來源文件的 hash
    - [ ] 來源 schema 指紋
    - [ ] 各表的記錄數、insert/update 統計
    - [ ] 工具版本
    - [ ] 失敗原因（如有）
  - [ ] Manifest 寫入 `manifests` 表
  - [ ] Manifest 文件存入 `manifests/` 目錄
  - [ ] 100% test coverage
- [ ] **M1.3.2** Hash Chain
  - [ ] 新 manifest 的 hash 串接前一個 manifest 的 hash
  - [ ] 形成 append-only ledger
  - [ ] Chain 驗證函數
  - [ ] 100% test coverage
- [ ] **M1.3.3** Git 審計工件
  - [ ] Manifests 文件自動 commit 到審計 Git repo
  - [ ] Schema snapshot commit
  - [ ] 不納入 Git 的項目：主 archive DB、raw 快照、cache、staging
  - [ ] 100% test coverage

---

## M1.4 — 排程系統

> 見 [features/archive.md § 排程](../features/archive.md)

- [ ] **M1.4.1** macOS LaunchAgent 支持
  - [ ] 生成 com.pathkeep.backup.plist
  - [ ] `RunAtLoad=true` + `StartInterval=3600`（每小時喚醒）
  - [ ] Worker 內部判斷是否真正需要備份（基於 `dueAfterHours` 設定）
  - [ ] 預設備份間隔 12 小時
  - [ ] 100% test coverage
- [ ] **M1.4.2** Preview / Manual / Execute 流程
  - [ ] Preview：顯示將要安裝的 plist 文件內容
  - [ ] Manual：提供複製命令和操作指南
  - [ ] Execute：用戶確認後代為安裝
  - [ ] 結果記入審計日誌
  - [ ] 100% test coverage
- [ ] **M1.4.3** 排程狀態監控
  - [ ] 顯示排程是否已安裝
  - [ ] 顯示上次觸發時間
  - [ ] 顯示下次預計觸發時間
  - [ ] 100% test coverage

---

## M1.5 — 加密 / 不加密選擇

> 見 [features/archive.md § 安全與加密](../features/archive.md)

- [ ] **M1.5.1** SQLCipher 整合
  - [ ] 加密模式：使用隨機 DB key，經 Argon2id 導出的 key 包裝
  - [ ] 包裝後的 secret 存入 Stronghold vault
  - [ ] 非加密模式直接使用 plain SQLite
  - [ ] 100% test coverage
- [ ] **M1.5.2** Keyring 便利解鎖
  - [ ] macOS Keychain 整合
  - [ ] 用戶可選是否啟用便利解鎖
  - [ ] Keychain 不可用時 fallback：每次啟動輸入密碼
  - [ ] 100% test coverage
- [ ] **M1.5.3** Rekey 流程
  - [ ] 明文 → 加密
  - [ ] 加密 → 明文
  - [ ] 更改密碼
  - [ ] Rekey 前自動備份
  - [ ] 100% test coverage

---

## M1.6 — Archive 快照（Safety Net）

> 見 [features/recall.md § Archive 快照](../features/recall.md)

- [ ] **M1.6.1** 快照觸發
  - [ ] Schema migration 前自動快照
  - [ ] 用戶手動觸發快照
  - [ ] 大型導入前快照（超過閾值）
  - [ ] 100% test coverage
- [ ] **M1.6.2** 快照管理
  - [ ] 預設保留 4-8 個最近快照
  - [ ] 用戶可配置保留數量
  - [ ] 超過上限時自動清理最舊的
  - [ ] 快照壓縮存儲
  - [ ] 100% test coverage
- [ ] **M1.6.3** 從快照恢復
  - [ ] 用戶可選擇某個快照恢復整個 archive
  - [ ] 恢復前確認提示
  - [ ] 恢復結果記入審計日誌
  - [ ] 100% test coverage

---

## M1.7 — 歷史紀錄瀏覽（基礎版）

> 見 [features/recall.md § 歷史紀錄瀏覽器](../features/recall.md)

- [ ] **M1.7.1** FTS5 全文搜尋
  - [ ] 建立 FTS5 虛擬表索引（url, title, search_terms）
  - [ ] 實現搜尋查詢接口
  - [ ] 搜尋結果按時間排序
  - [ ] Cursor-based pagination
  - [ ] 100% test coverage
- [ ] **M1.7.2** 基礎歷史紀錄查詢 API
  - [ ] 按時間範圍查詢
  - [ ] 按 profile 篩選
  - [ ] 按 domain 篩選
  - [ ] 複合篩選（疊加使用）
  - [ ] 100% test coverage
- [ ] **M1.7.3** 預聚合統計表
  - [ ] `daily_visit_counts` — 每日訪問數
  - [ ] `domain_daily_counts` — 每日各 domain 訪問數
  - [ ] 增量更新機制（備份完成後觸發）
  - [ ] 100% test coverage

---

## M1.8 — 匯出功能

> 見 [features/archive.md § 數據匯出](../features/archive.md)

- [ ] **M1.8.1** 本地匯出格式
  - [ ] HTML 格式匯出
  - [ ] JSONL 格式匯出
  - [ ] 匯出支援篩選（profile, 時間範圍, domain, search query）
  - [ ] 匯出結果記入審計日誌
  - [ ] 100% test coverage

---

## M1.9 — M1 前端 UI（照設計稿）

> 所有 UI 嚴格按照 `reference/PathKeep — Desktop UI Design/` 設計稿實現。

- [ ] **M1.9.1** Dashboard 頁面
  - [ ] Stats Row：Total Records、Archive Span、Profiles Tracked、Last Backup
  - [ ] Recent Runs 表（RUN ID, TYPE, SOURCE, RECORDS, STATUS, TIME）
  - [ ] Browsing Density 52-week heatmap
  - [ ] Storage Breakdown 面板
  - [ ] 「Backup Now」按鈕功能
  - [ ] 100% test coverage
- [ ] **M1.9.2** Explorer 頁面（基礎版）
  - [ ] Timeline bar（DAY/WEEK/MONTH/YEAR 切換）
  - [ ] Filter bar（篩選標籤、+ Add Filter、Clear All）
  - [ ] Record list（grouped by time session）
    - [ ] Record group header（時間範圍 + 頁數 + cluster 標籤）
    - [ ] Record item（favicon + title + url + time + transition tag）
  - [ ] Detail panel（記錄詳情：title, URL, visit time, count, transition, typed count, source, domain type, provenance, title versions）
  - [ ] 虛擬滾動實現
  - [ ] 100% test coverage
- [ ] **M1.9.3** Schedule 頁面
  - [ ] 排程配置顯示（Interval, Mechanism, Last/Next Trigger, Profiles）
  - [ ] LaunchAgent Preview（Preview / Manual / Execute tabs）
  - [ ] Copy to Clipboard / Download .plist 功能
  - [ ] 100% test coverage
- [ ] **M1.9.4** Security 頁面
  - [ ] 加密狀態顯示（ENCRYPTED / PLAINTEXT）
  - [ ] Keyring 狀態
  - [ ] Stronghold 路徑
  - [ ] 密碼丟失警告框
  - [ ] Change Password / Re-key / Disable Encryption 按鈕
  - [ ] 100% test coverage
- [ ] **M1.9.5** Settings 頁面（基礎版）
  - [ ] Browser Profiles 面板（profile 勾選列表 + Rescan 按鈕）
  - [ ] General 面板（Language, Data Directory, Version）
  - [ ] 100% test coverage
- [ ] **M1.9.6** Onboarding 頁面
  - [ ] 首次啟動引導：發現瀏覽器、選擇 profile、設定存儲、加密選擇
  - [ ] Step-by-step wizard
  - [ ] 100% test coverage

---

## M1.10 — Tauri Bridge 更新

- [ ] **M1.10.1** 清理舊 commands
  - [ ] 移除不再需要的 Tauri commands
  - [ ] 統一命名規範
- [ ] **M1.10.2** 新增 commands
  - [ ] 對齊新的 API 形狀
  - [ ] 類型安全：前後端共享 type 定義
  - [ ] 100% test coverage

---

## M1.11 — 驗收

- [ ] `bun run check` 全部通過
- [ ] `bun run coverage:js` 100%
- [ ] `bun run coverage:rust` 100%
- [ ] `bun run desktop:build:debug` 成功
- [ ] 端到端煙霧測試通過
- [ ] 手動驗收：首次啟動 → Onboarding → 選擇 profile → 手動備份 → 查看 Dashboard → 瀏覽歷史 → 搜尋 → 匯出
