# AI Redesign 2026 — Current-State Reconciliation & Execution Sequence

> 承 [02-architecture-decisions.md](02-architecture-decisions.md) + [03-implementation-plan.md](03-implementation-plan.md)。
> **這份文檔是所有 AI 實作 / review subagent 的第一份「讀先」。** 它把鎖定的架構決策對齊到 `main`（intelligence-overhaul squash-merge 後）的**真實代碼狀態**——02/03 與舊 memory 寫於 overhaul 之前，有數處與現實不符，以本文檔為準。
>
> 產出依據：2026-06-20 對現行 tree 的 7-charge 平行 recon（existing AI surface / job queue / storage planes / IPC / frontend / test-gate / supply-chain）。所有 file:line anchor 以當時 `main` 為基準，動手前自行覆核。

---

## 0. 與 02/03/memory 的現實差異（先讀，避免被舊假設誤導）

| 舊假設（02/03/memory） | 現實（main 上的真實狀態） | 影響 |
| --- | --- | --- |
| rig 已被移除、reference-only | **`rig-core 0.34.0` 已是 workspace dep 且被 `vault-core/src/ai.rs` + `ai/provider.rs` 實際使用**（openai/anthropic/gemini client + agent + embedding 呼叫） | LLM/embedding transport **大部分已存在**。M-AI1 是「收進 trait + 補 streaming + 清掉洩漏」，不是從零接入 |
| rmcp 待引入 | **`rmcp 1.5.0`（locked）已在 `vault-worker/src/mcp.rs` 使用** | MCP 對外邊界 scaffold 已存在 |
| LanceDB 是既有 reference 代碼 | **LanceDB / lance / arrow / datafusion 從未存在於本 tree**（`Cargo.lock` 零命中）。向量層是 `ai_sidecar.rs` 的**蓄意 stub**：`sync_provider_embeddings` 對非空輸入直接 `bail!("...deferred to v0.3.0")`，search/count 回 `None`/`0` | 向量儲存 + embedding 持久化是**真正全新**工作 |
| embedding backfill 待建 | `build_ai_index_with_control`（`vault-core/src/ai/indexing.rs:62-224`）**已有 chunked 讀取 + cancellation checkpoint + content-hash diff**，但在 `indexing.rs:170` `anyhow::bail!`——**embed 迴圈本體未實作** | backfill 是「補完迴圈 + 接 EmbeddingProvider + 持久化」，地基已在 |
| config 需重寫為 model-agnostic | `AiProviderConfig`/`AiSettings`（`vault-core/src/models/intelligence.rs:250-341`）**形狀已 model-agnostic**（model id = 自由字串、`dimensions: Option<u32>`） | 不需重寫；但 ↓ |
| — | **D4 違反**：`provider.rs:445-528` 在 `dimensions=None` 時**硬編 1536（openai/ollama/lm-studio）/ 768（google）fallback** | 必修：改成**讀實際回傳向量長度**（02 §C.3 鐵律 a） |
| agent durability 待建 | `ai_jobs` lease-based queue（`vault-core/src/ai_queue.rs`）真實存在，但 **resumability 是 job 粒度**——中斷的 `running` job 整個 requeue 重跑，無 journal/replay | M-AI5 必補 journal-before-observe / replay bytes |
| assistant UI 待建 | assistant page 真實存在（`src/pages/assistant/index.tsx`）但**job-based 輪詢**（`askAiAssistant`→job→`loadAiAssistantJob`），**無 streaming / 無 markdown lib / 無 reasoning / 無 tool-use UI**。paper atoms 已有：`PaperAssistantView/Message/Composer/Greeting` | 串流 + reasoning + tool-use + chat history 全新；evidence/citation panel（`onSelectEntry`）可復用 |

---

## 1. 已存在且 LIVE 的契約（DO NOT BREAK；改動需對齊）

