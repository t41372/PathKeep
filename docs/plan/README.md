# PathKeep — 工作計劃與進度追蹤

> **Status:** Living document · **Created:** 2026-04-06
> 
> 本目錄是 PathKeep 的 **WBS（Work Breakdown Structure）** 計劃中心。
> 所有待辦任務都以原子化的方式拆分，按里程碑和功能域組織。

---

## 如何使用本文檔

1. **看全局進度** → 看本頁的里程碑摘要表
2. **深入某個里程碑** → 點擊對應的里程碑連結
3. **深入某個具體功能** → 從里程碑文檔跳轉到具體的 task section
4. **了解產品願景和需求** → 看 [vision-and-requirements.md](../vision-and-requirements.md)
5. **了解設計方向** → 看 `reference/PathKeep — Desktop UI Design/` 設計稿

---

## 當前狀態摘要

### 代碼庫現狀

| 層面 | 現狀 | 需要的動作 |
|------|------|------------|
| **Rust backend (vault-core)** | 有大量既有代碼（archive, chrome, takeout, ai, insights 等），但實現粗糙、耦合重、schema 不完全符合新需求 | 大幅重構 — 見各里程碑詳細計劃 |
| **Rust workspace** | vault-core / vault-platform / vault-worker 三 crate 結構存在 | 需要新增 `browser-history-parser` 獨立 crate，重組模塊邊界 |
| **前端 UI** | 舊版 UI 已部分模塊化（React 19 + components），但 UX 設計詭異 | **全部打掉重寫**，照設計師新版設計稿來 |
| **Schema** | `archive-schema.sql` 存在，基本結構可用 | 需對齊新需求：增加 run 類型欄位、軟刪除、FTS5、schema_migrations、enrichment 表等 |
| **測試** | 有 Vitest + Rust test 基礎設施 | 新代碼必須 100% coverage + mutation test |
| **CI/CD** | 有 GitHub Actions 和 local verify script | 需補齊多平台 matrix |
| **文檔** | 全新的 vision + 需求文檔已完成 | 實現和文檔同步更新 |

### 設計資產

設計師已完成完整的 UI 設計 prototype，位於 `reference/PathKeep — Desktop UI Design/`。
設計稿覆蓋所有主要畫面：Dashboard、Explorer、Insights、AI Assistant、Import、Audit Ledger、Schedule、Security、Settings。

---

## 里程碑總覽

| 里程碑 | 主題 | 狀態 | 詳細計劃 |
|--------|------|------|----------|
| **M0** | 重構基礎 — 清理舊代碼、建立新架構骨架 | `[ ]` 未開始 | [m0-foundation.md](m0-foundation.md) |
| **M1** | Solid Archive — 核心備份做對 | `[ ]` 未開始 | [m1-solid-archive.md](m1-solid-archive.md) |
| **M2** | Recall & Trust — 多源導入、回滾、完整性 | `[ ]` 未開始 | [m2-recall-and-trust.md](m2-recall-and-trust.md) |
| **M3** | Intelligence — AI 語義搜尋與洞察 | `[ ]` 未開始 | [m3-intelligence.md](m3-intelligence.md) |
| **M4** | Full Intelligence & Polish | `[ ]` 未開始 | [m4-full-polish.md](m4-full-polish.md) |

> **M0 是新增的里程碑**，專門處理從舊代碼庫到新架構的過渡。這在原始 milestones.md 中沒有，但在實際工作中是必要的第一步。

---

## 進度符號說明

- `[ ]` — 未開始
- `[/]` — 進行中
- `[x]` — 已完成
- `[~]` — 部分完成 / 需要修改
- `[!]` — 阻塞 / 需要決策

---

## 與其他文檔的關係

```
docs/vision-and-requirements.md  ← 產品願景 hub（WHY + WHAT）
  ├── docs/architecture/         ← 技術架構決策
  ├── docs/features/             ← 功能需求詳細規格
  ├── docs/design/               ← UX 設計原則與畫面結構
  ├── docs/milestones.md         ← 里程碑概要
  ├── docs/standards.md          ← 品質標準
  └── docs/plan/                 ← 工作計劃與進度追蹤（HOW + WHEN）  ← 你在這裡
       ├── README.md             ← 本文件：計劃總覽
       ├── m0-foundation.md      ← M0: 重構基礎
       ├── m1-solid-archive.md   ← M1: Solid Archive
       ├── m2-recall-and-trust.md← M2: Recall & Trust
       ├── m3-intelligence.md    ← M3: Intelligence
       └── m4-full-polish.md     ← M4: Full Intelligence & Polish
```
