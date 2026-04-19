# ARCHIVE — 歸檔（核心）

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出。Archive 是產品的根基，優先級高於所有其他功能。

---

## 1. 增量備份

**作為**用戶，**我想要**定期自動備份我所有瀏覽器的歷史紀錄，**以便**我的歷史紀錄不會因為瀏覽器的過期策略而丟失。

### 需求要點

- 支援自動發現本機安裝的瀏覽器和 profiles。
  - 對外公開承諾目前只包含 `Google Chrome`，以及 macOS 上已授權 Full Disk Access 的 `Safari` 基礎支持。
  - `Google Chrome` 的 `Favicons` sidecar 屬正式 backup 範圍；Explorer row / detail 目前會顯示 archive 中已保存的 exact-page favicon。
  - Safari 為基礎支持：profile 會被偵測並保留在 UI 中；若缺少 Full Disk Access，必須顯示 needs-access guidance，而不是把 profile 靜默隱藏。
  - Safari richer favicon coverage 仍保持 deferred follow-up；day-one archive / Explorer truth 只承諾 Safari history baseline，不假裝 `History.db` 已自帶完整 icon payload。
  - Firefox 與其他 Chromium / Firefox-family adapter 可以繼續保留實作與 discovery，但在補齊獨立驗證 evidence 前，不應被寫進 README / onboarding / release docs 的 public support promise。
  - Firefox richer favicon coverage 同樣保持 additive follow-up，不把 visits / URLs baseline 誤寫成 full favicon parity。
  - browser support 的 promotion 規則以 [browser-support-and-adapter-playbook.md](../architecture/browser-support-and-adapter-playbook.md) 為準：discovery、parser / ingest、caveat copy、tests 與 local validation evidence 全齊後，才可升級成公開承諾。
- **Profile 選擇是備份的入口**：
  - 用戶在 UI 中看到所有被發現的瀏覽器和 profiles，以勾選的方式選擇要備份哪些。
  - 交互清晰：選中/未選中狀態有明顯視覺差異和動畫反饋。
  - **未被選中的 profile 不會被備份**，無論是手動備份還是排程自動備份。
  - 排程自動備份只會處理用戶明確勾選過的 profiles。
  - 用戶可以隨時調整選擇，新增或移除要備份的 profiles。
- 備份流程：
  1. 對瀏覽器的 History / Favicons SQLite 做一致性 snapshot，寫到 staging 區；原則上優先使用 SQLite Backup API，而不是單純 raw file copy。
  2. 對 staging 副本做完整性檢查（`PRAGMA quick_check`）。
  3. 解析並寫入 archive 數據庫。
  4. 無法讀取的 profile 仍保留在 onboarding / dashboard 清單中，並附帶權限或支援限制說明。
- M1 的 day-one onboarding 必須先把 storage path、browser detection、security choice、schedule preview 和 first backup boundary 全部展示出來，再允許任何 mutating run。
- Onboarding / Dashboard 的 profile boundary surface 必須誠實顯示 browser-retention reality：本地瀏覽器歷史可能在 PathKeep 下次 backup 前就被瀏覽器策略或使用者清除；只有成功寫進 archive 之後，PathKeep 才提供 append-only 的長期保存。
- Onboarding 不是陷阱頁：使用者可以在 setup 中途明確退出，PathKeep 會保留目前已選的 archive 選項，之後可從 Dashboard / Settings 回來繼續。
- Onboarding / first import 的 execute step 必須先讓 overlay repaint，再開始真正的 archive init / import / backup work；背景計算再重，也不能把 loading 動畫整段凍住到最後一幀才一次補完。
- 備份是**增量的**：只新增/更新有變化的記錄。
- Archive 是 **append-only** 的：即使瀏覽器端的紀錄已過期或被手動刪除，archive 中的歷史紀錄永不刪除。
- 對於會變動的記錄（如 URL metadata），採用 row versioning，保留所有歷史版本。
- 原始來源數據採 **checkpoint-first**：保留 source checkpoint、schema 指紋、瀏覽器版本、run ID 與 manifest trace，不再把每筆來源 row 的完整 JSON payload 長期熱存在 canonical archive。
- 備份不讀取正在運行的瀏覽器的 live 數據庫 — 一律先複製到 staging。

### 重複處理

