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
5. Session guard
   - 提供 app lock 狀態、鎖定 / 解鎖與 lock-aware refusal path

---

## Draft Domains

| Domain     | Read model                                          | Mutating / Preview                                                                | Artifact                                           |
| ---------- | --------------------------------------------------- | --------------------------------------------------------------------------------- | -------------------------------------------------- |
| `app`      | `app.snapshot`, `app.build_info`, `app.lock_status` | `app.lock.configure`, `app.lock.lock`, `app.lock.unlock`                          | -                                                  |
| `archive`  | `archive.status`, `explorer.query_history`          | `archive.backup.preview`, `archive.backup.execute`                                | `artifacts.list_for_run`, `artifacts.get_manifest` |
| `import`   | `import.list_batches`                               | `archive.import.preview`, `archive.import.execute`                                | `artifacts.get_preview_bundle`                     |
| `rollback` | `audit.get_run_detail`                              | `archive.rollback.preview`, `archive.rollback.execute`                            | `artifacts.list_for_run`                           |
| `schedule` | `schedule.get_status`                               | `schedule.install.preview`, `schedule.install.execute`, `schedule.install.remove` | `artifacts.get_preview_bundle`                     |
| `security` | `security.get_status`                               | `security.rekey.preview`, `security.rekey.execute`                                | `artifacts.list_for_run`                           |
| `doctor`   | `audit.list_runs`                                   | `jobs.start_doctor`                                                               | `artifacts.get_doctor_report`                      |

---

## Compatibility Note

現有 `src-tauri/src/lib.rs` 還保留舊的 command 名稱與 worker bridge 封裝，因為 M0-B 前端 shell 和 M1 archive engine 尚未切換到這套 surface。這不改變上面的凍結方向：

- 2026-04-06 audit follow-up：目前已經有對應的現行命令可承接這份草案的核心 read models / previews，例如 `schedule_status`、`security_status`、`preview_rekey_archive`、`preview_schedule`、`apply_schedule`
- 2026-04-07 trust UX follow-up：現行 schedule execute surface 也已補上 `remove_schedule`，讓原生排程安裝和解除安裝都能留在同一個 PME / audit 故事裡
- 2026-04-07 M2-A follow-up：`inspect_takeout`、`import_takeout`、`preview_import_batch`、`revert_import_batch`、`restore_import_batch`、`doctor_report`、`repair_health` 已把 import / rollback / doctor 的現行 command surface 接上 unified run / artifact story
- 2026-04-08 app lock follow-up：現行 surface 已加入 `app_lock_status`、`set_app_lock_passcode`、`clear_app_lock_passcode`、`lock_app_session`、`unlock_app_session`。`app_snapshot`、dashboard、Explorer、Insights、AI queue / MCP 等 data read commands 必須在 locked state 下回傳 refusal，而不是偷偷載入 archive data
- 新命令不要再以舊 UI 頁面名稱或 legacy product strings 命名
- 新 preview / execute 流程要直接對齊 PME
- 新 long-running 操作要以 unified `runs` ledger 為中心回報狀態

## Implemented Command Map（2026-04-09 / `WORK-QC-C`）

| 現行 Tauri command(s)                                                                                                                                                                                                                                                                                                                   | 對應 draft domain                                    | 主要 UI / consumer                          | Closeout note                                                                                                                                                                                                                     |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- | ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `app_build_info`, `app_snapshot`, `app_lock_status`, `set_app_lock_passcode`, `clear_app_lock_passcode`, `lock_app_session`, `unlock_app_session`                                                                                                                                                                                       | `app.*` / session guard                              | shell bootstrap、Settings、`/lock`          | 這條 surface 已有 lock-aware refusal path，是目前 desktop truth 的正式入口。                                                                                                                                                      |
| `save_config`, `initialize_archive`, `set_session_database_key`, `clear_session_database_key`, `keyring_*`, `security_status`, `preview_rekey_archive`, `rekey_archive`                                                                                                                                                                 | `security.*` / onboarding bootstrap                  | Onboarding、Security、Settings              | rekey preview / execute 已存在；approval metadata 尚未寫入 unified `runs`。                                                                                                                                                       |
| `run_backup_now`, `query_history`, `load_dashboard_snapshot`, `load_audit_run_detail`, `export_history`                                                                                                                                                                                                                                 | `archive.*`, `explorer.*`, `audit.*`                 | Dashboard、Explorer、Audit                  | 命名仍是 legacy single-command style，但實際上已對應 archive / explorer / audit read models。                                                                                                                                     |
| `inspect_takeout`, `import_takeout`, `preview_import_batch`, `revert_import_batch`, `restore_import_batch`, `doctor_report`, `repair_health`                                                                                                                                                                                            | `archive.import.*`, `archive.rollback.*`, `doctor.*` | Import、Audit、repair CTA                   | import / rollback / restore / doctor 已接到 unified run / artifact 故事；`restore` 與未來 `snapshot_restore` 需保持獨立語義。                                                                                                     |
| `preview_schedule`, `apply_schedule`, `remove_schedule`, `schedule_status`                                                                                                                                                                                                                                                              | `schedule.install.*`, `schedule.get_status`          | Schedule                                    | macOS 有實際 apply / remove / detect；Windows / Linux 維持 manual-review。                                                                                                                                                        |
| `preview_remote_backup`, `run_remote_backup`, `verify_remote_backup`                                                                                                                                                                                                                                                                    | `remote-backup.*`                                    | Settings remote backup                      | Preview / Manual / Execute / Verify 已完成；retention / prune 保持 manual-first。                                                                                                                                                 |
| `test_ai_provider_connection`, `load_ai_queue_status`, `run_ai_queue_jobs`, `replay_ai_job`, `cancel_ai_job`, `build_ai_index`, `search_ai_history`, `ask_ai_assistant`, `load_ai_assistant_job`, `run_insights_now`, `load_insights`, `load_thread_detail`, `explain_insight`, `preview_ai_integrations`, `clear_derived_intelligence` | `intelligence.*` / `jobs.*`                          | Assistant、Insights、Settings、MCP          | 這一簇在 M3 / M4 已可用，且現在補上了 model-scoped stale / cost read model、MCP consent / scope / audit preview 與 dedicated `mcp_query` run；但 60-year all-features performance 仍只到 honest support-envelope，不是 GA claim。 |
| `open_path_in_file_manager`                                                                                                                                                                                                                                                                                                             | support helper / artifact reveal                     | Audit、Import、Schedule、Security、Settings | 這是安全邊界內的 reveal/open helper，不等於檔案搬移或 retention policy。                                                                                                                                                          |
