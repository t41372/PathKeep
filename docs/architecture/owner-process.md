# Database owner process

PathKeep's desktop process owns database work, including when it runs only in
the tray. `--worker` commands and the bundled `pathkeep-worker` executable are
clients. The MCP server remains a long-lived stdio process; its three tool
handlers forward to the same owner endpoint. No archive key crosses this endpoint.

## Startup and ownership

The official `tauri-plugin-single-instance` plugin runs before app setup. Its
instance identifier includes a digest of the absolute data root, isolating users
and test archives. A second app launch passes its arguments to the running app
and exits before acquiring the archive lock. Foreground launches open the main
window; `--background` and login handoffs leave it hidden.

App setup acquires `vault_store::OwnerLock` at `<root>/archive/.pk-owner.lock`
before recovery, session initialization, tray status reads or either command
listener. This is the same OS lock implementation used by `Store::open`; the
lockfile must never be unlinked. The app retains its guard until the IPC service
and its blocking operations have stopped. The storage migration must pass that
ownership into its storage lifecycle rather than independently acquiring a second
guard in the same process. This change does not replace any database implementation.

`--background` creates the tray without creating a webview, even when the saved
tray preference is off. Opening PathKeep from the tray or a foreground launch
creates the window. This launch override does not change the saved preference.
The dev-only desktop bridge starts after ownership and session initialization.

A `--background` launch that never opens a window leaves on its own: once no owner
request is in flight, no queue worker (Insights, enrichment, page fetching, AI) is
running and the search index isn't catching up, it waits two idle minutes, closes
the archive and exits. Turning the menu bar icon on keeps it, and opening a window
makes it an ordinary launch that stays until quit. The next client starts it again.

When connecting reports a missing/refused endpoint, a client starts the app once
and waits up to 30 seconds for the endpoint. Other errors, including permissions,
fail immediately. Once a request is submitted it is never automatically replayed:
a disconnected client cannot know whether a mutation committed.

- **macOS bundle:** `/usr/bin/open -n -g -a <PathKeep.app> --args --background`.
  Launch Services starts it without bringing a window forward; a racing launch
  reaches the single-instance handoff instead of reopening a hidden window.
  `--env` carries the resolved `CHB_PROJECT_ROOT` and existing browser/keyring/platform
  test overrides, because Launch Services does not inherit the caller's environment.
- **macOS unbundled / Linux:** spawn the desktop executable with `--background`.
  A sidecar finds `pathkeep-desktop`, `PathKeep` or `pathkeep` beside itself.
- **Windows:** spawn the sibling desktop `.exe` with `--background`, detached from
  the client's console and process group.

Spawned processes have null stdin/stdout; stderr goes to
`<root>/logs/owner-startup.log`, including failures before the regular logger starts.
They cannot keep an MCP transport open or interleave text into its JSON-RPC output. Startup failures name the app or
missing installation; readiness timeout asks the user to start PathKeep and
inspect its startup log. Debug tests set `CHB_PROJECT_ROOT`, browser fixture roots,
`CHB_TEST_KEYRING_DIR` and `PATHKEEP_PLATFORM_TEST_SANDBOX_DIR` on child processes.

## Endpoint and security

The transport crate, `vault-owner-ipc`, has no storage or worker dependency.

- **macOS / Linux:** `<root>/ipc/owner.sock`, mode `0600`, inside `ipc/`, mode
  `0700`. The directory must be owned by the current effective user and cannot
  be a symlink. Both sides check Unix peer credentials. An owner holding the
  archive lock removes a socket that refuses connections after a crash; it never
  replaces a live listener.
- **Windows:** `\\.\pipe\PathKeep-<SHA-256(user SID + root)>`. Tokio creates the
  pipe with a protected DACL granting access only to the current token's user SID
  (`D:P(A;;GA;;;<SID>)`), rejects remote clients, and reserves the first instance.
  Clients retain Tokio's identification-only security quality of service.

