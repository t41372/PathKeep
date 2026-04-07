# M0-CO — Rename, Quality, And Cutover

> 讀這份文檔的時機：當你要把產品正式從舊命名和舊驗收目標切到 PathKeep，並確保重寫不是在失控狀態下進行。  
> 這份文檔是 M0 的風險控制層。

---

## Quick-Start Implementation Guide

以下是本工作包的建議執行順序。每個步驟都標註了要讀的文檔、要改的文件、和驗收方式。

> **前提**：本工作包建議在 M0-FE 和 M0-BE 開工後並行執行，而非等它們完全完成再開始。命名遷移和管線更新不依賴前端或後端的實際實作進度。

### Step 1: Build rename inventory with grep

**要讀的文檔**
- `docs/plan/program/repo-baseline.md` 的「命名遷移基線」段落 — 了解需要清除的舊名字串

**執行以下命令，產出完整幫存貨**
```bash
# 找出所有舊產品名稱出現的檔案
 grep -rn --include="*.json" --include="*.ts" --include="*.tsx" --include="*.rs" \
   --include="*.md" --include="*.yml" --include="*.yaml" --include="*.toml" \
   -e "Browser History Backup" \
   -e "Chrome History Backup" \
   -e "Chrome History Vault" \
   -e "browser-history-backup" \
   -e "browser_history_backup" \
   -e "BHB" \
   . \
   --exclude-dir=node_modules \
   --exclude-dir=target \
   --exclude-dir=.git \
   > /tmp/rename-inventory.txt 2>&1

cat /tmp/rename-inventory.txt | wc -l   # 查看總出現次數
cat /tmp/rename-inventory.txt           # 透過分類看各檔案的出現位置
```

**預期發現的帶有舊名稱的主要檔案**
```
package.json                              # name: "browser-history-backup"
src-tauri/Cargo.toml                      # name = "browser-history-backup-desktop"
src-tauri/tauri.conf.json                 # productName, identifier, window title
src-tauri/capabilities/default.json      # description
README.md                                 # 正文中的舊產品文案
.github/workflows/ci.yml                  # 工作流文案和 artifact 名稱
.github/workflows/release.yml             # release asset 名稱
src-tauri/src/lib.rs                      # BHB_GIT_COMMIT_FULL 等環境變數
src-tauri/build.rs                        # BHB_GIT_COMMIT_* 陸
```

**將清單記錄為文檔**
```
docs/plan/m0-foundation/rename-inventory.md   # 新建，記錄每檔每行的舊名批注和處理方案
```

**驗收**
```bash
# 確認幫存貨已建立且包含實際筆數
wc -l docs/plan/m0-foundation/rename-inventory.md
```

**Commit**: `docs(co): create rename inventory`

---

### Step 2: Update package.json and tauri.conf.json

**要讀的文檔**
- Step 1 產出的 `rename-inventory.md`
- `src-tauri/tauri.conf.json` — 确認所有 productName / identifier / window title 字段

**要修改的文件**

`package.json`：
```json
// 將
"name": "browser-history-backup"
// 改成
"name": "pathkeep"
```

`src-tauri/Cargo.toml`：
```toml
# 將
name = "browser-history-backup-desktop"
# 改成
name = "pathkeep-desktop"

[lib]
name = "pathkeep_desktop"   # 同步修改
```

`src-tauri/tauri.conf.json`：
```json
// 將
"productName": "Browser History Backup"
"identifier": "dev.codex.browser-history-backup"
// window title 也將 "Browser History Backup"
// 改成
"productName": "PathKeep"
"identifier": "dev.pathkeep.app"
// window title: "PathKeep"
```

`src-tauri/capabilities/default.json`：
```json
// 將
"description": "Default capability set for Browser History Backup"
// 改成
"description": "Default capability set for PathKeep"
```

`src-tauri/build.rs`：
```rust
// 將 BHB_GIT_COMMIT_FULL 等環境變數陥頭
// 改成
PK_GIT_COMMIT_FULL
PK_GIT_COMMIT_SHORT
PK_GIT_DIRTY
```

**驗收**
```bash
bun run typecheck
cargo build -p pathkeep-desktop 2>&1 | head -20   # 確認新名稱能編譯
# 檢查 src-tauri/tauri.conf.json 的 productName 已是 "PathKeep"
jq '.productName' src-tauri/tauri.conf.json   # 應輸出 "PathKeep"
```

**Commit**: `chore(rename): update package.json, Cargo.toml, tauri.conf.json to PathKeep`

---

### Step 3: Rewrite README

**要讀的文檔**
- `docs/vision-and-requirements.md` — 新產品定位和功能描述
- 現有 `README.md` — 先讀清楚舊內容結構再重寫

**要更新的文件**`README.md`

**新 README 必須包含的內容**
```markdown
# PathKeep

> Audit-first local desktop archive for your browser history.

## What is PathKeep
[依照 vision-and-requirements.md 重寫產品定位：對就各自的 M0—M4 等级]

## Status
- [ ] M0 基礎重構（進行中）
- [ ] M1 Solid Archive
- [ ] M2 Recall & Trust
- [ ] M3 Intelligence
- [ ] M4 Full Polish

## Development
[建置要求、展開命令、測試命令]

## Architecture
[指向 docs/architecture/ 地址，不要將細局影射內嵌在 README 裡]

## Contributing
[資料 / 鈲欧展開方式]
```

**要移除的舊內容**
```
- 所有提到 "Browser History Backup" 的句子
- 所有提到 "Chrome History Vault" 的句子
- 宣稱已完成的功能列表（M1—M4 尚未建置）
- 舊的 coverage badge 或 status badge（等 M1 穩定後再加）
```

