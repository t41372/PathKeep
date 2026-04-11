# 技術棧與平台

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出，方便架構決策查閱。

---

## 技術棧

| 層面            | 選型                                                       | 理由                                                                     |
| --------------- | ---------------------------------------------------------- | ------------------------------------------------------------------------ |
| 桌面框架        | Tauri 2                                                    | 跨平台、Rust 核心、輕量級                                                |
| 核心邏輯        | Rust workspace（vault-core, vault-worker, vault-platform） | 高性能、安全、跨平台                                                     |
| 瀏覽器解析      | `browser-history-parser` — 計劃獨立發布的 Rust crate       | 通用的瀏覽器歷史紀錄解析，可供社區使用                                   |
| 前端            | React 19 + TypeScript + Vite                               | 現代前端、型別安全                                                       |
| 工具鏈          | Bun                                                        | JS 側的包管理與腳本                                                      |
| 數據存儲        | SQLite（可選 SQLCipher 加密）                              | 本地優先、20 年持久性                                                    |
| Secret storage  | `keyring-core` + platform-native stores                    | 保持 native keyring truth，避免把多餘的 fallback runtime 打進桌面 binary |
| 全文搜尋        | SQLite FTS5                                                | 核心召回能力，不依賴外部服務                                             |
| 向量 / 語義檢索 | LanceDB sidecar                                            | 嵌入式、Rust 原生、disk-based indexing                                   |
| AI 框架         | rig.rs                                                     | Rust 原生的 LLM + Embedding 框架                                         |
| AI 推理         | 本地推理（Ollama / LM Studio）或雲端 API                   | 可選、可配置                                                             |
| 審計            | Git（只管理 manifests 和審計工件）                         | 可追溯性                                                                 |

## 數據庫分層架構

正式的數據庫選型決策見 [database-selection-decision-2026-04-05.md](../database-selection-decision-2026-04-05.md)。核心原則：

- **Canonical archive：SQLite / SQLCipher** — 唯一的 source of truth。
- **全文召回：SQLite FTS5** — 核心功能，不是 AI 附件。
- **向量 / 語義檢索：LanceDB sidecar** — 可替換的衍生狀態，使用 rig.rs 驅動 embedding pipeline。
- **重型分析：DuckDB（延後引入）** — 只在 SQLite 被證明不夠用時才加入，作為可重建的 analytics mart。

> **鐵律**：Canonical source of truth 永遠只在 SQLite 中。Embedding、向量索引、topic cluster、生成摘要等 AI 資產都是可刪除、可重建的衍生狀態。

## AI 框架決策：rig.rs

| 維度          | 說明                                                                                     |
| ------------- | ---------------------------------------------------------------------------------------- |
| 為什麼 rig.rs | Rust 原生 LLM/Embedding 框架，與我們的 Rust workspace 自然整合                           |
| Embedding     | 通過 rig.rs 統一的 provider abstraction 調用，支援 Ollama / OpenAI-compatible / 雲端 API |
| LLM           | 同上，用於摘要生成、topic 命名、問答等 Intelligence 功能                                 |
| 向量存儲      | rig.rs 產生的 embedding 存入 LanceDB sidecar                                             |

2026-04-07 implementation note：

- day-one provider matrix 採 request-format aware contract：OpenAI-compatible、Google、Ollama、LM Studio 同時支援 chat / embedding preset；Anthropic 目前只作 chat preset。
- semantic index 的 operational metadata 不放在 sidecar 內，而是回寫 canonical archive 的 SQLite：`ai_jobs` 保存 queue lifecycle，`ai_index_ledger` 保存 sidecar version / provider / model / source watermark / last run。
- 目前的 LanceDB 依賴鏈會經由 `tantivy 0.24.2` transitively 拉入 `lru 0.12.x`。RustSec `RUSTSEC-2026-0002` 影響的是 `IterMut`; PathKeep 目前只經由 tantivy `StoreReader` 使用 cache 的 `get` / `put` / `len` / `peek_lru` 路徑，因此暫時保留 allowlist，待上游提供兼容升級。

2026-04-10 backend size audit note：

- macOS secret storage 現在直接接 `apple-native-keyring-store`；Windows / Linux / FreeBSD 則由各自的 native store crate 接到 `keyring-core` default store。桌面 app 不再透過 umbrella `keyring` crate 把未使用的 `db-keystore` / `turso` fallback 一起編進 macOS binary。
- release profile 現在固定使用 `strip = "symbols"`、`lto = "thin"`、`codegen-units = 1`。這不是用來掩飾依賴膨脹，而是把可裁切的 release metadata 與跨 crate dead code 真正裁掉。

## 目標平台

- macOS（主要開發和測試平台）
- Windows 和 Linux（第一天做好 platform adapter 設計，後續補齊完整實機驗證）

## Module Boundary

- crate / desktop facade 的責任切分見 [module-boundary-map.md](module-boundary-map.md)
- Tauri IPC draft surface 見 [desktop-command-surface.md](desktop-command-surface.md)
