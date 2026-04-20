# Support Actions And Diagnostics Reuse Trade-off

> **狀態：Accepted**
> **日期：2026-04-19**
> **範圍：** M12 的 support-action / diagnostics inventory、shared path/open/copy grammar、Settings mega-route split boundary，以及 dev bridge / worker parity inventory
> **關聯文檔：**
>
> - [screens-and-nav.md](screens-and-nav.md)
> - [ux-principles.md](ux-principles.md)
> - [app-wide-review-grammar-tradeoff.md](app-wide-review-grammar-tradeoff.md)
> - [../plan/m12-support-actions-and-diagnostics/README.md](../plan/m12-support-actions-and-diagnostics/README.md)
> - [../plan/m13-broad-reuse-audit/README.md](../plan/m13-broad-reuse-audit/README.md)

---

## 1. 問題定義

M11 已把 neutral review primitive 推到 Settings / Schedule / Audit / Jobs，但 repo 仍有三類 drift：

1. `open path` / `copy path` / `support action` 仍在多個 route 各寫一套
   - Settings general diagnostics / App Lock config path
   - Import workflow copy / selected-batch audit path / doctor follow-through
   - Audit manifest / artifact actions
   - Schedule verify / manual detected files 與 latest audit quick-jump
   - Security / Lock path rows
   - Explorer export-path support action
2. repo 內已經有 [`src/components/ui.tsx`](../../src/components/ui.tsx) 的 `PathRow`，但它沒有成為真正 canonical owner
3. `src-tauri/src/dev_ipc_bridge.rs`、`src-tauri/src/worker_bridge/intelligence.rs` 與 `vault-worker` 尚留著薄薄的 parity / pass-through debt，但目前還不足以證明值得重開 codegen 或 manifest 專案

換句話說：

- M11 已 shared 的是 review shell
- M12 要 shared 的是 review shell 裡剩下的 support-action grammar

---

## 2. 約束

- 不推翻 M6–M11 已接受的 route grammar、payload shape、trusted-output boundary 與 neutral review primitive 邊界
- canonical owner 必須留在 `src/components/review/`；不得把 `PathRow` 升格成平行 single source
- 不把 M12 擴成 Settings mega-route 全面拆分或 transport automation / codegen 專案
- 新的 deferred gap 必須明確改記 `TODO: M13`

---

## 3. 候選方案

### 方案 A — 維持 page-local actions，只補小修

**做法**

- 保留各頁各自呼叫 `navigator.clipboard` / `openPathInFileManager`
- `PathRow` 繼續留在 `src/components/ui.tsx` 當半成品

**缺點**

- drift 會持續存在
- review shell shared 了，但最常用的 support-action 仍不是 single source

**結論**

- 不接受

### 方案 B — 直接把 path/support/diagnostics 與 transport 一次抽成大 framework

**做法**

- 同一輪處理 shared support rows、Jobs summary rows、Settings route split、dev bridge parity automation、worker pass-through decomposition

**缺點**

- scope 遠超 M12
- 會把原本清楚的 product payoff 稀釋掉
- 容易為了抽象而重開已 accepted 的 transport boundary

**結論**

- 不接受

### 方案 C — 先抽 shared support-action grammar，Jobs deeper summary 與 parity debt 只做 inventory

**做法**

- 在 `src/components/review/` 新增 shared clipboard helper 與 path/support row primitive
- 先遷移最明顯重複的 consumer：Settings、Audit、Import、Schedule、Security、Lock、Explorer export path
- Jobs plugin / module summary rows 與 transport parity 先只做 owner/defer 決策

**優點**

- 直接命中目前最高頻、最容易 drift 的 support grammar
- review 面積可控
- 保留 M13 作為更廣的 support / trust / workflow reuse audit

**結論**

- 採用

---

## 4. 最終決定

採用 **方案 C**。

### 4.1 canonical owner map

| 契約 / 能力                               | canonical owner                                                                                    | M12 結論                                                                      |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| neutral review shell                      | `src/components/review/`                                                                           | 維持 M11 結論                                                                 |
| shared clipboard feedback + action status | `src/components/review/`                                                                           | M12 正式升格                                                                  |
| labeled path/support row                  | `src/components/review/`                                                                           | M12 正式升格                                                                  |
| `PathRow`                                 | `src/components/ui.tsx`                                                                            | 不再作為 active owner；後續只保留 compatibility / fallback 角色，必要時再退休 |
| dev IPC mirror parity                     | `src-tauri/src/dev_ipc_bridge.rs`                                                                  | 只做 inventory                                                                |
| worker / `vault-worker` pass-through debt | `src-tauri/src/worker_bridge/intelligence.rs`、`src-tauri/crates/vault-worker/src/intelligence.rs` | 只做 inventory                                                                |

### 4.2 M12-B 優先採用範圍

- Settings general diagnostics path rows
- Settings App Lock config path
- Audit manifest / artifact support rows
- Import selected-batch audit path 與 doctor follow-through
- Schedule generated file / detected file / audit path rows
- Security / Lock config and snapshot path rows
- Explorer export-path support action（若 shared primitive 能自然覆蓋）

### 4.3 明確 deferred 到 M13

- Jobs plugin / module summary rows的 deeper reusable summary grammar
- Settings mega-route 更深的 owner split
- dev bridge / worker parity automation 或 manifest/codegen

以上 deferred 一律改記 **`TODO: M13`**，不再沿用 `TODO: M12`。

---

## 5. 後續里程碑

下一輪 seed milestone 為 [M13 — Broad Reuse Audit Across Support, Trust, And Workflow Surfaces](../plan/m13-broad-reuse-audit/README.md)：

- 以 support / trust / workflow surface 為主題繼續盤點 reusable grammar
- 擴大到 Jobs summary、Settings slice ownership、Explorer / Import / Audit follow-through composition
- transport parity 仍屬 subordinate inventory，不升格成主線
