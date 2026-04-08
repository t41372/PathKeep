# M1 — Solid Archive

> 目標：把 Archive 做成可信任的基礎設施，而不是只做一個能跑的 demo。  
> M1 不追求 Intelligence；M1 追求的是 migration、backup、audit、schedule、security、Explorer v1 都站得住。
>
> **狀態修正（2026-04-06）**：archive backup / explorer / audit 的 feature baseline 已落地，但 M1 仍未完成最終簽收。非前端殘項現在主要集中在 `M1-DB` / `M1-OPS` 的 acceptance matrix、security mode taxonomy 與 audit summary / retention policy；schedule status 與 rekey preview 的 command surface 已補上，但 UI trust closeout 仍未完成。
>
> **品質註記（2026-04-07）**：`WORK-QC-A` 已恢復 mainline `coverage:js` / `coverage:rust` / `build` / `test:e2e`，並把 `mutation:js` / `mutation:rust` 收斂成可追蹤的 deep checks。這代表 M1 baseline 現在有 honest gate 保護，但 prototype / product-flow / doc parity closeout 仍在 `WORK-QC-B`。

---

## M1 的完成定義

- 正式 migration system 已取代 ad-hoc schema bootstrapping。
- Chromium backup pipeline、staging copy、dedupe、manifest、snapshot、watermark 正常工作。
- Schedule 的 macOS preview / manual / apply / status command surface 已落地；trust copy 與 UI signoff 仍需完成。
- Plaintext / Encrypted archive init、keyring session basics、security status、rekey preview 與 snapshot-backed execute foundation 已可用；dedicated recovery / audit signoff 仍未完成。
- Explorer v1、Dashboard v1、Onboarding v1、Export v1 可供真實使用。
- Audit artifacts 和 manifest chain 可追蹤且可驗證。

---

## 本里程碑文檔

- [schema-backup-and-ledger.md](schema-backup-and-ledger.md)
- [schedule-security-and-storage.md](schedule-security-and-storage.md)
- [explorer-export-and-onboarding.md](explorer-export-and-onboarding.md)

---

## 工作包摘要

| 工作包   | 內容                                                       |
| -------- | ---------------------------------------------------------- |
| `M1-DB`  | schema、migration、backup engine、manifest、snapshot       |
| `M1-OPS` | schedule、security、storage、operation transparency        |
| `M1-UX`  | onboarding、dashboard、explorer、export、audit entrypoints |

---

## 里程碑檢查表

- [x] `M1-001` archive schema 和 migration 已凍結到可支撐 M2 的程度。（2026-04-06，WORK-M1-A）
- [x] `M1-002` 至少一個 Chromium profile 的完整 manual backup 流程可重複驗收。（2026-04-06，WORK-M1-A）
- [x] `M1-003` manifest chain、audit repo、snapshot safety net、doctor baseline 全部接通。（2026-04-06，WORK-M1-A）
- [~] `M1-004` 無 AI 配置下，產品已可完成 onboarding、備份、搜尋、匯出、查看 audit；但 schedule / security 的最終 trust signoff 與 M1 DB / OPS acceptance matrix 尚未收斂。（2026-04-06 審查修正）
