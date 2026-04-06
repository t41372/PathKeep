# M0-CO — Rename, Quality, And Cutover

> 讀這份文檔的時機：當你要把產品正式從舊命名和舊驗收目標切到 PathKeep，並確保重寫不是在失控狀態下進行。  
> 這份文檔是 M0 的風險控制層。

---

## Source Inputs

- [../../vision-and-requirements.md](../../vision-and-requirements.md)
- [../../standards.md](../../standards.md)
- [../program/repo-baseline.md](../program/repo-baseline.md)
- [../program/research-and-decisions.md](../program/research-and-decisions.md)
- [frontend-shell-and-design-system.md](frontend-shell-and-design-system.md)
- [backend-and-data-rearchitecture.md](backend-and-data-rearchitecture.md)

---

## 本工作包要交付什麼

- 對外產品命名、bundle 文案、filesystem / keyring / scheduler legacy 名稱的遷移策略
- README、workflow、CI、coverage、mutation 和驗收基線的重設
- 舊新架構切換順序、保留策略和刪除清單
- 可讓 M1 直接開工的最小 quality gate

---

## WBS

### Rename Inventory And Migration Strategy

- [ ] `M0-CO-RN-001` 盤點 `package.json`、README、Tauri config、workflow、release artifact、user-agent、export header、keyring service、schedule label 中所有舊名字串。
- [ ] `M0-CO-RN-002` 區分對外可見名稱和相容性需要保留的內部 legacy name，建立 migration matrix。
- [ ] `M0-CO-RN-003` 凍結 day-one 公開產品名稱、bundle name、app root、資料目錄名稱、CLI / MCP 名稱策略。
- [ ] `M0-CO-RN-004` 決定舊 app root 和舊 keyring service 的相容讀取窗口，避免 rename 時把資料鎖在舊位置。
- [ ] `M0-CO-RN-005` 為 schedule / launch agent / task scheduler / systemd artifact 決定新的命名和清理策略。
- [ ] `M0-CO-RN-006` 決定哪些舊名字串要在 M0 一次清乾淨，哪些需要等到 M1 / M2 遷移能力完成後再移除。

### Docs, README, And Workflow Alignment

- [ ] `M0-CO-DC-001` 重寫 README 的產品描述、功能列表、架構簡介和 roadmap，不能再宣稱舊產品假設已完成。
- [ ] `M0-CO-DC-002` 更新 `.github/workflows/ci.yml`、`mutation.yml`、`release.yml` 中的產品命名和輸出文案。
- [ ] `M0-CO-DC-003` 檢查 Tauri bundle metadata、window title、About 文案、installer metadata 是否都已切成 PathKeep。
- [ ] `M0-CO-DC-004` 在 docs 中加入明確導覽，讓讀者知道需求看哪裡、計劃看哪裡、決策看哪裡、畫面看哪裡。
- [ ] `M0-CO-DC-005` 把這次重整後的 `docs/plan/` 視為 implementation truth，未來功能變動時先更新對應 plan 文檔。

### Quality Gate Reset

- [ ] `M0-CO-QA-001` 定義新的 quality gate 套件：typecheck、unit、integration、parser fixtures、Rust tests、desktop smoke、coverage、mutation。
- [ ] `M0-CO-QA-002` 把 JS / Rust coverage 的實際現況和 `docs/standards.md` 的最終目標分開寫清楚，形成遷移路線，而不是只留一句口號。
- [ ] `M0-CO-QA-003` 重寫 Playwright smoke 驗收目標，從舊 setup shell 改成新 onboarding / dashboard / navigation smoke。
- [ ] `M0-CO-QA-004` 盤點目前 mutation test 適用範圍，決定 M0 期間哪些模組先恢復、哪些等拆模組後再啟用。
- [ ] `M0-CO-QA-005` 為 docs-only、frontend-only、rust-core-only 變更定義最小必跑驗證組合，避免每次變更都靠人工猜。
- [ ] `M0-CO-QA-006` 為大規模重構期間的 branch / PR 規範建立 guardrail：每個 PR 必須能說清楚刪了什麼、保留了什麼、下一步接什麼。

### Legacy Cutover

- [ ] `M0-CO-CT-001` 產出舊前端檔案的刪除 / 保留清單，包含 `AppNew`、舊 `pages/`、舊 setup shell 文案和大型 CSS。
- [ ] `M0-CO-CT-002` 產出舊 Rust 模組的拆分 / 過渡清單，標明哪些檔案在 M0 只重命名、哪些要真正拆模組。
- [ ] `M0-CO-CT-003` 決定 cutover 分段：先引入新 shell、再接新 command facade、再刪舊 UI，避免單次超大 PR。
- [ ] `M0-CO-CT-004` 決定新舊 schema / data dir / config 的 coexistence 窗口，避免測試和真實資料互相污染。
- [ ] `M0-CO-CT-005` 建立 rollback plan：如果新 shell 或新 schema cutover 出現重大問題，要怎麼快速回退到可工作的主幹。
- [ ] `M0-CO-CT-006` 為每個 cutover 階段定義明確完成訊號，例如「主入口切換完成」「舊 smoke test 下線完成」「PathKeep 命名完整可見」。

### Delivery Discipline

- [ ] `M0-CO-DD-001` 把 M0 拆成可 review 的原子 commit / PR 序列，避免一次提交前後端全量重寫。
- [ ] `M0-CO-DD-002` 每個重要技術決策補 ADR 或 docs，特別是 schema reset、migration story、parser boundary、legacy compatibility。
- [ ] `M0-CO-DD-003` 建立 milestone review checklist，M0 完成前逐項核對 README、docs、tests、product naming、bundle metadata。
- [ ] `M0-CO-DD-004` 在 M0 收尾時更新 `docs/milestones.md`、`docs/plan/README.md`、相關 milestone README 的狀態和後續入口。

---

## Exit Artifacts

- rename inventory 和 migration matrix
- 重新對齊的 README / workflow / bundle metadata
- 新 quality gate 定義
- cutover / rollback / branch sequencing 計劃
