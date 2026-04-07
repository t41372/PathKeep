# 品質標準、國際化與社區

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出。

---

## 國際化（i18n）

- 支援語言：英文、簡體中文、繁體中文。
- 自動檢測用戶設備語言，預設選擇匹配的語言。
- 可在設定中手動切換。
- 所有用戶可見的文字都走 i18n，包括錯誤信息和通知。
- 翻譯目錄採 namespace-based 結構（至少區分 `common`、`shell`、`navigation` 與 route / feature namespaces），避免單一巨檔持續膨脹。
- trust-critical flow 必須同步交付 `en` / `zh-CN` / `zh-TW`；不接受靠英語 fallback 混過核心操作、warning、empty state 或修復 CTA。
- 日期時間、相對時間、檔案大小等格式化要走共用 helper，並跟語系解析規則一起測試。
- route / page 級的 copy 變更至少要補一輪缺 key coverage 檢查，並保留 pseudo-locale 或等價 smoke 來提早發現 layout 溢出。

---

## 系統信息

- 前端顯示當前版本號和 git commit short SHA。
- 設定頁面中顯示數據存儲目錄，並提供「在文件管理器中打開」的按鈕。

---

## 品質標準

### 測試覆蓋

- 最終標準：
  - Rust 側：100% test coverage + mutation test + integration test。
  - JS/TS 側：100% statement/branch/function/line coverage + mutation test。
- E2E：Playwright spec 覆蓋關鍵用戶流程。

### M0 重寫期規則

- M0 期間，**repo-wide** coverage 和 mutation 暫時不作為 blocking gate；它們仍然有價值，但不應反過來保護舊架構。
- 所有**新建**或**整段重寫**的模組，仍必須有 colocated tests，並讓該 slice 達到 100% coverage + mutation verification。
- 大型前端重寫仍可用 targeted scripts 驗證，但不能把非 UI contract gate 當成前端 shell 已驗收的證據。
- 目前納入 blocking path 的 targeted JS gate 是 desktop contract slice：`bun run test:unit:desktop-contract`、`bun run coverage:js:desktop-contract`、`bun run mutation:js:desktop-contract`，保護範圍固定為 `src/main.tsx` 與 `src/lib/ipc/bridge.ts`。
- 前端 shell / route / sidebar / primitives 與新的 page-scoped data provider 不在這條 gate 內；它們必須由前端 owner 提供自己的 targeted tests / visual review，不能再誤報成已被 repo gate 完整覆蓋。
- route / feature 大改時，除了 unit / contract test，也要補至少一輪 keyboard / locale smoke review，尤其是設定、排程、安全、導入等高風險頁面。
- 驗證舊產品假設的測試應直接刪除或重寫，不保留作長期 legacy harness。
- 不接受把「目前 typecheck 會紅」「先關掉 coverage 再說」寫成完成狀態；重寫期只是調整 gate 的層級，不是放棄品質。

### 代碼品質

- Rust：clippy + cargo fmt + cargo deny（supply chain audit）。
- JS：ESLint + Prettier + TypeScript strict mode。
- Pre-commit hooks 執行所有 linter 和 formatter。

### CI/CD

- GitHub Actions：
  - M0 重寫期 blocking 檢查：lint + test + build。
  - 與目前 work block 直接相關的 targeted verification（例如 shell smoke / targeted mutation）仍要在本地或對應 workflow 補跑。
  - repo-wide coverage / mutation 在新架構穩定前可改成 on-demand 或 scheduled deep check。
  - M1 之後再恢復「coverage 也是 blocking gate」。
  - Release pipeline：多平台構建 + 自動產出安裝檔。
- README badges 顯示 CI 狀態、coverage。

---

## 開源與社區

- 協議：GPL v3。
- README：完整的功能介紹、構建指南、從源碼運行指南。
- CONTRIBUTING.md：開發環境設定、測試方式、commit 規範、PR 流程。
- Conventional Commits 規範。

---

## 不做的事情（Explicit Non-Goals）

- 不做雲端同步或雲端存儲（除非用戶主動配置 S3）。
- 不做人格心理分析或敏感維度推斷。
- 不寫回瀏覽器的 live 數據庫。
- 不做背景常駐的 autonomous agent。
- 不做 SaaS 或 subscription model。
- 不收集用戶數據或 telemetry。
