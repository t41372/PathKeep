# Program — Quality Matrix

> 這份文檔是 PathKeep quality gate 的權威定義。原則：**文檔怎麼寫，repo 就怎麼擋**。
>
> 2026-10 起的測試政策：不跑 mutation test，不強制 100% 覆蓋率，測試以 E2E 為主。原則見 [AGENTS.md](../../../AGENTS.md)「測試是契約」。更早的 closeout 記錄（`CHANGELOG.md`、`STATUS.md`、各 milestone 文檔）提到的 coverage / mutation gate 都是歷史，已不存在。

---

## Mainline Blocking Path

`bun run check` 是唯一的 per-commit gate，本地與 GitHub `CI` workflow 跑同一條。依序執行：

| 步驟           | Command                 | 保護什麼                                                                                                                                                            |
| -------------- | ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Format         | `bun run format:check`  | Prettier                                                                                                                                                            |
| Lint           | `bun run lint`          | ESLint（`--max-warnings 0`）                                                                                                                                        |
| i18n           | `bun run check:i18n`    | `en` / `zh-CN` / `zh-TW` key 對齊、不得有未翻譯的英文                                                                                                               |
| Typecheck      | `bun run typecheck`     | `tsc -b`                                                                                                                                                            |
| Unit           | `bun run test:unit`     | Vitest：少數必須單獨測的（例如 regex 方言對照表），寫的理由在檔頭                                                                                                   |
| Build          | `bun run build`         | TypeScript + Vite bundle                                                                                                                                            |
| Rust           | `bun run check:rust`    | `cargo fmt --check`、`cargo clippy -D warnings`、`cargo test --workspace`，再加帶 `devtools-bridge` feature 的 bridge dispatcher 測試（`test:desktop-bridge:rust`） |
| Release config | `bun run release:check` | updater URL、Windows bundle、WebView2、support link 不得漂移                                                                                                        |
| E2E            | `bun run test:e2e`      | 真實 Rust 後端 + Vite 前端 + Playwright，見下                                                                                                                       |

`bun run check:base` 是不含 build 與 E2E 的快速 triage（`check:js` + `check:rust`），不能代替 `check`。

## E2E

E2E 是證明功能的主要手段。環境是獨立的，但其餘都是真的：

- `playwright.config.ts` 透過 `scripts/pathkeep-dev-desktop-bridge.mjs` 啟動帶 `devtools-bridge` feature 的 debug Rust 後端與 Vite 前端，Playwright 驅動真正的畫面。E2E 時（`PATHKEEP_E2E_RESTART_FILE`）腳本不經 `tauri dev`：自己跑 worker sidecar、Vite 與一次 app 的建置，再自己顧著 app，讓 spec 能像當機一樣殺掉 app、由腳本在同樣的連接埠與資料上重開（`tests/e2e/support/app.ts`）；沒有 spec 要求的結束照舊結束整個 run。
- 網路：E2E 後端擋掉自己發起的抓取（`PATHKEEP_TEST_BLOCK_NETWORK`），只有連結預覽可以連到 spec 自己在 `https://127.0.0.1` 開的網站（`PATHKEEP_TEST_PREVIEW_SERVER_CA`，憑證在 `tests/e2e/support/tls/`）；使用者設定的 AI 服務（spec 自己開的假服務）照常連。兩個開關都只在 debug 建置有效。
- 測資是臨時目錄裡的合成瀏覽器 profile（兩個 Chrome、一個 Firefox，真實 SQLite，seed 固定，`scripts/fixtures/synthetic-browsers.mjs`），archive、keyring、project root 都在該目錄內；cargo target 放在 `var/playwright/desktop-bridge/cargo-target` 以便重用編譯快取。
- 預期數字一律由測資算出（`tests/e2e/support/fixture.ts`），不在 spec 裡手寫。
- spec 依序跑在同一個 archive 上：`first-run` → `read` → `change` → `wipe`（見 config 的 `projects`）。
- 不替後端寫假實作。需要 seam 的地方只 seam 在最外層 transport，真正的 compute、decode、I/O 留在被測的 build 裡。
- 新功能至少一條從真實入口出發的 medium-to-hard 場景：使用者做 X，觀察到 Y；要有「負 → 正」斷言（改動前找不到，改動後找得到）。

### Artifact

每次跑完（不論通過與否）都留下可驗證、可重複的產物，位置固定為 `artifacts/e2e/`：

- `fixture.json`：這次用的測資（seed、`now`、每個 profile 的每條訪問）；`PATHKEEP_E2E_NOW=<now> bun run test:e2e` 用同一份資料重跑
- `report/index.html`：HTML report，每個檢查都附上比對的 `{ expected, actual }`
- `results.json`：每個 test 的結果
- `test-results/`：trace、screenshot、失敗時的 video

