# 洞察的時區

> **狀態：用戶 2026-10-09 決定 B（洞察用自己固定的時區），並要求：電腦時區跟它不同時提示；存下來的資料盡可能不依賴時區，出差沒改時區、自動備份觸發時資料也不能壞；照最佳做法。**
> **範圍：** 洞察的建置與讀取（Intelligence 資料庫的所有本地日期、小時）、History 搜尋的 `after:` / `before:`、Ask agent 寫給模型的時間、Settings。
> **牽涉的已接受文檔：** [ADR-002](decisions/002-timestamp-contract.md)（時刻一律存 UTC 毫秒）、`deterministic-intelligence.md`（日 rollup、session、trail 都按本地日）。

## 1. 現況（2026-10-09 盤點）

archive 存的時刻全是 UTC：每個 parser（Chromium、Firefox、Safari、Takeout）都換成 Unix 毫秒，`*_iso` 與 `*_at` 用 UTC 的 RFC 3339。archive 裡沒有本地日期欄位。每次備份另外記下當時電腦的 IANA 時區（`runs.timezone`），只拿來顯示。設定與排程只存時長；到期判斷是 UTC 比 UTC。這部分照 ADR-002，與時區無關。

洞察不一樣。它按本地日與本地小時彙整，現在用 `chrono::Local`（電腦當下的時區）算：

- 日 rollup（網站、類別、搜尋引擎、每日摘要）與 `intelligence_day_counts` 的 `date_key`，每日摘要的 24 個小時格；
- `session_paths`、`site_flows_rows` 的 `date_key`；
- refind 頁的 `cross_day_count`、重開的調查的 `distinct_days`（決定這列存不存在）；
- 建置狀態裡存的本地日字串：`dirty_dates`、追加用的 `paths_from_day`、進行中路線的 `date_key`、重新分類的日子。

建置規則與建置狀態都不含時區。增量建置在原地寫（`INSERT OR REPLACE`，不刪那天舊的列），而且只從第一個要重算的日子的本地午夜開始重讀。所以系統時區一變（macOS 可以依位置自動改），下一次備份後的建置就會把資料寫壞：從台北（UTC+8）到洛杉磯，「10-09」那天原本涵蓋 10-08T16:00Z 到 10-09T16:00Z，新的建置從 10-09T07:00Z 重讀，中間那段造訪從再次造訪過的網站消失；反方向則同一批造訪被算兩次。中斷的建置換了時區續跑也一樣。沒有任何東西察覺時區變了。

另外兩處小的：History 搜尋的 `after:` / `before:` 用 UTC 日，而 History 其他日期篩選與顯示都用電腦時區；Ask agent 寫給模型的時間用開聊天當下的固定 offset，跨夏令時間的造訪會差一小時。

## 2. 選項（用戶已選 B）

**A. 跟著電腦時區，一變就整個重建。** 日子永遠是現在的時區；出差一週要整個重建兩次（14.4M 約 97 分鐘），舊的日子也會跟著搬到旅行地的時區。

**B. 洞察固定一個時區。** 第一次建置時取電腦的時區，存進設定，Settings 顯示、可以改，改了才整個重建。出差不動日子、結果穩定；洞察裡的日子與時刻都按這個時區顯示，跟 History（電腦時區）可能不同，要標出來。

## 3. 做法（B）

**存的資料。** archive 不變：只存 UTC 時刻，加上來源給的原值當證據。本地日期只出現在洞察這種可重建的衍生資料裡，而且只按洞察時區算。

**洞察時區。**

- 設定 `insightsTimeZone`（IANA 名稱，如 `Asia/Taipei`）。第一次建置時是空的，就取電腦的時區（`iana-time-zone`，`chrono` 已經間接用到）並存下來。
- 建置規則（`rule_version`）加入時區名稱，建置狀態（`BuildState`）也記下：時區一變就是規則變了，整個重建（現有的「規則不同就重建」機制）；中斷的建置若記的時區與現在設定不同，不續跑、從頭來。
- 建置裡所有本地日與本地小時（`local_date_key`、`date_range_bounds`、小時格、`strftime(…,'localtime')`）改用洞察時區換算，每個時刻各自換算（夏令時間照算），不再用 `chrono::Local`。時區資料庫用 `chrono-tz`（`chrono` 的維護組織 chronotope，把 IANA tzdb 編進程式，Windows 也能用）；它與既有的 `chrono` 型別相容，不必換整套日期庫。
- 讀取：前端送來的 `YYYY-MM-DD` 範圍按洞察時區解讀；「今天」「最近 30 天」也按洞察時區算。前端用 `Intl.DateTimeFormat` 的 `timeZone` 顯示洞察裡的日子與時刻，不需要新依賴。

**提示。** 電腦時區（每次打開 app 與每次備份時讀，`runs.timezone` 已經在記）跟洞察時區不同時，在洞察頁與 Settings 顯示一句話：「洞察用的是台北時間，這台電腦現在是洛杉磯時間。」附兩個選項：改成洛杉磯（整個重建）、保持台北。選了保持就不再提示，直到電腦時區再變。洞察的時刻在兩者不同時標上時區。

**其他兩處。** `after:` / `before:` 改用電腦時區，跟 History 的顯示一致；Ask agent 的時間逐筆按電腦時區換算。

**證明。** E2E 在兩種 `TZ` 下跑：用 A 時區建好洞察，換成 B 時區再備份一次：洞察的數字不變（仍按 A）、出現提示；改成 B 之後整個重建，每天的造訪數照 B 手算。另一個 E2E 驗 `after:` 照電腦時區切日子。

## 4. 不做的

- 不在 archive 存本地日期或 offset 的換算結果：那會讓時區變成資料的一部分。
- 不在時區變了時默默重建：用戶說要提示，而且出差時保持原時區通常才是對的。
