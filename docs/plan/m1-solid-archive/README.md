# M1 — Solid Archive

> 目標：把 Archive 做成可信任的基礎設施，而不是只做一個能跑的 demo。  
> M1 不追求 Intelligence；M1 追求的是 migration、backup、audit、schedule、security、Explorer v1 都站得住。
>
> **Closeout（2026-04-07）**：`WORK-QC-A` 已恢復 honest quality gate，`WORK-QC-B` 又把 onboarding / dashboard trust copy、On This Day / evidence / timezone 行為、以及 desktop-vs-preview 邊界重新對齊 source docs 與 acceptance。M1 作為 no-AI archive baseline 現在正式完成；更廣的 release polish 留給 M4。
>
> **Truth closeout（2026-04-09 / `WORK-QC-C` + `WORK-M1-C`）**：M1 補上了 recoverability / operations acceptance matrix。當時正式 shipping 的是 canonical migration、backup、manifest、snapshot safety net、doctor baseline、schedule PME、security mode / unlock / rekey foundation；snapshot restore preview / execute、auto retention / prune、與 richer rekey audit summary 仍被誠實標成 deferred。
>
> **Recoverability closeout（2026-04-10 / `WORK-M1-D`）**：M1 的 recoverability contract 現在真正收口了。Audit 可對 saved raw-source checkpoint 做 `snapshot_restore` Preview / Execute，Settings 可 explicit prune local snapshots / exports / staging / quarantine，rekey execute 也會留下 `rekey` run、manifest 與 safety snapshot path，Security 能直接 deep-link 回最新 review。仍維持 manual-first 的 boundary 是：若 archive-file safety snapshot 需要舊 key，v1 不假裝它一定能自動 restore。

---

## M1 的完成定義

- 正式 migration system 已取代 ad-hoc schema bootstrapping。
- Chromium backup pipeline、staging copy、dedupe、manifest、snapshot、watermark 正常工作。
- Schedule 的 macOS preview / manual / apply / status command surface已落地，desktop-vs-preview 邊界與 trust copy 也已寫清楚。
- Plaintext / Encrypted archive init、keyring session basics、security status、rekey preview 與 snapshot-backed execute foundation 已可用；後續更廣的 platform / release validation 留在 M4。
- saved raw-source checkpoint 的 preview / replay restore 已可用；archive-file safety snapshot 則維持 audit-visible、manual-first recovery。
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
- [x] `M1-004` 無 AI 配置下，產品已可完成 onboarding、備份、搜尋、匯出、查看 audit，且 schedule / security trust signoff 與 preview boundary 已收斂回 source docs。（2026-04-07，`WORK-QC-B`）
