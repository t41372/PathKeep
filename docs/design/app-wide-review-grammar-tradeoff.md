# App-Wide Review Grammar And Single-Source Map Trade-off

> **狀態：Accepted**
> **日期：2026-04-19**
> **範圍：** M11 的 app-wide review / PME / diagnostics grammar inventory、`src/lib/intelligence.ts` ownership split、dev IPC mirror / worker pass-through inventory，以及 M12 seed boundary
> **關聯文檔：**
>
> - [screens-and-nav.md](screens-and-nav.md)
> - [ux-principles.md](ux-principles.md)
> - [../features/intelligence-current-state.md](../features/intelligence-current-state.md)
> - [../plan/m11-app-wide-reuse/README.md](../plan/m11-app-wide-reuse/README.md)
> - [../plan/m12-support-actions-and-diagnostics/README.md](../plan/m12-support-actions-and-diagnostics/README.md)

---

## 1. 問題定義

M6–M10 已把 shared insights entity、route grammar、shared composition、以及 intelligence subtree 的 workbench / transport hygiene 收斂成 accepted baseline，但 repo 仍有三類 drift：

1. **全 app review / PME / diagnostics grammar 漂移**
   - Settings external outputs / local host 已有 shared review shell
   - Schedule、Audit、Jobs、Import、Settings 其他支線仍大量重寫相似的 review row、generated artifact viewer、verify row、PME tab
2. **`src/lib/intelligence.ts` mixed owner**
   - route href grammar、AI/provider presentation、evidence link helper 還共住同一個檔案
3. **transport debt 是否真的值得繼續拆**
   - `src-tauri/src/dev_ipc_bridge.rs` 是 dev-only mirror
   - `src-tauri/src/worker_bridge/intelligence.rs` 與 `vault-worker` 多數已是 thin pass-through
   - 若沒有 owner payoff，再拆只是移動薄 wrapper

---

## 2. 約束

- 不重開 M6–M10 已 accepted 的 route path、query grammar、payload shape、trusted-output boundary
- 不把 M11 擴成 transport codegen / registry / macro 專案
- review / PME / diagnostics 的 shared grammar 要優先服務 shipping route，而不是先追求檔案形式整齊
- 新的 deferred gap 必須明確改記 M12，而不是留下新的浮動 `TODO`

---

## 3. 候選方案

### 方案 A — 維持現狀，只把 `src/lib/intelligence.ts` 拆小

**缺點**

- route helper 雖然拆出去，但 app-wide review / PME drift 還在
- Schedule / Audit / Jobs / Settings 仍會繼續各自長新 shell

**結論**

- 不接受

### 方案 B — 直接把所有 review / diagnostics / transport 一次抽成 framework

**缺點**

- scope 遠超 M11
- 會重開太多 page-local decision
- 沒有直接 product payoff

**結論**

- 不接受

### 方案 C — 先做 single-source map，再只抽第一輪高價值 neutral primitive

**做法**

- 先盤點 canonical owner 與 consumer-local drift
- 把 `review-surface` 升格成 app-wide neutral primitive
- 補齊 `PmeTabBar`、`GeneratedArtifactViewer`、`VerifyCheckList`
- `src/lib/intelligence.ts` 只做 owner split，不重寫 public import contract
- transport debt 只做 inventory 與 boundary 決策，不機械再拆

**結論**

- 採用

---

## 4. 最終決定

採用 **方案 C**。

### 4.1 canonical owner map

| 契約 / 能力 | canonical owner | M11 結論 |
| --- | --- | --- |
| entity route / search-param grammar | `src/lib/core-intelligence/routes.ts` | 正式升格成單一來源 |
| AI/provider/assistant status presentation | `src/lib/intelligence-ai-presentation.ts` | 從 `src/lib/intelligence.ts` 拆出 |
| evidence / assistant link + citation dedupe | `src/lib/intelligence-links.ts` | 從 `src/lib/intelligence.ts` 拆出 |
| archive / import / security / schedule status tone policy | `src/lib/trust-review.ts` | 維持單一來源 |
| deterministic runtime / plugin / module label policy | `src/lib/intelligence-runtime.ts` | 維持單一來源 |
| runtime-job summary / compact error policy | `src/lib/intelligence-presentation.ts` | 維持單一來源 |
| app-level diagnostics capture | `src/lib/runtime-diagnostics.ts` | 維持單一來源 |
| neutral review shell | `src/components/review/` | M11-B 正式升格 |
| transport chain | front-end client → IPC bridge → Tauri command / dev bridge → worker bridge → `vault-worker` | M11 只做 inventory，不開 codegen |

### 4.2 consumer-local drift inventory

**M11-B 立即抽取**

- Settings remote backup PME tabs / verify rows
- Settings AI integration preview generated files / review rows
- Settings external-output local host generated artifact viewer
- Schedule PME tab chrome、generated file preview、verify result rows
- Audit artifact rows
- Jobs recent AI/runtime job rows

**留到 M12**

- Settings general diagnostics path rows / support actions
- Import browser-profile review cards、batch review / doctor follow-through
- Jobs plugin / module summary rows與更多 support actions
- Audit restore preview / related import review deeper extraction
- copy / open-path action reuse 與 Settings mega-route further split

### 4.3 `src/lib/intelligence.ts` boundary

- 保留為 **thin barrel only**
- route href / label owner → `src/lib/core-intelligence/routes.ts`
- AI/provider/assistant presentation → `src/lib/intelligence-ai-presentation.ts`
- evidence / assistant link + citation dedupe → `src/lib/intelligence-links.ts`
- 後續不得再把新 helper 塞回 barrel

### 4.4 transport debt boundary

- `src-tauri/src/dev_ipc_bridge.rs`：保留 dev-only mirror，不在 M11 建 shared manifest / codegen
- `src-tauri/src/worker_bridge/intelligence.rs` / `vault-worker`：目前多數是 thin pass-through，不為了「看起來更碎」而再拆
- 只有當 M12 inventory 證明 owner 漂移、命名 drift、或 parity 維護成本持續上升時，才重新立項

---

## 5. 後續里程碑

下一輪 seed milestone 為 [M12 — Shared Support Actions And Diagnostics Decomposition](../plan/m12-support-actions-and-diagnostics/README.md)：

- 聚焦 copy/open-path action reuse
- 收斂 support / diagnostics rows 與 Settings mega-route further split
- 決定是否需要更輕量的 transport parity automation
- 不回頭重開 M6–M11 已接受的 route / payload / trusted-output 邊界
