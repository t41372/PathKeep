# M0 — 重構基礎

> 目標：切斷舊產品骨架，建立新的前端、後端和資料平面起點。  
> 這個里程碑不追求功能完整，而是追求「後續所有功能都能在正確骨架上落地」。
> 2026-04-06 基線：`bun run check` / `bun run build` 已恢復通過，可再次作為 M0 期間的硬 gate。

---

## M0 的完成定義

- 舊 `AppNew` shell 不再是主入口，新 shell / route tree / layout / token 已建立。
- 舊版 UI 被正式標記為淘汰或刪除，不再和新結構長期並存。
- `browser-history-parser` 的 crate 邊界、`vault-core` 的 archive 邊界、`vault-worker` 的 orchestration 邊界都已定稿。
- canonical schema 和 migration story 有正式設計，不再靠 ad-hoc `ensure_column()` 演進。
- PathKeep 命名、app root、workflow 文案、對外文檔已切換完成。
- 新的測試與質量基線對準新 shell 和新路徑，而不是繼續驗證舊 setup shell。

---

## 本里程碑文檔

- [frontend-shell-and-design-system.md](frontend-shell-and-design-system.md)
- [backend-and-data-rearchitecture.md](backend-and-data-rearchitecture.md)
- [rename-quality-and-cutover.md](rename-quality-and-cutover.md)

---

## 工作包摘要

| 工作包  | 內容                                                                            |
| ------- | ------------------------------------------------------------------------------- |
| `M0-FE` | 新 app shell、route tree、design system、prototype token extraction、舊 UI 移除 |
| `M0-BE` | parser crate、vault-core / vault-worker 重切、schema 與 migration foundation    |
| `M0-CO` | 命名遷移、README / workflow 修正、測試基線重設、cutover 策略                    |

---

## 依賴關係

1. 先完成設計 token 和頁面 IA，避免新前端重工。
2. 先完成 parser / core / worker / module boundary，避免 M1 又把功能堆回巨檔。
3. 先完成 schema / migration 設計，再開始 M1 backup engine 正式實作。
4. 在 M0 結束前，必須把命名和測試基線一起切過去，否則後面的里程碑會持續被舊名稱和舊測試干擾。

---

## 里程碑檢查表

- [ ] `M0-001` 新前端 shell 已可啟動並承載 Dashboard / Explorer / Insights / Assistant / Import / Audit / Schedule / Security / Settings / Onboarding 路由。
- [ ] `M0-002` 所有新界面缺口都已補設計稿或已有明確設計決策。
- [ ] `M0-003` `browser-history-parser` work package 已開工，責任邊界明確。
- [ ] `M0-004` 正式 migration system 設計已完成，M1 可以直接在此之上落地。
- [ ] `M0-005` PathKeep 命名遷移完成到足以支撐後續所有里程碑。