`scripts/run-playwright.mjs` 結束時會印出這個路徑。該資料夾不進 git（見 `artifacts/e2e/README.md`）；CI 以 `if: always()` 上傳成 `e2e-artifacts`。重現同一條 run：`PATHKEEP_E2E_NOW=<fixture.json 的 now> bun run test:e2e`。

## 什麼時候才寫 unit test

功能由 E2E 證明（AGENTS.md「測試是契約」）。unit test 只在這幾種情況寫，而且寫在程式碼之前：先在測試檔頭列出這段邏輯會怎麼壞（failure modes），每個情況一個有名字的測試，再寫程式碼。

- 規格清楚的純邏輯，E2E 要逐一跑太貴：例如 regex 方言對照表（60 個 pattern）、query family 的歸組規則（每個 failure mode 一個例子，另拿「逐一比較」的參考實作對照）。
- 資料完整性與當機復原：寫到一半被殺掉、重開後資料要對（`vault-crypto` 的 crash / tamper 測試，種子固定、可用環境變數換）。
- 安全邊界：SSRF 每一跳、`file://` 不能被打開、鎖定時不讀 Keychain。
- 修過的 bug，而 E2E 重現不了（時間、競爭、14.4M 才出現的查詢計畫：`intelligence_read_plans`、`append_plans`）。

不寫的（2026-10 審查的 B / C，見 `docs/plan/test-audit-2026-10/`）：

- 鎖住沒人看得到的細節：內部字串、私有 helper 的輸出、計數、內部順序；
- 重述程式碼、探測常數，或為了 coverage / mutation 分數的近似重複；
- 照抄程式碼現在的行為（寫在程式碼之後的測試就是這樣）。E2E 的手算也一樣：用 fixture 想表達的結果去驗，例如 `home.spec` 驗「每個搜尋與它的改寫是一組」，不只照抄後端規則。

測試照它要保證的事命名（`a_search_goes_to_the_first_search_it_is_most_like`），不照實作命名。

## Slow / Optional

不在 `bun run check` 內，但要在 CI 或 release 前跑：

| Command                                                   | 用途                                                                                      |
| --------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `bun run check:slow`                                      | `check:supply-chain` + `check:platform`                                                   |
| `bun run check:supply-chain`                              | `cargo audit` + `cargo deny`（需要網路與對應 cargo 工具）；CI 另有獨立步驟                |
| `bun run check:platform`                                  | host-matched keyring / scheduler / launcher / updater native 測試，含 debug desktop build |
| `bun run verify`                                          | `check` + `desktop:build:debug`；release / milestone closeout 的本地預演                  |
| GitHub `Platform Native` / `Native Dependencies` workflow | manual / path-triggered，見各 workflow                                                    |

## Honest Boundaries

- E2E 證明前端能透過 dev-only localhost bridge 打到真實 Rust command façade。它不安裝真的排程（onboarding 選 Manual），也不寫真的系統 keychain。它不是 Tauri WebView / plugin guest API（Stronghold、updater progress events）的最終驗收；那些要在真實 Tauri 視窗驗證。
- 原生 scheduler、keyring、簽名、公證與檔案系統副作用要靠 `check:platform`、Rust 測試與真實主機驗收。Windows Task Scheduler apply / status / remove 仍需要真實 Windows 主機。
- `release:check` 只證明 release config 還允許 unsigned Windows installer 與 WebView2 download bootstrapper，不證明特定主機能裝起來。
- 沒有 coverage 數字。看到綠燈不代表功能被測過：review 時問「使用者實際操作有沒有一條 E2E 從真實入口斷言預期結果？」答不出就不算 ship-ready。

---

## 資料完整性：「為什麼沒測到」與此後的 blocking 鐵律（2026-06-30）

> 教訓：一個會 **整個 app 變磚** 的缺陷在「100%-GREEN gate」下溜給了使用者——磁碟上兩個 canonical DB 都被 SQLCipher 加密，`config.json` 卻寫 `Plaintext`，salt 被歸零、又沒有 journal，下一次開檔命中 `SQLITE_NOTADB` 直接死在啟動。根因和 2026-06-28 同源、但更嚴重：gate 量的是 **EXECUTION（coverage）**，不是 **BEHAVIOR / INVARIANT**。具體四個漏洞：(a) 沒有任何測試斷言跨檔不變式「`config.json` 記的 at-rest mode == canonical DB 磁碟上的真實 at-rest mode」；(b) 測試手搓 `AppConfig` struct，不走 `load_config`，真正的 config-load/drift 路徑從沒被跑過；(c) 部分真實 I/O 引擎被 `#[cfg(not(any(test, coverage)))]` **從 coverage binary 編譯掉**、換成永遠成功的 stub；(d) 完全沒有 crash-window / 併發 / 交錯測試。Phase A–D 補了真正的 crash-window 測試（`fault_inject` seam + 真 SQLCipher round-trip），Phase E 把它系統化成可複用方法論並補齊剩餘缺口。

