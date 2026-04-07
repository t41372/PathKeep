# M1 — Solid Archive

> 目標：把 Archive 做成可信任的基礎設施，而不是只做一個能跑的 demo。  
> M1 不追求 Intelligence；M1 追求的是 migration、backup、audit、schedule、security、Explorer v1 都站得住。

---

## M1 的完成定義

- 正式 migration system 已取代 ad-hoc schema bootstrapping。
- Chromium backup pipeline、staging copy、dedupe、manifest、snapshot、watermark 正常工作。
- Schedule 的 PME 流程在 macOS 完整落地，狀態監控可用。
- Plaintext / Encrypted / Rekey 的主要流程可用且可驗收。
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

- [ ] `M1-001` archive schema 和 migration 已凍結到可支撐 M2 的程度。
- [ ] `M1-002` 至少一個 Chromium profile 的完整 manual backup 流程可重複驗收。
- [ ] `M1-003` manifest chain、audit repo、snapshot safety net、doctor baseline 全部接通。
- [ ] `M1-004` 無 AI 配置下，產品已可完成 onboarding、備份、搜尋、匯出、查看 audit。