- **資料模型**：`AiProviderConfig` / `AiSettings`（`vault-core/src/models/intelligence.rs:250-341`），TS 鏡像 `src/lib/types/intelligence.ts`（手寫、`camelCase` 上線）。
- **12 個 Tauri 命令**（註冊於 `src-tauri/src/lib.rs:158-221`，façade `src-tauri/src/commands/intelligence/ai.rs`，FE `src/lib/backend-client/intelligence.ts:48-114`）：`store_ai_provider_api_key`、`clear_ai_provider_api_key`、`test_ai_provider_connection`、`load_ai_queue_status`、`run_ai_queue_jobs`、`replay_ai_job`、`cancel_ai_job`、`load_ai_assistant_job`、`build_ai_index`、`search_ai_history`、`ask_ai_assistant`、`preview_ai_integrations`。
- **`AppSnapshot` AI 欄位**：`AiIndexStatus` / `AiQueueStatus` / `AiSettings`（worker `context.rs:93`）——FE 多處消費（settings/ai-providers-section、assistant、explorer runtime panels）。
- **browser-preview fixture**：`src/lib/backend.ts` + `src/lib/backend-preview-ai-commands.ts` 是**凍結**的瀏覽器預覽契約，鏡像上述命令——**不擴展其 contract**（AGENTS.md 受保護入口規則）。
- **受保護入口**：`src/main.tsx`、`src/lib/ipc/bridge.ts`（mutation-gated 100% 覆蓋）。`invokeCommand` 已 command-agnostic → 加新命令**不需**改 bridge.ts。
- **release gate**：`optionalAiFeaturesAvailable`（`src/lib/release-capabilities.ts:30`）目前 `false`；`evaluateOptionalAiAvailability()` 控制可用性。新 UI 必須尊重此 gate 並優雅降級（intelligence-optional 契約）。

**策略**：保持上述 12 命令 + snapshot 欄位**形狀穩定**；在其後/其下替換實作（stub sidecar、硬編 dim、metadata-only schema 都可自由換）。新增能力以**新命令 + 新事件**交付，不破舊。

---

## 2. 真正缺口（要做的工）

1. **向量儲存**（全新）：`VectorIndex` trait + 引擎；取代 `ai_sidecar.rs` stub。
2. **embedding 持久化 + backfill 補完**（`indexing.rs:170` bail）：接 `EmbeddingProvider`、寫 sidecar、chunked/resumable job。
3. **in-app embedding 引擎**（candle，全新）——**可延後**（先用 LM Studio 外部 embedding 跑通全鏈）。
4. **streaming transport**：rig `chat_stream` → Tauri event `pathkeep://ai-stream`（token/reasoning/tool-call）。事件基建已有（import/backup/updater 用 `AppHandle::emit` + FE `listen`）；AI 尚未接。
5. **頂尖 AI 前端可觀測性**（全新，marquee）：streaming markdown、reasoning chain、tool-use 可視、chat history 持久化 + explorer。
6. **agent harness durability**（journal/replay/idempotency/PME long-pause）。
7. **hybrid + rerank**、**code-mode（Wasmtime+Javy）**、**MCP 對外面 + skills**。
8. **D4 修正**（硬編 dim → runtime 偵測）、**`secrecy::SecretString`**（in-memory key 處理）、**新 storage planes**（`vectors_dir` / `agent.sqlite` / `models/`）。

---

## 3. 重排後的執行序（grounded in reality + LM Studio）

> 理由：(1) 02 的「先地基後模型」不變；(2) rig transport 已在 → external LLM 路徑可立刻跑通；(3) 本機有 **LM Studio**（LLM `google/gemma-4-26b-a4b-qat` + embedding `text-embedding-qwen3-embedding-0.6b` @ `http://localhost:1234/v1`）→ **external 路徑可立即真機 e2e**。
>
> **candle in-app embedding 與 external embedding 同步做、不延後**（使用者 2026-06-20 明示）。兩者都是 `EmbeddingProvider` 的實作，並列一等公民；**S1（embedding 吞吐）/ S2（Turbovec 規模）benchmark 是內嵌的即時 gate，不是收尾項**。LM Studio 只是 external 程式碼路徑的真機測試工具——**不是延後 in-app 推理的理由**。
>
> 鎖定的架構決策（D1-D8）**不變**；這只是 milestone 排序，不推翻任何 accepted decision。

