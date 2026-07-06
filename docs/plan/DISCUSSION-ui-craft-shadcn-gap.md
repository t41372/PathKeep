# DISCUSSION — UI 精緻度問題:shadcn 到底有沒有用?為什麼還是像手搓?

> ✅ **ACTIONED (2026-07-05) — 三條線都做完了。** 見 CHANGELOG `WORK-UI-SHADCN-ADOPTION`。分支 `feat/ui-craft-shadcn-adoption`。
> 一句更新:調查前提被一份 8-reader understanding pass 修正——`tailwind.css` 的 `@theme` 其實**已經**把大多 shadcn slot 別名到 paper tokens,所以 button 的活是 **5 個精準缺口**(未定義的 `text-accent-foreground`、裸 `border`→currentColor、非 paper shadow、`rounded-xs`、字面 `text-white`)+ 一個 inert-`danger` bug,不是全量 class 重寫。
>
> - **線 1 — Button primitive + 採用**:`ui/button.tsx` 重 theme 成 paper tokens,variants 覆蓋 audit 出的六個真實 cluster(435 個手寫 `<button>`)+ 28px icon size + reduced-motion-safe `loading`;destructive 改用 `error` tokens(`--color-danger` 根本不存在)。試點採用 **Activity /jobs**(全 7 個)+ **Settings**(8 section / 26 個 action button,退掉重複的 `BUTTON_*` const),passcode/restore/migration 由專用 sensitive-flow review lens 逐一驗過。commits `e44a6eee` / `2b2402f4` / `0c31f651`。
> - **線 2 — 自建 SVG chart primitive**:ADR `docs/design/chart-primitive-tradeoff.md`(自建、零新依賴)。新 `src/components/charts/`(geometry + CalendarHeatmap + Sparkline);year-heatmap 從 365 div 遷成 SVG 並**修復回退的 a11y**(role=grid + roving-tabindex 鍵盤模型 + per-cell name);兩個手搓 sparkline 併入共用 Sparkline、legacy-var 百分比 bar 重上 paper token。commits `23143c8c` / `9b160d80`。
> - **線 3 — 統一 Skeleton**:`primitives/skeleton.tsx` 重建成 paper-token,修掉 2026-06-14 記錄的 layout-shift 缺陷(DashboardSkeleton / SkeletonExplorer 現在對齊真實 paper 佈局);單一 keyframe(§9 紅線)、刪 3 個 dead export。commit `cc6794b9`。
> - **未做(→ BACKLOG `WORK-UI-SHADCN-SWEEP`)**:其餘 ~400 個手寫 button;Intelligence `.intelligence-skeleton` dialect + 34 個 legacy `.skeleton` 檔;重啟 `desktop-bridge:truth`。一次全掃不現實也不可 review,刻意留成 follow-up。
>
> 以下為 2026-06-29 的原始調查,保留作背景與 traceability(數字為當時 grep,已被上面的 understanding pass 更新)。

> 目的是把「為什麼 UI 看起來粗糙」這件事查清楚、沉澱下來,再決定要不要開 work block。結論若被採納,再拆進 `STATUS.md` / `BACKLOG.md` 並同步 `docs/design/`。
>
> 調查日期:2026-06-29(feat/ai-redesign-2026)。數字是當下 `grep` 實測,不是印象。

---

## 0. 一句話結論

**shadcn 只被引進到「foundation 裝好」這一步就停了。** 真正讓 UI 精緻的三件事——(a) 各頁採用 primitive、(b) 把 primitive 對齊 paper tokens、(c) 圖表有個像樣的畫法——**一件都沒做完**。所以「像手搓」不是錯覺:**大半的 UI 字面上就是手搓的**,shadcn 幾乎只是躺在 `src/components/ui/` 裡沒被用。

---

## 1. 我們到底用了 shadcn 嗎?——用了,但只有骨架

先澄清一個常見誤會:shadcn **不是一個 npm 套件**,而是「把組件原始碼 copy 進你 repo、由你維護」的模式。所以它不會出現在 `dependencies`,出現的是它底下真正的 runtime 依賴。我們確實把這套骨架裝起來了:

- **依賴組合就是 shadcn 標準班底**:`radix-ui` + `class-variance-authority` + `clsx` + `tailwind-merge` + `cmdk` + `lucide-react` + `tw-animate-css` + Tailwind v4。
- **`src/components/ui/` 有 committed primitives**:`button.tsx` / `command.tsx` / `dialog.tsx` / `popover.tsx`(4 個)。
- **`src/lib/cn.ts`** 就是 shadcn 標配的 `cn` helper。
- docs 也白紙黑字:`STATUS.md:288`「Foundation shipped:Tailwind v4 + shadcn primitives + cn helper」;design handoff `README.md:39`「Tailwind v4 + shadcn primitives」。