**驗收**
```bash
grep -i "Browser History Backup\|Chrome History\|Chrome Vault" README.md
# 應輸出空白（沒有舊產品名稱殘留）
grep "PathKeep" README.md | head -5
# 應看到新產品名稱
```

**Commit**: `docs(readme): rewrite README for PathKeep product positioning`

---

### Step 4: Update CI workflows

**要讀的文檔**
- `.github/workflows/ci.yml`、`release.yml`、`mutation.yml` — 確認現有文案和 artifact 命名

**要修改的文件**

`.github/workflows/ci.yml`：
```yaml
# workflow name
name: CI  # 保留原名不變，但檢查 step 文案和 artifact 名稱

# 修改 artifact 名稱
- name: Upload coverage artifacts
  uses: actions/upload-artifact@v4
  with:
    name: pathkeep-coverage-artifacts   # 舊名稱是 coverage-artifacts
```

`.github/workflows/release.yml`：
```yaml
# 修改 release notes 和 asset 命名樣式
# 舊樣式："Browser History Backup vX.Y.Z"
# 新樣式："PathKeep vX.Y.Z"

# 修改 macOS 封裝地 (tauri.conf.json 變化後 build 會自動使用新 productName)
```

`.github/workflows/mutation.yml`：
```yaml
# 檢查最上層的 name 和 參數，確認沒有舊產品名稱
```

**驗斖**
```bash
grep -n "Browser History Backup\|Chrome History\|browser-history-backup" \
  .github/workflows/ci.yml \
  .github/workflows/release.yml \
  .github/workflows/mutation.yml
# 應輸出空白（舊名稱已全清除）
```

**Commit**: `chore(ci): update workflow names and artifact names to PathKeep`

---

### Step 5: Reset e2e tests to new shell

**要讀的文檔**
- `tests/e2e/shell.spec.ts` — 先讀現有斷言，了解它在驗證什麼
- `docs/plan/m0-foundation/frontend-shell-and-design-system.md` 的 Step 4 — 新 shell 的 HTML 結構

**現有 `tests/e2e/shell.spec.ts` 所驗證的舊目標**
```typescript
// 現有失敗點：測試舊 setup shell 的 heading 和文案
// 例：wait page.getByRole('heading', { name: 'Setup' })
// 這就是為什麼 e2e 目前一定失敗
```

**要更新的文件** `tests/e2e/shell.spec.ts`：
```typescript
import { test, expect } from '@playwright/test'

test.describe('PathKeep shell smoke', () => {
  test('app loads and shows main shell or onboarding', async ({ page }) => {
    await page.goto('/')
    // 新驗證：確認詞性有 app-shell
    await expect(page.locator('[data-testid="app-shell"]')).toBeVisible()
  })

  test('sidebar is visible after initialization', async ({ page }) => {
    await page.goto('/')
    // 新驗證：確認 sidebar 存在和可見
    await expect(page.locator('.sidebar')).toBeVisible()
  })

  test('can navigate to explorer route', async ({ page }) => {
    await page.goto('/explorer')
    // 新驗證：確認 URL 和畫面標題
    await expect(page).toHaveURL(/\/explorer/)
    await expect(page.locator('h1')).toContainText('Explorer')
  })

  test('onboarding route exists and renders', async ({ page }) => {
    await page.goto('/onboarding')
    await expect(page.locator('[data-testid="onboarding-shell"]')).toBeVisible()
  })
})
```

**要更新的 playwright.config.ts**（如有舊產品名稱利徕）：
```typescript
// 修改 baseURL、專案名稱等，移除舊產品利徕
```

**驗斖**
```bash
bun run test:e2e   # shell smoke tests 會 pass，舊的 "Setup" heading 驗證已不存在
```

**Commit**: `test(e2e): reset shell smoke tests for new PathKeep shell`

---

### Step 6: Final rename sweep and cutover verification

**執行最終游揳**
```bash
# 1. 再次执行 Step 1 的 grep，確認舊名稱完全清除
grep -rn \
  -e "Browser History Backup" \
  -e "Chrome History Backup" \
  -e "Chrome History Vault" \
  -e "browser-history-backup" \
  -e "browser_history_backup" \
  --include="*.json" --include="*.ts" --include="*.tsx" --include="*.rs" \
  --include="*.md" --include="*.yml" --include="*.toml" \
  --exclude-dir=node_modules --exclude-dir=target --exclude-dir=.git \
  . \
  | grep -v "docs/plan/m0-foundation/rename-inventory.md"   # 排除清單本身

# 2. 驗證新名稱正確出現
jq '.productName' src-tauri/tauri.conf.json     # "PathKeep"
jq '.name' package.json                         # "pathkeep"
grep 'name = ' src-tauri/Cargo.toml | head -1   # name = "pathkeep-desktop"

# 3. 全量驗證組
 bun run typecheck
cargo test --manifest-path src-tauri/Cargo.toml --workspace --all-targets --quiet
bun run test:unit
bun run test:e2e
```

**M0-CO 完成訊號 checklist**
```
[✓] README 正文不含舊產品名稱
[✓] package.json name = "pathkeep"
[✓] tauri.conf.json productName = "PathKeep", identifier = "dev.pathkeep.app"
[✓] src-tauri/Cargo.toml name = "pathkeep-desktop"
[✓] .github/workflows 文案不含舊產品名稱
[✓] tests/e2e/shell.spec.ts 驗證新 shell，不驗證舊 "Setup" heading
[✓] bun run test:e2e pass
[✓] cargo test --workspace pass
[✓] bun run typecheck pass
```

**Commit**: `chore(co): final rename sweep, M0-CO cutover complete`

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