| Work block | 對應 02/03 | 做什麼（against reality） | 可 e2e 測？ |
| --- | --- | --- | --- |
| **W-AI-0 Foundations** | M-AI0 | `LlmProvider`/`EmbeddingProvider`/`VectorIndex` traits（vault-core）；rig 收進 adapter；**修 D4 硬編 dim → 讀實際向量長度**；`secrecy::SecretString` in-memory key；新 storage planes（`vectors_dir`/`agent_database_path`/`models_dir` in `config.rs`+`ensure_paths`+tests）；embedding fingerprint struct（`hash(provider,model,dim,dtype,normalized,pooling,instruction,version)`，鏡像 intelligence watermark pattern）。**無新模型呼叫**。 | 單元 |
| **W-AI-1 Streaming external LLM** | M-AI1 | rig 藏進 `LlmProvider`；`chat_stream` → `pathkeep://ai-stream`（token+reasoning+tool-call 分型）；capability/connection probe（已有，補 streaming cap）；LLM functions 退 deterministic（已有 fallback 慣例）。 | ✅ LM Studio |
| **W-AI-2 Streaming chat UI** | M-AI1(FE) | markdown streaming（streamdown，供應鏈先 vet）；reasoning chain 折疊塊（參考 lobehub/LM Studio）；tool-use 可視塊；evidence/citation panel 復用 `onSelectEntry`；全 off-main-thread、`useViewportMount` 虛擬化、`paper.css` keyframes、i18n ×3。 | ✅ LM Studio |
| **W-AI-3 Chat history persistence + explorer** | M-AI5(部分) | `derived/agent.sqlite`（自有 migration system，鏡像 intelligence schema）存 conversations/messages；chat history explorer 前端；保存/檢視/刪除/深鏈 evidence。 | ✅ |
| **W-AI-4 Embedding：external + candle in-app（並行）+ backfill + S1** | M-AI2 | **同一 block 同時交付兩個 `EmbeddingProvider`**：(a) OpenAI-compat `/v1/embeddings` adapter（LM Studio qwen3-0.6b 真機測）；(b) **candle in-app 引擎**（純 Rust、MKL/Accelerate OFF）+ 模型下載（hf-hub、SHA-256 pin、離線、off-thread、可取消、進度）。補完 `indexing.rs:170` embed 迴圈接 `EmbeddingProvider`；正確性鐵律（讀實際 dim / 防禦 normalize / query-doc role）；chunked/resumable backfill job（擴 `ai_jobs` payload cursor + heartbeat）。**S1 吞吐 spike 內嵌即時跑**（candle+預設模型，4 核，量 docs/sec + peak RAM + 14.4M ETA）→ 決定 model2vec fast tier 回檔點。 | ✅ LM Studio + 真機 candle |
| **W-AI-5 Vector store + semantic search + S2** | M-AI3 | **先驗 Turbovec 是否在 crates.io**（見 §6）→ `VectorIndex` impl（Turbovec；不過關時 flat int8 scan fallback）；`search_vector` 回真實 rows+score；**S2 benchmark 內嵌即時跑**（1M/5M/14.4M recall@10 / 延遲 / 常駐 RAM / build 時間 + 成熟度審計）。 | ✅ |
| **W-AI-6 Hybrid + rerank** | M-AI4 | RRF(k=60)；`search_bm25`/`search_hybrid(rerank?)`；FTS5 復用避 `ORDER BY rank` 大表陷阱；rerank=candle cross-encoder lazy-load opt-in（與 embedding 共 candle 引擎）。 | ✅ |
| **W-AI-7 Agent harness durability** | M-AI5 | thin tokio loop（streaming/硬取消/budget/loop 偵測）；journal-before-observe + replay bytes + idempotency + PME long-pause；context engineering（recency prune + citation table）；sub-agents ≤2。 | ✅ LM Studio |
| **W-AI-8 Code-mode** | M-AI6 | Wasmtime+Javy 沙箱；read-only query host API；budget/epoch；capability-gated。 | ✅ |
| **W-AI-9 MCP face + skills + consent UX + i18n + hardening** | M-AI7/8 | rmcp 對外硬化；SKILL.md；consent/provider UI 三語；14.4M profiling；prompt-injection 紅隊；ADR closeouts。 | 真機 |

依賴：W-AI-0 → {W-AI-1 → W-AI-2 → W-AI-3}（external chat 可先獨立交付）；W-AI-0 → W-AI-4 → W-AI-5 → W-AI-6 → W-AI-7 → W-AI-8 → W-AI-9。**W-AI-4 內 external 與 candle in-app 兩個 `EmbeddingProvider` 並行交付（不分先後）**；S1 在 W-AI-4 內、S2 在 W-AI-5 內即時跑，皆非收尾。

---

## 4. 本機 LM Studio（e2e 真機測試用）

```
base_url:    http://localhost:1234/v1   (OpenAI-compatible，也支援 Anthropic 格式)
api_key:     任意非空字串
llm_model:   google/gemma-4-26b-a4b-qat   (temperature 0.6, reasoning level 最高)
embedding:   text-embedding-qwen3-embedding-0.6b
```
- external 路徑（W-AI-1/2/4/5/6/7）一律對此 server 跑真實 e2e（e2e 可直接在本機跑）。
- **LM Studio 只是 external 程式碼路徑的真機測試工具**；in-app **candle** embedding 在 W-AI-4 內**同步開發**，不因 LM Studio 可用而延後（使用者明示）。
- **DB 內絕不寫死這些 id**（D4/文案規則）；它們只是測試 fixture 的 runtime config。
- 注意：dev HTTP bridge **不傳遞 emit 事件**（`dispatch.rs` drop progress callback）→ streaming 只在真 Tauri 下有效；desktop-bridge e2e 驗命令、不驗事件流。

