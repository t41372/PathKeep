# M0-RQ — Rename, Quality, And Rewrite Discipline

> 讀這份文檔的時機：當你要在 M0 期間清掉舊產品命名、刪掉舊驗收目標、重設重寫期品質規則時。  
> 這份文檔的核心是「怎麼直接把錯的骨架換掉，同時維持重寫紀律」。

---

## 核心立場

- **M0 是直接重寫**。舊架構不保留長期並存窗口。
- **沒有臨時兼容方案**。既然沒有正式用戶，就不要為了保護舊 shell、舊命名、舊 smoke target、舊 data-dir 敘事而投入額外成本。
- **沒有故意接受紅燈中間態**。可以在本地短暫拆壞，但不能把「typecheck 預期失敗」「先關掉再說」當成正式 checkpoint。
- **work block 粒度放大，但 commit 不必跟著失控**。`STATUS.md` 的單位是半個 milestone 的 work block；實作仍應拆成可 review 的 commit。
- **品質要求分兩層**：
  - 舊碼：M0 重寫期間不再用 repo-wide coverage / mutation gate 阻塞。
  - 新碼或整段重寫的模組：必須有測試，且該 slice 要達到 100% coverage + mutation verification。

---

## Rewrite-Phase Quality Policy

### Day-one blocking gates

- `bun run check`
- `bun run build`
- 與本 work block 直接相關的 targeted tests（unit / integration / e2e smoke / Rust crate tests）

目前已固化並納入 blocking path 的 non-UI JS 驗收組合：

- `bun run test:unit:desktop-contract`
- `bun run coverage:js:desktop-contract`
- `bun run mutation:js:desktop-contract`
- `bun run test:e2e`

frontend shell / route / sidebar / primitives 的驗收必須由前端 owner 補 dedicated tests 或 visual review；不能再把 desktop contract gate 當成 UI 已完成的證據。

### 暫時拿掉的 blocking gate

- repo-wide JS coverage
- repo-wide Rust coverage
- 對整個 monorepo 做的 mutation sweep

這些深度檢查不是被否定，而是**暫時不當作 M0 重寫期間的 blocking CI gate**。原因很簡單：現在的覆蓋率和 mutation 成本，仍高度受 legacy code 形狀影響；如果還把它們掛在整倉 gate 上，團隊會被迫保護舊架構，而不是刪掉它。

### 不能退讓的底線

- 新建模組、或被整段重寫的模組，不得裸奔進主幹。
- JS/TS 新碼要有 colocated tests，該 slice 的 statement / branch / function / line coverage 要到 100%。
- Rust 新碼要有對應 unit / integration tests；如果該模組可獨立量測，也要把該 slice 的 coverage 補到 100%。
- 新碼或重寫模組需要 mutation verification。M0 期間可以先用 targeted / manual 的方式跑，不要求整倉 sweep；但沒有 mutation 驗證，就不能宣稱這個 slice 完成。
- 驗證舊產品假設的測試應直接刪除或重寫，不保留作「暫時 safety blanket」。

---

## Quick-Start Implementation Guide

### Step 1: One-shot rename cleanup

**要讀的文檔**

- `docs/plan/program/repo-baseline.md` 的命名與品質基線段落
- `README.md`
- `package.json`
- `src-tauri/tauri.conf.json`
- `.github/workflows/*.yml`

**要做的事**

- 一次性清掉 repo 中對外可見的 `Browser History Backup` / `Chrome History Backup` / `Chrome History Vault` 舊名字串。
- package、Tauri config、bundle metadata、README、workflow artifact names、release notes 全部改成 PathKeep。
- 不為舊 app root、舊 shell 名稱、舊 smoke 文案建立長期兼容窗口；真的需要保留的，必須是「保資料」而不是「保舊敘事」。

**驗收**

```bash
rg -n "Browser History Backup|Chrome History Backup|Chrome History Vault|browser-history-backup|browser_history_backup" \
  . \
  --glob '!node_modules/**' \
  --glob '!src-tauri/target/**' \
  --glob '!.git/**'
```

上面命令只允許在歷史紀錄或明確 legacy reference 文檔中命中；不得再出現在現行產品敘事與 build metadata。

### Step 2: Reset rewrite-time quality rules

**要讀的文檔**

- `docs/standards.md`
- `.github/workflows/ci.yml`
- `.github/workflows/mutation.yml`
- `package.json`

**要做的事**

