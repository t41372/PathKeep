# BACKLOG — 後續工作塊佇列

> 這裡只放 **half-milestone 粒度**的 work blocks。  
> `STATUS.md` 清空或完成當前 block 後，才從這裡把下一個未被阻塞的 block 搬上去。

---

## 工作塊佇列（按順序）

- [ ] **WORK-QC-O** — Intelligence And Jobs UX Reset Implementation
  - 讀先：
    `docs/features/intelligence-current-state.md`
    `docs/features/deterministic-intelligence.md`
    `docs/features/intelligence.md`
    `docs/design/intelligence-ui-redesign-brief.md`
    `docs/design/jobs-ui-redesign-brief.md`
    `docs/design/screens-and-nav.md`
  - 目標：把 intelligence / Jobs 目前「資料是真的，但 UI 階層與閱讀順序失控」的問題，收斂成 designer-signed-off 並真正落地的 shipping UX，而不是再往現有 panel wall 上補小修小改。
  - 契約：Insights 必須明確回到 `analysis first, runtime second`；Jobs 必須先回答 `running now / needs review / deferred backlog`；threads 要從 backend-core 概念提升成前台清楚可見的結構；queue 錯誤與 deferred backlog 必須以人話呈現，不能再把 raw runtime status 或大批待抓正文誤導成整條功能失敗。
  - 驗收：source docs、route IA、shared components / copy、以及手動 truth pass 一起更新；`bun run check && bun run build` 維持通過，並留下 designer handoff artifact / screenshots / state inventory 供後續 review。

- [ ] **WORK-QC-M** — Large-Archive Performance Envelope And Chunked Deterministic Runtime
  - 讀先：
    `docs/features/intelligence.md`
    `docs/features/deterministic-intelligence.md`
    `docs/plan/program/research-and-decisions.md`
    `docs/plan/m4-full-polish/intelligence-60-year-envelope.md`
    `docs/architecture/data-model.md`
  - 目標：把 deterministic intelligence 在大資料量下的時間 / 記憶體 / I/O 邊界做成可重跑 artifact，而不是只靠主觀體感；必要時把 full rebuild 拆成 chunked / resumable / amortized pipeline。
  - 契約：以 4 核 3GHz CPU / 8GB RAM、60 年高強度瀏覽資料、無 LLM / 無 embedding 為 baseline，對 backup 後 auto rebuild、Explorer/Insights deterministic surfaces、Jobs recovery / resume 做真實 benchmark 與 complexity audit；如現行複雜度或記憶體模型不滿足基線，需提出並實作 chunking / checkpoint / resume 策略與對應驗收。
  - 驗收：留下 benchmark artifact、source doc 更新、targeted regression / perf guard，以及至少一條能在 large-archive sample 上重跑的 honest automation / manual recipe。
  - 2026-04-12 progress：bounded joins、thread accumulator、scope/window partitioned persistence 與 replayable 100k / 1M synthetic benchmark artifact 已落地；剩餘 signoff 收斂到 10M / low-RAM envelope、queue recovery RSS、以及真實 large-profile replay。

---

## 依賴關係圖

```
WORK-M0-A ──┐
WORK-M0-B ──┴── WORK-M1-A → WORK-M1-B → WORK-M2-A → WORK-M2-B → WORK-M3-A → WORK-M3-B → WORK-QC-A → WORK-QC-B → WORK-M4-A → WORK-M4-B → WORK-M4-C / WORK-M4-D / WORK-M4-E / WORK-M4-F / WORK-M4-G / WORK-M4-H → WORK-QC-D → WORK-M4-J → WORK-M4-I → WORK-M4-K → WORK-M4-L → WORK-M5-A → WORK-M5-B
                     └──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────→ WORK-QC-C → WORK-M1-C → WORK-M1-D
```

---

## 維護規則

1. `STATUS.md` 清空時，從這裡頂部取最多 2 個未被阻塞的 work blocks
2. 新工作一律先放進這裡，除非使用者明確要求立刻調整 `STATUS.md`
3. 如果某個 block 需要再拆，只在 milestone 文檔的 WBS 裡拆，不把 `STATUS.md` / `BACKLOG.md` 再退回原子 task
4. 依賴解除後，直接更新行尾的 `[!blocked: ...]`
