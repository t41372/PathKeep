# Desktop Command Surface

> 這是 M0 凍結的 Tauri command surface 草案。正式 IPC facade 仍可暫留單檔，但命名與 envelope 之後要向這裡收斂。

本草案與 [module-boundary-map.md](module-boundary-map.md) 的 command sections 同步維護；若兩者出現衝突，以本文件的 API 分類為準。

---

## Interface Types

1. Read model
   - 回傳給 onboarding、dashboard、explorer、audit、settings 的純讀取資料
2. Preview / manual / execute
   - 高風險操作一律先產生 preview artifact，再進 execute
3. Long-running job trigger
   - 啟動 backup / import / doctor / future reindex，回傳 `run_id`
4. Artifact fetch
   - 用 `run_id` 或 artifact id 拉 manifest、snapshot metadata、preview bundle

---

## Draft Domains

| Domain     | Read model                                 | Mutating / Preview                                                                | Artifact                                           |
| ---------- | ------------------------------------------ | --------------------------------------------------------------------------------- | -------------------------------------------------- |
| `app`      | `app.snapshot`, `app.build_info`           | -                                                                                 | -                                                  |
| `archive`  | `archive.status`, `explorer.query_history` | `archive.backup.preview`, `archive.backup.execute`                                | `artifacts.list_for_run`, `artifacts.get_manifest` |
| `import`   | `import.list_batches`                      | `archive.import.preview`, `archive.import.execute`                                | `artifacts.get_preview_bundle`                     |
| `rollback` | `audit.get_run_detail`                     | `archive.rollback.preview`, `archive.rollback.execute`                            | `artifacts.list_for_run`                           |
| `schedule` | `schedule.get_status`                      | `schedule.install.preview`, `schedule.install.execute`, `schedule.install.remove` | `artifacts.get_preview_bundle`                     |
| `security` | `security.get_status`                      | `security.rekey.preview`, `security.rekey.execute`                                | `artifacts.list_for_run`                           |
| `doctor`   | `audit.list_runs`                          | `jobs.start_doctor`                                                               | `artifacts.get_doctor_report`                      |

---

## Compatibility Note

現有 `src-tauri/src/lib.rs` 還保留舊的 command 名稱與 worker bridge 封裝，因為 M0-B 前端 shell 和 M1 archive engine 尚未切換到這套 surface。這不改變上面的凍結方向：

- 2026-04-06 audit follow-up：目前已經有對應的現行命令可承接這份草案的核心 read models / previews，例如 `schedule_status`、`security_status`、`preview_rekey_archive`、`preview_schedule`、`apply_schedule`
- 2026-04-07 trust UX follow-up：現行 schedule execute surface 也已補上 `remove_schedule`，讓原生排程安裝和解除安裝都能留在同一個 PME / audit 故事裡
- 2026-04-07 M2-A follow-up：`inspect_takeout`、`import_takeout`、`preview_import_batch`、`revert_import_batch`、`restore_import_batch`、`doctor_report`、`repair_health` 已把 import / rollback / doctor 的現行 command surface 接上 unified run / artifact story
- 新命令不要再以舊 UI 頁面名稱或 legacy product strings 命名
- 新 preview / execute 流程要直接對齊 PME
- 新 long-running 操作要以 unified `runs` ledger 為中心回報狀態
