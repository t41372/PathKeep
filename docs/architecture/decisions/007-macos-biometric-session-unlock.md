# ADR-007 — macOS Touch ID Unlock Is An Additive Session Convenience

## 狀態

Accepted

## 背景

`WORK-M4-K` 要把 `ADR-005` 裡原本誠實標成 deferred 的 biometric integration 重新打開，但不能把整個 App Lock 邊界偷偷改寫。這一輪真正要回答的問題不是「要不要把 App Lock 升級成更強安全模型」，而是：

1. macOS 上能不能在不改變 session-only boundary 的前提下，把 Touch ID 接成一個真實可用的 unlock path
2. Touch ID 成功、失敗、取消、不可用時，PathKeep 應該如何說真話
3. 這個 integration 會不會繞過 passcode-first / fallback-first 的既有契約

## 決策

### 1. 只在 macOS shipping 真正的 biometric unlock

- PathKeep 在 M4-K 只對 macOS 接上 `LocalAuthentication` / Touch ID。
- Windows / Linux 仍維持 truthful capability / degradation state，不假裝已經有 native biometric。

### 2. Touch ID 是 additive convenience，不是新的 security boundary

- App Lock 仍然是 **UI session lock**。
- Archive encryption 仍然獨立保護 at-rest database。
- shared profile scope 仍然只是 viewer / filter contract。
- Touch ID 只是在 macOS 上新增一條真正可用的 session unlock path，不會把 PathKeep 升級成新的 database-key 或 profile-partition security layer。

### 3. passcode 仍然是 required fallback

- 啟用 App Lock 前仍然必須先設定 passcode。
- Touch ID 不能取代 passcode 成為唯一憑證。
- 當 Touch ID 不可用、取消、失敗、lockout 或未註冊時，PathKeep 明確回退到 passcode。

### 4. shipped capability state 只表達三種狀態

- `touch-id-available`
- `touch-id-unavailable`
- `unsupported`

macOS 會在 `available / unavailable` 之間切換；其他平台維持 `unsupported`。

## 理由

- 這樣可以把 macOS 的 native convenience 真正接進產品，而不會破壞 `ADR-005` 已經簽收的 session boundary。
- 它延續了 PathKeep 的 trust stance：使用者看到的不是「看起來像 Touch ID」，而是 macOS 上真的能用、其他平台就誠實標 unsupported。
- 保留 passcode-first 要求，能讓 Touch ID failure path、support story 與 recovery story 維持一致，不需要引入另一套秘密管理生命週期。

## 後果

### 正面

- macOS 使用者可用 Touch ID 解鎖當前 PathKeep session。
- Lock screen / Settings 可以對 macOS 顯示真實 Touch ID copy，而不是 generic future-integration 文案。
- Windows / Linux 繼續保有 honest degradation，不需要假裝 cross-platform parity。

### 負面

- PathKeep 的 biometric UX 現在是平台不對稱的：只有 macOS 有真正 unlock path。
- 測試與 release validation 必須新增 macOS native biometric capability / refusal path。
- 若未來 Windows Hello 或 Linux PAM / polkit 要接進來，仍需新的平台決策和 shipping evidence。

## 相關

- `WORK-M4-K`
- [005-app-lock-session-boundary.md](005-app-lock-session-boundary.md)
- [../../features/archive.md](../../features/archive.md)
- [../../design/screens-and-nav.md](../../design/screens-and-nav.md)
