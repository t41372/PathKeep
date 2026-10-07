# Area 3 / area 4 integration and Modal validation

This report records the integration of the area-4 work with search commit `4380f977`.
The rebase was later committed as `ba9cd9f7`. The microVM benchmark attempts, the account
switch and the checkpoint/cipher profiling that followed are in
[the benchmark report](2026-10-turso-area4-vm-benchmark.md).
The conflict contents are resolved and a complete Modal gate run passed. The full scale
benchmark remains incomplete: both the microVM benchmark and its gate rerun lost the Modal
connection with a DNS resolution error. No complete area-4 acceptance claim is made.
Builds, checks, tests and generated benchmark data run in Modal. The local checkout is used
only for source inspection, edits and individual-file formatting. `.modal-offload.toml`
remains untracked and is not part of the proposed repository changes.

## Conflict resolutions

| File                                | Resolution                                                                                                                                                                                                                  |
| ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `docs/architecture/data-model.md`   | Combined native vault-search ownership with native Intelligence/Agent/enrichment storage, encryption and cursor contracts; retained the outstanding maintenance boundary.                                                   |
| `docs/architecture/store-schema.md` | Kept area 3's consolidated Archive v1 and area 4's Intelligence generation views and tables. Every durable database remains v1; no migration steps were added.                                                              |
| `docs/features/intelligence.md`     | Kept complete native lexical/semantic coverage and cursor semantics, updated the Intelligence and Agent file paths, and retained area-4 queue/enrichment/output contracts.                                                  |
| `docs/plan/CHANGELOG.md`            | Retained both the search entry and the intelligence/queue/enrichment/agent entry.                                                                                                                                           |
| `vault-core/src/ai.rs`              | Kept area 3's search/assistant modules and entry points, with area 4's native ledger, report paging and working-set page type. Removed retired candidate-pool exports.                                                      |
| `vault-core/src/ai/agent_tools.rs`  | Kept area 3's complete search and coverage reporting; retained area 4's output fragments, native user-record tools and split test module. Ported area 3's changed/new search tests into that module.                        |
| `vault-core/src/ai/tests.rs`        | Retained area 3's native search fixture adapters and complete-pool regressions alongside area 4's native indexing/status/working-set suites. Native indexing tests call search directly without the legacy fixture adapter. |
| `vault-core/src/lib.rs`             | Exported the combined supported API, including `WorkingSetPage`, without reintroducing retired recall/pool constants or `prepare_query`.                                                                                    |
| `vault-core/src/test_support.rs`    | Retained native fixture helpers and area 3's search fixtures, sequence checks and handoff assertions.                                                                                                                       |
| `vault-core/src/vault.rs`           | Combined search index/session/lifetime ownership with AI/intelligence claim recovery and the shared intelligence build mutex.                                                                                               |
| `vault-worker/src/tests.rs`         | Kept native browser import/revert fixtures, native audit reads and explicit search cycles, avoiding a copy of legacy data over native writes.                                                                               |

The working-tree conflict markers are removed. Git's index remains unmerged until the user
stages the resolved files and continues the rebase; no git writes are performed here.

## Integration changes

- Assistant run and artifact persistence now uses the session's Vault and native ledgers.
  A plaintext/encrypted regression checks both databases and reloads the saved answer.
- Search freshness reads the canonical `page_content_enrichments` table. The independent
  legacy fixture adapter supplies native embedding totals and canonical cache rows, without
  copying search results or changing candidate selection/ranking.
- Enrichment replay and fan-out tests observe the durable outbox sequence and complete a real
  search cycle. Pending-row counts race area 3's acknowledgement/pruning worker. The tests
  still require exact publication deltas, both canonical variants, and no second fetch. The
  observed replay-test failure was `OwnedElsewhere` on `.pk-owner.lock`: closing Store alone
  left the search owner alive. Reopen now shuts down through Vault so that owner closes first.
- Native freshness tests retain the visibility-watermark and enrichment-change assertions.
  The superseded SQLite freshness and AI run-ledger implementations are removed.
- The public legacy AI schema helper is removed. Legacy DDL is confined to the existing
  maintenance/fixture projection opener. `archive::wipe::request_running_jobs_stop` still
  calls that opener, so removing it would require changing the excluded maintenance path.
- Native index fixtures publish the real search document/outbox and cycle vault-search. A
  fabricated document row without that handoff cannot stand in for a searchable page. The
  MCP visibility fixture likewise waits for its initial index publication, then verifies
  that a native revert hides the result before another index cycle.
- The retained Takeout source-evidence test exposed a real completion bug: an evidence-only
  payload runs no canonical batch flush, so its default watermark overwrote the prior
  `last_source_batch_id` with null. Completion now stamps the already-recorded evidence batch.
  A new plaintext/encrypted import regression checks that this advances provenance while
  preserving visit cursors. The existing three-payload regression is retained.