- 把 repo-wide coverage 從 blocking CI 拿掉，避免重寫期被舊碼覆蓋率反向綁架。
- 保留 lint / typecheck / test / build / current smoke 的 blocking gate。
- 把「新碼 100% coverage + mutation」寫進 standards、AGENTS 和 M0 文檔，作為 work block 完成條件。
- 把 mutation sweep 保持為 manual / scheduled deep check，而不是每次都卡主線。

**驗收**

- `docs/standards.md`、`AGENTS.md`、`.github/workflows/ci.yml` 三者對同一套規則沒有互相打架。

### Step 3: Delete legacy checkpoints, not preserve them

**要讀的文檔**

- `docs/plan/m0-foundation/frontend-shell-and-design-system.md`
- `docs/plan/m0-foundation/backend-and-data-rearchitecture.md`

**要做的事**

- 刪掉「先保留舊 shell 一段時間」「分好幾段才切換」「新舊資料路徑一起活著」「壞了就回舊主幹」之類的計劃敘事。
- 把舊頁面、舊 shell、舊 smoke tests、舊 naming scaffolding 視為刪除對象，不再寫成中繼資產。
- detailed work package 可以描述刪除順序，但不能把舊新並存當成正式設計。

**驗收**

- `docs/plan/` 中不再把 M0 描述為中繼式切換計劃。

### Step 4: Keep the tree green at each checkpoint

**要做的事**

- 大 work block 允許跨多個 commit，但每個提交點都應盡量維持可驗證狀態。
- 如果某一步驟必然造成大面積紅燈，就不把它當成獨立 checkpoint；把刪除和替換合併在同一個 work session 裡完成。

**驗收**

```bash
bun run check && bun run build
```

---

## Source Inputs

- [../../vision-and-requirements.md](../../vision-and-requirements.md)
- [../../standards.md](../../standards.md)
- [../program/repo-baseline.md](../program/repo-baseline.md)
- [../program/research-and-decisions.md](../program/research-and-decisions.md)
- [frontend-shell-and-design-system.md](frontend-shell-and-design-system.md)
- [backend-and-data-rearchitecture.md](backend-and-data-rearchitecture.md)

---

## 本工作包要交付什麼

- PathKeep 命名的一次性清理策略
- 重寫期 quality policy（哪些是 blocking，哪些暫時不是）
- 刪舊代碼、刪舊測試、刪舊文案的直接規則
- 可支撐 M0 前後半 work blocks 的驗證紀律

---

## WBS

### Rename Cleanup

- [x] `M0-RQ-RN-001` 一次性盤點並清除 package、README、Tauri config、workflow、release artifact、bundle metadata 中的舊產品名字串。
- [x] `M0-RQ-RN-002` 凍結 day-one 公開產品名稱、bundle name、app root、資料目錄名稱、CLI / MCP 名稱策略，直接對齊 PathKeep。
- [x] `M0-RQ-RN-003` 清理 schedule / launch agent / task scheduler / systemd artifact 的名稱敘事，不再保留舊品牌詞。

### Rewrite-Time Quality

- [x] `M0-RQ-QA-001` 把 repo-wide coverage 從 blocking CI 拿掉，並保留 lint / typecheck / unit / Rust / build gate。
- [x] `M0-RQ-QA-002` 在 `docs/standards.md`、`AGENTS.md`、M0 文檔中明確寫下：新碼與整段重寫模組必須 100% coverage + mutation verification。
- [x] `M0-RQ-QA-003` 重寫 Playwright smoke 驗收目標，只驗證新 shell / onboarding / dashboard / navigation，不再保護舊 setup shell。
- [x] `M0-RQ-QA-004` 為 docs-only、frontend-only、rust-core-only work block 定義最小必跑驗證組合。

### Rewrite Discipline

- [x] `M0-RQ-RW-001` 從 plan 文檔中刪除中繼式切換、舊新並存與回舊主幹敘事。
- [x] `M0-RQ-RW-002` 規定 detailed step 不得把「typecheck 預期失敗」或「先紅燈再補」寫成正式 acceptance。
- [x] `M0-RQ-RW-003` 更新 `STATUS.md` / `BACKLOG.md` 的粒度，改成半個 milestone 的 work blocks，而不是原子 task。

---

## Exit Artifacts

- PathKeep 命名清理清單
- rewrite-phase quality policy
- 更新後的 CI / standards / AGENTS 對齊結果
- 直接重寫語言一致的 M0 planning docs
