# M2 — Recall & Trust

> 目標：把多來源導入、回滾、Doctor、PME、i18n 和跨平台支持做完整，讓產品從「可備份」進化到「可長期信任地使用」。
>
> **狀態註記（2026-04-07）**：`WORK-M2-A` 與 `WORK-M2-B` 都已完成，Google Takeout import / rollback / restore、Firefox 正式支持、Safari baseline support、doctor / repair、trust UX、namespace-based i18n 與跨平台 guidance 都已落地。M3 接手 optional intelligence。

---

## M2 的完成定義

- Google Takeout import 有完整 dry-run / preview / quarantine / import / rollback 路徑。
- Firefox 支持完成，Safari 基礎支持完成。
- Run 歷史、manifest chain、rollback / un-revert、Doctor 報告在 UI 裡都可操作。
- PME 模式從 schedule 擴展到 import / rekey / high-risk operations。
- i18n 和跨平台排程支持正式落地。

---

## 本里程碑文檔

- [imports-browsers-and-rollback.md](imports-browsers-and-rollback.md)
- [trust-ux-i18n-and-platforms.md](trust-ux-i18n-and-platforms.md)

---

## 里程碑檢查表

- [x] `M2-001` Google Takeout 端到端導入、回滾、再恢復都已驗收。（2026-04-07，WORK-M2-A）
- [x] `M2-002` Firefox backup 和 Explorer recall 可驗收。（2026-04-07，WORK-M2-A）
- [x] `M2-003` Doctor 能發現至少 integrity / chain / orphan / index 類問題。（2026-04-07，WORK-M2-A）
- [x] `M2-004` `en` / `zh-CN` / `zh-TW` 的核心流程已可用。（2026-04-07，WORK-M2-B）
- [x] `M2-005` Windows / Linux 的排程規劃和 manual / apply story 已驗證。（2026-04-07，WORK-M2-B）