- The scale harness uses the original 40 GB guard on Modal and closes through Vault's owner
  lifetime, preserving the search-service shutdown order. Its allocation-counting test wrapper
  is disabled for this process; process peak RSS remains measured.

## Gates

A complete gate run passed on Modal as UID 1000 before the final benchmark-only progress
logging change. That run's test binary includes the new provenance regression (1,883 core
tests discovered); its presence and passing result are checked explicitly. After the logging
change, the microVM release build, formatting and clippy passed. The microVM rerun of the
remaining gates lost its connection before a final result was received. Logs are summarized inside the Sandbox under `/state/checklogs-acceptance`;
no gate logs are committed or copied into this checkout.

| Gate                                                                   | Result                                                                                                                         |
| ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `cargo fmt --all --check`                                              | Pass                                                                                                                           |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Pass                                                                                                                           |
| Five-crate `cargo test --no-fail-fast`                                 | Pass: core 1,845 passed / 38 ignored; worker 149 passed; all integration, search, store, owner-IPC and doc-test targets passed |
| `bun run test:desktop-bridge:rust`                                     | Pass: 25 passed                                                                                                                |
| `cargo build --workspace`                                              | Pass                                                                                                                           |

The targeted regressions also passed: 12 enrichment storage tests and 41 Takeout tests.
Existing ignored network and subprocess-only cases retain their ignore annotations. These
are Linux results; they do not execute macOS-only platform checks.

## Benchmark and profiling

The completed 14.4M-visit / 2.88M-page benchmark is still outstanding. The harness reuses one
generated encrypted archive for the full rebuild, the 10,000-visit increment and cold/warm
reads, with deletion afterward. Neither attempted run reached all measurements.

| Measurement                      | Result available                                                                                                                    |
| -------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| Full rebuild time and peak RSS   | Not obtained; gVisor run exited 137 during visit-fact derivation; microVM retry lost its Modal connection during archive generation |
| Increment after 10,000 visits    | Not measured                                                                                                                        |
| Overview cold / warm             | Not measured                                                                                                                        |
| Domain cold / warm               | Not measured                                                                                                                        |
| Sessions cold / warm             | Not measured                                                                                                                        |
| Interactive write during rebuild | 1.546 ms in the incomplete gVisor diagnostic run; not a native microVM result                                                       |

The benchmark workload is provisioned with 4 CPUs and 8 GiB, pinned to four CPUs. Rust is
pinned to 1.96.0, Turso is 0.8.1, and the release build retains debug symbols for profiling.
The retry used the offload configuration's `runtime = "vm"` microVM: Linux 7.2.9,
AMD EPYC 9654 reported by `/proc/cpuinfo`, affinity CPUs 0–3 and 8,142,816 KiB reported
`MemTotal`. No particular physical 3 GHz CPU is assumed. Its resource log reported zero
OOM kills and more than 7 GiB available memory during the observed page-generation stage.

The first attempt used Linux 4.19.0-gvisor on x86_64. It exposed 20 virtual CPUs, with the
workload pinned to 0–3 and the CPU model reported as `unknown`. That command exited 137
after 13,867 seconds, before completing visit-fact derivation. The saved log contains no
panic or completed rebuild result, so the termination cause is unconfirmed. Its 170,727,269
bytes of streamed profile evidence were preserved; its `/tmp` archive was removed with
Sandbox teardown. The updated offload skill warns that gVisor's syscall overhead makes
I/O-heavy benchmark timings unrepresentative of native hardware. This attempt is diagnostic
evidence and does not supply the final benchmark table.

The fixture has 14,400,000 visits and 2,880,000 stored page rows in one profile, across
20,000 domains. Visits are ten seconds apart, so it stresses one continuously open session;
its session-list timing does not characterize a large population of short sessions. Every
144th visit goes to the same hot page. Some stored pages consequently have no visible visit.
The read window is 2025-03-01 through 2025-04-07, and the domain probe is `site1.test`.

“Cold” means the first read in a fresh process and Vault; OS caches are not flushed. “Warm”
is the immediately repeated read in that process. Full-rebuild wall time includes the recorded
GDB sampling pauses. RSS is the workload process's high-water mark, not GDB's memory.
The diagnostic profiling evidence is recorded below. Completed microVM measurements remain unavailable.

## Diagnostic environment corrections

The first Linux run used root. Tests that deliberately remove directory/file permissions then
failed because root bypasses those permissions. The final gate run uses Ubuntu's normal user.
That diagnostic run was stopped after it established the failures; it is not a passing gate.

The large search coverage fixture also exposed slow **development-profile** execution. A GDB
sample of the remaining test showed its test thread waiting for a Store write while the Archive
writer was in `turso_core::vdbe::builder::ProgramBuilder::emit_constant_insns`, reached through
`epilogue`, SQL translation, `Connection::compile_cmd` and `prepare`. The fixture was still
seeding real search documents, rather than stuck in cursor traversal. The workspace now compiles
`turso_core` with optimization in the development profile, as it already does for cryptography.
No fixture sizes, assertions or coverage caps were reduced. This observation is separate from
the release-profile rebuild measurements below.