鐵律（文檔怎麼寫，repo 就怎麼擋；每條都要能被 check 兌現）：

1. **Durability（原子 + F_FULLFSYNC + dir-fsync）。** 所有 archive 寫入路徑必須走原子持久化路徑——`durable_io::atomic_durable_write` / `install_file_durably`（temp → `F_FULLFSYNC` → rename → parent-dir fsync）與 `save_config` / `remove_file_durably`；archive 寫入路徑內 **不得** 出現裸 `fs::write` / 裸 `fs::rename` 去落地一個 canonical 檔或 `config.json`。crash 中斷只能留下「完整舊值」或「完整新值」，絕不是把 mode 改動悄悄吞掉的截斷/空檔。
2. **Config↔disk consistency（跨檔不變式）。** 每一個 archive-mutation 測試（改密碼 / 模式切換 / 匯入 / 還原 / 啟動恢復）的 post-condition 都要斷言 config 和檔案一致：原生 archive 用 `archive::at_rest::check_config_disk_consistency`（經 **真正的** `load_config` 讀 `config.json`，用 `detect_disk_encryption_mode` 讀每個持久化原生資料庫的真實 at-rest mode，header-only、key-free）；舊格式（SQLCipher）的啟動恢復用轉換器測試裡的 `assert_config_matches_disk`。config 記的 at-rest mode **永遠不得** 和已安裝的 DB 分歧——這就是能擋下本次事故的那條 check。專屬的事故重現測試（加密檔 + `Plaintext` config）斷言 checker **FAIL**；crash-window 測試（`rekey.after_swap_before_config`）斷言 checker + launch recovery **一起** 抓到「config 落後於已安裝的檔案」。
3. **Crash-window coverage。** 每個破壞性 archive op 都要有一條 kill-at-checkpoint regression（`fault_inject` 具名 checkpoint + `FaultGuard::error_at_must_fire` / `panic_at`），且該測試在 **未硬化的舊碼** 上會 FAIL（舊碼沒有 crash seam、在 swap 前就寫 config）。測試必須斷言 **被注入的** 錯誤有沿 `format!("{err:#}")` / `err.chain()` 傳播，不能只 `is_err()` / 只驗「可恢復」。每個 op 還要有 per-checkpoint 的 **窮舉** torture：對每個 checkpoint 注入 crash → launch recovery → 斷言收斂到 **consistent-or-fail-closed**（config 對得上檔案、canonical rows 還在，或 marker 留著 fail-closed），永不 empty/mixed。torture 必須 **DETERMINISTIC**：以固定 checkpoint 列舉或固定 seed 驅動，禁止 wall-clock / `Math.random` / thread-timing 造成的 flakiness（gate 會跑這些）。
4. **不得把 production I/O 用 `cfg` 編譯出 coverage binary。** 真正的 compute / I/O 路徑要留在 **被量測** 的 build 裡；只有最外層 endpoint（socket / syscall fd）可以 seam，且 seam 用的 thread-local FIFO 在 production 恆為空（見 `fault_inject`、`write_lock` 的 `next_flock_fault`、`durable_io` 的 `inject_fsync_faults`）。**Coverage 量的是執行，不是行為**：每個 user-facing invariant 都要一條明確的 behavioral 斷言（負→正：斷言「被 heal / 被 restore / rows 還在」，不是只斷言「沒丟錯」）。

新功能 / 硬化 review 必問：「破壞性 op 有沒有 kill-at-checkpoint 測試、且在舊碼上會 FAIL？成功路徑有沒有斷言 config↔disk 一致？測試走的是 `load_config` 還是手搓 config？真實 I/O 有沒有被編譯掉？」答不出＝不算 ship-ready，無論 coverage 幾趴。

對應實作：原生 archive 在 `vault-core`：`archive::at_rest::check_config_disk_consistency`，斷言在 `maintenance::tests`（模式切換、快照還原、中斷安裝的啟動回滾）；crash window 由 `maintenance::install` 的具名 checkpoint 與 `install_tests` 覆蓋。舊格式 archive 在 `vault-convert-sqlcipher::legacy_recovery`：舊 app 留下的匯入 / 改密碼 / 還原 journal 的啟動恢復，測試直接在磁碟上造出當時的 crash 狀態（舊流程已刪除，無法再「跑到一半殺掉」）。2026-10 之前的舊流程（SQLCipher 的 rekey、import、full restore、reconcile）及其 torture 測試隨區塊 6 刪除。flaky `diagnostics::tests::rust_panic_payloads_keep_owned_strings_and_fallback_text` 以 thread-local opt-in 隔離 process-global panic hook（外來 thread 的 panic forward 給 previous hook，不再污染擷取）。
