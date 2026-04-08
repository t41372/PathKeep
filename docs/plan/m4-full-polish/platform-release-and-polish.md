# M4-RL — Platform, Release, And Polish

> 讀這份文檔的時機：當產品功能面已基本完成，你要把 PathKeep 從內部可用提升到可穩定發布、可維護、可讓外部人上手。  
> 這份文檔是最後的發版控制塔。

**2026-04-08 closeout (`WORK-M4-B`)**：這一輪已把多平台發版前 runbook、artifact matrix、release workflow preflight、README / CONTRIBUTING / DEVELOPMENT / TESTING / RELEASE / TROUBLESHOOTING / SUPPORT 文檔、bug report template、以及 Settings 的 build / path diagnostics 全部補齊。blocking path、coverage、`mutation:js`、browser-preview `test:e2e` 與 `desktop:build:debug` 都已通過；`mutation:rust` 作為 pre-release deep check 也已實跑，但暴露出 `browser-history-parser` / `vault-core` AI 的存活 mutants，因此被誠實切出成後續 `WORK-M4-D`，而不是留在模糊 TODO。

**2026-04-08 performance follow-up**：release closeout 後再用真實大型 Chromium profile 做 manual backup，發現匯入雖能完成，但完成後整個 shell 仍明顯不流暢。這代表 M4 的「Performance, Accessibility, And Observability」並沒有因為 release gate 全綠就真的完成，因此另外切出 `WORK-M4-G`。它的 focus problem 不是一般 route 切換 polish，而是 large-archive baseline：whole-app profiling artifact、parser / ingest hot path、Explorer FTS5 recall 契約，以及大型 backup 的 progress / observability。

---

## Source Inputs

- [../../standards.md](../../standards.md)
- [../../architecture/tech-stack.md](../../architecture/tech-stack.md)
- [../../vision-and-requirements.md](../../vision-and-requirements.md)
- [../program/research-and-decisions.md](../program/research-and-decisions.md)
- [enrichment-advanced-intelligence-and-remote.md](enrichment-advanced-intelligence-and-remote.md)
- [e2e-workflow-rehearsal.md](e2e-workflow-rehearsal.md)
- [../m0-foundation/rename-quality-and-rewrite-discipline.md](../m0-foundation/rename-quality-and-rewrite-discipline.md)

---

## 本工作包要交付什麼

- macOS / Windows / Linux 的發版前驗收和平台 runbook
- signing / notarization / installer / secret management 的發版流程
- README / CONTRIBUTING / support / troubleshooting 文檔
- 效能、accessibility、observability、final QA 收尾

---

## WBS

### Platform Validation

- [x] `M4-RL-PF-001` 建立 macOS 真機驗收 runbook，覆蓋 onboarding、backup、schedule、security、AI optional、uninstall / reinstall。
- [x] `M4-RL-PF-002` 建立 Windows 真機驗收 runbook，覆蓋 installer、scheduler、keyring、path permissions、upgrade path。
- [x] `M4-RL-PF-003` 建立 Linux 真機驗收 runbook，覆蓋 packaging 形式、scheduler、keyring fallback、desktop environment 差異。
- [x] `M4-RL-PF-004` 為 archive migration、data dir move、encrypted archive、remote backup restore 建立跨平台 smoke。
- [x] `M4-RL-PF-005` 對每個平台列出 known limitations、deferred items、support stance。

### Release Engineering

- [x] `M4-RL-RE-001` 定義 release artifact matrix：debug build、signed build、notarized build、portable artifact、symbol / debug info。
- [x] `M4-RL-RE-002` 完成 macOS signing / notarization runbook 和 CI secrets 設置。
- [x] `M4-RL-RE-003` 完成 Windows code signing 和 installer strategy，至少形成可運作的內部發版路徑。
- [x] `M4-RL-RE-004` 完成 Linux release packaging strategy，明確 day-one 支援哪些格式和發佈管道。
- [x] `M4-RL-RE-005` 重整 release workflow，確保命名、artifact、notes、version bump、changelog 都對齊 PathKeep。
- [x] `M4-RL-RE-006` 為發版建立 rollback plan，涵蓋壞 migration、壞 build、壞 scheduler artifact、AI provider regression。

### Docs And Developer Experience

- [x] `M4-RL-DX-001` 重寫 README，讓新使用者能快速理解產品定位、功能邊界、資料主權、AI optional 原則。
- [x] `M4-RL-DX-002` 新增或重寫 CONTRIBUTING / DEVELOPMENT / TESTING / RELEASE 文檔，對齊現實工作流。
- [x] `M4-RL-DX-003` 建立 user-facing troubleshooting 文檔，涵蓋 scheduler、permissions、keyring、archive lock、index rebuild、remote backup。
- [x] `M4-RL-DX-004` 確認 docs 和產品 UI 入口互相對得上，避免 README 寫的畫面或功能在實際 app 中找不到。
- [x] `M4-RL-DX-005` 為 support / issue template / bug report 建立需要的診斷資訊清單，避免使用者回報時資訊不足。

### Performance, Accessibility, And Observability

- [x] `M4-RL-PO-001` 建立前端啟動和 route 切換的效能基線，避免 shell 和圖表模組拖慢桌面體驗。
- [x] `M4-RL-PO-002` 建立 archive engine、import、index rebuild、remote backup 的性能基線和 regression guard。
- [x] `M4-RL-PO-003` 進行一輪完整 accessibility review，覆蓋主要路徑、鍵盤操作、語系切換、reduced motion。
- [x] `M4-RL-PO-004` 補齊觀測性：structured logs、run correlation id、user-visible diagnostics、support bundle 策略。
- [x] `M4-RL-PO-005` 對隱私敏感資訊建立 log redaction 和 support export guardrail。

### Final Acceptance

- [x] `M4-RL-QA-001` 執行 release closeout sweep：`bun run check`、`bun run build`、`bun run desktop:build:debug`、coverage、`mutation:js`、browser-preview `test:e2e` 均通過；`mutation:rust` 已預演並把 parser / AI misses 收斂成後續 `WORK-M4-D`。
- [x] `M4-RL-QA-002` 執行一次從全新安裝到長期使用場景的 end-to-end rehearsal，覆蓋 backup、import、rollback、AI optional、remote backup。browser-preview rehearsal inventory 見 [e2e-workflow-rehearsal.md](e2e-workflow-rehearsal.md)。
- [x] `M4-RL-QA-003` 執行 docs/plan 和 source docs 的最後一次 traceability 檢查，修正任何過期入口和錯誤鏈接。
- [x] `M4-RL-QA-004` 準備發版前決策清單：哪些 feature GA、哪些標 beta、哪些明確 deferred。
- [x] `M4-RL-QA-005` 在正式對外發版前，完成一輪 CTO review：產品定位、風險、相容性、資料安全、維運成本全部過關。

---

## Exit Artifacts

- 多平台發版和驗收 runbook
- signing / notarization / release workflow
- 外部可讀的 README / CONTRIBUTING / troubleshooting
- 最終 performance / accessibility / observability 收尾