- 增量備份的核心挑戰是去重。
- canonical facts 的去重以來源主鍵 / canonical identity / payload hash 組合處理；search / intelligence / semantic 都是後續 projection，不參與 source-of-truth dedupe。
- 多次導入相同數據不會產生重複記錄。
- 需要考慮的邊緣案例：
  - 同一 URL 在不同 profiles 下的 visit 是獨立的。
  - 同一 URL 在不同時間點的 visit 是獨立事件。
  - Chrome 的 `urls` 表中同一 URL 的 metadata 可能變化（如 title 更新），這些變化應作為新 version 保存。

### 瀏覽器版本追蹤與前向兼容

我們的目標是**永遠跟進最新版本的瀏覽器**，盡可能從中獲取最多信息，同時**保持對舊版瀏覽器數據格式的向後兼容**。

- **主動追蹤瀏覽器更新**：每個主要瀏覽器的新版本發布時，檢查其 History / Favicons 數據庫的 schema 是否有變化。如果有新增的表或欄位，評估是否應該捕獲。
- **深度研究瀏覽器數據**：在實現各瀏覽器的 parser 時，不要只看「最小可用的 visits + urls 表」，而是要**深入研究最新版本能給我們的所有東西**。例如：
  - Chrome 近期新增了歷史紀錄分組（history clusters / journey）功能，相關數據已反映在 History DB 中。我們應該理解並捕獲這些 cluster 信息。
  - Chrome 的 `segments`、`segment_usage` 表記錄了用戶最常訪問的網站排名。
  - Chrome 的 `content_annotations` 和 `context_annotations` 表可能包含頁面內容摘要和上下文信息。
  - Firefox 的 `moz_origins`、`moz_meta` 等表包含額外的元數據。
  - 每個瀏覽器都可能有我們尚未利用的有價值數據。
- **向後兼容**：parser 必須能處理舊版瀏覽器的數據格式。新欄位或新表不存在時，gracefully degrade — 只是少了一些信息，不會導致備份失敗。
- **Schema 指紋**：每次備份記錄來源數據庫的完整 schema 指紋，當偵測到 schema 變更時觸發原生快照保存。
- **Raw capture 保底**：即使 parser 不認識某些新表或新欄位，raw capture 層仍會把所有原始數據落盤，確保不會因為 parser 更新滯後而丟失信息。
- canonical archive 的 run ledger 使用共用 `runs` 表；backup、import、rollback、restore、doctor、snapshot restore 都要帶上 `run_id` 與 artifact 關聯。
- rollback 採 soft-hide visibility：user-visible facts 以 `reverted_at` / `reverted_by_run_id` 隱藏，checkpoint / manifests / snapshots 保持 immutable。
- canonical archive 已和 keyword recall / intelligence runtime 拆成不同 storage plane；canonical archive 只保留 source-of-truth facts 與 immutable audit facts，rebuildable search / intelligence / semantic state 全部留在 derived/search/intelligence/sidecars。
- import batch 的 un-revert / restore 必須留下獨立 `restore` run，而不是繼續冒充 `rollback`；`snapshot_restore` 則保留給未來真正的 archive snapshot restore flow。
- v1 shipping 的 `snapshot_restore` 是 **checkpoint replay**：使用者從 Audit review 一個已保存的 browser source checkpoint（`raw-source-checkpoint`）做 Preview / Execute，PathKeep 會把該 checkpoint 重新 ingest 回 canonical archive，留下獨立 `snapshot_restore` run 與 artifact 關聯；既有可見 facts 不會因為 restore 被 destructive 覆寫，重複資料則由 dedupe / watermark 邊界跳過。
- rekey flow 產生的 archive safety snapshot file 也必須寫進 audit / security summary，保留 open / copy path 與 manifest / run detail；但若該 snapshot 需要舊的 database key，v1 仍保持 manual-first recovery，而不是假裝任何 archive file snapshot 都能自動 replay。

---

## 2. 排程

**作為**用戶，**我想要**設定自動備份的排程，**以便**不需要手動記得去備份。

### 需求要點

- 使用 OS 原生排程機制，不依賴 app 常駐運行：
  - macOS：LaunchAgent（`RunAtLoad=true` + 固定週期檢查）
  - Windows：Task Scheduler
  - Linux：systemd user timer（支援 persistent timer）
