# 洞察的時區

> **狀態：用戶 2026-10-09 決定 B（洞察用自己固定的時區），並要求：電腦時區跟它不同時提示；存下來的資料盡可能不依賴時區，出差沒改時區、自動備份觸發時資料也不能壞；照最佳做法。2026-10-09 實作（§3 寫的是現在的做法）。**
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

**洞察時區（`vault-core/src/intelligence/insights_zone.rs`）。**

- 一個 IANA 時區（`InsightsZone`，`chrono-tz`：`chrono` 的維護組織 chronotope 把 IANA tzdb 編進程式，Windows 也能用，與既有的 `chrono` 型別相容）。每個時刻各自換算，夏令時間照算：23 / 25 小時的日子、重複的那個小時、午夜被跳過的日子（從那天第一個存在的時刻起算）都有測試。
- 建置用哪個時區（`for_build`）：Settings 選了就用它（`config.insightsTimeZone`）；沒選就用已發佈的資料建置時的時區（`intelligence_generations.time_zone`），所以電腦時區變了也不搬動日子；從沒建置過才取電腦的時區。電腦的時區（`utils::computer_time_zone`）：`TZ` 是 IANA 名稱時用它（與 C 函式庫和 `chrono::Local` 一致），否則讀系統設定（`iana-time-zone`），都讀不到是 UTC。設計時寫「第一次建置把它存進設定」；改成記在發佈的資料上，同樣固定、少一次寫設定，Settings 顯示的是同一個值。
- 時區寫進建置規則（`rule_version` 結尾的 `zone=…`）與建置狀態（`BuildState.zone`），發佈時寫進 `intelligence_generations.time_zone`。時區一變就是規則變了：整個重建；中斷的建置若是別的時區，不續跑、從頭來。追加建置一律用建置狀態裡的時區，不看電腦。
- 讀取用哪個時區（`for_reading`）：已發佈資料的時區（多個設定檔逐一重建的期間，取 `profile_id` 最小那個，讓同一時間的讀取一致），沒有才用電腦的。前端送來的 `YYYY-MM-DD` 範圍、「今天」、On this day、中斷的習慣，全按它算。
- 建置裡的本地日與小時（日 rollup 與 `intelligence_day_counts` 的 `date_key`、每日 24 小時格、`session_paths` 與 `site_flows_rows` 的日子、refind 的跨日數、重開的調查的天數、比較集的日子、建置狀態裡的日子字串）全部經 `InsightsZone` 換算；`chrono::Local` 只剩偵測電腦時區、History 與 Ask 用。

**改時區。** Settings → General →「洞察的時區」：可搜尋的清單（城市、時區名稱、IANA 名稱），每列是地名、名稱與目前的 offset；標籤在列出現後分批算好（約 400 個時區，每個兩個 `Intl` formatter，一次算完在 Mac 上要 50–80 ms）。選了就存設定，後端（`save_user_config_with_vault`）若下次建置的時區因此跟畫面上的不同就排一次建置（`rebuild_in_chosen_zone`，上一次建置失敗或取消過也照排，因為選時區就是要求重建）；有建置正在跑時，背景佇列在它結束後讀到新的設定再排一次（`drain_one_priority_intelligence_job`，每個工作都用當下存的設定）。重建完成前，列下方寫「正在按新時區重新整理洞察，完成之前仍是舊時區」；前端每 5 秒讀一次 `get_insights_time_zone`（`{ shown, next }`）直到兩者相同，然後讓所有 Insights 與 Home 的查詢重新讀。設定存了無效的時區名稱會被拒絕（`invalid-time-zone`）。

**提示。** 視窗看到的電腦時區（`Intl` 的 `timeZone`，也就是 History 顯示日子用的時區）跟洞察的時區不同、而且使用者沒有對這個電腦時區說過「保持」時，Insights 頁頂與 Settings 那一列顯示：「洞察用的是台北標準時間（GMT+8），這台電腦現在是太平洋時間（GMT-7）。」附「改用太平洋時間」（整個重建）與「保持台北標準時間」。兩個選項與 Settings 裡的選擇都記下當時的電腦時區（`insightsZoneKeptFor`）：電腦在那個時區時不再提示，換到別的時區才再出現。兩個時區名稱相同時（例如都叫中歐時間）加上地名。

**顯示。** 洞察與 Home 的洞察卡用 `useInsightsFormat`：時刻按洞察時區顯示，洞察時區不是電腦的時就帶上時區（「下午 2:12 GMT+8」）；日子（`YYYY-MM-DD`）用 `format.day`，任何時區下都是同一天；小時格與時段是牆上時鐘，不經時區換算。範圍與日子的算術在 `lib/time-zone.ts`，用 UTC 午夜，不受瀏覽器的夏令時間影響。Insights 查詢的 key 含時區。

**其他兩處。** History 搜尋的 `after:` / `before:` 改按電腦時區切日子（與 History 的顯示一致；`archive/search_query.rs` 用 `utils::local_day_range_ms`）。Ask agent 寫給模型的時間改用電腦的 IANA 時區逐筆換算（`AgentToolContext.time_zone`），不再是開聊天當下的固定 offset。2026-10-17 起時間旁寫星期，報告的日子旁寫 `weekday`（見 `features/intelligence.md` §2）。

**已知的小落差。** History 的「網站」篩選清單的次數來自 Insights 的資料，所以按洞察的日子算；兩個時區不同時，選某幾天時次數與 History 列表可能差幾筆。多個設定檔換時區重建時逐一發佈，重建中「所有瀏覽器」的數字可能一部分是新時區；卡片這時顯示「正在更新」。讀取在第一個設定檔發佈時就換到新時區，所以「重建完了沒」不能只看 `shown` 與 `next` 相不相同：`get_insights_time_zone` 另外回報還沒換過去的設定檔的時區（`rebuildingFrom`，查 `intelligence_generations`），Settings 一直說「正在按…重新整理」到最後一個設定檔發佈，那時前端才丟掉快取的洞察讀取。（2026-10-10 之前只看 `shown`，Settings 在第一個設定檔發佈時就說好了，之後讀到的數字是兩個時區混著的，而且留在快取裡；`insights-time-zone.spec` 真的第一個跑時讀到柏林那天 52 筆，應該是 59。）

**證明。**

- `intelligence/tests/zone_change.rs`：在台北建置、設定改回未選後再備份一次（電腦時區是 UTC）→ 仍是追加、仍在台北，結果等於在台北從頭建置；選洛杉磯並同時有新的備份 → 整個重建，結果等於在洛杉磯從頭建置；兩個時區的日子確實不同（造訪落在台北的午夜前後）。
- `insights_zone.rs` 的測試：同一時刻在兩個時區是不同的日子、紐約的 23 / 25 小時日、聖保羅 2018-11-04 被跳過的午夜、只接受 IANA 名稱。
- E2E `insights-time-zone.spec.ts`：在 Settings 選台北 → 重建，某一天的造訪數等於照台北手算的、時刻帶 GMT+8、沒有提示；洛杉磯的瀏覽器看到提示，「保持」後消失、重新載入也不出現；再到柏林又出現，「改用」後重建，那天的造訪數等於照柏林手算的、時刻不帶時區；最後改回 UTC。
- Ask：`agent_tools` 測試裡紐約冬天與夏天的中午都換成 12:00。

## 4. 不做的

- 不在 archive 存本地日期或 offset 的換算結果：那會讓時區變成資料的一部分。
- 不在時區變了時默默重建：用戶說要提示，而且出差時保持原時區通常才是對的。
