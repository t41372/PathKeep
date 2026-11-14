# 視窗外框：Windows、Linux 上主畫面以外沒有視窗按鈕（2026-11-14）

`WORK-WINDOW-FRAME-EVERY-SCREEN` 的證據。基準是 `main` 的 `e999ca83`（v0.5.0）。

## E2E

`e2e-red/`：修之前的結果。程式碼只多了 `windowFrame()`，讓 dev bridge 的頁面能代表桌面視窗；正式版的行為沒有變。

- `first-run-window.json`：四個測試都失敗。Linux、Windows 的新手引導，以及 Linux 的啟動中，都找不到視窗按鈕。macOS 的新手引導，面板上方與右邊的邊拖不動。
- `window-frame.json`：主畫面有按鈕，也點得到；按鈕旁那條拖不動。指令面板開著時，三顆按鈕都被對話框的遮罩蓋住；點最小化之後，面板的輸入框失去焦點。App Lock 沒有按鈕，背景也拖不動。macOS 那個測試在修之前就通過，它防的是這次的改動弄壞 macOS。
- 這一輪暫時把 `first-run-window.spec.ts` 拿出設定檔：`first-run` 專案一失敗，後面的專案都不跑。
- 截圖：`onboarding-linux.png`、`palette-linux.png`、`lock-linux.png`。

`e2e-green/`：修好之後的完整 `bun run check`。

- `gate.json` 只留新 spec 的結果與全套統計：主設定 131 個過、1 個略過，沒有 flaky。略過的是真視窗那個 spec（`first-run-native-window.spec.ts`），這台 Mac 沒辦法跑它。backup-integrity 兩組（9 個與 1 個）另外也全過。
- 截圖是各畫面在最小視窗（1080 × 760）下的樣子。

重跑這次的 fixture：`PATHKEEP_E2E_NOW=1794645197029 bun run test:e2e`（seed 7）。

## Linux 桌面

`linux-desktop/`：在 boxd 的 Ubuntu 24.04 上跑真的 app，用 `bun run dev:demo` 的合成資料，debug 版加 dev bridge。顯示器是 Xvfb 1920 × 1200，網頁引擎是 WebKitGTK 4.1。

輸入都經 xdotool 送出，按壓節奏照人的速度：點三顆按鈕的中心，從按鈕旁那條拖動，在那條上雙擊，打開指令面板後按最小化，進 App Lock，最後按關閉。每個動作之後記下視窗的狀態。修前跑 `main`，修後跑這個分支。修前與修後的 Rust 後端只差一行註解，所以兩輪用同一個執行檔，前端各自由那棵樹的 Vite 提供。

三種環境：

| 環境                                       | 腳本                    | 修之前                 | 修之後                | 視窗狀態從哪讀                                     |
| ------------------------------------------ | ----------------------- | ---------------------- | --------------------- | -------------------------------------------------- |
| X11，xfwm4                                 | `wincheck.sh`           | `before.txt`           | `after.txt`           | `_NET_WM_STATE`、視窗位置                          |
| GNOME 46，X11                              | `gnomecheck.sh x11`     | `gnome-x11-before.txt` | `gnome-x11-after.txt` | GNOME Shell 的 `MetaWindow`（`Eval`，unsafe mode） |
| GNOME 46，Wayland（app 是 Wayland client） | `gnomecheck.sh wayland` | `gnome-wl-before.txt`  | `gnome-wl-after.txt`  | 同上                                               |

Wayland 那輪，GNOME Shell 以 nested compositor 跑在 Xvfb 裡，xdotool 的輸入由它轉給 app。

三種環境的結果一致：