This is a same-user command channel, not an authentication boundary against code
already running as the user or an administrator. MCP consent and App Lock checks
remain in the existing owner-side worker functions and are evaluated on each
call. Due-only scheduled backups retain their existing background backup exception
and keyring policy. Normal commands can use the app's current session key.

## Protocol v1

Each frame is a four-byte unsigned **big-endian** JSON byte length followed by
UTF-8 JSON. Empty frames and frames above 8 MiB are rejected before allocation.
There is one request per connection. Requests have `version`, `method`, `params`:

```json
{ "version": 1, "method": "backup", "params": { "dueOnly": true } }
```

Replies have `version`, zero-based `sequence`, and one of these bodies:

```json
{"version":1,"sequence":0,"type":"progress","value":{"phase":"starting"}}
{"version":1,"sequence":1,"type":"result","value":{"dueSkipped":true}}
{"version":1,"sequence":0,"type":"error","error":{"code":"version_mismatch","message":"unsupported owner protocol version 2; expected 1"}}
```

The progress example illustrates the envelope; actual values are the existing
`BackupProgressEvent` DTO. Progress frames remain in production order and precede
exactly one final result/error. The client checks every sequence and version.
Typed errors are `version_mismatch`, `invalid_request`, `unknown_method`,
`operation_failed`, `unavailable` and `protocol`; diagnostic messages retain causes.

| Method     | Parameters                                | Result                                                                       |
| ---------- | ----------------------------------------- | ---------------------------------------------------------------------------- |
| `ping`     | `{}`                                      | Owner PID, background flag, initial webview count, last handed-off arguments |
| `backup`   | `{ "dueOnly": boolean }`                  | Existing `BackupReport`, with progress first                                 |
| `doctor`   | `{}`                                      | Existing health report                                                       |
| `ai-index` | `{}`                                      | Existing default AI index request result                                     |
| `ai-queue` | `{}`                                      | Existing queue drain result                                                  |
| `mcp`      | `{ "tool": string, "arguments": object }` | Existing MCP tool result                                                     |

External MCP tool names are only `search-history`, `archive-status`, and `usage-guide`.
The stdio adapter first sends `mcp` with `tool: "initialize"` and empty arguments
for the existing startup consent/App Lock/keyring check; this private preflight is
not listed as an external MCP tool. A refused startup still exits 1.
Unknown methods and unexpected parameter fields are refused; there is no generic
Tauri command, SQL, filesystem or shell execution method. The MCP JSON-RPC handshake and tool discovery stay in the stdio process. Enabling/disabling MCP is checked when
a tool is invoked, so an already connected external tool cannot retain old consent.

Blocking worker functions run away from the listener and main thread. The server
admits 16 concurrent connections, buffers at most 16 progress values per request,
and applies backpressure. An incomplete request has a five-second deadline; a
client that stops reading is disconnected after a 30-second write deadline.
Losing the client does not undo or cancel an accepted backup. Long-running work
has no arbitrary completion deadline.

## Scheduler and validation

Generated launchd / Task Scheduler / systemd commands stay unchanged. The worker
still writes its final pretty JSON to stdout, progress to stderr, and exits 0 on
success (including due-only skips) or 1 on failure. Existing scheduled attempt
phase/outcome recording now runs inside the owner. The macOS legacy app-command
form still uses `open -n -W -g ... --args --worker backup --due-only`; `--worker`
routing happens before Tauri single-instance setup, so it remains a short-lived
client. Bundled sidecars still run directly. Windows tasks retain their current
user's interactive token and least-privilege setting.

Transport failures live in `vault-owner-ipc/tests/transport.rs`; public lock tests
in `vault-store/tests/app_owner.rs`; owner business failures in
`vault-worker/src/owner_requests/`. `vault-worker/tests/owner_client.rs` separately drives the real sidecar and stdio
MCP processes through the owner dispatcher, including consent revocation during a
live connection (artifact: `artifacts/owner-process/sidecar-mcp.json`).
`src-tauri/tests/owner_process.rs` runs real
app, CLI and MCP processes against a deterministic browser fixture. Its artifact
is `artifacts/owner-process/result.json`. On macOS a sandbox denies the client
all reads/writes under `archive/` and `derived/`, so successful backup and MCP
calls prove those database opens happened in the owner process.

