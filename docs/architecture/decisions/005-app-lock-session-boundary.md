# ADR-005 — App Lock Uses A Session-Only Boundary

## 狀態

Accepted

## 背景

`WORK-M4-C` 與 `PG-RD-PLAT-006` 需要回答一個不能模糊帶過的問題：PathKeep 的 App Lock 到底保護什麼。若沒有先釐清這個邊界，就很容易做出看起來像安全功能、實際上卻只是前端遮罩的假安全。

這個決策同時牽涉四個面向：

1. App Lock 是否要和 archive encryption 綁成同一把鑰匙
2. 啟動鎖定、閒置逾時、自動重新鎖定的行為要如何定義
3. macOS / Windows / Linux 的 biometric 能力在 day-one 能誠實支援到哪裡
4. shared profile scope 是否要趁這次升級成真正的 per-profile partition

## 決策

### 1. 保護邊界

App Lock 在目前產品中是 **desktop UI session lock**，不是 archive encryption 的替代品，也不是新的 database key 管理層。

- App Lock 會在啟動時、手動鎖定時、以及閒置逾時後阻擋 shell 與資料讀取 surface。
- Archive encryption 仍獨立負責資料庫檔案的 at-rest 保護。
- App Lock 不會重新包裝、替換、或自動衍生 archive encryption key。

### 2. 鎖定後必須阻擋的 surface

當 App Lock 處於 locked 狀態時：

- React shell 不渲染主應用 chrome，直接導向 `/lock`
- desktop read / query commands 必須回傳 locked refusal，而不是回傳 stale snapshot
- MCP 的 history query surface 必須回傳 locked refusal；`mcp-server` worker 在 locked 狀態下不能啟動

仍允許的安全例外 surface 只有：

- `app_lock_status`
- `unlock_app_session`
- `open_path_in_file_manager`
- build / diagnostics 類的非 archive data status

### 3. 解鎖與恢復模型

目前 build 採 **passcode-first** 模型。

- 啟用 App Lock 前必須先設定 passcode
- 閒置逾時範圍固定為 1 到 60 分鐘
- Settings 與 lock screen 都會明講「這只保護 UI session」
- recovery story 不是假裝能還原密碼，而是提供 recovery hint、config path、以及 support / troubleshooting guidance

清除或重設 App Lock 不會解密、重寫或刪除 archive 資料；它只影響 UI session lock 的本地 state / passcode 檔案。

### 4. 平台能力矩陣

| 平台    | 目前 shipped unlock path | day-one 真實狀態                                                             | 後續可整合方向                                     |
| ------- | ------------------------ | ---------------------------------------------------------------------------- | -------------------------------------------------- |
| macOS   | passcode                 | Touch ID 尚未接 LocalAuthentication；UI 必須顯示 truthful degradation note   | LocalAuthentication / Secure Enclave-backed prompt |
| Windows | passcode                 | Windows Hello 尚未接到 desktop bridge；UI 必須顯示 truthful degradation note | Windows Hello / OS credential prompt               |
| Linux   | passcode                 | 不提供 biometric；維持 passcode-only                                         | 視 PAM / polkit / desktop stack 能力另議           |

因此目前不 shipping 真正的 biometric unlock，只 shipping：

- biometric availability / degradation state
- disabled control 與誠實說明
- passcode fallback

### 5. Shared profile scope 不升級為 partition

shared profile scope 仍然是 **viewer / filter contract**，不是新的安全邊界。

- Dashboard / Explorer / Insights / Assistant 的 profile scope 繼續作為 UI read-model 篩選
- App Lock 不會把單一 archive 切成多個安全 partition
- 若未來真的要做 per-profile partition，必須另開新的架構 / migration 決策，不可偷偷混進 App Lock

## 理由

- 這個邊界最符合目前產品已經存在的 archive encryption 與 session model，不需要引入第二套資料庫鑰匙生命週期。
- 它能誠實地保護「有人碰到你正在開著的 PathKeep 視窗」這個威脅模型，同時避免誤導使用者以為資料檔案也被同一把密碼保護。
- 在 native biometric integration 尚未真正落地前，把它當成 future capability 而不是假功能，能保持 trust。
- 不把 shared profile scope 假裝成 partition，可避免用戶錯誤理解資料隔離等級，也能避免現在的 schema / read-model 被迫做不完整的安全升級。

## 後果

### 正面

- App Lock 與 archive encryption 的責任邊界清楚
- shell、desktop commands、MCP 都有一致的 locked refusal path
- settings / lock screen / troubleshooting 文檔可以說真話，不需要捏造生物辨識或忘記密碼流程

### 負面

- App Lock 目前不能單獨保護 archive 檔案；使用者若要 at-rest 保護，仍需另外啟用 encryption
- macOS / Windows 的 biometric 目前只能顯示為 future integration，而不是可用能力
- shared profile scope 仍不是資料隔離模型；需要更強隔離時，之後必須另做專案級改動

## 相關

- `PG-RD-PLAT-006`
- `WORK-M4-C`
- [desktop-command-surface.md](../desktop-command-surface.md)
- [screens-and-nav.md](../../design/screens-and-nav.md)
- [archive.md](../../features/archive.md)
- [intelligence.md](../../features/intelligence.md)