- 預設邏輯：短週期（如每 1 小時）喚醒檢查，但只有超過設定的備份間隔才真正執行備份。真正「該不該跑」的判斷由 worker 內部決定（檢查上次 run 時間），timer 只負責喚醒。
- **預設備份間隔為每 12 小時**。用戶可以在設定中自定義間隔（從最短 1 小時到最長數天）。
- 「沒有一直開機也能補跑」：開機/登入後，如果距離上次備份已超時，自動補跑。
- 排程的設定走 Preview/Manual/Execute 流程：
  - 用戶能看到將要安裝的 plist / XML / service 文件的內容。
  - 用戶能選擇手動複製並安裝，或讓 app 代為安裝。
  - 如果原生排程已安裝，Schedule 頁也必須提供明確的移除 / 解除安裝 CTA，而不是要求使用者自己回到系統資料夾手動刪檔。
  - 安裝結果記入審計日誌。
  - 移除 / 解除安裝同樣要留下 verify 訊號與 audit artifact，讓使用者能確認 PathKeep 實際移除了哪些檔案。
- Dashboard / Settings / Schedule 必須共享平台 capability 與 troubleshooting 語法，至少能清楚暴露 manual-review、mismatch、legacy install、permission warning 等狀態，並直接引導回排程頁修復。
- Schedule 的 Verify surface 必須直接列出 install state、detected files、warnings、latest audit artifact 與最近一次 apply / remove 結果，並保留 Preview / Manual / Execute / Verify 的快速切換，避免 verify 只剩一段模糊狀態字串。
- App 的「開機啟動」（autostart）和備份排程是兩件分開的事。

### 平台注意事項

- **macOS**：`LaunchAgent` + `StartInterval` 即可，語義直接。
- **Windows**：`Task Scheduler` + `StartWhenAvailable=true`，能正確補跑錯過的任務。
- **Linux**：必須使用 `OnCalendar=` 搭配 `Persistent=true`，而**不是** `OnUnitActiveSec=`。原因：systemd 的 `Persistent=` 只對 `OnCalendar=` 有效，monotonic timer 無法保證補跑。

---

## 3. 數據導入

**作為**用戶，**我想要**從 Google Takeout 導入歷史數據，**以便**能把 Google 帳號端保留的更早的歷史紀錄也納入 archive。

### Google Takeout 導入

- GUI wizard 流程：
  1. 拖入 Takeout 的 zip 或解壓後的資料夾。
  2. 先做 dry-run：掃描文件、識別格式、產生報告。
  3. 已知格式（如 `BrowserHistory.json`）進入 importer。
  4. 未知文件進入 quarantine（隱離區），不會被導入，在 UI 中顯示原因和文件內容摘要，讓用戶決定如何處理。
  5. 用戶確認後，才正式寫入 archive。
- 導入前的預覽：用戶能看到將導入多少筆記錄、時間範圍、會不會與現有記錄重複。
- dry-run / preview 必須回報 candidate item 數量、preview entries、warnings、quarantine 結果，以及可回看的 audit artifact 路徑。
- import / onboarding finalization 的 progress surface 必須持續回報 typed phase、current/total、percent、detail 與近期 log lines；post-import 的 backup / rebuild / shell refresh 只能作為 background-style follow-up，不能讓前景 overlay 因等待整串收尾而卡死。
- 若 import batch 的 audit artifact 遺失或上一次 post-commit 寫入失敗，recent batch preview 與 doctor repair 都必須能重建同一批 JSON review artifact，而不是讓 committed batch 永久失去 review surface。
- 導入後可回滾：用戶可以查看每次導入的記錄，如果發現導入的數據有問題（髒數據），可以回滾整次導入。
  - Takeout rollback 走和 backup / revert 相同的 soft-hide visibility model：imported rows 從正常 recall / export 中隱藏，但 raw facts、manifest 和 snapshot artifact 保持可審計。
  - 回滾後必須支援 un-revert / restore，讓整批 import 能恢復可見。
- Import review surface 應把 preview、recent batch detail、revert / restore、doctor report 與 repair CTA 放在同一條 trust workflow 裡，避免使用者切頁後失去驗證上下文。
- 提供詳細的操作指南：怎麼從 Google Takeout 請求導出、怎麼下載、怎麼找到歷史紀錄文件。

### 瀏覽器直接導入

