# Program — 控制塔

> 這一層不直接寫功能怎麼做，而是先把整個交付節奏、依賴關係、現況認知和未決策事項講清楚。  
> 如果你現在在想「到底先做哪個、為什麼不是先做另一個」，先從這裡看。

---

## 本層包含什麼

- [repo-baseline.md](repo-baseline.md)  
  這次掃 repo 後得到的現況盤點。哪些地方還能保留，哪些地方其實該果斷重寫，都在這裡。
- [research-and-decisions.md](research-and-decisions.md)  
  目前還沒真正落地，但會直接影響實作順序的研究和技術決策待辦。
- [quality-matrix.md](quality-matrix.md)  
  當前 repo 的 blocking path、deep checks、CI/workflow 對應關係與驗收邊界。
- [traceability-map.md](traceability-map.md)  
  把 `vision / features / architecture / design` 文檔，直接映射到對應的 plan 文檔，讓讀者知道下一跳在哪裡。

---

## 這一版計劃的立場

這份計劃有幾個很明確的立場，不先寫清楚，後面會很容易回到舊習慣：

- 我們不把現有 UI 視為資產。它頂多是參考資料，不是應該努力保全的基礎。
- 我們不做「先湊合著接上」的臨時橋接。既然還沒公開發佈，就應該在 M0 把骨架換對。
- 我們不把已經提前寫進 repo 的 AI / insight / export / scheduler 功能，直接當作 roadmap 已完成。要先看它們是不是長在對的地方。
- 我們不把未定的技術問題留到實作時再說。像 schema reset、migration story、rollback visibility、sidecar 邊界，這些都先進待辦。

---

## 交付順序

1. `PG` 先把 repo 現況、決策 backlog、文檔導覽和依賴順序拉平。
2. `M0` 先切乾淨舊 UI 和舊資訊架構，建立新前端 shell、資料平面和 module boundary。
3. `M1` 把 Archive / Audit / Schedule / Security 的可信基礎打穩。
4. `M2` 再補 Import / Rollback / Doctor / 多瀏覽器 / PME / i18n / 跨平台排程。
5. `M3` 把 AI 視為可選增值層疊上去，不影響沒有 AI 的產品可用性。
6. `M4` 最後補 enrichment、進階洞察、remote backup 和公開發版前的 polish。

---

## 真正的 critical path

不是所有 todo 的重要性都一樣。真正會卡住整體交付的，是下面這幾條路：

1. 先確認新的資訊架構和 design token，不然前端會一直返工。
2. 先凍結 canonical archive schema 和 migration story，不然 backup / import / rollback 都沒有穩定地基。
3. 先拆清 `browser-history-parser`、`vault-core`、`vault-worker`、Tauri command 的責任邊界，不然之後每個功能都還是堆回巨檔。
4. 先把 run / manifest / rollback / snapshot 這些操作模型做對，再談 intelligence。
5. 先讓沒有 AI 配置的 PathKeep 是完整可用的，再把 optional intelligence 疊上去。

---

## 里程碑 gate

| Gate | 進入條件                 | 完成條件                                                             |
| ---- | ------------------------ | -------------------------------------------------------------------- |
| `PG` | vision / docs 已重寫完成 | baseline、研究 backlog、traceability、分層 WBS 都建立完成            |
| `M0` | `PG` 完成                | 新 shell 可跑、舊 UI 不再是主流程、parser/core/worker 邊界定稿       |
| `M1` | `M0` 完成                | backup、migration、manifest、schedule、security、Explorer v1 可驗收  |
| `M2` | `M1` 完成                | import、rollback、Doctor、i18n、跨平台排程可驗收                     |
| `M3` | `M2` 完成                | optional AI pipeline、semantic search、assistant、insights v1 可驗收 |
| `M4` | `M3` 完成                | enrichment、advanced insights、remote backup、release readiness 完成 |

---

## Program 級別 checklist

- [ ] `PG-001` 維持 `docs/plan/` 為 implementation truth；需求改了之後要同步回寫 plan，不能讓 plan 默默過期。
- [ ] `PG-002` 建立「需求來源 → 設計稿 → work package → 測試驗收」的 traceability。
- [ ] `PG-003` 把缺少設計稿的畫面先補設計決策，再開做對應實作。
- [ ] `PG-004` 在 M0 結束前，完成「哪些舊代碼保留作 reference、哪些正式淘汰」的判定。
- [ ] `PG-005` 在 M1 開始前，確認 migration / rollback / snapshot / manifest 四個操作模型已經一致。
- [ ] `PG-006` 在 M3 開始前，確認 AI provider 未配置、index 被刪除、AI pipeline 失敗三種情況都不會影響核心 archive。
- [ ] `PG-007` 每個 milestone 結束時回寫 docs。如果實際決策和 vision 子文檔不同，先修文檔，再進下一階段。
