# Intelligence Workbench And Transport Hygiene Trade-off

> **狀態：Accepted**
> **日期：2026-04-19**
> **範圍：** `refind` shared workbench chrome、Explorer grouped workbench rows、Settings external-output/local-host review chrome、promoted route split、front-end Core Intelligence API split、以及 Tauri command / worker bridge intelligence facade split
> **關聯文檔：**
>
> - [screens-and-nav.md](screens-and-nav.md)
> - [../features/intelligence-current-state.md](../features/intelligence-current-state.md)
> - [../features/core-intelligence-ultimate-design.md](../features/core-intelligence-ultimate-design.md)
> - [../plan/m10-workbench-reuse/README.md](../plan/m10-workbench-reuse/README.md)
> - [../plan/m11-app-wide-reuse/README.md](../plan/m11-app-wide-reuse/README.md)

---

## 1. 問題定義

M9 已把 route-level shared composition 收斂到 single source，但 repo 仍有兩類明顯 drift：

1. `refind`、Explorer session/trail grouped view、以及 Settings external-output / local-host review surface 還留著 consumer-local workbench rows / review chrome
2. `src/pages/intelligence/promoted-entity-routes.tsx`、`src/lib/core-intelligence/api.ts`、`src-tauri/src/{commands,worker_bridge}/intelligence.rs` 仍是 ownership 混雜的大檔，後續維護成本高

這些問題的共同點是：

- 已接受的 route grammar、payload shape、entity-first CTA 並沒有錯
- 錯的是 reusable presentation 與 thin transport glue 還沒有正式拆開

---

## 2. 約束

- 不推翻 M6–M9 已接受的 `Insights first` / entity-first / focus / trusted-output 邊界
- 不更動 route path、query grammar、Tauri command name、TS API name 或 payload shape
- 不把 M10 擴成新的 intelligence entity、focus type、desktop contract rewrite 或 codegen 專案
- 新的 deferred gap 必須明確改記 `TODO: M11`

---

## 3. 候選方案

### 方案 A — 維持現狀，只修局部 bug

**做法**

- 保留 `refind` / Explorer / Settings 各自 hand-roll workbench shell
- 保留前端 / Rust intelligence glue 大檔，只在需要時追加函數

**缺點**

- drift 會持續累積
- 下一輪 reuse audit 仍得先穿越同一批 mega-file

**結論**

- 不接受

### 方案 B — 一次把 transport 全面抽象成 registry / macro / codegen

**做法**

- 直接重寫 front-end API / Tauri command / worker bridge pass-through
- 讓 route / desktop glue 全部改成 generated 或 registry-driven

**缺點**

- 風險過高
- scope 遠超 M10
- 沒有直接 product payoff

**結論**

- 不接受

### 方案 C — 先抽 shared workbench，再做 ownership-based split

**做法**

- 先把明確重複的 workbench / review chrome 收斂到 shared primitive
- 再把 promoted routes、front-end API、Tauri command / worker bridge 按 `ai / core / runtime` 與 per-route ownership 拆檔
- 保持 public contract 完全不變

**優點**

- 直接命中目前最實際的維護成本來源
- review 面積仍可控
- 能把剩餘沒拆的項目清楚改記 M11

**結論**

- 採用

---

## 4. 最終決定

採用 **方案 C**。

### 4.1 本輪正式升格成 shared workbench contract 的內容

- `refind` summary/factor shell：overview、day insights、dedicated refind route 共用同一套 workbench module
- Explorer `session` / `trail` grouped card 與 member row primitive
- promoted route member rows 沿用 shared workbench row，不再各寫一份 keyboard / click glue
- Settings external-output / trusted local host review row、code preview、target-link grammar 共用同一套 workbench primitive

### 4.2 本輪正式接受的 transport hygiene boundary

- `src/pages/intelligence/promoted-entity-routes.tsx` 拆成 per-route module + shared helper
- `src/lib/core-intelligence/api.ts` 拆成 `shared` / `overview` / `entities` / `runtime-outputs`
- `src-tauri/src/commands/intelligence.rs` 與 `src-tauri/src/worker_bridge/intelligence.rs` 拆成 `ai` / `core` / `runtime`
- 對外 route / command / payload contract 完全不變

### 4.3 本輪明確不做的事

- 不重開 `src/lib/intelligence.ts` mixed helper ownership
- 不重寫 `src-tauri/src/dev_ipc_bridge.rs` intelligence mirror
- 不把 `vault-worker` pass-through 再往下拆成更細的 purely mechanical layer

這三項全部改記 **`TODO: M11`**。

---

## 5. 後續里程碑

下一輪正式進入 [M11 — App-Wide Reuse Audit And Shared Review Grammar](../plan/m11-app-wide-reuse/README.md)：

- 盤點 app 內剩餘的 shared review / PME / diagnostics drift
- 決定 `src/lib/intelligence.ts`、dev IPC mirror、以及更深一層 transport glue 是否值得繼續拆
- 把 workbench reuse 從 Intelligence / Settings 擴大到全 app review surface，而不是只收 intelligence 子樹
