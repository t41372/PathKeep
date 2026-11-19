# 跨平台測試與修復：給 Devin 的工作說明

這份說明給在雲端 Windows、Ubuntu、macOS 虛擬機上工作的 agent。PathKeep 平常只在一台 Mac 與 Linux 上開發，Windows 的完整端對端測試從沒跑過，macOS 與 Windows 的真視窗也沒人每次去點。這裡列出要在三個平台上跑的測試、要查的問題，以及怎麼交回成果。

主要開發者（下面稱「主 agent」）交接時會告訴你從哪個 commit 開始。在那之前不要動手。

## 開工前先讀

- `AGENTS.md`：專案規則。特別是「測試是契約」「構建可重現」「供應鏈信任」「不對外」「Commit 說 Why」。
- `docs/plan/program/quality-matrix.md`：`bun run check` 的內容，以及什麼時候才寫 unit test。
- `docs/design/back-forward-window-frame-tradeoff.md`：視窗外框、按鈕、拖動區在三個平台上的定案與原因。
- `docs/design/screens-and-nav.md`、`docs/design/ui-review-guardrails.md`：改到畫面時要守的規則。
- `docs/plan/BACKLOG.md` 裡下面提到的每一項，原文寫了已知的根因線索與量過的數字。

## 分支、提交與交回

- 從主 agent 指定的 commit 開分支，一個工作項目一條：`devin/<項目名稱小寫>`，例如 `devin/work-native-window-check`。
- 每條分支開一個 draft PR，目標是 `feat/agent-history-product`。不要 merge，不要推到 `feat/agent-history-product` 或 `main`。主 agent 會讀過 diff、再請獨立審查，之後才合併。
- Commit 用 Conventional Commits，subject 72 字以內，body 寫為什麼改、改了什麼。不要加 `Co-Authored-By` 或任何 AI 署名行。
- PR 說明用繁體中文，只寫目前的狀態（做了什麼、怎麼驗證、證據在哪、還剩什麼）。修過幾輪的經過放在收合的 `<details>` 裡。

## 三個平台的環境

CI 的設定是最準的參考：`.github/workflows/ci.yml`（Linux 完整 gate）與 `.github/workflows/platform-native.yml`（三個平台的原生測試）。

- **共同**：Rust 1.96.0、`protoc`、Node 22、Bun；`bun install --frozen-lockfile`；`bunx playwright install chromium`。不要依賴機器上全域裝好的其他東西；缺什麼照 CI 的清單裝。
- **Ubuntu**：`ci.yml` 與 `platform-native.yml` 列出的 apt 套件（WebKitGTK 4.1、GTK 3、appindicator、`gnome-keyring`、`dbus-x11`、`xvfb`、`mutter`、`wmctrl`、`xdotool` 等）。CI 這樣跑完整 gate：`PATHKEEP_E2E_NATIVE_WINDOW=required dbus-run-session -- xvfb-run -a bun run check`。真視窗的檢查另外要在有桌面的 session 裡跑：GNOME（X11 與 Wayland）、Xfce（xfwm4），以及這次新增的 KDE Plasma（Wayland）。
- **macOS**：Xcode Command Line Tools。CI 跑 `bun run check:platform` 與 `bun run test:e2e`。真視窗的檢查要給終端機輔助使用（Accessibility）權限。
- **Windows**：Visual Studio Build Tools（MSVC、Windows SDK）、WebView2 Runtime（Windows 11 內建）。CI 目前只跑 `bun run check:platform`，從沒跑過 E2E。

測試資料一律用合成的：`bun run dev:demo`（真的 app 加合成瀏覽器資料）、`scripts/fixtures/synthetic-agents.mjs`（四種 Agent 工具的合成資料夾）。不要放任何真人的瀏覽紀錄或對話進虛擬機。

## 工作項目

依序做。每一項開始前先在對應平台重現，重現不了就照實寫下怎麼試過的。

### 1. 三個平台各跑一次完整 gate

