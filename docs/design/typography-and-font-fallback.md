# Typography And Font Fallback Strategy

> 2026-04-10 first closeout (v0.2 brutalist phase).
> 2026-05-20 v0.3 paper redesign：Newsreader / 系統 sans / JetBrains Mono 三套字體（已隨舊前端退役）。
> **2026-10-02 更新（M18 redesign）**：只剩兩套字體 Geist / Geist Mono，經 `@fontsource-variable` 打包；沒有 serif，也沒有「只用系統字體」開關。CJK 仍交給系統字體。

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

PathKeep 採 **B + C**：Latin 用打包的 Geist variable font，其他 script 交給系統字體；monospace 只給 code-like 內容。

### 2026-10 現行做法

- 兩套字體，定義在 `src/index.css` 的 `@theme inline`：
  - `--font-sans`：`'Geist Variable', 'PingFang TC', 'PingFang SC', 'Noto Sans TC', 'Noto Sans SC', 'Microsoft JhengHei', system-ui, sans-serif`。所有 UI 文字、標題、按鈕。
  - `--font-mono`：`'Geist Mono Variable', ui-monospace, 'SF Mono', Menlo, Consolas, monospace`。只用在路徑、指令、版本號、排程檔內容、MCP 指令這類 code-like 內容。
- 字體檔由 `@fontsource-variable/geist` 與 `@fontsource-variable/geist-mono` 打包（`src/index.css` 頂部 `@import`），只含 Latin。沒有 runtime 網路字體。
- Geist 沒有 CJK 字形，中文直接落到上面列的系統字體。
- `html[lang]` 在 `src/main.tsx` 首屏設定，語言切換時由 `src/lib/i18n/provider.tsx` 更新。
- 數字對齊用 `.tabular`（`font-variant-numeric: tabular-nums`），不是 mono。
- 舊的 `--font-serif`、`data-fonts="system"` 與 legacy alias 都已隨舊前端刪除。

### 已知缺口

- v0.2 的 `:root:lang(zh-CN)` / `:root:lang(zh-TW)` 覆寫沒有搬過來。現在兩種中文共用同一條 stack，先 `PingFang TC` 後 `PingFang SC`，所以 macOS 上的 zh-CN 介面會用 PingFang TC 的字形顯示簡體字；Windows 只列了 `Microsoft JhengHei`，沒有 `Microsoft YaHei`。修法是在 `src/index.css` 依 `:lang()` 調整兩種中文的字體順序。

---

## 風險與緩解

- Linux 某些發行版若缺少 Noto：由 generic `system-ui` / `sans-serif` 續接；若未來某個 locale 出現真實 QA 問題，再加 script-aware override，而不是預先手寫 180 份 map
- 打包字體只允許本地 Latin variable font，新增前要先看 bundle size 與 license；不可回到 runtime network fonts

---

## 實作要求

- `src/index.css`（`--font-sans` / `--font-mono`）是字體 stack 的 source of truth
- `src/main.tsx` 與 `src/lib/i18n/provider.tsx` 必須在首屏與 runtime 保持 `document.documentElement.lang` 正確
- 新 UI copy / labels / badges 不得預設使用 monospace；只有 code-like content 才能用 `font-mono`

---

## 參考

- MDN variable fonts guide：說明 variable font 可把多個 variations 收進單一檔案，通常比多個靜態字體檔更省  
  <https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Fonts/Variable_fonts>
- W3C i18n language declaration note：`lang` 會影響 text processing 與 automatic font assignment  
  <https://www.w3.org/TR/2007/NOTE-i18n-html-tech-lang-20070412/>
- Fontsource variable font docs：自帶本地字體採 self-hosted package，而不是 runtime CDN import  
  <https://fontsource.org/docs/getting-started/variable>
