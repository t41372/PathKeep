# Chrome 的停留時間單位 — Trade-off

> **狀態：用戶 2026-10-08 同意 A；2026-10-09 實作（parser 換算、升級轉換時除一次；`export.spec` 驗新備份，`upgrade.spec` 驗 v0.3.0 的資料）。**
> **範圍：** archive `visits.visit_duration_ms`、Chromium parser、讀它的 History / 匯出 / Insights / agent。
> **牽涉的已接受文檔：** [ADR-002](decisions/002-timestamp-contract.md)（`*_ms` 一律是毫秒；raw capture 保留來源原值）、`docs/plan/program/import-dedup-audit.md` §6 E12（把「原值照搬」記成現行契約）、BACKLOG `WORK-IMPORT-TEST-MINOR-A`。

## 1. 問題

Chromium 的 `visits.visit_duration` 是**微秒**。parser（`browser-history-parser/src/chromium/mod.rs`，`visit_duration_ms: row.get(7)?`）原樣放進 archive 的 `visits.visit_duration_ms`，所有讀它的地方都當毫秒用，所以每個 Chrome 系瀏覽器的停留時間都**大了 1000 倍**：18 秒的停留讀出來是 5 小時。

讀它的地方：

- History 的 timeline 回給前端的 `durationMs`，以及用同一份資料的 JSONL 匯出（匯出檔裡每筆的 `durationMs` 是錯的）。
- Insights 的 `engagement.visit_duration_ms` 證據（`intelligence/phase_three.rs`），會進「觀察到的互動」等要補回的卡。
- agent 工具讀到的造訪記錄。

E12 那筆測試是 2026-05 補覆蓋率時加的，那個 work block 明寫「不修 product code」，所以它只是把當時的行為記下來，不是一個單位上的設計決定。它和 ADR-002「`_ms` 就是毫秒」相衝突。

## 2. 選項

**A. 寫入時換成毫秒，舊資料一次轉好（建議）**

- parser 把微秒除以 1000 再放進 `visit_duration_ms`，source evidence 的 `engagement.visit_duration_ms`（`visit_engagement_evidence`，現在也是微秒）一起改。少掉的只有不到 1 毫秒的精度，對停留時間沒有意義。
- 已經存在的資料：Turso 格式還沒發佈，每個用戶的既有資料都會經過升級時那一次從舊 SQLCipher 格式的轉換（`vault-convert-sqlcipher`）。轉換時把 Chrome 系 profile 的 `visits.visit_duration_ms` 與 `visit_engagement_evidence` 的同一個值除以 1000；轉換本來就只跑一次，所以不會除兩次，也不需要另外的背景遷移。（若 Turso 格式已經發佈才發現，就得在原地分批轉、記下做到哪裡；現在不必。）
- 做完後 Insights 依新的證據版本重建（現有的 `built_with_other_rules` 機制）。
- 好處：符合 ADR-002，所有讀取端不用改；欄位名和 metric 名說什麼就是什麼。
- 代價：parser 一行換算、轉換器一個步驟，以及證明它們的 E2E（舊格式的 archive 升級後、以及新備份進來的停留時間都對）。

**B. 欄位改名 `visit_duration_us`，讀取端各自換算**

- 不用轉舊資料，只要 schema rename。
- 但每個讀取端都要記得換算，下一個新讀取端很容易再犯同一個錯；也和 ADR-002 的命名規則不一致（`_ms` 是全 schema 的單位約定，多一個 `_us` 例外）。

**C. 維持現狀** — 不考慮：資料是錯的，匯出檔已經在把錯的時間交給用戶。

## 3. 建議

選 A。它讓 archive 裡的值符合 ADR-002 的約定。要推翻的只是 E12 那筆「照抄現行行為」的測試和它在 audit 文檔裡的一行，不是單位上的既有決定。

需要用戶確認的原因：A 會改寫用戶既有的資料（每一筆 Chrome 系造訪的停留時間，含 source evidence 裡的同一個值），在升級轉換時做。改寫是除以 1000，可以乘回來；舊格式的 archive 檔在轉換後仍保留，週期性的 raw snapshot 裡也留著瀏覽器原始的 `History` 檔。

影響範圍：所有 Chromium 系瀏覽器（Chrome、Chromium、Edge、Edge Dev、Brave、Vivaldi、Arc、Opera、Opera GX）與用 Browser Direct 匯入的 Chromium `History` 檔。Firefox、Safari、Google Takeout 的歷史檔沒有每次造訪的停留時間，PathKeep 存的是空值，不受影響。造訪時間本身各瀏覽器都換算正確（Chrome 自 1601 年起的微秒、Firefox 自 1970 年起的微秒、Safari 自 2001 年起的秒，都換成 Unix 毫秒）。
