# v0.5.0 發布審查

首次調查：2026-11-10；本輪更新：2026-11-13（America/Phoenix）。這次授權包括依賴 PR 的審查、驗證、合併與發布準備；公開發布仍須使用者明確批准。

**既有合併 gate 已通過，發布仍等待使用者 approval。** 2026-11-10 使用者指出既有 14.4M 升級／備份與跨平台安裝已驗證，本次審查據此修正：本機受限環境的失敗不否定既有 CI／驗收；預演時的 JavaScript 稽核曾命中 19 個套件，更新後的鎖檔稽核已清零；資料格式升級的降級限制是發布告知與回退考量，不是升級故障。前一版將這些混列成發布阻斷的總結已撤回。 [本輪核對紀錄](assessment-correction.json) 保留 CI 連結、19 套件版本比較與使用者驗收確認；原始失敗日誌和 hash 不修改。簽章路徑與最終產物的例行核對仍按發布計畫確認。

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

## 本次 workflow 保護

v0.4.0 的 release API 顯示 `published_at=2026-10-19T07:36:16Z`；隨後
[push run 39217061687](https://github.com/t41372/PathKeep/actions/runs/39217061687)
於 `07:36:18Z` 建立，actor 為使用者。公開 draft 會建立 tag，不能假設不會
再次啟動 tag workflow。

本候選增加已公開版本的保護：先以 GitHub API 查詢 tag；已公開就跳過
後續 matrix／版本／簽章檢查、所有 build 與 manifest upload。查詢、認證或
metadata 失敗必須讓準備失敗。新 tag 與私有 draft 仍走原本流程，不改動
新 tag 的既有發布入口。

九個 CLI／HTTP 場景在實作前列出；公開／新建／draft、服務與認證錯誤、
錯誤 JSON／tag／狀態及缺 token 都有斷言。舊實作八個失敗，新增 guard
後九個通過；本機前端檢查共 119 個測試通過。正式草稿建置前，先在
Actions 對已公開的 v0.4.0 做保護預演，核對 build／upload 全部略過且
舊 assets 不變，將結果附於發布準備 PR。候選版本是 0.5.0，既有版本
檢查亦會攔下錯誤續跑。遠端紀錄保留於 PR／草稿，以維持候選 commit 不變。

GitHub 的 job `if` 在 matrix 展開前執行，參見
[官方 workflow 語法](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idif)。
沒有已公開版本時，完整版本同步與 updater key 檢查照常執行。

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

## 發布 workflow 的實際行為

| 入口／選項               | 實際作用                                                                                                                                        |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| push 新的 `v*` tag       | 預設全部平台、`draft=false`、`unsigned_preview=false`；要求 updater key；若有 key，第一個 matrix 成功時即可公開，早於其他 build 與 checksum job |
| 手動 `draft=true`        | 未存在同名 release 時建立 draft；不代表「沒有遠端副作用」                                                                                       |
| `unsigned_preview=true`  | 所選平台全都帶 `--no-sign`，停用 updater artifacts；不是只對 Windows 生效                                                                       |
| `unsigned_preview=false` | 要求 `TAURI_SIGNING_PRIVATE_KEY`，但這只提供 updater signature，不代表 Apple Developer ID 已配置                                                |
| 手動 `release_tag`       | 改 release 名稱／版本檢查，不會自動 checkout 那個 tag；實際 build 的是 dispatch ref 的 `github.sha`                                             |
| 選擇較少平台             | 只能繼續同 tag 的私有 draft；已公開 tag 整輪略過。Checksum 掃全部現有 assets，不能單獨證明相同來源 SHA                                          |

已核對固定版本 [tauri-action v0.6.2 的 release 查找](https://github.com/tauri-apps/tauri-action/blob/v0.6.2/src/create-release.ts) 與 [asset upload](https://github.com/tauri-apps/tauri-action/blob/v0.6.2/src/upload-release-assets.ts)：draft 查找只比 tag，未篩 `draft`；找到同名公開 release 也會沿用，既有同名 asset 會先刪再傳。**該 action 自身會覆寫公開產物，`draft=true` 不是保護。** 本次新增的 workflow guard 在 action 執行前攔下已公開 tag；不得繞過它。

v0.4.0 的 [手動 unsigned run](https://github.com/t41372/PathKeep/actions/runs/39211157788) 四個 matrix 與 checksum job 全成功。公開 tag 後的 [push run](https://github.com/t41372/PathKeep/actions/runs/39217061687) 在 updater key 預檢失敗。公開資產有 9 個平台檔案與 2 個 manifest，沒有 `latest.json`／`.sig`。因此現有下載路徑可用，不代表自動更新可用。

Tauri updater 強制驗簽；沿用 updater 必須找回和現有 `pubkey` 配對的私鑰，不能直接產生新 key 期待舊 app 信任它。見 [Tauri updater signing](https://v2.tauri.app/plugin/updater/#signing-updates)。若本次仍是 unsigned preview，發布筆記只能承諾手動下載安裝。

## 發布流程與批准邊界

下列流程保留操作次序。本輪準備進度見上節；公開發布始終等使用者 approval。

1. 保留已通過的 CI 與使用者確認的驗收，完成補充 audit 的適用性判讀，確認簽章／unsigned preview 路徑與本次發布內容。
2. 把四處版本 patch 與批准納入的文檔整理成 release commit；Conventional Commit subject 可用 `chore(release): prepare v0.5.0`，body 說明為何發布與關鍵變化。`release:bump` 目前只改三個宣告，須另更新 Cargo.lock 的 desktop package 版本並用 Prettier 格式化 JSON；本次 patch 沒有依賴更新。
3. 在最終 commit 跑 `bun run verify`、`bun run check:slow` 與平台驗收，鎖定完整 SHA。HEAD 變動即重新評估證據；不能直接用本次 `0.5.0-dev` main 的綠燈替代新 release commit。
4. 查遠端 tag、包含 draft 的 release 清單、執行中的 Release jobs；`v0.5.0` 必須尚不存在。已存在時停下查明，不沿用、不刪除或覆寫。確認遠端 release branch 正是批准的 SHA。
5. **不先推 `v0.5.0` tag。** 從批准的 release ref 手動 dispatch：`release_tag=v0.5.0`、`draft=true`、`prerelease=false`、`platforms=all`，unsigned 值依批准的路徑。這一步會建立遠端 draft 並上傳 assets，屬本次準備授權；公開仍另等 approval。
6. 四個 matrix、checksum job 全完成後，按下面清單下載所有檔案，逐檔比對 SHA-256、manifest size 與 GitHub digest；檢查沒有多餘舊檔、來源 SHA 與版本一致，沿用使用者已確認的安裝／升級驗收，依本次變更補適用的 smoke。任何一項失敗，release 保持 draft，不開始公開。
7. 用本目錄三語 notes 取代自動產生的模板文字，填入**這次實際**驗證資訊與必要的 unsigned 安裝說明；不得沿用 v0.4.0 的 checksum。
8. 對照使用者批准的內容與最終 SHA／assets，才將 draft 公開並設 latest。使用人員憑證公開 draft 會建立 tag 並再觸發 push workflow；本次 guard 應將它判為已公開版本，跳過建置與上傳，保留已審閱的 binaries。公開前先用舊版 tag 預演這條路；不能在發布後繞過 guard、重跑公開 tag 或補傳不同來源 binary。`GITHUB_TOKEN` 建立的 push 通常不再觸發另一個 workflow，見 [GitHub 觸發規則](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow)。
9. 公開後核對 `/releases/latest` 指到 v0.5.0、所有平台 URL 與 checksums 可下載；signed updater 路徑須從舊安裝驗證查詢／下載／驗簽／重啟，unsigned 路徑只驗手動安裝。若部分平台不可用，不能宣稱全平台已發布。
10. Append CHANGELOG，回寫 STATUS 與 source docs，然後才把 main 版本改成下一個 `-dev`；不要在 release build 前推進版本。移除本次可重建的編譯產物，保留報告、hash、日誌與 E2E artifact。

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