- 同樣走 Preview/Manual/Execute 流程。
- Step-by-step UI：
  - 每個步驟都說明我們在做什麼、為什麼要做這件事。
  - 自動化模式下：逐步展示進度，且 phase/log/progress 需要在整個執行期間持續更新。
  - 手動模式下：每步有操作指南，能複製命令，做完再進下一步。

### 模塊化設計：獨立 Rust crate

瀏覽器歷史紀錄的解析和導入邏輯應設計為**可獨立發布的 Rust crate**（例如 `browser-history-parser`），而不是深度耦合在 vault-core 中。

- **解析層**：各瀏覽器的 History DB parser（Chromium、Firefox、Safari、Google Takeout）封裝為獨立 crate，提供結構化的歷史紀錄數據，不依賴 archive schema 或 Tauri。
- **公開 API**：對外暴露 schema detection、history/visit/URL parsing，以及對「已提供的 profile / DB 路徑」做 metadata inspection 的能力。
- **平台邊界**：OS 級的已安裝 browser / profile discovery、權限檢查、staging copy 仍留在 `vault-platform` / `vault-core`，不塞進 parser crate。
- **對社區的價值**：瀏覽器歷史紀錄的解析是一個通用需求（其他備份工具、分析工具、研究項目都可能用到），把這部分獨立出來可以讓其他開發者直接使用。
- **vault-core 作為消費者**：vault-core 依賴這個 crate，在其之上實現 archive 寫入、去重、run 管理等業務邏輯。
- **版本獨立**：parser crate 可以獨立發版，追蹤瀏覽器更新更靈活。

---

## 4. 數據匯出

**作為**用戶，**我想要**把 archive 數據匯出成通用格式，**以便**能在其他工具中使用或做長期備份。

### 本地匯出

- 支援格式：HTML、Markdown、純文本、JSONL。
- 匯出支援篩選：profile、時間範圍、domain、搜尋 query。
- 匯出只包含**當前可見** query 結果；已回滾或 hidden 的 facts 不會進入 artifact。
- 匯出報告記入審計日誌。

### 遠端備份

- 支援備份到 S3 相容的對象存儲。
- day-one provider scope 是 S3-compatible bucket，並支援自訂 endpoint、object prefix 與 path-style URL，讓使用者能對 AWS S3 和常見相容服務做同一套配置審查。
- 用戶在設定中配置 endpoint、bucket、region、prefix、credentials。
- 遠端備份是明確的用戶操作，不會自動上傳。
- v1 遠端備份必須走完整 PME：
  - Preview：先顯示 `bundlePath`、`objectKey`、`uploadUrl`、warnings 與 preview curl command。
  - Manual：保留 manual upload steps、retention reminder 與 restore checklist，避免把遠端物件存儲變成黑盒。
  - Execute：只有在 config 已 review 且 credential 已儲存時才允許上傳；成功 / 失敗都要回寫 last uploaded object key、時間與錯誤訊息。
  - Verify：對實際生成的 bundle 重算 checksums、檢查 bundle version / required entries、核對 zip entry set、驗證 detached manifest checksum，並做本地 restore readiness 驗證，不能只把 upload 成功當作 restore 成功。
- v1 bundle format 是 `pathkeep.remote-backup.v1` zip，至少包含 `archive/history-vault.sqlite`、`config/config.json`、`metadata/bundle-manifest.json` 與 `metadata/bundle-manifest.sha256`；如果本地已有 audit manifests / scheduler artifacts，也應一併打包。manifest 需記錄 `createdAt`、`appVersion`、`archiveMode`、`objectKey`，以及每個 entry 的 `sha256` / `sizeBytes`。
- remote verify 的 manifest hash / entry-set 檢查屬於 **corruption / drift detection**，不是 detached signing 或 cryptographic authenticity proof；v1 仍不宣稱已提供 remote-trust attestation。
- Verify 對 plaintext archive bundle 必須提出明確 warning；encrypted archive 的 verify / restore 則必須要求使用者先用相同資料庫金鑰解鎖 PathKeep。
- v1 的 retention / prune / retry 仍是 manual-first：PathKeep 提供 guidance 與 upload / verify trace，但不在使用者尚未信任 bundle format 與 restore workflow 前自動清理遠端物件。
- remote backup 的 support / diagnostics 預設只收集 metadata：bundle path、object key、bundle version、checksum / restore-readiness 結果、app version、run id。不得把 credentials、secret values 或 bundle 內容默認視為 support payload。