> 註:`HANDOFF-2026-05-19` 裡那串 badge/input/select/sheet/tooltip… 是「未 committed 的 stale prettier reformat,stash and forget」,**不是實際在用的**。實際 committed 的就是上面 4 個。

**所以「有沒有用 shadcn」的答案是:骨架有,肉沒長上去。**

---

## 2. 為什麼還是像手搓?——三個實測到的缺口

### 缺口 A:圖表是真・手搓 —— 根本沒有圖表庫

| 檢查                                 | 結果                                                                  |
| ------------------------------------ | --------------------------------------------------------------------- |
| `package.json` 裡的 charting library | **0 個**(無 recharts / visx / d3 / chart.js / nivo / echarts / uPlot) |
| `year-heatmap.tsx` 用什麼畫          | **4 個 `<div>` 堆疊**,連 SVG 都不是                                   |

那句常被引用的「shadcn recharts」只是設計稿 `pk-contactsheet.jsx:396` 裡的一句 `NOTE: implement with tailwind + shadcn recharts in production`,**從沒落地**。dashboard 的 heatmap / this-week / active-threads 全是 div + `animate-pulse` 手刻。圖表的「粗糙感」不是感覺問題,是它字面上就是 div 手搓的。

### 缺口 B:shadcn 的按鈕基本沒人用 —— 411 個手寫 `<button>`

| 檢查                               | 結果                                                                    |
| ---------------------------------- | ----------------------------------------------------------------------- |
| import `ui/button` 的檔案數        | **1**,而且是 `dialog.tsx`(primitive 內部互引)——**沒有任何業務頁面用它** |
| 全 app 手寫 `<button>`(排除 `ui/`) | **411**                                                                 |

意思是:整個 app 的按鈕幾乎都是各自用 raw Tailwind class 手刻的。風格、尺寸、padding、hover、focus ring、disabled 態全靠每個檔案的手感 → **必然參差不齊**,而且沒有單一改動點能一次拉齊。

### 缺口 C:連那唯一的 `button.tsx` 都沒對齊 paper 設計

`ui/button.tsx` 目前是 **shadcn 出廠預設,一個字沒改**:

- 用的 token:`rounded-md`(×5)、`bg-primary`、`bg-background`、`bg-accent`、`shadow-xs`、`border-input`、`text-primary-foreground` —— **全是 shadcn 出廠命名**。
- paper tokens(`rounded-paper` / `bg-paper` / `text-ink` / `font-serif` / `bg-accent-soft` / `border-border-*`)出現次數:**0**。

這違反 `design-tokens.md:162`「新 shell / page 工作要透過 Tailwind 消費 paper tokens」。所以就算有人真的用了這個 primitive,出來的也是「通用 shadcn 樣子」——圓角、字體、配色全跟你們 cream-paper / 三字體 / 3px radius 的美學**脫節**。

### 缺口 D:loading / skeleton 沒有統一元件

| 檢查                                    | 結果                                                                                      |
| --------------------------------------- | ----------------------------------------------------------------------------------------- |
| 用 legacy v0.2 `.skeleton` class 的地方 | **80 處**                                                                                 |
| loading 動畫                            | 散落的 `animate-pulse`(dashboard 各 card、status-bar、contact-sheet…),無統一 loading 元件 |

`docs/review/2026-06-14` 已經抓到:這些 skeleton 用 v0.2 grid class,跟最終 paper 版面不一致,**載入完會 layout 跳動**——直接違反 `ux-principles.md`「skeleton 必須匹配最終版面、避免 layout shift」。

---

## 3. 根因分析:不是「沒選 shadcn」,是「adoption / theming 沒收尾」

把上面串起來,問題**不在技術選型**——shadcn + Radix + Tailwind + cva 這套是對的,跟 PathKeep 的原則也高度契合:

- **供應鏈信任**(AGENTS.md):vendored source 沒有黑盒套件在背後升級/破壞你。
- **視覺主權**:primitive 進來後可直接改源碼對齊 paper tokens,不用跟外部套件預設樣式打架。

問題出在**這套模式的後三步從沒走完**。shadcn 的價值曲線是這樣的:

```
裝依賴 + copy primitive   →   換成你的 design tokens   →   全 app 採用 primitive
   (我們停在這 ✅)              (沒做 ❌)                  (沒做 ❌)
```

只做第一步,拿到的就是「一個裝了 Radix 的空殼」,精緻度為零;甚至比全手搓更糟,因為現在**兩套視覺語言並存**(4 個未 theme 的 shadcn primitive + 411 個手寫按鈕 + 80 處 v0.2 skeleton),形成 review 講的「style islands」。

圖表則是另一條線:**它根本沒進 shadcn/組件化的軌道**,一直是 dashboard 各 card 自己 div 手刻,沒有共用的繪圖 primitive。

---

## 4. 收尾方案(建議優先序)