- Ubuntu 與 macOS 跑 `bun run check`；Windows 先跑 `bun run check:platform`，再跑 `bun run test:e2e`。
- Windows 的 E2E 從沒跑過，測試框架本身（啟動 debug app、dev bridge、路徑與 shell 的寫法、sidecar `src-tauri/binaries/pathkeep-worker-<target>` 的名稱）大概會先壞。修框架讓它在 Windows 跑起來，不要靠略過或放寬 spec。某條 spec 在某平台真的不適用時，在 spec 裡寫明原因再限定平台。
- 交回：每個平台一份結果（通過、失敗、略過各幾條，失敗的 spec 與錯誤），與 `artifacts/e2e/` 的報告。之後每個失敗各自當一個工作項目。

### 2. macOS 上兩個沒查明的 E2E 失敗

- `tests/e2e/schema-update-quit.spec.ts:94`：按「Keep PathKeep open」之後對話框沒有消失。原因未知。
- `tests/e2e/quit-during-work.spec.ts:53`：「Export progress」進度條沒出現。trace 顯示匯出本身跑了 11 秒並成功，但頁面在這 11 秒內沒有發出任何 `get_export_progress` 或 `load_dashboard_snapshot` 請求，畫面一直在更新（Stop 按鈕在、磁碟用量變成 skeleton）。Linux 上這條會過。
- 兩條都要先證明根因，不要先改 spec 的等待時間。

### 3. 真視窗檢查（`WORK-NATIVE-WINDOW-CHECK`）

- Linux 已有 `tests/e2e/first-run-native-window.spec.ts`：用 xdotool 對真的 WebKitGTK 視窗送輸入，讀視窗狀態。macOS 與 Windows 還沒有。
- macOS：紅綠燈是系統畫的，要驗的是拖動區、雙擊放大、紅綠燈讓位，以及新手引導、主畫面、App Lock 三個畫面。
- Windows：三顆按鈕是我們畫的。要驗最小化、放大、還原、關閉、拖動、雙擊放大，以及指令面板開著時按鈕仍能點。WebView2 支援 CDP；輸入與視窗狀態可用 UI Automation。
- 選工具前先看 `AGENTS.md` 的供應鏈規則；需要新依賴就先停下來，寫風險評估交給主 agent。
- 做完要能放進 CI（`platform-native.yml`），像 Linux 那份一樣缺工具時失敗，而不是悄悄略過。

### 4. 視窗的其他待辦

- `WORK-WAYLAND-UNDECORATED`：在 KDE Plasma 的 Wayland session 開 PathKeep，看會不會同時出現 KWin 的標題列和我們的三顆按鈕。BACKLOG 寫了讀 tao 原始碼看到的原因與建議方向。
- `WORK-WINDOW-INACTIVE-LOOK`：Windows 上視窗失焦時，我們畫的按鈕要像系統按鈕一樣變淡。
- `WORK-STATUS-SCREEN-OVERFLOW`：最小視窗下，置中的全螢幕畫面（啟動、復原、鎖定等）內容太長時會不會被切掉；會就改成可捲動，放得下時仍然置中。
- `WORK-E2E-WINDOW-FRAME-DEFAULT` 與 `WORK-E2E-WINDOW-OFF-THE-DESKTOP`：E2E 預設的頁面不代表任何平台的視窗；E2E 啟動的 app 在 macOS、Windows 上會搶前景。照 BACKLOG 原文的「要做」處理。

### 5. Windows 的資料正確性