---

## 5. 安全與加密

**作為**用戶，**我想要**可以選擇加密我的 archive 數據庫，**以便**就算有人拿到我的硬碟也讀不了我的瀏覽紀錄。

### 需求要點

- 加密方案：
  - 使用隨機數據庫金鑰，由用戶主密碼經 Argon2id 導出的金鑰包裝。
  - 包裝後的 secret 存入 Stronghold vault（Tauri 提供的加密安全存儲，類似於 macOS Keychain 但跨平台）。
- 可選擇把便利解鎖信息存入系統 keyring（macOS Keychain, Windows Credential Manager, Linux Secret Service/KWallet）。
- 若使用者選擇記住 database key，PathKeep 只在啟動後的第一個 unlocked app session 讀取一次 keyring，之後把 active session key 保留在記憶體直到使用者手動鎖定 archive 或結束 app；不得在每次 shell refresh / status hydrate 時反覆重讀 keyring 並造成原生授權提示。
- 不加密模式也可以使用，但 UI 必須明確標示「數據庫為明文」。
- Linux 沒有可用 keyring 時：仍允許加密模式，但每次啟動都需要輸入主密碼。不做弱保護 fallback。
- keyring unavailable、session locked、password-loss 風險與 rekey boundary 不能只留在 Security；Dashboard 與 Settings 也要保留可見 warning 與導向 Security 的修復入口。
- 提供完整的 rekey 流程：更改密碼、明文→加密、加密→明文。
- rekey execute 必須留下可 review 的 audit summary：至少包含 `rekey` run、safety snapshot path、manifest artifact、以及 Security / Audit 可直接打開的 review surface，而不是只改完 config 就結束。
- 即使 archive 目前重新上鎖，Security 仍必須保留最近一次 rekey review 的時間、run id 與 safety snapshot path，避免 recoverability 線索只在 unlocked session 內短暫可見。
- **密碼遺忘等於數據丟失**：UI 中必須有明確、醒目的警告，要求用戶把密碼妥善保存，並且告知風險。

---

## 6. 審計與可信性

**作為**用戶，**我想要**能驗證我的 archive 數據完整性，**以便**確信數據沒有被篡改或丟失。

### 需求要點

- 每次備份 run 產生不可變的 manifest：
  - 上一個 manifest 的 hash
  - 來源文件的雜湊
  - 來源 schema 指紋
  - 各表的記錄數、insert/update 統計
  - 工具版本
  - 失敗原因（如有）
- Manifest 形成 hash chain（可審計的 append-only ledger）。
- Git 只管理審計文字工件：manifests, schema snapshots, 排程設定草稿, 導入/匯出報告, 完整性報告。
- 主 SQLite archive、raw 快照、cache 和 staging 不進 git。
- Audit run detail 應直接暴露 manifest / artifact 路徑，並提供 open / copy path 動作，避免使用者自行猜測資料夾位置。
- Audit run detail 可以用 `Summary / Artifacts / Warnings` 分頁降低資訊牆密度，但 open / copy path 動作與 warning honesty 不能被藏起來或延後載入。
- Doctor / 健康檢查命令：重算 archive table hash 並出報告。
- doctor / repair baseline 至少要涵蓋 missing import audit artifact、broken visibility references、stale derived state，並把 repair 本身寫回 unified `runs` ledger。
- local retention / prune 走 manual-first，但不是 hidden cleanup：Settings 必須提供 local snapshot / export / staging / quarantine 的 preview + explicit prune，並在 prune 後保留 run summary / manifest，而不是把 retention 寫成「只能自己去資料夾刪」。
- prune snapshot artifact 時，PathKeep 會把對應 artifact path 從未來的 review surface 移除，但不刪除原本的 run summary / manifest / audit trail。
- 如果用戶已配置 GPG 簽名 commit，就沿用。
- support / release 診斷預設只導出 metadata-first 事實：run id、audit path、manifest path、platform state、build info、checksum 結果；不自動包含 canonical archive facts、raw snapshots、API keys 或 master password。
- packaged desktop build 必須持久化 Rust log、frontend log 與 latest crash report。panic / uncaught frontend failure 不可只留在 ephemeral console；Settings 至少要能顯示 log / crash-report 路徑並提供 open folder 動作。

