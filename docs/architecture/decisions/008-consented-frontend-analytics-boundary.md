# ADR-008 — Frontend Analytics Requires Explicit Consent And Stays Coarse

## 狀態

Accepted

## 背景

PathKeep 在現有 `README` / `vision-and-requirements` / `standards` 裡把 telemetry 明確列為 non-goal。`WORK-M4-K` 這次要重新打開的不是「可不可以默默收集資料」，而是另一個更窄的問題：

1. 我們能不能在不破壞 data sovereignty / trust 的前提下，加入非常有限的 frontend analytics
2. 如果要加，它的 consent、payload、transport 和 failure story 必須收斂到什麼程度
3. 哪些資料永遠不能進 analytics payload

## 決策

### 1. 只允許 explicit opt-in 的 frontend analytics

- analytics 預設關閉。
- 只有使用者在 Settings 明確開啟 consent 後，PathKeep 才會送出事件。
- 關閉 consent 後，不再送出任何新事件。

### 2. 事件範圍只限 coarse frontend behavior

目前允許的事件類型只有：

- route view
- major CTA click
- update check / download / install / restart lifecycle

### 3. transport 保持 first-party JSON boundary

- 不引入第三方 analytics SDK。
- frontend 使用 plain `fetch` 對 first-party HTTPS endpoint 送出 JSON。
- 不做背景 queue、不做離線補送、不做隱形 retry。

### 4. 永久禁止的資料

analytics payload 不得包含：

- archive facts / page content
- URL
- search query
- profile id
- run id
- filesystem path
- AI prompt / free-form user text
- credentials / secrets

### 5. browser preview / dev / test 都不送

- browser preview 只顯示 honesty surface，不送事件。
- 開發與測試模式不送事件。
- 沒有 configured endpoint 時，即使使用者開啟 consent，也必須誠實顯示 delivery 仍未生效。

## 理由

- 這讓 PathKeep 可以非常保守地收集產品面粗粒度訊號，而不會滑向 background telemetry。
- first-party JSON transport 比第三方 SDK 更容易審計 payload boundary，也更符合 local-first / trust-first stance。
- 把 event family 限縮在 coarse UI behavior，可避免 archive data 與 private research context 被混進 analytics。

## 後果

### 正面

- 設定、更新與 release surface 可以有最小必要的 coarse usage evidence。
- 使用者能在 UI 中清楚看到 consent boundary，而不是被動接受 hidden telemetry。
- payload schema 容易審計、測試、與在 docs 中精確描述。

### 負面

- 沒有 background queue / retry，代表事件 delivery 故意不追求完整性。
- analytics 只能回答 coarse product questions，無法用來做更細的 usage reconstruction。
- 這個決策正式把「zero telemetry」改成「zero hidden telemetry」，所以 docs 必須同步改寫，避免對外說法失真。

## 相關

- `WORK-M4-K`
- [../../vision-and-requirements.md](../../vision-and-requirements.md)
- [../../standards.md](../../standards.md)
- [../../../README.md](../../../README.md)