| 畫面         | 修之前                                                                                                                                                                             | 修之後                                                                                                 |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| 新手引導     | 右上角沒有按鈕，點了沒反應；頂端拖不動，雙擊沒反應；只有步驟清單中段那塊空白拖得動                                                                                                 | 最大化、還原、最小化都有效；頂端拖得動，雙擊最大化；按關閉，app 結束（沒有選單列圖示，也沒有工作在跑） |
| 主畫面       | 按鈕有效；按鈕旁那條拖不動，雙擊沒反應                                                                                                                                             | 按鈕有效；那條拖得動，雙擊最大化，再雙擊還原                                                           |
| 指令面板開著 | 按最小化沒反應（被遮罩蓋住）                                                                                                                                                       | 視窗最小化；叫回來時面板還開著，輸入框還有游標                                                         |
| App Lock     | 沒有按鈕，背景拖不動（GNOME 兩輪）。xfwm4 修前那輪沒有真的進到鎖定畫面：腳本只設了密碼，沒打開 App Lock，Ctrl+L 把視窗帶到了 Settings，`before.txt` 裡 lock 那幾行量的其實是主畫面 | 有按鈕，三顆都有效；背景拖得動，雙擊最大化                                                             |

每一輪都量了：從右邊縫拉，視窗會變寬（Linux 上沒有系統標題列的視窗，仍然可以拉邊縮放）。GNOME Wayland 下，視窗沒有任何標題列，只有我們的三顆按鈕（`gnome-wayland-after-onboarding.png`）；動態關掉標題列在 GNOME 上沒有問題，KDE 沒看過。

### 雙擊與輕點

xdotool 自己的點擊是按下立刻放開。

- 在 xfwm4 上，用這種點擊雙擊拖動區，視窗沒有最大化；每一下按住 50 ms，就最大化了（`xfwm4-double-click.txt`，腳本 `dblclick.sh`）。
- 輕點拖動區之後，不按鍵移動指標，視窗會跟著走，直到再點一下（`xfwm4-tap.txt`，腳本 `fastclick.sh`）。修之前就是拖動區的導航列中段也一樣，所以這是 Tauri 拖動方式的老問題；按住 60 ms 的點擊不會。觸控板輕點就是這種點擊。GNOME 的 X11 與 Wayland 都不會（`gnome-*-after.txt` 的 tap 那幾行）。見 BACKLOG `WORK-DRAG-ON-MOVE`。
- 推論，沒有證實：Tauri 的 `drag.js` 在第一下 mousedown 就請視窗管理員開始移動，請求到的時候鍵已經放開，xfwm4 就進入「再點一下才結束」的移動。頁面裡的事件沒有記到（`dblclick.sh` 想把 mousedown 記進 Vite 的輸出，沒有成功）。

### 照 CI 的方式跑

在同一台機器上，用 CI 的方式（`CI=1`、`dbus-run-session -- xvfb-run -a`）跑 E2E 到 restart 專案，125 個過，1 個是 Linux 本來就略過的排程測試。`first-run-native-window.spec.ts` 也在其中，它在 Linux 上操作真視窗，用 mutter 當視窗管理員，跑了且通過。之後把所有真視窗檢查（含關閉後重啟）集中到 first-run 專案，又跑了一次 first-run 專案：6 個全過，原生 spec 走完按鈕、拖動、雙擊與關閉，app 重啟後 first-run.spec 照常通過（`e2e-green/linux-ci-runs.txt`）。同一份 spec 對修前的前端跑，原生 spec 等了 90 秒最大化都沒有發生，失敗（`e2e-red/linux-ci-first-run-old-frontend.txt`）。

穩定度：另開一台照 CI 裝好的機器（`linux-desktop/stress-setup.sh`），4 顆 CPU 全被 `stress-ng` 佔滿，把原生 spec 連跑 20 次（`linux-desktop/stress-run.sh`）：20 次全過，每次 7.0–7.9 秒（`e2e-green/linux-native-stress.txt`）。

### 沒看過的

- KDE。
- Windows：沒有機器。
- macOS：這台 Mac 的 Claude 沒有螢幕錄製與輔助使用的權限，真視窗沒看；只有瀏覽器代身的 spec。
- 視窗顯示之後、頁面載入之前的那一下：視窗是透明的，Xvfb 沒有合成器，截圖只拍到一片黑。
