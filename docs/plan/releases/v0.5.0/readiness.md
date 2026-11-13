# v0.5.0 發布審查

首次調查：2026-11-10；本輪更新：2026-11-13（America/Phoenix）。這次授權包括依賴 PR 的審查、驗證、合併與發布準備；公開發布仍須使用者明確批准。

**本輪仍只做發布準備，不公開。** 原先等待另一 worktree 的嚴重
embedding／長讀阻斷修復。現在 [PR #24](https://github.com/t41372/PathKeep/pull/24)
的固定 head `aec2004de37298c027893ac681e145cff6824753` 已通過遠端 CI，
既有審查 thread 已解決；本分支已把它整合到隔離預演，沒有變更 PR #24
的主分支合併狀態。修復避免既有索引的計數反覆掃整個 lineage、限制
checkpoint 阻擋新讀取的等待，並移除閒置索引器的空寫入。

整合來源連同 updater 明確 tag 設定須重新跑完整 gate、凍結新 SHA 並
建置所有平台，再交付實際安裝包審閱。之後若 PR #24 或候選內容再變，
要重新判斷驗證範圍並重建產物。公開仍須使用者明確批准；這次的既有
14.4M／原生驗收維持原適用範圍，不把它當作新修復的驗證。

**既有合併 gate 已通過，發布仍等待使用者 approval。** 2026-11-10 使用者指出既有 14.4M 升級／備份與跨平台安裝已驗證，本次審查據此修正：本機受限環境的失敗不否定既有 CI／驗收；預演時的 JavaScript 稽核曾命中 19 個套件，更新後的鎖檔稽核已清零；資料格式升級的降級限制是發布告知與回退考量，不是升級故障。前一版將這些混列成發布阻斷的總結已撤回。 [本輪核對紀錄](assessment-correction.json) 保留 CI 連結、19 套件版本比較與使用者驗收確認；原始失敗日誌和 hash 不修改。簽章路徑與最終產物的例行核對仍按發布計畫確認。

## 整合後的離線驗證

固定 PR #24 head 與 updater 明確 tag 設定已一起通過完整
`bun run verify`、`bun run check:slow`：141 個前端測試（含 31 個
發布 CLI／HTTP 場景）、134 個 E2E 通過／1 個平台略過、desktop debug
build、Rust／平台與供應鏈檢查通過，兩份鎖檔 hash 未變。
JavaScript audit 掃描 532 套件為零，Rust 既有例外政策未變。
[整合驗證 receipt](release-integrated-rehearsal.json) 記錄實際輸入、修復
head、code hashes、三組 E2E 結果與已下載報告的 SHA-256。

原生建置與遠端只讀／併發預演在這份簽名來源凍結後執行，記錄留在
PR #22 與私有草稿，避免為追加操作紀錄而移動安裝包的 source SHA。
公開發布與政策變更仍等明確 approval。

## 本輪恢復與準備

原 stash `36b428e21782f30e63a9e3fdbb8955d815caa942` 已恢復到
`var/release-v0.5.0-prep/`，保留原 stash。恢復時 46 個檔案與 stash 原文
逐位元相同，凍結證據的 28 個檔案亦符合原 manifest，見
[stash-recovery.json](stash-recovery.json)。恢復內容中的準備筆記、三語 release
notes 隨候選狀態更新；README banners 沿用 stash 原件。

[PR #20](https://github.com/t41372/PathKeep/pull/20) 更新 JS／TS 依賴。
兩個有效的審查問題已重現並修正：清除搜尋時舊動畫幀寫回資料，以及
Modal setup 的安裝失敗被條件 subshell 吞掉。原始依賴驗證見
[dependency-refresh.json](dependency-refresh.json)；本輪另跑完整 gate。

發布候選包含同一組修正、README banners 與四處 `0.5.0` 版本更新，
也獨立通過 `bun run verify`、`bun run check:slow`（119 個前端測試、
134 個 E2E／1 個平台略過）；證據見 [release-rehearsal.json](release-rehearsal.json)。PR #20 的最新 CI 已通過，並合併至
`f4e4759e73f34f0a4a3e62ed9e16c0f9f7dfbe38`。此候選已對齊該 main；來源與
產物核對結果附於發布準備 PR／Release 草稿，未公開發布。

Actions secrets 名稱清單於本輪再次核對，仍為空。目前能準備的是
unsigned preview 草稿：手動安裝、沒有 app 內自動更新，macOS 未經
Developer ID 簽章或公證。產物可先供審閱；將它作為公開版本仍須批准
本次 unsigned preview 政策例外。三語 notes 已按此候選路徑準備。

## 發布流程修正與批准範圍

第一版 guard 的九個 CLI／HTTP 場景及公開 v0.4.0 的只讀 Actions 預演
通過，舊 release 與 11 個 assets 不變。不過 PR #22 的審查指出它只在
開頭讀一次狀態，且不同 ref 沒共用同 tag 的鎖；這不足以阻止後續覆寫。
該判斷已接受，第一版紀錄保留為歷史，不把它當成完整競態保證。

[提議的 trade-off](../../../architecture/release-publication.md) 將手動 tag
改為必填，`prepare`／`verify`／`publish` 共用目標 tag 鎖。建置一律是私有
草稿，綁定 source SHA；公開獨立執行，只核對及發布已批准的 SHA、draft
ID 與 manifest digest。Source 或 bytes 變動即停止，不重新建置替換。

GitHub immutable releases 的實際查詢回覆為 HTTP 200、`enabled=false`。
公開前由管理員啟用並重新核對 boolean，才確認
`immutable_releases_confirmed=true`。伺服器鎖定公開後的資產與 tag；
workflow token 不增加 administration 權限。這項設定尚未變更。

新的發布政策與 immutable 設定啟用，以及 macOS unsigned preview 例外，
均留在最終使用者 approval。此時僅從提案分支準備私有候選。遠端
Actions／產物核對紀錄附於發布準備 PR 與草稿，候選 commit 不因新增
操作紀錄而移動。

## 本輪 gate 的測試環境修正

完整驗證兩次停在 worker 的 fallback-isolation 測試，測試 server 等待
第三個 embedding 請求逾時。對照使用同一個既有測試 binary：在 D-Bus
session 使用原生 keyring 時失敗（10.29 秒），設定隔離的測試 keyring
後通過（0.28 秒）。原 fixture 未隔離 provider resolution 的 keyring
讀取，意外進入 headless secret service。

修正使用既有環境鎖與 temporary keyring，並要求收到 fixture 的 HTTP
拒絕原因，避免任何提早失敗都被 `is_err()` 接受。沒有放寬 timeout、
略過測試或修改 embedding 產品程式。兩次失敗和對照日誌均保留；這個
測試環境問題不代表另一 worktree 的產品 bug 已解決。

其後的完整 E2E 在 Insights 重新載入時未於 15 秒內出現 heading；trace
停在程式碼分片的 skeleton，沒有 JavaScript exception。原時區情境的
獨立重現保留原期限及所有功能斷言後通過（5.2 分鐘，該次 reload 為
4.335 秒），但測試 worker 的 RSS 達約 8GiB。測試為 22,257 筆 fixture
反覆建立 ICU formatter 及重掃整份資料，是可確認的測試負擔；不能只憑
一次超時斷言產品的時區功能回歸，也不能忽略那次失敗。

改為重用每個時區的 formatter、每個時區只算一次 fixture 筆數，保留
原本 15 秒斷言期限。三時區完整筆數對照的 hash 相同；各自獨立 Node
process 的計算為 15,078ms／1,846,916KiB peak RSS 與
119ms／91,388KiB。這是測試 reference 計算的量測，不是產品或
14.4M benchmark。同一合成存檔與真實後端的載入診斷另測得 production
bundle 三次 735–846ms、開發模式 1,885–2,573ms；它沒有重現原始超時。
完整 gate 必須在上述測試修正後重新執行，先前失敗與診斷均保留。

## 初次預演的版本與範圍

| 項目          | 已確認的值                                                                           |
| ------------- | ------------------------------------------------------------------------------------ |
| Repository    | `t41372/PathKeep`，公開倉庫                                                          |
| 原始碼        | `89ed2040bbb0cc1a36450f7004a08a51024bf70b`，與調查時遠端 `main` 一致                 |
| 公開最新版    | [v0.4.0](https://github.com/t41372/PathKeep/releases/tag/v0.4.0)，2026-10-19 公開    |
| v0.4.0 原始碼 | `8ee91193cbfc760fec4e78cadc7528d546bb892d`                                           |
| 建議版本      | `0.5.0`，延續 repo 的 `0.5.0-dev`；新功能與資料格式升級適合 minor version            |
| 自 v0.4.0 起  | 47 個 commits（含 merge），391 個檔案改動；主體為 PR #16、#18 與文檔 PR #19          |
| 本機預演副本  | `var/release-preflight-2026-11-10/`，本機 shared clone；首次調查時沒有向 GitHub 寫入 |
| 版本 patch    | [release-version.patch](release-version.patch)，僅四處版本，不變更依賴               |

首次調查時，`README.md`、`README.CN.md`、兩張 banner 與 `scripts/readme-banner.mjs` 是主工作目錄既有的工作，當時沒有納入預演。本輪已從 stash 恢復並納入發布準備副本。

新版本的重點：搜尋排序與搜尋建構器；備份不再把瀏覽器列號當造訪身分；可區分完整、部分完成與失敗的備份；排程與選單列狀態修正；語意索引分代發布，重建完成前繼續使用現有完整索引；退出時保護進行中的工作。三語發布筆記：[English](notes.en.md)、[简体中文](notes.zh-CN.md)、[繁體中文](notes.zh-TW.md)。另備有 [Conventional Commit 範本](release-commit.txt)。

## 初次預演的 Gate 與驗證證據

| 驗證                             | 結果與邊界                                                                                                                                                                                                                                            |
| -------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| main 的 Linux gate               | [CI 42782511470](https://github.com/t41372/PathKeep/actions/runs/42782511470) 成功；SHA 與上表一致；主 E2E 124 passed／1 skipped，備份完整性 healthy 9 passed、damaged 1 passed                                                                       |
| main 的 Rust 供應鏈 gate         | 同次 CI 成功；不是 macOS／Windows 安裝驗證                                                                                                                                                                                                            |
| 0.5.0 本機候選                   | `bun run verify` 退出 101，停在兩個 `owner_process` 冷啟動測試；前端 format／lint／i18n／typecheck、107 tests、build、Rust fmt／clippy 通過；這次本機 run 未完成完整 gate；不否定已成功的 main CI                                                     |
| 本機 Rust 供應鏈                 | `bun run check:supply-chain` 通過：8 項明列的 advisory 例外；bans／licenses／sources 通過                                                                                                                                                             |
| JavaScript audit                 | 補充的 `bun audit --json` 退出 1；19 套件、88 項去重 advisory，適用性分述於下節；正式 CI 沒有查這項                                                                                                                                                   |
| 其餘 Rust 測試                   | `vault-core` 495 passed／12 ignored；其後 crypto cache guard 呼叫 `ps` 被 EPERM 拒絕，未取得記憶體數據。另行完成 owner-ipc／platform／search／store／worker 的原有測試，退出 0                                                                        |
| Desktop bridge Rust              | `bun run test:desktop-bridge:rust` 通過，4 個 bridge tests；不涵蓋真正 WebView                                                                                                                                                                        |
| 本機 E2E                         | app 啟動後視窗服務連線無效、health 端點未就緒；900,000ms webServer timeout，退出 1、**0 tests 執行**；healthy／damaged 配置未進行                                                                                                                     |
| 發布 metadata 預演               | 16 個案例通過：5 種平台選擇 × 2 種 unsigned 模式、4 種 tag 解析、版本相符／不符；執行 workflow 內的 Python／shell，Mac 僅為 `python` 提供 `python3` 別名；沒有 dispatch                                                                               |
| 本次 macOS native／debug         | `check:slow` 與獨立 debug build 退出 0；但兩個 native tests 的實際 Keychain／launchd 驗證都因主機權限跳過，不能算驗收                                                                                                                                 |
| 本次 macOS release bundle        | Release ARM64 `.app` 已產出；desktop／worker 的版本、架構、大小與 hash 已核對，worker 與 release staging 一致，動態連結只看到系統 library。DMG 在 `hdiutil: create failed - Device not configured` 失敗；本機這次未完成安裝驗收，不能據此否定既有驗收 |
| Size audit                       | 目前 web 3,239,065 bytes／106 assets；排除繼承的舊 bundle 後，標準腳本未掃到本次架構專屬 target 內的 app，該 app 另附 inventory；沒有假算出成功的 release installer                                                                                   |
| Windows／Linux／Intel 新版安裝包 | 使用者已確認跨平台安裝驗收完成；本次未重新建置，不代表既有驗收未完成。最終 tag 的版本、架構與 checksum 仍須核對                                                                                                                                       |
| 14.4M 備份與升級                 | 使用者 2026-11-10 已確認完成；撤回本次根據舊待辦推導的「尚未驗證」判斷                                                                                                                                                                                |

可保留的原始證據、逐項結果與 hash inventory 已存入 [`artifacts/release/2026-11-10-v0.5.0-preflight/`](../../../../artifacts/release/2026-11-10-v0.5.0-preflight/README.md)；`checks.json` 記錄各命令退出碼，E2E 的 report 與 results JSON 也在其中。預演副本的 `var/release-preflight/` 另留原日誌與僅供本機審閱的 unsigned app archive。候選副本以 lockfile 重新安裝依賴。第一次直接複製 node_modules 的預演碰到舊 Vite cache 內的絕對路徑，載入兩份 React；乾淨安裝後 107 個前端測試全部通過。這個環境失敗沒有被算成產品通過。

## 補充調查與本次預演限制

### JavaScript 供應鏈

2026-11-10 已更新 30 個既有直接依賴，將 Bun 鎖檔稽核納入 `bun run check`，並新增每日 Dependabot 版本更新。新版稽核掃描 532 套件，結果為零。Linux 完整 gate、desktop debug build 與供應鏈檢查通過。完整證據見 [dependency-refresh.json](dependency-refresh.json)。以下保留初次預演的依賴快照。

補充執行 `bun audit --json`，退出碼 1：19 個套件、88 項以 advisory URL 去重的紀錄（13 low、39 moderate、35 high、1 critical）。這是 installed／lockfile dependency 報告，**不是 88 個已證實可利用的產品漏洞**。預演當時 `check:supply-chain` 只查 Rust，main CI 成功沒有涵蓋這些紀錄。

| 路徑                                                    | 已查明的範圍                                                                                                                                                                                                                                          |
| ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `streamdown@2.5.0 → mermaid@11.15.0 → dompurify@3.4.11` | lockfile 的 production dependency 路徑；目前 Ask 的 Streamdown 沒有配置 Mermaid plugin，已建 web assets 的 `DOMPurify`／`SAFE_FOR_TEMPLATES`／`mermaidAPI`／`securityLevel` 字串掃描沒有命中。這是縮小範圍的線索，不是完整 bundle 可達性證明。        |
| DOMPurify critical `GHSA-cg2g-xh64-v4q7`                | [上游公告](https://github.com/advisories/GHSA-cg2g-xh64-v4q7) 要求 `SAFE_FOR_TEMPLATES`、DOM-returning mode、帶 shadow root 的 DOM 輸入與後續 template evaluation。沒有在產品程式看到這些條件，不能宣稱 app 已能因此被利用；公告列出修正版本 3.4.17。 |
| `react-router@7.14.0`                                   | 產品使用 `createHashRouter` 的 client Data Mode；[turbo-stream RCE 公告](https://github.com/advisories/GHSA-49rj-9fvp-4h2h) 明確限 Framework Mode。這一條不適用，不代表其他 8 項 Router 紀錄也都不適用。                                              |
| Vite／Vitest、Babel、ESLint 相關                        | 屬於開發／建置／測試工具，不是 desktop release 的 dev server；仍需更新與評估開發環境。例：[Vite file-read 公告](https://github.com/advisories/GHSA-v2wj-q39q-566r)。                                                                                  |
| `zod@4.3.6`、`nanoid@3.3.11`                            | 這次 `bun why` 分別追到 ESLint hooks 與 Vite→PostCSS 的 dev 路徑，不能直接列為產品 runtime 漏洞。                                                                                                                                                     |

完整 JSON 保留在本機預演證據中。補查 v0.4.0 的 bun.lock 後，這 19 個套件的版本集合全部與預演當時相同：這是補充掃描找到的既有依賴警報，不是本次 PR 新引入的 19 個回歸。本輪依使用者要求更新可升級的依賴，以上公告已從新版鎖檔稽核結果消失。後續依賴維護規則見 [dependency-maintenance.md](../../../architecture/dependency-maintenance.md)。

### 本機 gate 與主機限制

`bun run verify` 在 `pathkeep-desktop --test owner_process` 的兩個冷啟動場景失敗：`kLSNoExecutableErr`，之後 owner endpoint 等待 30 秒逾時。獨立的最小 `.app`（`/usr/bin/true` 的本機副本，分別用 hardlink 與 copy 放入 bundle）也得到同一 Launch Services 錯誤，且檔案存在、權限為 0755。這支持本次主機／bundle 啟動環境有問題，尚未證實是產品回歸。本次 run 失敗是事實；「專案的 gate 沒跑過／沒過」則不成立，因目前 main 的完整 CI 已成功。

`check:slow` 的 native tests 顯示 2 passed。以同一個已編譯測試 binary 加 `--nocapture` 再看一次，Keychain 明確因 `Operation not permitted` 跳過，launchd 因 `Bootstrap failed: 5` 跳過真正 bootstrap 驗證。只有 plist 檔案驗證與允許的部分執行；不能當作 macOS native 驗收。兩段訊息都保留在證據的 `native-host-visible.txt`。

補跑其餘 Rust workspace 時，crypto cache memory guard 的子程序在執行 `ps` 就被 EPERM 拒絕，沒有得到 RSS，不能說是記憶體超標，也不能算通過。單獨執行一個與 app 無關的 32MB HFS+ image 建立命令，同樣出現 `Device not configured`，所以不把 DMG 失敗直接歸咎於產品或修改 Tauri 的 dependency script。若要再做本機預演，須使用可正常存取這些服務的環境；這不是要求重新完成使用者已驗收的全部平台測試。

### 14.4M 與既有驗證

使用者於 2026-11-10 明確確認 14.4M 升級／備份與跨平台安裝已驗證，本次不再把它們列為未完成，也不要求整套重跑。repo 本來就有 14.4M、4 vCPU／8GB 的量測，見 [ipc-performance.md](../../../architecture/ipc-performance.md) 的 2026-10-25 表與 [14.4M 搜尋比較](../../../architecture/research/2026-11-typo-walk-benchmark.md)。

前次推論來自 BACKLOG 的 `WORK-BACKUP-INTEGRITY-SCALE` 舊狀態，以及 [1M 報告](../../../architecture/research/2026-11-backup-integrity-1m/README.md) 在當時留下的限制。它們不足以推導「整體沒有驗證」。BACKLOG 已依使用者最新確認修正。這次沒有新跑量測，因此也不填造新的耗時、RSS 或平台結果；後續如有實作變更，再依變更範圍判斷需要補的驗證。

### macOS 穩定簽章與公證

`RELEASE.md` 的 primary-channel 政策與 `WORK-RELEASE-SIGNING-A` 要求 Developer ID signing、notarization、stapling，且要證明升級後 FDA 授權仍可用。2026-11-10 的 repository Actions secrets 名稱清單為空；未讀取任何 secret 值。沒有據此判定本機是否另有憑證，但目前 workflow 無法依靠 repo secrets 完成這條路。

需要的 workflow 變數是 `APPLE_CERTIFICATE`、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_SIGNING_IDENTITY`、`APPLE_ID`、`APPLE_PASSWORD`、`APPLE_TEAM_ID`。若這次決定沿用 v0.4.0 的 unsigned preview，必須由使用者明確同意這次的政策例外，筆記須說明 Gatekeeper 與更新後重新授權 FDA 的可能性；不能把它標成已符合 signed primary-channel gate。簽章與公證是兩件事，見 [Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/)。

### 跨平台驗收與最後產物

使用者已確認跨平台安裝驗收完成。本次只在受限主機上重新嘗試 macOS ARM64，不應因為本次沒有新跑 Windows／Linux／Intel，就把既有驗收當成未完成。前次用 `Platform Native` workflow 的最後執行日期推定所有平台驗收都欠缺，也忽略了不經該 workflow 完成的驗收。

發布時仍需按 [既有 runbook](../../m4-full-polish/release-readiness-runbook.md) 核對最終 tag 產物的版本、來源 SHA、架構、worker 與 checksum。這項最終產物核對不代表先前功能驗收被撤銷；Release workflow 只 build／upload，也不會代替已經通過的 CI gate。

## 升級與回退

v0.4.0 的 Archive／Intelligence 都是 schema 1；本次分別升到 23／2。Archive 會建索引並每 50,000 筆回填開啟者；Intelligence 以可續作批次搬移 embedding provenance。語意索引平鋪檔移到分代目錄，舊 binary 也不理解新的位置。Insights 的結構規則更新會觸發背景重建，大型存檔的行為與耗時沿用既有驗證的適用範圍，本次不另作沒有量測根據的估計。

`vault-store::migration::check_version` 會拒絕較新的 schema。新版 portable export 也標記 archive schema 23，v0.4.0 的 importer 只接受 1。**升級後不能直接換回 v0.4.0，也不能把新版匯出檔當作舊版的回退備份。**

**這是降級相容性的限制，不是升級已知會壞或遺失資料，也不是發布阻斷。** 新版會自動遷移舊格式，正常繼續使用新版不需還原副本。若要保留降回舊版的能力，升級前以 app 的 PME 流程處理排程並等備份／匯入／背景工作結束，完全退出 app；在其他位置保存整份 data root（含 config、archive、source evidence、derived、chat、audit 與加密材料檔案），保留原本密碼／鑰匙圈與 v0.4.0 安裝包。只複製開著的 DB 或只留 archive 主檔不足以回退。不要在使用者正式資料上預演。

遇到問題先停止發布與排程並保存失敗後的完整副本。要回退時，使用 v0.4.0 搭配**升級前整份副本**，在另一個 data root 先驗證；新版之後新增的資料另行保留，不覆蓋或刪除。若沒有可驗證的舊副本，提供修復版，不能指示使用者刪 canonical data。這段說明交代降級時如何保留資料，不代表既有升級驗證未完成。

## 提議 workflow 的實際行為

| 入口／選項              | 實際作用                                                                     |
| ----------------------- | ---------------------------------------------------------------------------- |
| push 新的 `v*` tag      | 準備私有 draft，不自動公開                                                   |
| `operation=prepare`     | 明確指定 tag，驗版本與簽章模式，草稿綁定當前 SHA，建置與上傳                 |
| `operation=verify`      | 持有 tag 鎖，核對批准的 source／draft／manifest／所有 assets，不寫入         |
| `operation=publish`     | 同樣核對後公開；沒有 build，需 user approval 與管理員 immutability preflight |
| 已公開 tag 的 prepare   | 跳過建置與上傳，不覆寫產物                                                   |
| `unsigned_preview=true` | 所選平台全部 `--no-sign`，沒有 updater artifacts／`latest.json`              |
| 較少平台                | 只準備所選平台；本次 v0.5.0 必須四平台完整                                   |

tauri-action v0.6.2 本身會找到同 tag 的公開 release 並替換同名 assets，
不能單靠 `draft=true`。新流程的 tag 鎖、草稿 source 所有權與獨立公開
處理正常 workflow 競態；GitHub native immutability 阻止公開後的晚到
寫入。不要從舊 ref 重跑歷史上傳定義。

## 發布流程與批准邊界

下列流程保留操作次序。本輪準備進度見上節；公開發布始終等使用者 approval。

1. 保留既有 CI 與使用者確認的驗收；直接與間接依賴更新重新稽核。
2. 四處版本對齊 0.5.0，簽名候選 commit，凍結 ref；完整 gate 記錄實際來源。
3. 新 tag／draft 必須不存在；重試僅使用相同 SHA 的私有 draft，先停掉所有過期建置。
4. 從該 ref 以 `operation=prepare`、`release_tag=v0.5.0`、`platforms=all`、`unsigned_preview=true` 建置。不得先推 tag。
5. 四平台與 checksum job 全成功，下載核對 digest、manifest、版本、架構與 worker。沿用使用者已確認的原生／14.4M 驗收，按變更補適用 smoke。
6. 三語 notes 寫入私有草稿。保存 source SHA、release ID、manifest digest 與全部 assets 核對結果，執行 `operation=verify` 確認不寫入。
7. 將實際候選、政策 trade-off、unsigned 模式與 immutable 鎖定交給使用者批准；未批准就維持私有草稿。
8. 批准後才合併提案，管理員啟用並核對 immutable releases，再從凍結 ref 以 `operation=publish` 與批准的 SHA／ID／manifest digest 公開，不重建。
9. 核對 `immutable=true`、tag commit、latest、下載與 checksum 一致。晚到 tag workflow 只能略過，不補傳不同 bytes。
10. 公開後更新 STATUS／CHANGELOG，再推進 `-dev`；保留 reports、fixture、hash 與發布 receipts。

GitHub 建議先建立 draft、附齊產物再公開，見 [管理 releases](https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository)。本次仍須使用者明確 approval，沒有從時間經過或既有發布歷史推定同意。

## 產物核對清單

Unsigned all-platform 路徑預期 **11 個** release assets：

| 平台        | 檔名                                                                                                       |
| ----------- | ---------------------------------------------------------------------------------------------------------- |
| macOS ARM64 | `PathKeep_0.5.0_darwin_aarch64.dmg`、`PathKeep_0.5.0_darwin_aarch64.app.tar.gz`                            |
| macOS Intel | `PathKeep_0.5.0_darwin_x64.dmg`、`PathKeep_0.5.0_darwin_x64.app.tar.gz`                                    |
| Windows x64 | `PathKeep_0.5.0_windows_x64-setup.exe`、`PathKeep_0.5.0_windows_x64.msi`                                   |
| Linux x64   | `PathKeep_0.5.0_linux_amd64.AppImage`、`PathKeep_0.5.0_linux_amd64.deb`、`PathKeep_0.5.0_linux_x86_64.rpm` |
| 共用        | `SHA256SUMS.txt`、`RELEASE-MANIFEST.json`                                                                  |

macOS 的 tar.gz 即使 unsigned 也會由 tauri-action 打包 `.app`，不能把它當成已有 updater signature。Signed-updater 路徑另外需要 `latest.json`、各平台 updater bundle 與 `.sig`，manifest 的四個 target 應為 `darwin-aarch64`、`darwin-x86_64`、`windows-x86_64`、`linux-x86_64`，逐一核對下載 URL 與實際 signature。套件自身的發行者簽章與 updater minisign 要分開驗。

本機 debug／release 共用 `src-tauri/binaries/pathkeep-worker-<triple>` 的 staging 檔，不應在同一 checkout 同時建置兩種 profile。本次最終 `.app` 的 worker 已與 release staging 比對 SHA-256 一致。ARM64 兩個 executable 的實際 deployment minimum 是 macOS 11.0；Info.plist 的預設 10.13 不能當成 Apple Silicon 支援 10.13 的承諾。

`bun run release:size-audit` 要同時清楚標示 web、local bundles 與真正下載的 release assets；空的 release 資料夾產生一份 JSON，不能算所有平台的 size audit 通過。

## 已知問題與發布筆記邊界

- 14.4M 搜尋沒有精確結果時的錯字回退，既有量測要 92–139 秒；可取消，但仍不符合流暢搜尋的期望（`WORK-SEARCH-TYPO-WALK`）。
- 4 vCPU／8GB 的 Insights 首次讀取曾有約一分鐘的卡片；需保留 loading 狀態並追查，不可宣稱全規模都即時（`WORK-INSIGHTS-COLD-READS`）。
- 後來才補上的開啟者在 Navigation 即時可見，已建好的 Insights 流向要等完整重建（`WORK-INSIGHTS-LATE-OPENERS`）。
- 「刪除所有資料」不等正在跑的備份，備份可能以錯誤結束；不要在備份中途作清除的 release smoke（`WORK-WIPE-WAITS-FOR-BACKUP`）。
- 瀏覽器開著時 staging 可能每個 profile 空等約 10 秒；新版本仍會顯示讀副本的警告（`WORK-STAGING-BUSY-WAIT`）。
- 既有語意索引沒有單獨清除的 UI；大型索引可能佔大量磁碟（`WORK-SEMANTIC-INDEX-CLEAR-ENTRY`）。
- 舊 v1 存檔已錯配的 URL 不會憑空還原；瀏覽器仍保留的紀錄可在重讀時補齊，已被瀏覽器刪除的不能承諾救回。
- SQLCipher／SQLite `RUSTSEC-2026-0459` 仍明列例外：匯入惡意 database／WAL 的風險不能寫成「使用者挑的檔案所以安全」。其餘例外與理由以 `scripts/check-rust-security.mjs` 為準；新 advisory 仍會讓 gate 失敗。

BACKLOG 的歷史待辦、本次受限環境的失敗與 advisory 的版本匹配，都不能未經適用性判讀就一概升格為本次發布阻斷。已確認的產品風險照實保留，既有 gate 與使用者驗收也照實保留。最後發布仍須使用者明確 approval。

## 本機收尾

已用 `cargo clean` 移除這次候選副本的編譯產物（87,167 個檔案、44.0GiB logical size），並移除該副本的 node_modules、安裝 cache、繼承的舊 bundle 與本次臨時 fixture。保留原始工作目錄既有的 5.8GB target cache；沒有刪除使用者資料。候選副本剩約 95MB，含 35MB 的本機 `.app` 預演封存。兩個 E2E port 都已關閉，測試 app 的 PID 已不存在。收尾時檔案系統仍顯示約 29GiB 可用；logical deletion 量不是實際騰出空間的量，後續大規模量測仍須先重新檢查。見證據的 `cleanup.json`。
