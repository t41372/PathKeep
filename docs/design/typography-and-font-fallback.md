# Typography And Font Fallback Strategy

> 2026-05-18 closeout (overrides 2026-04-10)。`feat/v0.3-redesign` 階段，使用者明確要求視覺對齊 brutalist 設計稿 (`reference/PathKeep — Desktop UI Design/`)，授權推翻 2026-04-10 sans-primary 政策。新政策見下一節；2026-04-10 區段保留為歷史紀錄。

---

## 2026-05-18 — Brutalist Redesign Override (user-authorized)

### 推翻授權

`feat/v0.3-redesign` 進行中，使用者編譯 app 後回報「跟之前一樣，跟新設計完全不一樣」。經過 agent 調查確認 token color / spacing / radius 已對齊設計稿，但 `--font-ui` / `--font-body` / `--font-mono` 都指向 system sans，導致 shell chrome 讀起來和舊版本一致，brutalist 視覺 (mono 主導) 沒有出現。

使用者授權原文：「視覺設計，美術風格，和各種與 UI 和功能相關的設計，全部以新的設計 redesign 為主。舊的全部推翻。」

### 新政策

- **`--font-code` 升為 primary UI font stack**。stack 開頭為 `JetBrains Mono`, `Cascadia Code`, `Fira Code`, `ui-monospace`, `SFMono-Regular`, `SF Mono`, `Cascadia Mono`, `Consolas`, `Liberation Mono`, `Menlo`，接著是 CJK 系統字體 (`PingFang SC/TC`, `Hiragino Sans GB`, `Microsoft YaHei UI`, `Microsoft JhengHei UI`, `Noto Sans CJK SC/TC`, `Source Han Sans SC/TC`)，最後 fallback `monospace`。
- **`--font-ui`、`--font-body`、`--font-mono` 全部 alias 到 `var(--font-code)`**。整個 UI 預設 mono；任何顯式 `font-family: var(--font-code)` 仍然有效，但不再有差異化效果 (這是刻意的：讓未來如果要再分流時有 selector hook)。
- **CJK 字元改走 per-glyph fallback**。CSS 字體匹配是逐字元的，所以 `RUN #1847 · 已完成` 這種混合字串：Latin / 數字 / 標點分到 `JetBrains Mono`，`已完成` 分到 PingFang / YaHei (依 locale)。同一 baseline 區域，沒有 string-level re-layout。
- **`:root:lang(zh-CN)` / `:root:lang(zh-TW)` 仍保留**，但只是把該 locale 對應的 CJK family 排在另一個地區 CJK family 之前 (例如 `lang(zh-TW)` 把 PingFang TC / JhengHei 放 PingFang SC / YaHei 前面)。三個 token (`--font-ui` / `--font-body` / `--font-mono`) 一起指向同一個 stack。

### 保留的約束 (沒變)

- 禁止 runtime 遠端字體載入 (Google Fonts / CDN 之類)
- 禁止把整套 Noto / Source Han 超大字體 bundle 進 desktop binary
- `html[lang]` 必須在首屏與 runtime locale 切換時同步更新

### 接受的 trade-off

- CJK 字元在主 UI 上由 PingFang / YaHei / JhengHei / Noto Sans CJK 渲染，而非原本的 `Segoe UI Variable Text` / `PingFang` 混排。視覺上 CJK 行距略寬、字形偏方正。使用者已明確接受這個 trade-off，視為 brutalist 風格的一部分。
- 小字級 (10-11px) 的 CJK 在某些低 DPI 顯示器上可能略糊；如果未來 QA 真的回報問題，rollback path 見下方。
- `--font-mono` legacy alias 留著只是為了不大規模 refactor 既有 CSS 規則；新 UI 不應主動引用 `--font-mono`，而要顯式寫 `--font-code` / `--font-ui` / `--font-body`。

### Rollback path

如果未來必須回到 sans-primary：

1. 把 `src/styles/tokens.css` `--font-ui` / `--font-body` / `--font-mono` 改回原本的 system sans stack (參考下方 2026-04-10 區段的 decision 文字，原 stack 在當時 commit history 內)。
2. `:root:lang(zh-CN)` / `:root:lang(zh-TW)` 同形回 sans-with-CJK 排序。
3. 不要動 `--font-code` — 它本身就是 mono evidence-only 用途，無需改動。
4. 同時恢復 `src/styles/app/sidebar.css` `.nav-item`、`src/styles/app/topbar.css` `.page-title` 的 `font-family` 從 `--font-code` 回到 `--font-ui` (因為 commit 2 把它們顯式 pin 到 mono)。

---

## 問題定義

