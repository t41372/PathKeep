# 技術棧與平台

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出，方便架構決策查閱。

---

## 技術棧

| 層面 | 選型 | 理由 |
|------|------|------|
| 桌面框架 | Tauri 2 | 跨平台、Rust 核心、輕量級 |
| 核心邏輯 | Rust workspace（vault-core, vault-worker, vault-platform） | 高性能、安全、跨平台 |
| 瀏覽器解析 | `browser-history-parser` — 計劃獨立發布的 Rust crate | 通用的瀏覽器歷史紀錄解析，可供社區使用 |
| 前端 | React 19 + TypeScript + Vite | 現代前端、型別安全 |
| 工具鏈 | Bun | JS 側的包管理與腳本 |
| 數據存儲 | SQLite（可選 SQLCipher 加密） | 本地優先、20 年持久性 |
| 全文搜尋 | SQLite FTS5 | 核心召回能力，不依賴外部服務 |
| 向量 / 語義檢索 | LanceDB sidecar | 嵌入式、Rust 原生、disk-based indexing |
| AI 框架 | rig.rs | Rust 原生的 LLM + Embedding 框架 |
| AI 推理 | 本地推理（Ollama / LM Studio）或雲端 API | 可選、可配置 |
| 審計 | Git（只管理 manifests 和審計工件） | 可追溯性 |

## 數據庫分層架構

正式的數據庫選型決策見 [database-selection-decision-2026-04-05.md](../database-selection-decision-2026-04-05.md)。核心原則：

- **Canonical archive：SQLite / SQLCipher** — 唯一的 source of truth。
- **全文召回：SQLite FTS5** — 核心功能，不是 AI 附件。
- **向量 / 語義檢索：LanceDB sidecar** — 可替換的衍生狀態，使用 rig.rs 驅動 embedding pipeline。
- **重型分析：DuckDB（延後引入）** — 只在 SQLite 被證明不夠用時才加入，作為可重建的 analytics mart。

> **鐵律**：Canonical source of truth 永遠只在 SQLite 中。Embedding、向量索引、topic cluster、生成摘要等 AI 資產都是可刪除、可重建的衍生狀態。

## AI 框架決策：rig.rs

| 維度 | 說明 |
|------|------|
| 為什麼 rig.rs | Rust 原生 LLM/Embedding 框架，與我們的 Rust workspace 自然整合 |
| Embedding | 通過 rig.rs 統一的 provider abstraction 調用，支援 Ollama / OpenAI-compatible / 雲端 API |
| LLM | 同上，用於摘要生成、topic 命名、問答等 Intelligence 功能 |
| 向量存儲 | rig.rs 產生的 embedding 存入 LanceDB sidecar |

## 目標平台

- macOS（主要開發和測試平台）
- Windows 和 Linux（第一天做好 platform adapter 設計，後續補齊完整實機驗證）
