# ADR-009 — Default Desktop Build Ships Optional Intelligence In-Process

## 狀態

Accepted (2026-04-10)

## 背景

`WORK-QC-E` 已先把 macOS release executable 從約 `190M` 降到約 `104M`，證明上一輪最嚴重的膨脹並不是前端 bundle，而是桌面 binary 本體混進了可裁切的 keyring baggage 與未開啟的 release-size optimization。

但在那輪修正之後，release-size artifact 也清楚顯示：PathKeep 剩下最重的成本，仍是 default desktop build 內一起 shipping 的 optional intelligence runtime，包括 `lancedb` / `lance` / `datafusion` / `rig-core`，以及與 archive 一起工作的 semantic / assistant / MCP support surface。

接下來的決策不是單純「再做一次瘦身」而已，而是要回答：

- PathKeep 是否要把 optional intelligence 從 default desktop binary 拆成 helper / sidecar / feature-gated build？
- 還是接受較大的桌面 binary，換取 assistant / semantic / MCP 能力 day-one 內建在 app 安裝包內？

這件事會改變 default shipping surface、安裝體驗、升級路徑、support story、supply-chain review 邊界，以及產品對「optional AI」的實際定義，因此不能只靠局部 cleanup 或 agent 臨場判斷決定。

2026-04-10，使用者已明確 sign off：**PathKeep 仍要內建 optional AI，default desktop build 不做拆包。**

## 決策

### 1. Default desktop build 繼續同 binary shipping optional intelligence runtime

PathKeep 的預設桌面版，維持把 optional AI / assistant / MCP / semantic runtime 與 archive / shell-critical desktop runtime 一起 shipping 在同一個桌面 binary 內。

這包含目前桌面 runtime 依賴的 `lancedb` / `lance` / `datafusion` / `rig-core` 相關能力。

### 2. `optional` 的意思是 capability-gated，不是 packaging-gated

在 PathKeep 內，`optional AI` 的正式含義是：

- 功能預設關閉
- 需要使用者在 Settings 明確開啟
- 需要 provider / model / consent / lock-state 條件滿足後才可用

它**不是**：

- 第一次使用時再下載另一個 helper
- 另裝 plugin / sidecar executable 才能啟用
- 預設安裝包不含 AI runtime，只在需要時臨時補抓

### 3. `LanceDB sidecar` 仍是 data sidecar，不是 code/runtime helper

既有的 LanceDB sidecar 角色維持不變：

- sidecar 儲存的是 rebuildable semantic index data
- operational metadata 仍寫回 canonical SQLite
- sidecar 不代表另一個獨立 shipping runtime 或需要額外安裝的 helper process

### 4. Bundle-size evidence 仍然是正式 operator contract

保留 in-process shipping 並不代表 size evidence 可以消失。

`bun run release:size-audit` 仍是正式 release artifact command。2026-04-11 UTC 生成的 `artifacts/release/2026-04-11-size-audit/` 顯示：

- web payload 仍低於 `1 MB`
- unsigned macOS executable 仍約 `109,198,880` bytes（約 `104 MiB`）
- 主要重量仍在 desktop executable，而不是 web shell

這個體積現在屬於 **accepted trade-off**，不是 hidden debt。

### 5. 未來若要改變 default shipping surface，必須重新開 ADR

如果未來要把 optional intelligence 從 default desktop build 拆出去，必須：

- 重新產出 trade-off 決策文檔
- 補新的 packaging / upgrade / rollback / support evidence
- 再次取得明確 product sign-off

不能把這次已接受的 in-process shipping 決策，偷偷在一般 cleanup 或 size triage 裡推翻。

## 理由

- **產品方向清楚**：使用者已明確要求 optional AI 必須是 app 內建能力，而不是額外安裝物。
- **體驗更直接**：assistant / semantic / MCP 是 default product surface 的一部分；不需要再用 helper download 或 feature SKU 把產品切碎。
- **Trust boundary 更單純**：比起 hidden helper、late download、額外 process lifecycle，單一桌面 app + 明確 capability gating 更容易誠實描述與審核。
- **Support / packaging 成本更低**：少一條 helper release、version skew、path resolution、bootstrap failure、uninstall 殘留與 platform-specific launch story。
- **剩餘重量已知且可量測**：在 keyring / LTO cleanup 後，現在留下的是刻意保留的能力成本，不是 accidental bloat。

## 後果

### 正面

- PathKeep 安裝後就具備完整 optional intelligence runtime，不需要額外 bootstrap
- optional AI 的產品語義更一致：disabled-by-default，但 shipped-in-app
- Settings / Assistant / MCP / semantic search 的 support story 更直觀
- release-size triage 可以聚焦在一般 dependency hygiene，而不是反覆爭論 helper split

### 負面

- default desktop binary 會持續維持 ~`100 MiB` 等級，而不是回到更輕的 archive-only binary
- desktop build 會攜帶更多 AI / semantic 相關 supply-chain surface
- 若某平台未來對 bundle size 更敏感，仍可能需要再次打開 packaging boundary 決策
- `vault-core` / `vault-worker` 的 AI surface 仍留在同一 workspace 內，不能把 helper split 當成現成的未來逃生門

## 相關

- `WORK-QC-F`
- [006-deterministic-intelligence-boundary.md](006-deterministic-intelligence-boundary.md)
- [../tech-stack.md](../tech-stack.md)
- [../../features/intelligence.md](../../features/intelligence.md)
- [../../plan/m4-full-polish/release-size-audit.md](../../plan/m4-full-polish/release-size-audit.md)
- [../../plan/m4-full-polish/code-health-audit.md](../../plan/m4-full-polish/code-health-audit.md)
