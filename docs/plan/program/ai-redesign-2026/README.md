# AI Redesign 2026

> 這是 PathKeep AI 接入的 **乾淨重做（clean-slate redesign）** 提案集，獨立於主線開發（在 `feat/ai-redesign-2026` worktree 上工作）。
>
> 既有的 `rig.rs` / `LanceDB` / `ai_sidecar` 等 AI 相關決策與實作，**僅供參考、不具約束力**——使用者已明確指示重新設計。
> 仍然具約束力的，是與 AI 無關的產品事實：性能信封、storage-plane truth model、native-dependency / supply-chain 規則、deterministic-intelligence optional 邊界、local-first / data sovereignty、以及「文案不寫具體 model id」。

## 文檔順序

| 文檔                                                                   | 內容                                                                                                          | 狀態 |
| ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- | ---- |
| [00-scope-use-cases-constraints.md](00-scope-use-cases-constraints.md) | 需求、use case、要設計的子系統、硬約束、decision forks、可重用既有基礎設施                                    | ✅   |
| [01-research-findings-2026.md](01-research-findings-2026.md)           | 2026-06 SOTA 深度研究結論（LLM / embedding / vector store / agent / hybrid search），由獨立 subagent 集群產出 | ✅   |
| [02-architecture-decisions.md](02-architecture-decisions.md)           | 選定方案、理由、tradeoff、邊界、benchmark gate（forks 已鎖定 2026-06-20）                                     | ✅   |
| [03-implementation-plan.md](03-implementation-plan.md)                 | 早期 spike、分階段 milestones、依賴、風險登記、驗收                                                           | ✅   |
| [research-appendix/](research-appendix/)                               | 8 領域原始研究 digest（candidates / deep-dive / adversarial critic）                                          | ✅   |

## 工作流程（本提案如何產生）

1. **Frame** ✅：盤點現況、鎖定硬約束 vs. 開放議題，定義研究 charges。
2. **Research** ✅：8 個研究領域，各走 _landscape → deep-dive → adversarial critic_，由獨立 subagent 集群（39 agents，workflow `wf_18f54e54-e57`）以 web research 取得 2026-06 最新方案與最佳實踐。
3. **Decide** ✅：綜合研究結論，與使用者鎖定 4 個高槓桿 decision forks（2026-06-20）。
4. **Plan** ✅：產出可執行的分階段實作計畫（03）。

下一步（需使用者啟動）：把 03 的 milestones 拆進 `STATUS.md` work blocks；先跑 S1/S2 兩個 benchmark spike。本分支 `feat/ai-redesign-2026` 的變更尚未 commit。