---

## 7. 可擴展的記錄增強系統（Enrichment）

**作為**用戶，**我想要**我的歷史紀錄不只有 URL 和標題，還能附帶更豐富的上下文信息，**以便**搜尋和洞察功能更準確、更有價值。

瀏覽器原生提供的歷史紀錄信息非常有限（URL + title + visit time），但 URL 本身暗含了大量可提取的信息。

### 增強層級

**第一層：立即可用（不需要外部請求）**

- URL 結構解析：domain, subdomain, path tokens, query parameters
- Domain 分類：docs / forum / video / news / social / shopping / code 等
- 搜尋引擎 query 提取（從 URL 參數中解析 `q=`, `search_query=` 等）
- Transition / referrer 信息
- Favicon

**第二層：背景 refetch**

- 訪問 URL 抓取頁面內容，提取 readable text、meta description、OG tags
- 提取頁面語言
- Best-effort，失敗不阻塞

**第三層：基於 URL 的專屬 enrichment 插件**

- 設計為**可擴展的插件架構**：每個插件匹配一組 URL pattern，負責提取特定的結構化信息。
- 插件範例：
  - **arXiv 插件**：匹配 `arxiv.org/abs/*`，調用 arXiv API 獲取論文標題、作者、abstract、分類、發表日期
  - **GitHub 插件**：匹配 `github.com/*/*`，提取 repo description、language、stars、README 摘要
  - **YouTube 插件**：匹配 `youtube.com/watch*`，提取影片標題、頻道、時長、描述
  - **Wikipedia 插件**：匹配 `*.wikipedia.org/wiki/*`，提取文章摘要
  - **Stack Overflow 插件**：匹配 `stackoverflow.com/questions/*`，提取問題、最佳答案摘要
  - **HN 插件**：匹配 `news.ycombinator.com/item*`，提取討論主題和熱度
- 插件以 trait / interface 的方式定義，第三方可以貢獻新的插件。
- 用戶可以在設定中啟用/禁用個別插件。
- 插件的增強結果存入統一的 enrichment 表，以 JSON 格式保存，欄位隨插件不同而不同。

**第四層：未來擴展 — 瀏覽時即時捕獲**

- 未來可能透過瀏覽器擴充套件在瀏覽時即時抓取頁面內容。
- Schema 預留 `content_source` 欄位標記來源（`plugin`, `refetch`, `realtime_capture`），讓即時捕獲的數據可以取代 refetch 結果。

### 設計原則

- 所有 enrichment 都是**非阻塞的** — 核心備份流程不依賴 enrichment 成功。
- Enrichment 結果和原始歷史紀錄鬆耦合 — 存在獨立的表中，以 history_id 關聯。
- Enrichment 可以隨時重跑 — 插件升級後可以重新增強舊記錄。
- Enrichment 的版本和來源有記錄 — 知道每條增強是哪個插件、什麼時候產生的。
- 調用外部 API 的插件必須內建 **rate limiter** — 避免觸發 API 速率限制。重試策略由 Job Queue（見 [intelligence.md](intelligence.md) 7.6）統一管理。

### M4-A / v1 已落地邊界

- 目前已交付的內建 plugin 只有 `readable-content-refetch`，預設啟用、版本為 `m4-v1`，設定存在 `AppConfig.enrichment.plugins[*]`，使用者可在 Settings 明確 enable / disable。
- `readable-content-refetch` 目前掛在 insights rebuild flow，而不是獨立的 canonical ingest pipeline；它只能寫入 derived enrichment / insight tables，不能修改 canonical `visits` / `downloads` / `search_terms` / `manifests`。
- manual backup / import 完成後不應同步阻塞等待 enrichment 或 insights rebuild；derived refresh 仍然是 explicit follow-up，由 Insights / Settings 的 rebuild surface 觸發，避免大型真實 profile 把核心 archive run 拖成整段不可操作。
- v1 的 freshness window 是 7 天；refetch client 採 10 秒 timeout、最多 5 次 redirect，`fetch-error`、`decode-error`、`unsupported-content`、`empty` 都視為 non-blocking derived failure，會留在 run notes / rebuild report，而不會讓核心 archive run 失敗。
- `readable-content-refetch` v1 目前也包含一小組 built-in site adapters，先正式支援影片站 metadata parse（`youtube.com` / `youtu.be` / `vimeo.com`）。這些 adapter 只會回寫 derived enrichment metadata（如影片標題、頻道 / 作者、時長、描述、發布時間），不會改寫 canonical archive facts，也不代表 day-one 已有完整的多 plugin sandbox / queue family。
- Settings 必須提供 plugin version、queue、freshness、derived tables、storage impact、latest growth signal，以及 `rebuild derived state` / `clear derived state` controls。`clear derived state` 只能清除 enrichment / insight tables，不可影響 canonical archive facts 或 rollback ledger。
- 第三方 plugin API、獨立 enrichment queue、以及 provider-specific structured plugins 仍在後續 M4 work 中，不應假裝 v1 已經完整落地。