Run with cached dependencies only:

```sh
CARGO_NET_OFFLINE=true cargo test --manifest-path src-tauri/Cargo.toml -p pathkeep-desktop --test owner_process -- --nocapture
CARGO_NET_OFFLINE=true cargo test --manifest-path src-tauri/Cargo.toml -p vault-owner-ipc -p vault-worker -p vault-store
CARGO_NET_OFFLINE=true cargo check --manifest-path src-tauri/Cargo.toml -p vault-owner-ipc --all-targets --target x86_64-pc-windows-gnu
```

## Failure modes and test names

Every new module lists its responsibility boundary and failure modes in its header.
The executable checks are:

| Failure mode                                                                                                                                             | Test                                                                               |
| -------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Cold client cannot start owner; second app takes ownership or loses arguments; background creates webview; client opens databases; MCP bypasses endpoint | `cold_client_backup_single_instance_handoff_and_mcp_use_only_owner`                |
| App reservation opens databases, allows another owner, or fails to release                                                                               | `app_reserves_owner_lock_without_opening_databases`                                |
| Wipe deletes ownership inode while the owner still holds it                                                                                              | `wipe_keeps_owner_lock_file_and_exclusive_ownership`                               |
| Live socket replaced; progress reordered/lost; final reply missing                                                                                       | `progress_is_ordered_before_final_reply_and_existing_owner_is_not_replaced`        |
| Crashed owner's socket blocks restart; another user can access endpoint                                                                                  | `stale_socket_is_recovered_and_directory_and_socket_are_user_only`                 |
| Symlink redirects the private socket directory                                                                                                           | `symlinked_socket_directory_is_rejected`                                           |
| Incompatible client executes business work                                                                                                               | `protocol_mismatch_returns_typed_error_without_dispatch`                           |
| Malformed, truncated or oversized input is accepted                                                                                                      | `malformed_truncated_and_oversized_frames_fail`                                    |
| Oversized output closes without a typed final error                                                                                                      | `oversized_reply_is_typed_error_and_owner_survives`                                |
| Handler panic loses final error                                                                                                                          | `handler_panic_is_a_final_error_and_owner_survives`                                |
| Startup waits forever or launches repeatedly                                                                                                             | `no_endpoint_start_failure_is_clear_and_does_not_retry_launch`                     |
| Unknown method or bad parameters reach a worker                                                                                                          | `owner_rejects_unknown_methods_and_invalid_parameters_before_work`                 |
| Due-only work/consent checks bypass owner dispatch                                                                                                       | `owner_serves_ping_due_backup_and_mcp_consent_through_endpoint`                    |
| Sidecar runs locally; MCP stdout deadlocks; disabled startup succeeds; live consent revocation is ignored                                                | `sidecar_and_mcp_forward_to_owner_and_recheck_consent`                             |
| Direct app launch omits background argument                                                                                                              | `desktop_launch_uses_same_executable_and_background_argument`                      |
| Missing sibling app recursively starts sidecar                                                                                                           | `sidecar_without_app_reports_installation_error`                                   |
| macOS bundle is brought forward                                                                                                                          | `bundled_clients_use_launch_services_without_frontmost_window`                     |
| Launch Services drops archive/sandbox overrides                                                                                                          | `launch_services_keeps_the_client_data_root_and_sandbox`                           |
| Windows pipe grants another user access                                                                                                                  | `current_user_is_the_only_dacl_trustee` (type-checked; native Windows run pending) |

The existing scheduled-report tests retained in `owner_requests/scheduled.rs` cover
success/skip/defer classification, keyring/config/archive failure evidence, and a
failure to persist the failure ledger. The worker regression suite continues to
cover the scheduled App Lock exception and the CLI's failure exit behavior.
