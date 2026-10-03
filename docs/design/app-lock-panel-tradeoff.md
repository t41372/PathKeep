# App Lock 面板與鎖定畫面：補齊、刪減或放棄 — Trade-off

> **狀態：待用戶決定（2026-10-02 提出）**
> **範圍：** Settings → Security 的 App Lock 列、App Lock 鎖定畫面
> **牽涉的已接受文檔：** [archive.md](../features/archive.md) §8、[screens-and-nav.md](screens-and-nav.md)「App Lock 畫面與導航規則」、[ADR-005](../architecture/decisions/005-app-lock-session-boundary.md)（recovery story）、[ADR-007](../architecture/decisions/007-macos-biometric-session-unlock.md)（macOS Touch ID）、`TROUBLESHOOTING.md`「You forgot the App Lock passcode」
>
> 在用戶決定之前，上述文檔的要求不變；本文只列選項。

---

## 1. 落差

已接受的要求：

- **Settings 的 App Lock 面板**：開關、閒置逾時、biometric 開關、passcode 設定 / 更改 / 清除、**recovery hint**、立即鎖定、**config 路徑**、**上次解鎖時間**。
- **鎖定畫面**：品牌、鎖定原因、**config 路徑**、**上次解鎖時間**、passcode 輸入、**recovery hint 提示框與「打開 config 路徑」動作**；macOS 有 Touch ID 按鈕（不可用時停用並說明會退回 passcode，用戶在 Settings 關掉時不顯示）；其他平台顯示「沒有生物辨識」的說明。

2026-10 redesign 做了的：

- Settings → Security：App Lock 開關（沒有 passcode 時先要求設定）、Passcode 更改 / 移除、自動鎖定（1 / 5 / 15 / 30 / 60 分鐘）、立即鎖定（⌘L）。
- 鎖定畫面（`src/app/shell/lock-screen.tsx`）：品牌、標題、一句說明、passcode 輸入、輸錯提示。

缺的：biometric 開關與 Touch ID 解鎖（`session.unlockApp` 固定送 `useBiometric: false`）、recovery hint（設定時不能填，鎖定時看不到；而且在新 UI 更改 passcode 會把舊的 hint 清掉，因為後端以請求裡的 hint 覆寫）、config 路徑、上次解鎖時間、鎖定原因。

## 2. 後端已經有什麼

不需要改後端或 IPC：

- `AppLockStatus` 已回傳 `biometricAvailable`、`biometricEnabled`、`biometricState`（`touch-id-available` / `touch-id-unavailable` / `unsupported`）、`configPath`、`lockReason`、`lockedAt`、`lastUnlockedAt`、`recoveryHint`。
- `AppLockConfig.biometricEnabled` 是設定（預設關），用 `save_config` 寫。
- `set_app_lock_passcode` 收 `{ passcode, recoveryHint }`。
- `unlock_app_session({ useBiometric: true })` 會跑 macOS 的 Touch ID 提示（`vault-platform/src/biometric.rs`），失敗回 `biometric-unavailable` / `-not-enrolled` / `-lockout` / `-canceled` / `-turned-off` / `-failed` 等錯誤碼（`src-tauri/src/command_error.rs`）。
- 鎖定時仍可呼叫 `open_path_in_file_manager`（ADR-005 的例外清單）。

所以這是一個純前端的決定。

## 3. 忘記 passcode 時現在會怎樣

新的鎖定畫面沒有任何出路。passcode 雜湊在 `<app 資料夾>/app-lock-passcode.json`，App Lock 開關在 `config.json` 的 `appLock.enabled`。使用者要自己知道去找 app 資料夾（macOS 是 `~/Library/Application Support/com.yi-ting.pathkeep`）、打開 `config.json`、把 `appLock.enabled` 改成 `false`。`TROUBLESHOOTING.md` 說「從鎖定畫面或 Settings 打開 config 路徑」，兩者現在都不存在。archive 資料不受影響（App Lock 不加密任何東西），但使用者被鎖在自己的 app 外面，只能靠文件或支援。

## 4. 選項

### A. 四項全做，照已接受文檔

做什麼：

