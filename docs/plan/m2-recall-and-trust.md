# M2 — Recall & Trust

> 核心目標：多來源導入、完整回滾、Doctor 健康檢查，全面落地 Preview/Manual/Execute。  
> **前置條件**：M1 完成  
> **需求來源**：[features/archive.md](../features/archive.md) · [features/recall.md](../features/recall.md) · [reference-review.md](../reference-review.md)

---

## M2.1 — Google Takeout 導入

> 見 [features/archive.md § 數據導入](../features/archive.md)

- [ ] **M2.1.1** Takeout 文件解析
  - [ ] 解析 Google Takeout ZIP / 解壓後的資料夾
  - [ ] 識別 `BrowserHistory.json` 格式
  - [ ] 解析 Takeout 時間格式（ISO 8601）→ 統一為 Unix epoch 毫秒
  - [ ] 處理編碼和特殊字符
  - [ ] 100% test coverage
- [ ] **M2.1.2** Dry-run 和 Preview
  - [ ] 掃描文件、識別格式、產生報告
  - [ ] 計算候選記錄數、時間範圍
  - [ ] 檢測與現有 Archive 的重複數量
  - [ ] 未知格式的文件進入 quarantine
  - [ ] 在 UI 中顯示 quarantine 原因和文件內容摘要
  - [ ] 100% test coverage
- [ ] **M2.1.3** 正式導入
  - [ ] 用戶確認後寫入 Archive
  - [ ] 建立 `import_batches` 記錄
  - [ ] 所有寫入關聯 run_id + batch_id
  - [ ] 重複記錄自動 skip
  - [ ] 導入完成後觸發聚合統計更新
  - [ ] 100% test coverage
- [ ] **M2.1.4** 導入回滾
  - [ ] 按 import batch 回滾：軟刪除該 batch 的所有記錄
  - [ ] 回滾操作記入審計日誌
  - [ ] 回滾是可逆的（取消回滾）
  - [ ] 100% test coverage

---

## M2.2 — Firefox 支持

> 見 [reference-review.md](../reference-review.md)

- [ ] **M2.2.1** Firefox Profile Discovery
  - [ ] 解析 `profiles.ini` 找到所有 Firefox profiles
  - [ ] 支援 Firefox, LibreWolf, Floorp, Waterfox
  - [ ] 多平台路徑（macOS 為主）
  - [ ] 100% test coverage
- [ ] **M2.2.2** Firefox Places DB 解析器
  - [ ] 解析 `moz_places` 表（URLs）
  - [ ] 解析 `moz_historyvisits` 表（visits）
  - [ ] 解析 `moz_bookmarks` 表（bookmarks — 可選元數據）
  - [ ] 解析 `moz_origins` 表（origins metadata）
  - [ ] 解析 `moz_meta` 表（DB metadata）
  - [ ] Firefox 時間格式（Unix epoch 微秒）→ 統一為 Unix epoch 毫秒
  - [ ] Schema 指紋計算
  - [ ] 向後兼容舊版 Firefox
  - [ ] 100% test coverage + mutation test
- [ ] **M2.2.3** Firefox Favicons 解析器
  - [ ] 解析 Firefox favicons.sqlite
  - [ ] 提取 icon 數據
  - [ ] 100% test coverage

---

## M2.3 — Safari 支持（基礎版）

- [ ] **M2.3.1** Safari History 解析器
  - [ ] 解析 Safari `History.db`
  - [ ] 解析 `history_items` + `history_visits` 表
  - [ ] Safari 時間格式（Mac absolute time）→ 統一為 Unix epoch 毫秒
  - [ ] macOS 特有的權限處理（Full Disk Access）
  - [ ] 100% test coverage

---

## M2.4 — Run 歷史與回滾 UI

> 見 [features/recall.md § 版本管理與回滾](../features/recall.md)

- [ ] **M2.4.1** Audit Ledger 頁面（完整版）
  - [ ] Manifest Chain 視覺化（hash chain blocks 串接）
  - [ ] Run 列表（所有 backup runs + import batches）
  - [ ] Run Detail 面板
    - [ ] Run metadata（ID, Type, Source, Time, Hashes）
    - [ ] Insert / Update / Skip / Fail 統計
    - [ ] Schema Fingerprint
    - [ ] Source DB Hash
  - [ ] 「Revert This Run」按鈕
    - [ ] 確認對話框，顯示影響範圍
    - [ ] 回滾執行後 UI 更新
  - [ ] 「View Records」按鈕（跳轉到 Explorer 並篩選該 run）
  - [ ] 「Export Manifest」按鈕
  - [ ] 「Verify Integrity」按鈕（running Doctor check）
  - [ ] 100% test coverage
- [ ] **M2.4.2** Run 回滾邏輯
  - [ ] Run-level 軟刪除：該 run 寫入的所有記錄標記 reverted
  - [ ] Reverted 記錄從正常搜尋中隱藏
  - [ ] 回滾操作本身生成新 manifest
  - [ ] 取消回滾（un-revert）
  - [ ] 100% test coverage

---

## M2.5 — Doctor 完整性檢查