- `WORK-WINDOWS-RAW-COPY-CHECK`：Chrome 開著時直接複製它的歷史檔，Windows 上可能收下一份複製途中被寫過的檔案。BACKLOG 寫了要先怎麼量（用能判斷交易是否完整的資料，不是只會 append 的假資料）。先量，量完把數字和建議交回，不要沒量就改。
- 內建語意模型的換檔：模型檔正被打開或映射時，新版本要能裝在旁邊、切換過去，舊的之後清掉。在 Windows 上實際走一次：語意搜尋開著、模型使用中，觸發換新版本、取消、再開始，確認不會兩個下載同時跑、取消不會打到另一個下載、舊檔最後會清掉。
- 鑰匙圈：Windows Credential Manager、Linux Secret Service（`gnome-keyring`）、macOS Keychain 三條路徑，各走一次「記住金鑰、重開、解鎖、忘記金鑰」。
- 對話檔被拒絕存取和被刪除的分辨：Windows 對「檔案正在被刪除」和「權限不允許」回同一個錯誤碼 5。PathKeep 拿自己開著的 handle 問檔案是不是 delete-pending，只有是的時候才當成檔案不見了，其餘都是暫時的 `access-denied`，「再試一次」就能恢復。這段只在 Mac 上交叉編譯過，沒有在 Windows 上跑過。在 Windows 上跑 `cargo test -p vault-store --test windows_access`，並且實際走一次：PathKeep 開著時拿掉對話檔的讀取權限，把視窗叫到前面，恢復權限後按「再試一次」，對話要回來，不能要求修復；再走一次真的刪掉對話檔，PathKeep 要把它開著的那份存成安全副本。
- 模型下載的讀取逾時：連線 15 秒、每次讀取 30 秒沒有資料就算失敗，持續有資料的下載不會被切斷。在 Windows 上用慢速或中途斷線的連線各走一次，確認失敗後可以重試、取消在 30 秒內生效。

### 6. Agent 對話的工具資料夾

- 在每個平台上，用合成資料放在各工具的預設位置（Claude Code、Codex、Pi、DeepSeek Harness、Claude 桌面版），確認 PathKeep 不設 `CHB_AGENT_TOOLS_HOME` 時也找得到、預覽列出的檔案和實際保存的一樣。
- Windows 特別看：漫遊與本機 AppData 的差別、大小寫、長路徑、檔案被工具開著時的讀取。
- 虛擬機上可以把合成資料放在預設位置，因為虛擬機是拋棄式的；任何一台開發者自己的電腦上都不行。

## 每一項怎麼做

- **先證明根因再修。** 把推論的每一環都接上，在對應平台重現，問「為什麼現在才出現」。迴歸測試先寫，在舊程式上看到它失敗。
- **E2E 是主要證明。** 盡量用真的 app、真的視窗、真的輸入。不要寫一個理想化的 mock 讓測試全過。
- **每次執行都留下能重複的證據**：截圖、錄影、log、實際下的指令，放在 `artifacts/investigations/<日期>-<主題>/<平台>/`，附一支能重跑的腳本。
- **改了功能就更新文件**：`docs/features/`、`docs/design/`、`docs/architecture/` 對應的地方。BACKLOG 該項的內容改成你查到的事實；勾選與 CHANGELOG 由主 agent 做。
- **使用者看得到的文字**一律同時交付 `en`、`zh-CN`、`zh-TW`（含 aria-label 與各種狀態），`bun run check:i18n` 要過。

遇到下面任何一種，停下來寫清楚交給主 agent，不要自己決定：

- 需要新的第三方依賴（`AGENTS.md` 的門檻以下）。
- 要改存檔的格式、資料庫 schema 或設定檔的意義。
- 跟 `docs/` 裡已定案的設計衝突。
- 使用者看得到的行為要選一種（例如某個按鈕在某平台該做什麼）。
- 唯一的修法是改依賴本身。這時把根因與最小重現記在本 repo，不要去上游開 issue、PR 或留言，也不要起草要送上游的東西。

## 回報

每個工作項目交回一段，放在 PR 說明，也貼一份給主 agent：

1. 做了什麼，在哪個平台、哪個桌面環境。
2. 重現步驟與結果；根因（連到程式碼行數）。
3. 改了什麼，為什麼選這個改法。
4. 證明：先失敗的測試、修好後的結果、證據資料夾。
5. 沒做完的、沒驗證到的、卡住的地方，再小都寫。