---

## 5. Review pipeline 協議（每個工作單元 + 每次合併都走）

1. **實作**：subagent（或主 agent）在隔離 worktree 完成一個工作單元，自跑 `bun run check:base`（快 triage）求綠。
2. **Review round（找問題）**：≥1 個**獨立** subagent（fresh context）依維度找問題（correctness / 性能 baseline（14.4M）/ 契約相容 / 安全(egress/secrets/injection) / coverage 缺口 / i18n 三語 / UI-UX 蘋果級 + HCI / 無主線程凍結）。
3. **Verify round（驗證 findings）**：另一組**獨立** subagent 對抗性驗證每條 finding（預設 refuted；多數驗證為真才存活），濾掉誤報。
4. **修復**：確認的 finding 直接修（主 agent 或 fix subagent），補測試到 100% 覆蓋。
5. **合併**：合進整合分支前**再走一遍** 2-4（merge review）。每階段最終 `bun run check`（macOS 上 `/dev/shm` migration test 為已知非回歸，其餘須綠）。

效率/正確性 hooks：性能敏感改動須 profiling artifact 後才宣稱「夠快」；新 `.ts/.tsx`（`src/`）與新 `.rs`（`src-tauri/**/src/`）**自動進 100% 覆蓋 gate**（無 opt-out，除 governed exclusion / `#[cfg(test)]`）。

---

## 6. 動手前必驗的供應鏈 blocker

- **Turbovec 是否發佈到 crates.io**（W-AI-5 之前）：`deny.toml` `sources.unknown-git = "deny"` → 若只在 GitHub，需 `[sources.allow-git]` + 理由，否則 `deny:rust` 紅。未發佈時 W-AI-5 先出 **flat int8 scan fallback**。
- **candle 加入後新 advisory**（W-AI-4 之前——candle 在 W-AI-4 即加入，不延後）：須 triage 進 **`deny.toml` ignore + `scripts/check-rust-security.mjs` allowedAdvisories 兩處同步**（stale 條目也會 FAIL）。MKL/Accelerate/CUDA features **一律 OFF**（可重現 build）。hf-hub 同 block 加入。
- **rig 升級時**：`RUSTSEC-2026-0097` 是 stale-trap，存在於 deny.toml + check-rust-security.mjs 兩處；rig 變動使其消失時兩處都要清。
- **streamdown（JS）**：新 npm dep，AGENTS.md 門檻（stars>6k 或 risk doc）→ W-AI-2 前 vet。
- 工具鏈：edition 2024、Rust **1.94.1**（`rust-toolchain.toml`）、`Cargo.lock` committed。新 crate 用 `name.workspace = true`（rig 依 §J **不 hard-pin**）。

---

## 7. 關鍵 file anchors（per work block 的起點）

- **traits / config / dim 修正**：`vault-core/src/ai.rs`、`ai/provider.rs:445-528`、`ai/read_model.rs`、`models/intelligence.rs:250-341`、`config.rs:26-135`（paths）。
- **storage / migration**：canonical 在 `archive/schema.rs:74-244`（max v13）；intelligence 自有 migration（`intelligence/intelligence_schema.rs`）= agent.sqlite 範本；fingerprint 範本 = `intelligence/incremental.rs` watermark。
- **job queue**：`vault-core/src/ai_queue.rs`、`indexing.rs:62-224`（backfill 地基）；worker `vault-worker/src/job_runtime.rs`、`intelligence/ai_queue.rs`、`runtime.rs`。
- **IPC / streaming**：`src/lib/ipc/bridge.ts`（凍結）、`import-progress.ts`（listen 範本）；Rust `commands/intelligence/ai.rs`、`worker_bridge/intelligence/`、`lib.rs` generate_handler、`dev_ipc_bridge/dispatch.rs`（+ off-thread hop）。emit 範本 `commands/import.rs:31`。
- **frontend**：`src/pages/assistant/index.tsx`、`components/explorer-paper/paper-assistant-*.tsx`、`src/styles/tokens.css` + `paper.css`（keyframes）、i18n `src/lib/i18n/catalog/assistant.ts`、虛擬化 `use-viewport-mount.ts`、狀態 `src/app/shell-data-context.ts`。
- **test gate**：`docs/plan/program/quality-matrix.md`；`vitest.config.ts`（100/100/100/100）；`scripts/verify-rust-coverage.mjs`；`mutation:rust:ai-helpers` 為新 AI rust mutation 範本。
