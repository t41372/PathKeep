# M12 — Shared Support Actions And Diagnostics Decomposition

> 目標：延續 M11 的 neutral review grammar，把剩餘的 support-action、diagnostics row、以及 Settings mega-route 分解做成下一輪高價值 reuse，而不是再讓 copy/open-path/support affordance 在各頁各寫一套。

---

## 這輪 accepted source of truth

- [../../design/support-actions-and-diagnostics-tradeoff.md](../../design/support-actions-and-diagnostics-tradeoff.md)

---

## M12 的完成定義

- [x] 收斂 app-wide copy / open-path / support-action grammar
- [x] 盤點並抽出 diagnostics rows / support summary 的 reusable primitive
- [x] 決定 Settings mega-route 還有哪些 slices 值得繼續拆
- [x] 評估是否需要更輕量的 transport parity automation，而不是直接上 codegen

---

## 首批範圍

- Settings general diagnostics path rows / crash-report actions
- Import browser-profile review、batch review 與 doctor follow-through 的 support actions
- Audit restore preview / related import review deeper extraction
- Jobs plugin / module summary rows與 common support actions
- dev bridge / command parity 的 lightweight automation feasibility

---

## Single-Source Map

| 契約 / 能力                                      | canonical owner                                                                                    | M12 結論                                                      |
| ------------------------------------------------ | -------------------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| neutral review shell                             | `src/components/review/`                                                                           | 延續 M11，不重開 owner                                        |
| shared clipboard helper / copy feedback          | `src/components/review/`                                                                           | M12-B 正式升格                                                |
| labeled path / open-path / copy-path support row | `src/components/review/`                                                                           | M12-B 正式升格                                                |
| legacy `PathRow`                                 | `src/components/ui.tsx`                                                                            | 不再作為 active owner；只保留 compatibility / retirement 候選 |
| dev IPC mirror parity                            | `src-tauri/src/dev_ipc_bridge.rs`                                                                  | M12 只做 inventory                                            |
| worker / `vault-worker` pass-through debt        | `src-tauri/src/worker_bridge/intelligence.rs`、`src-tauri/crates/vault-worker/src/intelligence.rs` | M12 只做 inventory                                            |

---

## Consumer Inventory

### M12-B 立即抽取

- Settings general diagnostics + App Lock config-path rows
- Audit manifest / artifact open-copy-preview rows
- Import selected-batch audit-path action、doctor follow-through、workflow copy action
- Schedule generated file / detected file / audit-path support rows
- Security / Lock path rows
- Explorer export-path support action（只在 shared primitive 能自然覆蓋時）

### 明確延到 M13

- Jobs plugin / module summary rows與更深一層 support summary grammar
- Settings mega-route 更細的 owner split
- dev bridge / worker parity automation / manifest generation feasibility

---

## Extraction Boundary

- 先抽 `open path` / `copy path` / `support action` grammar，不重開 route / payload contract
- shared owner 只能放在 `src/components/review/`
- `PathRow` 不得升格成平行 single source；如果 adoption 完成後已無 owner payoff，後續直接退休
- 若 Jobs summary rows 需要不同 shape，寧可 defer 到 M13，也不要把第一版 primitive 抽成過度抽象的 mega-component

---

## 後續 seed

- [../m13-broad-reuse-audit/README.md](../m13-broad-reuse-audit/README.md) — Broad Reuse Audit Across Support, Trust, And Workflow Surfaces

---

## 建議工作塊

- `WORK-M12-A` — Shared Support Actions And Diagnostics Inventory
- `WORK-M12-B` — Support Action / Diagnostics Primitive Extraction

---

## 邊界

- 不回退 M11 已接受的 neutral review primitive boundary
- 不重開 M6–M11 的 route grammar、payload shape、trusted-output boundary
- 不為了「把 Settings 拆完」而做沒有 owner payoff 的 mechanical split

---

## 2026-04-19 WORK-M12-A Inventory Note

- M12 現在正式接受 `src/components/review/` 作為 support-action grammar 的唯一 owner；`src/components/ui.tsx` 的 `PathRow` 不再視為 active single source。
- inventory 已明確把 Settings / Import / Audit / Schedule / Security / Lock / Explorer export path 列為 M12-B 優先 adoption surface。
- Jobs plugin / module summary rows與 transport parity debt 則明確 deferred 到 M13，不在 M12 內硬抽成不自然的共用元件。

---

## 2026-04-19 WORK-M12-B Closeout

- shared support-action grammar 現在正式升格到 `src/components/review/`：新增 shared clipboard helper 與 `ReviewPathActionRow`，不再讓路徑 / copy / open affordance 分散在各頁各寫一套。
- Settings general diagnostics / App Lock、Audit manifest / artifact rows、Import selected-batch audit path、Schedule detected-file / audit-path actions、Security / Lock path rows，以及 Explorer export path 現在都改吃 shared review-layer support-action contract。
- Jobs plugin / module summary rows與 dev bridge / worker parity follow-up 已明確改記 `TODO: M13`，並轉交給 [../m13-broad-reuse-audit/README.md](../m13-broad-reuse-audit/README.md)。