分成三條獨立的線,每條都能單獨出成果、單獨驗收。順序按「投入小 / 見效快 / 風險低」排。

### 線 1 — Primitive theming + 試點採用(建議先做)

**做什麼**

1. 把 `ui/button.tsx` 的 cva variants 從 shadcn 出廠 token 換成 paper tokens(`rounded-paper` / `bg-paper` / `bg-accent-soft` / `text-ink` / `font-*`),variant 對齊實際在用的語義(primary / ghost / outline / destructive / link)。
2. 挑 **1–2 個高頻頁面**(如 Settings、Activity)把手寫 `<button>` 換成 `<Button>`,當試點驗證 primitive 在 paper 設計下站得住。
3. 補一個 codemod / lint,防止之後又長出手寫按鈕(可選,後置)。

**為什麼先做**:投入最小、一改就看得到全域一致性提升,也驗證「primitive 換 token」這條路走得通再擴大。411 個一次換完不現實,但**證明第一批換完是對的**很重要。

**風險**:低。button.tsx 只有 1 個 internal import,改它幾乎不會炸東西;試點頁面可控。

### 線 2 — 圖表繪製方案(需一個決策)

heatmap / this-week / active-threads 目前是 div 手刻。要決定的是:**引庫 vs 自建 SVG primitive**。這牽涉 AGENTS.md 的供應鏈門檻(stars > 6k 或知名組織)+ 效能約束(1440 萬條),值得一個 ADR。粗略選項:

| 方案                             | 優點                                                                                       | 代價 / 風險                                                     |
| -------------------------------- | ------------------------------------------------------------------------------------------ | --------------------------------------------------------------- |
| **自建 SVG primitive**(推薦起點) | 完全掌控 paper 美學、零依賴、契合供應鏈謹慎;我們的圖表其實不複雜(日曆格 / bar / sparkline) | 要自己寫刻度/比例尺,但量不大                                    |
| Recharts(~24k⭐)                 | 生態成熟、上手快                                                                           | 包 D3、較重,大資料量已知有 perf 坑,且預設樣式要花力氣蓋成 paper |
| visx(Airbnb,~19k⭐)              | 低階 primitive,適合客製 paper 外觀                                                         | 偏底層,工比 recharts 多                                         |
| uPlot(~9k⭐)                     | canvas,巨量序列極快                                                                        | canvas 不是 DOM,難對齊 paper 的 CSS 美學與 a11y                 |

**傾向**:PathKeep 的圖表都是簡單型別,`選長期最優解` + `供應鏈信任` 兩條原則下,**先自建一個小的 SVG chart primitive**(共用 scale / axis / paper token 樣式),把 heatmap 從 div 遷過去;真的遇到複雜圖(多序列互動)再單獨評估引 visx。這條要寫成 ADR 讓你拍板。

### 線 3 — 統一 loading / skeleton

1. 收斂出**一個 paper-token 的 `Skeleton` primitive**(matched-layout 版),取代散落的 `animate-pulse`。
2. 清掉那 80 處 legacy `.skeleton` class 的 layout shift,對齊 `ux-principles.md`。

**為什麼**:直接消掉一個 review 已記錄的缺陷(load 完跳版),且 loading 態是「~100ms 內給視覺回饋」硬指標的一部分。

---

## 5. 工作方式備註(給執行的 agent)

- **視覺 / copy 的決策交給 Sonnet 4.6**(見 memory `feedback_ui_copy_model`):token 對應、variant 命名、視覺細節不要 Opus 拍板。
- **實作走 subagent implement + review loop**(memory `feedback_subagent_implement_review_loop`):main agent 不親手寫重要代碼;clean-context subagent 實作,獨立 subagent review+fix 到無 major issue。
- **i18n / a11y 是 shipping contract**:換按鈕時 `aria-label`、disabled/loading 態的 copy 三語同交。
- **效能**:圖表遷移要過「1440 萬條時這會怎樣」——heatmap 的資料聚合不能放 render path。
- **gate**:動了任何 `.tsx` 就跑 `bun run check`;commit 前跑 repo-wide `format:check`(memory `feedback_authoritative_gate_before_commit`)。

---

## 6. 待你決定

1. 這三條線要不要開成 work block 進 `BACKLOG.md`?還是先只做**線 1(button theming + 試點)**看效果?
2. **圖表**要不要我先寫一份「自建 SVG primitive vs 引 visx/recharts」的 ADR 給你拍板?
3. 有沒有哪個頁面是你覺得「最醜、最該先動」的?可以直接當線 1 的試點。

---

_附:本文所有數字來自 2026-06-29 對 `feat/ai-redesign-2026` 的實測 grep(圖表庫 0、heatmap 4×div、ui/button 被 import 1 次、手寫 button 411、button.tsx paper token 0 次、legacy skeleton 80 處)。若後續代碼變動,重跑再更新。_
