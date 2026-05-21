# Typography And Font Fallback Strategy

> 2026-04-10 first closeout (v0.2 brutalist phase).
> **Updated 2026-05-20** for the v0.3 paper redesign: three-font system
> (`--font-serif` Newsreader + `--font-sans` system + `--font-mono`
> JetBrains Mono), bundled Latin subsets, Settings → Appearance "system
> fonts only" override. The CJK fallback contract from the v0.2 phase
> (system stack for non-Latin scripts) carries forward unchanged.

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

PathKeep 採 **方案 C + bundled Latin (paper redesign 升級)**。

### v0.2 contract (carried over)

- `zh-CN` / `zh-TW` 透過 `:root:lang(...)` 提升 PingFang / Microsoft YaHei / Microsoft JhengHei / Noto CJK 類字體的優先級。CJK 永遠走系統字體。
- `html[lang]` 必須在首屏與 runtime locale 切換時同步更新。

### v0.3 paper redesign (additions)

- 三套字體 tokens（取代舊的 `--font-ui` / `--font-body` / `--font-code`）：
  - `--font-serif` `'Newsreader', Georgia, serif` — 編輯體 headings、敘述體 body、Dashboard / On This Day / Settings hero copy。
  - `--font-sans` 系統 sans stack — UI chrome、按鈕、列表標題、密集 labels、status bar。
  - `--font-mono` `'JetBrains Mono', ui-monospace, SFMono-Regular, monospace` — path / ID / command / mono badge / heatmap labels。
- Bundled Latin subsets via `@fontsource/newsreader` and `@fontsource/jetbrains-mono`. **No runtime Google Fonts import**, no CDN dependency.
- Settings → Appearance "Fonts" 提供 `bundled`（預設）與 `system` 兩段切換。`data-fonts="system"` 在 `<html>` 上會 swap to system fallback stack on the fly。
- 舊 `--font-ui` / `--font-body` / `--font-code` 仍以 legacy alias 形式存在 (`tokens.css` 底部)，直到所有 v0.2 routes 改寫完成後一併刪除。新 UI 不得再寫這些別名。

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