- Settings → Security：「用 Touch ID 解鎖」開關（只在 macOS 出現；Touch ID 暫時不可用時停用並說明）；passcode 對話框加一個可選的 recovery hint 欄位（更改時帶入舊值）；「設定檔位置」列（路徑 + 在 Finder 中顯示）；「上次解鎖」時間。
- 鎖定畫面：macOS 上 Touch ID 按鈕（依 `biometricState` 與設定顯示或停用）；鎖定原因與上次解鎖時間一行小字；「忘記 passcode？」展開後顯示 recovery hint、設定檔路徑、「在 Finder 中顯示」，以及一句說明要把哪個設定改掉；Windows / Linux 一句「這台電腦不支援生物辨識解鎖」。
- 三語文案；E2E 的 lock 場景加上 recovery hint 顯示與錯誤碼對應的提示（Touch ID 本身只能在真 Mac 上手測，dev bridge 沒有生物辨識）。

代價：鎖定畫面與 Security 分頁比 prototype 多四到五個元素，鎖定畫面不再只有一個輸入框；Windows / Linux 多一句沒有功能的說明。

給使用者：Mac 上一碰就解鎖；忘記 passcode 時畫面上就有提示和出路；與 ADR-005 / ADR-007 / TROUBLESHOOTING 一致，不用改任何已接受文檔。

### B. 只做使用者用得到的兩項：Touch ID 與 recovery（建議）

做什麼：

- Touch ID：同 A（Settings 開關 + 鎖定畫面按鈕，只在 macOS）。
- Recovery：passcode 對話框的 recovery hint 欄位；鎖定畫面的「忘記 passcode？」（顯示 hint、在 Finder 中顯示設定檔所在資料夾、一句怎麼關掉 App Lock）。
- 不做：Settings 的 config 路徑列與上次解鎖時間、鎖定畫面的鎖定原因與上次解鎖時間、Windows / Linux 的「不支援生物辨識」說明（不支援就不顯示）。

代價：要修改 archive.md §8、screens-and-nav.md、ADR-007 的平台表（Windows / Linux 不再顯示 honesty copy），並更新 TROUBLESHOOTING.md 的路徑說明。前端工作量約為 A 的三分之二。

給使用者：保留兩個實際有用的東西（快速解鎖、被鎖住時的出路），鎖定畫面仍接近 prototype 的簡潔。拿掉的兩項（上次解鎖時間、config 路徑列）是診斷資訊；需要時 Settings → Storage 已能顯示 app 資料夾。

### C. 全部不做，保留現狀

做什麼：不改 UI。改文檔：archive.md §8、screens-and-nav.md、ADR-005 的 recovery story、ADR-007 改為 superseded、TROUBLESHOOTING.md 改寫成手動編輯 `config.json`。後端的 biometric 程式碼（`vault-platform/src/biometric.rs`、`AppLockConfig.biometricEnabled`、相關錯誤碼）變成沒人呼叫，應刪除或標明保留理由。

代價：Mac 使用者失去已經上線過的 Touch ID 解鎖（回退）；忘記 passcode 只能靠文件，在 app 裡沒有任何提示；後端留一塊死碼或要再花工夫刪。

給使用者：介面最簡單，與 prototype 完全一致。

## 5. 比較

|                           | A 全做 | B Touch ID + recovery  | C 不做                 |
| ------------------------- | ------ | ---------------------- | ---------------------- |
| 後端改動                  | 無     | 無                     | 刪死碼（可選）         |
| 前端工作                  | 最多   | 中                     | 無                     |
| 要改的已接受文檔          | 無     | 3 份 + TROUBLESHOOTING | 4 份 + TROUBLESHOOTING |
| Mac 上 Touch ID           | 有     | 有                     | 沒有（回退）           |
| 忘記 passcode 的出路      | 有     | 有                     | 只有文件               |
| 鎖定畫面與 prototype 差距 | 大     | 小                     | 無                     |

## 6. 建議

選 **B**。Touch ID 與 recovery 是使用者真的會用到的，而且後端已經做好，只差介面；上次解鎖時間與 config 路徑列是診斷資訊，放進 prototype 的極簡鎖定畫面得不償失。C 會讓一個已上線的功能消失，並把忘記 passcode 的人留在死路，不建議。

用戶選定後：照選項改程式碼與文檔，把本文狀態改成 Accepted 並記下選了哪一項。