Modal's kernel rejected `perf stat` with `sys_perf_event_open() ... 19 (No such device)` for
`task-clock`. The fallback profiler uses GDB's asynchronous MI interface to periodically record
all-thread stacks and `/proc/<pid>/io`. It records debugger pause time separately. Stack sample
frequencies are observations, not CPU-time percentages. A non-root CPU-loop smoke check produced
15 samples and returned the child's successful exit status; no raw diagnostic logs are copied locally.

## Source freshness in restored build caches

A restored cache reused an old 1,882-test core executable while the uploaded source already
contained `mod payload_watermarks;` and the new provenance regression. This was a timestamp
ordering issue: an edit made locally while an earlier Sandbox compiled its older upload can
have an mtime earlier than that older binary's completion time. Preserving the edited file's
mtime on the next upload makes Cargo's timestamp check consider the older binary fresh.

Minimal reproduction: upload source A and start compilation; edit the local copy to B before
A finishes; save A's build cache; upload B with its original edit timestamp. B's timestamp is
older than A's executable even though its contents differ. A timestamp-only freshness check
can reuse A. The local offload configuration now touches workspace crate roots before setup,
and the final verification explicitly requires the new regression to appear and pass. No
Cargo caches or build output are moved onto the local machine, and the offload tool itself
is not modified.

## Encryption path observed in the profile

The gVisor attempt advertises `aes` and `vaes`, but the portable release fingerprint for
`aegis 0.9.20` has `features = [default, pure-rust, softaes, std]` and `rustflags = []`.
`turso_sdk_kit 0.8.1` enables `pure-rust-crypto` in its defaults, which selects Aegis's
`pure-rust` feature. In `aegis/src/pure_rust/mod.rs`, hardware AES is selected by the
compile-time `target_feature = "aes"`; otherwise it selects `aes_soft`. The sampled
rebuild executes `softaes::bitsliced::{sbox_core,block_encrypt,sub_bytes_shift_rows}`.
This is software encryption in the measured binary despite hardware AES being available.

A minimal selection reproduction on x86_64 is to compare `/proc/cpuinfo`'s AES flag with
`rustc --print cfg` under the portable build flags, inspect Aegis's `pure_rust/mod.rs`
condition above, and check its Cargo fingerprint's enabled features. The full encrypted
rebuild supplies the execution evidence. No dependency files or minimum CPU requirements
are changed for the measurement. This observation identifies an encryption contribution;
the completed microVM profile needed to quantify its share remains unavailable.

## Checkpoint work observed in the profile

The gVisor attempt's sampled checkpoint stack reaches `vault_store::checkpoint::run` at `checkpoint.rs:106`,
through Turso's `op_checkpoint` and `WalFile::checkpoint_inner`. This is application-requested
checkpoint work, not just an inferred effect of file growth.

`vault-store/src/worker.rs` runs a PASSIVE checkpoint after each Batch write. It then checks
whether its queue is empty **before replying** to that write. The intelligence producer waits
for the reply before reading and submitting its next batch, so an otherwise idle Intelligence
writer also reserves and runs TRUNCATE at that boundary. The profile therefore includes both
batched row/index persistence and repeated checkpoint work. The existing store regression
`autocheckpoint_pragma_is_ignored_and_native_threshold_is_measured` separately records that
Turso 0.8.1 ignores `wal_autocheckpoint`; this report does not assume that pragma can tune it.

At the 247th sample, the workload's logical I/O counters were 60,028,370,157 bytes read and
10,998,556,307 bytes written, across 7,956,194 read calls and 2,698,198 write calls. gVisor
reported zero for physical `read_bytes`/`write_bytes`, so these figures describe syscall
traffic, not physical disk traffic. Completed-run per-phase stack counts remain unavailable.

## Modal connection failure and cleanup

Both the microVM benchmark and gate clients exited 1 while reading the Sandbox output stream.
The exact terminal error was:

```text
ConnectionError: [Errno 8] nodename nor servname provided, or not known
```

The underlying traceback is `socket.gaierror` from `getaddrinfo`, through Modal's
`task_command_router_client._stream_stdio_with_retries`. No local build/test fallback was run:
the user explicitly required stopping if offload could not run from this sandbox.

After both clients exited, `offload ps` listed no area-4 Sandboxes. It listed only an unrelated
`offload-selftest`, which was left alone. Both benchmark data directories were under `/tmp`
inside their terminated Sandboxes; no generated archive was copied into this checkout or into
a saved `/state` directory. The benchmark table cannot be completed without a working Modal
connection and a completed run.
