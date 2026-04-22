# 技術棧與平台

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出，方便架構決策查閱。

---

## 技術棧

| 層面            | 選型                                                           | 理由                                                                     |
| --------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------ |
| 桌面框架        | Tauri 2                                                        | 跨平台、Rust 核心、輕量級                                                |
| 核心邏輯        | Rust workspace（vault-core, vault-worker, vault-platform）     | 高性能、安全、跨平台                                                     |
| 瀏覽器解析      | `browser-history-parser` — 計劃獨立發布的 Rust crate           | 通用的瀏覽器歷史紀錄解析，可供社區使用                                   |
| 前端            | React 19 + TypeScript + Vite                                   | 現代前端、型別安全                                                       |
| 工具鏈          | Bun                                                            | JS 側的包管理與腳本                                                      |
| 本地瀏覽器驗證  | Chrome + Playwright + feature-gated desktop bridge             | 讓 AI agent / local QA 直接在瀏覽器驗證真實 Rust command surface         |
| 數據存儲        | SQLite storage planes（可選 SQLCipher 加密 canonical archive） | 本地優先、20 年持久性                                                    |
| Secret storage  | `keyring-core` + platform-native stores                        | 保持 native keyring truth，避免把多餘的 fallback runtime 打進桌面 binary |
| 全文搜尋        | SQLite FTS5                                                    | 核心召回能力，不依賴外部服務                                             |
| 向量 / 語義檢索 | LanceDB sidecar                                                | 嵌入式、Rust 原生、disk-based indexing                                   |
| AI 框架         | rig.rs                                                         | Rust 原生的 LLM + Embedding 框架                                         |
| AI 推理         | 本地推理（Ollama / LM Studio）或雲端 API                       | 可選、可配置                                                             |
| 審計            | Git（只管理 manifests 和審計工件）                             | 可追溯性                                                                 |

## 數據庫分層架構

正式的數據庫選型決策見 [database-selection-decision-2026-04-05.md](../database-selection-decision-2026-04-05.md)。核心原則：

- **Canonical archive：`archive/history-vault.sqlite` / SQLCipher** — 唯一的 source of truth。
- **全文召回：`derived/history-search.sqlite` + SQLite FTS5** — 核心功能，不是 AI 附件。
- **Intelligence runtime：`derived/history-intelligence.sqlite`** — queue、assistant trace、deterministic read model、enrichment metadata 與 compact semantic metadata / rebuild accounting。向量 payload 不進 SQLite。
- **向量 / 語義檢索：LanceDB sidecar** — 可替換的衍生狀態，使用 rig.rs 驅動 embedding pipeline。
- **重型分析：DuckDB（延後引入）** — 只在 SQLite 被證明不夠用時才加入，作為可重建的 analytics mart。

> **鐵律**：Canonical source of truth 永遠只在 `archive/history-vault.sqlite` 中。FTS、assistant trace、deterministic projections、embedding、向量索引、topic cluster、生成摘要等都屬可刪除、可重建的衍生狀態。

## AI 框架決策：rig.rs

| 維度          | 說明                                                                                     |
| ------------- | ---------------------------------------------------------------------------------------- |
| 為什麼 rig.rs | Rust 原生 LLM/Embedding 框架，與我們的 Rust workspace 自然整合                           |
| Embedding     | 通過 rig.rs 統一的 provider abstraction 調用，支援 Ollama / OpenAI-compatible / 雲端 API |
| LLM           | 同上，用於摘要生成、topic 命名、問答等 Intelligence 功能                                 |
| 向量存儲      | rig.rs 產生的 embedding 存入 LanceDB sidecar                                             |

2026-04-07 implementation note：

- day-one provider matrix 採 request-format aware contract：OpenAI-compatible、Google、Ollama、LM Studio 同時支援 chat / embedding preset；Anthropic 目前只作 chat preset。
- semantic index 的 operational metadata 與 intelligence runtime state 不再回寫 canonical archive；它們進 `derived/history-intelligence.sqlite`，與 hot archive facts 隔離。
- 目前的 LanceDB 依賴鏈會經由 `tantivy 0.24.2` transitively 拉入 `lru 0.12.x`。RustSec `RUSTSEC-2026-0002` 影響的是 `IterMut`; PathKeep 目前只經由 tantivy `StoreReader` 使用 cache 的 `get` / `put` / `len` / `peek_lru` 路徑，因此暫時保留 allowlist，待上游提供兼容升級。
- 2026-04-21 follow-up：`rustls-webpki` 已升到 `0.103.13`，先前為 `0.103.10` 留下的 `RUSTSEC-2026-0098` / `RUSTSEC-2026-0099` supply-chain allowlist 已移除；目前的 security allowlist 只保留仍存在於實際依賴圖中的 advisories。
- 2026-04-22 follow-up：`core2 0.4.0` 先前只以 cargo-audit `yanked` warning 出現，現在已升格成 `RUSTSEC-2026-0105` unmaintained advisory。它仍只經由 `libsodium-sys-stable -> libflate` 出現在 Stronghold build 依賴鏈裡；PathKeep 的 owned code surface 不直接依賴 `core2`，而 upstream 目前也尚未提供可直接替換的 maintained path，因此 allowlist 轉為 advisory 形式並保留 provenance。

2026-04-10 backend size audit note：

- macOS secret storage 現在直接接 `apple-native-keyring-store`；Windows / Linux / FreeBSD 則由各自的 native store crate 接到 `keyring-core` default store。桌面 app 不再透過 umbrella `keyring` crate 把未使用的 `db-keystore` / `turso` fallback 一起編進 macOS binary。
- release profile 現在固定使用 `strip = "symbols"`、`lto = "thin"`、`codegen-units = 1`。這不是用來掩飾依賴膨脹，而是把可裁切的 release metadata 與跨 crate dead code 真正裁掉。

2026-04-10 packaging boundary note：

- 使用者已在 2026-04-10 明確 sign off：default desktop build 維持把 optional AI / MCP / semantic runtime 與 archive / shell-critical flow 一起 shipping；`optional` 指 disabled-by-default + provider / consent gated，不是第一次使用時再安裝另一個 helper。
- `LanceDB sidecar` 仍是 semantic index data sidecar，而不是獨立 code/runtime helper。相關 packaging 邊界已由 [ADR-009](decisions/009-default-desktop-optional-intelligence-shipping.md) 凍結。

## 目標平台

- macOS（主要開發和測試平台）
- Windows 和 Linux（第一天做好 platform adapter 設計，後續補齊完整實機驗證）

2026-04-10 local automation note：

- `devtools-bridge` 是 **開發期 feature-gated surface**，只在 `bun run desktop:dev:bridge` 或 `tauri dev --features devtools-bridge` 時啟用。
- 這條 bridge 只把 typed desktop command facade 鏡射到 localhost，方便 Chrome / Playwright / CDP 調試；它不是 shipping API，也不應被當成 plugin sandbox 或 remote control surface。

## Module Boundary

- crate / desktop facade 的責任切分見 [module-boundary-map.md](module-boundary-map.md)
- Tauri IPC draft surface 見 [desktop-command-surface.md](desktop-command-surface.md)