---

## 8. App Lock（應用程式鎖定）

**作為**用戶，**我想要**在啟動和閒置後要求解鎖才能存取 PathKeep，**以便**即使電腦未鎖定，別人也無法瀏覽我的歷史紀錄。

### 需求要點

- App Lock 是 **UI session lock**：啟動時、手動鎖定時，以及可配置的閒置逾時（預設 5 分鐘，可調 1–60 分鐘）後出現。
- 目前 shipped unlock path 是 **app-lock passcode + macOS Touch ID**。
- Touch ID 只在 macOS 上作為真正可用的 session unlock path；Windows / Linux 仍維持 capability / degradation state，不可假裝已有 native biometric parity。
- biometric toggle 是 authoritative user preference：若使用者在 Settings 關閉 biometric unlock，lock screen 不得再顯示 Touch ID / biometric CTA，backend 也不得繞過設定直接允許 biometric unlock。
- 目前不 shipping 獨立 PIN mode；passcode 是唯一正式的解鎖憑證。
- Lock screen 顯示 PathKeep branding、鎖定原因、config path、上次解鎖時間、passcode prompt，以及帶 recovery hint / open-config-path 動作的 recovery callout。
- 鎖定時必須阻斷 shell rendering、desktop query/read commands，以及 MCP 的 history query surface；不能只靠前端遮罩。安全例外 surface 只保留 lock status、unlock、config-path recovery 與非資料型 diagnostics。
- Settings 的 App Lock panel 必須包含：enable / disable toggle、idle timeout、biometric toggle（若平台未接線或暫不可用則 disabled + honesty note）、passcode set / update / clear、recovery hint、`Lock now`、config path、last unlocked timestamp。
- shared profile scope 仍然只是 viewer / filter contract，不會因為 App Lock 而升級成真正的 per-profile partition。

### 與 Archive Encryption 的區別

| 層級           | 保護對象              | 啟用條件             | 密碼遺失後果                      |
| -------------- | --------------------- | -------------------- | --------------------------------- |
| **App Lock**   | UI session            | 用戶在 Settings 啟用 | 重置 App Lock 不影響 archive 資料 |
| **Encryption** | 資料庫檔案（at-rest） | 用戶在 Security 啟用 | 密碼遺失 = 資料丟失               |

- 兩者可獨立啟用、也可同時啟用。
- App Lock 的密碼可以和 encryption 主密碼不同，但 UI 必須清楚提示用戶此區別。

### 平台考量

| 平台        | 目前 shipped unlock path | truthful stance                                                                                       |
| ----------- | ------------------------ | ----------------------------------------------------------------------------------------------------- |
| **macOS**   | passcode + Touch ID      | Touch ID 現在可用於解鎖當前 UI session；passcode 仍是 required fallback，不取代 archive encryption    |
| **Windows** | passcode                 | Windows Hello 尚未接進目前 build；Settings / lock screen 顯示 truthful unsupported / degradation note |
| **Linux**   | passcode                 | 維持 passcode-only；不宣稱有 biometric 支援                                                           |

- `PG-RD-PLAT-006` 已定案：App Lock 保護的是 UI session 與 read/query surface，不是 database key；macOS Touch ID 的 additive amendment 見 [ADR-007](../architecture/decisions/007-macos-biometric-session-unlock.md)，session-only boundary 本身仍以 [ADR-005](../architecture/decisions/005-app-lock-session-boundary.md) 為準。

### 導航規則

- 畫面與導航結構 → `docs/design/screens-and-nav.md` §App Lock
- 決策記錄 → `docs/architecture/decisions/005-app-lock-session-boundary.md`