- [ ] **M2.5.1** Health Checks
  - [ ] Archive DB integrity check（`PRAGMA integrity_check`）
  - [ ] Hash chain 驗證（遍歷所有 manifest，驗證 chain 連續性）
  - [ ] Schema version 一致性檢查
  - [ ] Orphan record 檢測
  - [ ] FTS5 index 一致性檢查
  - [ ] Watermark 一致性檢查
  - [ ] 100% test coverage
- [ ] **M2.5.2** Health Report
  - [ ] 生成結構化健康報告
  - [ ] 報告記入審計日誌
  - [ ] 在 Settings 或 Audit Ledger 頁面可手動觸發
  - [ ] 100% test coverage

---

## M2.6 — Preview / Manual / Execute 全面落地

> 見 [design/ux-principles.md § 操作透明性](../design/ux-principles.md)

- [ ] **M2.6.1** 排程設定走 PME 流程 ✅（M1 已開始）
  - [ ] 完善 Manual 模式的指南和可複製命令
- [ ] **M2.6.2** 導入走 PME 流程
  - [ ] Preview：顯示將導入的記錄統計
  - [ ] Manual：每步有操作指南
  - [ ] Execute：逐步顯示進度
  - [ ] 100% test coverage
- [ ] **M2.6.3** 加密/Rekey 走 PME 流程
  - [ ] Preview：顯示將要做什麼
  - [ ] Manual：提供命令和複製按鈕
  - [ ] Execute：確認後執行
  - [ ] 100% test coverage

---

## M2.7 — Import Wizard UI

> 照設計稿的 Import 頁面。

- [ ] **M2.7.1** Import 頁面完整實現
  - [ ] Import 方法選擇卡（Google Takeout / Browser Direct）
  - [ ] 5-step Wizard（Upload → Scan → Preview → Confirm → Import）
  - [ ] Step 1：拖入文件 / 選擇文件
  - [ ] Step 2：掃描中（進度顯示）
  - [ ] Step 3：Preview（Records Found, Time Range, Duplicates, New Records + Detected Files 列表）
  - [ ] Step 4：確認（影響範圍摘要）
  - [ ] Step 5：導入進度 + 完成報告
  - [ ] Quarantine 警告：未知文件的原因和摘要
  - [ ] 100% test coverage

---

## M2.8 — i18n 國際化

> 見 [standards.md § 國際化](../standards.md)

- [ ] **M2.8.1** i18n 框架重構
  - [ ] 重構 `src/lib/i18n.ts`（目前 69KB — 結構有問題）
  - [ ] 拆分為 `i18n/index.ts`, `i18n/en.ts`, `i18n/zh-CN.ts`, `i18n/zh-TW.ts`
  - [ ] 所有 UI 文字走 i18n
  - [ ] 100% test coverage
- [ ] **M2.8.2** 翻譯覆蓋
  - [ ] 英文翻譯完整
  - [ ] 簡體中文翻譯完整
  - [ ] 繁體中文翻譯完整
  - [ ] 錯誤信息和通知也走 i18n
  - [ ] 語言偵測和手動切換

---

## M2.9 — 其他平台排程驗證

- [ ] **M2.9.1** Windows Task Scheduler 支持
  - [ ] 生成 Task Scheduler XML
  - [ ] `StartWhenAvailable=true` 確保補跑
  - [ ] Preview / Manual / Execute
  - [ ] 100% test coverage
- [ ] **M2.9.2** Linux systemd user timer 支持
  - [ ] 生成 `.timer` + `.service` 文件
  - [ ] `OnCalendar=` + `Persistent=true`（不用 `OnUnitActiveSec=`）
  - [ ] Preview / Manual / Execute
  - [ ] 100% test coverage

---

## M2.10 — 匯出格式補齊

- [ ] **M2.10.1** Markdown 格式匯出
  - [ ] 100% test coverage
- [ ] **M2.10.2** 純文本格式匯出
  - [ ] 100% test coverage

---

## M2.11 — Explorer 增強

- [ ] **M2.11.1** 互動式時間軸
  - [ ] 可拖動的時間軸控件
  - [ ] 年 → 月 → 週 → 天 多級縮放
  - [ ] 拖動時即時視覺反饋（日期 + 記錄密度）
  - [ ] 記錄密度的色彩/柱狀可視化
  - [ ] 快速跳轉：年→月→天
  - [ ] 日期輸入跳轉
  - [ ] 「回到今天」按鈕
  - [ ] 流暢動畫和過渡效果
  - [ ] 100% test coverage
- [ ] **M2.11.2** 鍵盤快捷鍵
  - [ ] 上下鍵切換記錄
  - [ ] Enter 展開詳情
  - [ ] Esc 關閉詳情
  - [ ] ⌘K 打開全局搜尋
  - [ ] 100% test coverage

---

## M2.12 — 驗收

- [ ] 所有質量門通過
- [ ] Google Takeout 端到端導入、回滾驗收
- [ ] Firefox 備份端到端驗收
- [ ] 多語言切換驗收
- [ ] Audit Ledger 完整性驗證驗收