PathKeep 是本地優先、跨 macOS / Windows / Linux 的桌面 app。它目前 day-one 就必須可靠支援 `en`、`zh-CN`、`zh-TW`，而長期目標是擴展到更多語言。舊 shell token 直接把大部分 UI chrome 和 dense labels 指向 monospace，並透過 runtime Google Fonts import 載入 `Inter` / `JetBrains Mono`。這造成三個 shipping 問題：

- 小字級 monospace 在 sidebar、topbar、filters、callout microcopy 上可讀性顯著偏差
- desktop app 離線啟動時不應依賴遠端字體服務
- 若 `html[lang]` 不跟著 locale 切換，CJK glyph fallback 與異體字選擇會失真

---

## 約束

- 不接受為了覆蓋 180+ 語言而把整套 Noto / Source Han 類超大字體直接打進 desktop bundle
- 不接受依賴 Google Fonts 之類的 runtime network font fetch
- 不接受為每個 locale 手寫一長串完全獨立的字體 map，再把它變成另一套 maintenance surface
- 必須優先保證閱讀性，而不是保留 prototype 的 terminal flavor

---

## 方案比較

### A. 打包完整全球字體超集

優點：

- 視覺最可控
- 某些低配 Linux 主機上也能自行兜底

缺點：

- bundle size、license tracking、升級與 QA 成本過高
- 大多數語言其實會重複覆蓋作業系統已經提供的優質 UI fonts

### B. 打包單一 Latin 品牌字體，再把其他 script 交給系統

優點：

- 英文與數字可獲得更一致的品牌感
- 仍可把 CJK / 其他 script 交給 OS

缺點：

- 仍需處理 font asset、license、packaging 與 fallback QA
- 對 PathKeep 目前的主要問題來說，收益不如先把 monospace 濫用與 `lang` 缺失修掉

### C. Curated system UI stack + locale-aware overrides + monospace only for evidence

優點：

- 離線安全、零遠端依賴、bundle 幾乎不增加
- 可直接使用 macOS / Windows / Linux 各自最成熟的 UI sans
- 只需對目前 shipping 的 `zh-CN` / `zh-TW` 補精準 fallback，其餘語言維持 generic sans fallback

缺點：

- Linux 發行版之間仍可能存在少量字型差異
- 若日後要追求更強品牌一致性，仍可能需要再引入自帶 Latin font

---

## 決策

PathKeep 預設 shipping 採 **方案 C**。

- 主 UI 字體使用 `--font-ui` / `--font-body`，以 curated system sans stack 為主
- `zh-CN` / `zh-TW` 透過 `:root:lang(...)` 提升 PingFang / Microsoft YaHei / Microsoft JhengHei / Noto CJK 類字體的優先級
- `--font-code` 只用於 path、ID、command、純 evidence value 等真正需要 monospace 對齊的內容
- `--font-mono` 只保留為 legacy shell CSS alias，不再代表新的產品設計意圖
- `html[lang]` 必須在首屏與 runtime locale 切換時同步更新

---

## 風險與緩解

- Linux 某些發行版若缺少 Noto：由 `Ubuntu` / `Cantarell` / generic `sans-serif` 續接；若未來某個 locale 出現真實 QA 問題，再加 script-aware override，而不是預先手寫 180 份 map
- 未來若需要更一致的 Latin 品牌感：只允許 **本地打包** 的小型 Latin font asset，且必須先做 bundle-size / license review；不可回到 runtime network fonts
- 既有 shell CSS 還留有大量 `--font-mono` 引用：本次先以 token alias 讓 UI 立即可讀，後續如有大規模 shell refactor，再逐步改名到 `--font-ui`

---

## 實作要求

- `src/styles/tokens.css` 是字體 stack 的 source of truth
- `src/main.tsx` 與 `src/lib/i18n/provider.tsx` 必須在首屏與 runtime 保持 `document.documentElement.lang` 正確
- 新 UI copy / labels / badges 不得再預設使用 monospace；只有 code-like content 才能用 `.mono`

---

## 參考

- MDN variable fonts guide：說明 variable font 可把多個 variations 收進單一檔案，通常比多個靜態字體檔更省；若未來需要自帶 Latin font，可優先走這個方向  
  <https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Fonts/Variable_fonts>
- W3C i18n language declaration note：`lang` 會影響 text processing 與 automatic font assignment  
  <https://www.w3.org/TR/2007/NOTE-i18n-html-tech-lang-20070412/>
- Fontsource variable font docs：若未來需要自帶本地字體，採 self-hosted package / asset，而不是 runtime CDN import  
  <https://fontsource.org/docs/getting-started/variable>
