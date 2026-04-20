# M12 — Shared Support Actions And Diagnostics Decomposition

> 目標：延續 M11 的 neutral review grammar，把剩餘的 support-action、diagnostics row、以及 Settings mega-route 分解做成下一輪高價值 reuse，而不是再讓 copy/open-path/support affordance 在各頁各寫一套。

---

## M12 的完成定義

- [ ] 收斂 app-wide copy / open-path / support-action grammar
- [ ] 盤點並抽出 diagnostics rows / support summary 的 reusable primitive
- [ ] 決定 Settings mega-route 還有哪些 slices 值得繼續拆
- [ ] 評估是否需要更輕量的 transport parity automation，而不是直接上 codegen

---

## 首批範圍

- Settings general diagnostics path rows / crash-report actions
- Import browser-profile review、batch review 與 doctor follow-through 的 support actions
- Audit restore preview / related import review deeper extraction
- Jobs plugin / module summary rows與 common support actions
- dev bridge / command parity 的 lightweight automation feasibility

---

## 建議工作塊

- `WORK-M12-A` — Shared Support Actions And Diagnostics Inventory
- `WORK-M12-B` — Support Action / Diagnostics Primitive Extraction

---

## 邊界

- 不回退 M11 已接受的 neutral review primitive boundary
- 不重開 M6–M11 的 route grammar、payload shape、trusted-output boundary
- 不為了「把 Settings 拆完」而做沒有 owner payoff 的 mechanical split
