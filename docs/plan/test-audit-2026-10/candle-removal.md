# Review: removing the candle embedding tier (`embedding_candle.rs`, 2056 lines)

Decision: user, 2026-10-09 (`docs/plan/STATUS.md:96`, WORK-TEST-AUDIT-2026-10 item 4). Remove the in-app
Qwen3 GGUF engine, the tests that only test it, the branches that fall back to it, and the docs that call it
shipped. Must keep working: the default `static-in-app` provider
(`models/intelligence.rs:295,304-315,351-358`, applied in `models/app.rs:32-33`) and external
OpenAI-compatible providers.

All paths below are relative to the repo root; `ai/` means `src-tauri/crates/vault-core/src/ai/`.
`EC` means `ai/embedding_candle.rs`.

## Summary

- About a third of the file is not candle: roughly 355 production lines plus roughly 295 test lines that
  survive the audit, out of 2056. The model manifest type, the on-disk layout, the SHA-256 checks, and the
  whole consent-gated download (`EC:91-105, 166-172, 296-628`) are what the static tier and the worker use
  to fetch and verify `minishlab/potion-multilingual-128M`. They move to a new module; they are not
  deleted.
- `select_embedding_provider` (`EC:194-271`) is the one selector for every tier. `backfill.rs:617` and
  `search.rs:380` reach it through the candle module. It moves and shrinks to "static sentinel → static,
  anything else → external".
- The rest is candle-only: templating, pooling, config parsing, the vendored quantized decoder, the
  provider, Metal device selection, `degrade_candle_to_external`, the sentinel, the `DEFAULT_CANDLE_*`
  constants, and about 25 tests.
- Dependencies: `candle-nn`, `candle-transformers`, `hf-hub` and the whole `metal` feature chain
  (including the `vault-metal-backend` crate) go. `candle-core` is still used by the static loader
  (`ai/embedding_static.rs:421-454`). `tokenizers` stays, because the static tier uses it.
- No shipped build could create a candle provider or download candle weights. `git grep` on
  `v0.3.0 -- src/` finds no caller of `download_ai_embedding_model` and no `candle:in-app`. So data
  migration is close to nil. `gpuEnabled` was persisted by v0.3.0's GPU section and is dropped harmlessly.
- No E2E proves semantic search works with any provider, before or after. Removing candle takes away no
  E2E coverage, but "the default still works" has no automated proof (§4).

---

## 1. Architecture map of `EC`

| Lines     | Section / items                                                                                                      | Verdict                                                                                                                                                                                                                                                                                     | Callers outside `EC`                                                                                                                                              |
| --------- | -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1-47      | Module docs (header)                                                                                                 | Delete (candle text)                                                                                                                                                                                                                                                                        | —                                                                                                                                                                 |
| 49-61     | Imports                                                                                                              | Delete                                                                                                                                                                                                                                                                                      | —                                                                                                                                                                 |
| 67-89     | `DEFAULT_CANDLE_MODEL_REPO`, `DEFAULT_CANDLE_TOKENIZER_REPO`, `DEFAULT_CANDLE_QUANT`                                 | Delete                                                                                                                                                                                                                                                                                      | re-exports `ai.rs:111-113`, `lib.rs:67-69`; `tests/candle_embedding_e2e.rs:36-37,65,191`                                                                          |
| 91-105    | `ModelFile` (manifest entry: name, sha256, repo)                                                                     | **Shared → move**                                                                                                                                                                                                                                                                           | `embedding_static.rs:32,76-91,588`; worker `model_download.rs:140,162,265-270,292`; `ai.rs:113`; `lib.rs:77`; candle e2e `:102-106`                               |
| 107-139   | `DEFAULT_CANDLE_MODEL_FILES` (pinned digests)                                                                        | Delete                                                                                                                                                                                                                                                                                      | `ai.rs:112`, `lib.rs:68`, candle e2e `:36,66`                                                                                                                     |
| 141-155   | `gguf_file_name`                                                                                                     | Delete                                                                                                                                                                                                                                                                                      | `ai.rs:115`, `lib.rs:88` only                                                                                                                                     |
| 157-164   | `CANDLE_INAPP_BASE_URL` = `"candle:in-app"`                                                                          | Delete                                                                                                                                                                                                                                                                                      | `ai.rs:111`, `lib.rs:67`, `ai/tests.rs:3024`, doc link `embedding_static.rs:59`                                                                                   |
| 166-172   | `model_dir_for_repo` (one flattened dir per repo under `models_dir`)                                                 | **Shared → move**                                                                                                                                                                                                                                                                           | `embedding_static.rs:34,176,589`; `read_model.rs:291,794` (via `use super::*`, `read_model.rs:13`); worker test `model_download.rs:262`; `ai.rs:115`, `lib.rs:89` |
| 174-192   | `runtime_uses_candle`, `candle_repo_for_runtime`                                                                     | Delete                                                                                                                                                                                                                                                                                      | `ai.rs:114,117`, `lib.rs:85,94` only                                                                                                                              |
| 194-243   | `select_embedding_provider` (real build: static → candle (gated on verified files) → external)                       | **Shared → move + simplify**                                                                                                                                                                                                                                                                | `ai/indexing/backfill.rs:617`, `ai/search.rs:380`; `ai.rs:117`; `lib.rs:95` (no caller outside vault-core)                                                        |
| 245-271   | `select_embedding_provider` (`cfg(any(test, coverage))` twin)                                                        | Collapse into the one above (identical once candle goes; the static stub lives in `embedding_static.rs:191-201`)                                                                                                                                                                            | same                                                                                                                                                              |
| 273-294   | `degrade_candle_to_external`                                                                                         | Delete. The external arm can't be reached: it is only called when `base_url` is the candle sentinel (audit `test-audit-2026-10/vault-core-ai.md:264-267`)                                                                                                                                   | `ai.rs:114`, `lib.rs:86`                                                                                                                                          |
| 296-315   | `model_is_present_and_verified` (full SHA-256 of every manifest file)                                                | **Shared → move**                                                                                                                                                                                                                                                                           | `embedding_static.rs:177`; `read_model.rs:292`; `ai.rs:116`; `lib.rs:90`                                                                                          |
| 317-331   | `VERIFIED_MARKER_FILE`, `write_verified_marker` (private)                                                            | **Shared → move** (required by `model_is_loadable` and `ensure_model_downloaded`)                                                                                                                                                                                                           | internal only                                                                                                                                                     |
| 333-348   | `model_is_loadable` (marker plus non-empty files; cheap per-load check)                                              | **Shared → move**                                                                                                                                                                                                                                                                           | `embedding_static.rs:590`; `ai.rs:116`; `lib.rs:89`                                                                                                               |
| 350-365   | `verify_file_sha256`                                                                                                 | **Shared → move** (used by `ensure_model_downloaded`)                                                                                                                                                                                                                                       | doc link `embedding_static.rs:74`                                                                                                                                 |
| 371-394   | `ModelDownloadProgress` trait (`file_started` / `file_progress` / `file_finished` / `cancelled`)                     | **Shared → move** (cancellation and progress)                                                                                                                                                                                                                                               | worker `model_download.rs:7,28,55-81`; `models/intelligence.rs:1254` (doc); `ai.rs:113`; `lib.rs:77`                                                              |
| 396-399   | `NoopDownloadProgress`                                                                                               | **Shared → move**                                                                                                                                                                                                                                                                           | `tests/static_embedding_e2e.rs:53,78`; candle e2e `:38,62`; `ai.rs:113`; `lib.rs:77`                                                                              |
| 401-413   | `download_needed` (offline-first, per file)                                                                          | **Shared → move**                                                                                                                                                                                                                                                                           | internal only                                                                                                                                                     |
| 415-482   | `ensure_model_downloaded` (consent gate, offline-first, verify, marker, cancel between files)                        | **Shared → move**. Reword the two candle error strings at `EC:452` and `EC:461`. A cancelled _static_ download carries "candle model download … was cancelled" in its `error` event, but nobody sees it: `src/lib/ipc/model-download.ts:217-222` drops the message when the user cancelled. | worker `model_download.rs:4,29,169`; `tests/static_embedding_e2e.rs:53,79`; candle e2e `:39,63`; `ai.rs:115`; `lib.rs:88`                                         |
| 484-491   | `DOWNLOAD_CHUNK_BYTES`, `DOWNLOAD_PROGRESS_STRIDE_BYTES`                                                             | **Shared → move**                                                                                                                                                                                                                                                                           | internal                                                                                                                                                          |
| 493-565   | `stream_to_file`, `PartialFileGuard` (throttled progress, cancel mid-stream, removes partial file)                   | **Shared → move**                                                                                                                                                                                                                                                                           | internal                                                                                                                                                          |
| 567-604   | `fetch_one_file` and `open_download_stream` (real build; `ureq::get` of `huggingface.co/{repo}/resolve/main/{file}`) | **Shared → move**                                                                                                                                                                                                                                                                           | internal                                                                                                                                                          |
| 606-628   | `fetch_one_file` and `stub_file_bytes` (`cfg(any(test, coverage))` test double)                                      | **Shared → move** as is. Removing it is the STATUS item 4 "test doubles in production" work, not this change.                                                                                                                                                                               | internal                                                                                                                                                          |
| 630-667   | `QWEN3_QUERY_TASK`, `query_instruction_template`, `apply_role_instruction` (instruction templating)                  | Delete                                                                                                                                                                                                                                                                                      | `ai.rs:114,116`, `lib.rs:78,83`; candle e2e `:38,95,226`                                                                                                          |
| 669-684   | `last_token_pool` (pooling helper)                                                                                   | Delete. Only the stub embed calls it (audit `vault-core-ai.md:879`).                                                                                                                                                                                                                        | `ai.rs:115`                                                                                                                                                       |
| 686-720   | `CandleModelMetadata`, `parse_model_metadata`, `CANDLE_MAX_INPUT_TOKENS`                                             | Delete                                                                                                                                                                                                                                                                                      | `ai.rs:111` (`CANDLE_MAX_INPUT_TOKENS`)                                                                                                                           |
| 722-1032  | `mod quant_engine` (vendored quantized Qwen3 decoder: RoPE, attention, KV cache, `QuantizedQwen3Embedding`)          | Delete. This is the only user of `candle-nn` and `candle-transformers` (`EC:734-739`).                                                                                                                                                                                                      | none                                                                                                                                                              |
| 1034-1074 | `CandleEmbeddingProvider`, `CandleEngine`                                                                            | Delete                                                                                                                                                                                                                                                                                      | `embedding_external.rs:26,451,848-854`; `ai.rs:111`; `lib.rs:67`; candle e2e                                                                                      |
| 1076-1123 | `SelectedDevice`, `select_device`, `candle_device_for` (Metal; `cfg(feature = "metal")`)                             | Delete                                                                                                                                                                                                                                                                                      | none                                                                                                                                                              |
| 1125-1260 | `impl CandleEmbeddingProvider` (`model_id_for`, `load_default`, `load`, `new_stub`, `descriptor_with`)               | Delete                                                                                                                                                                                                                                                                                      | candle e2e `:73,90`; `embedding_external.rs:848`                                                                                                                  |
| 1262-1367 | `EmbeddingProvider` impl, `embed_impl` (real and stub)                                                               | Delete                                                                                                                                                                                                                                                                                      | via `AnyEmbeddingProvider::Candle` (`embedding_external.rs:451,476,485,493`)                                                                                      |
| 1369-2056 | `#[cfg(test)] mod tests`                                                                                             | Split (table below)                                                                                                                                                                                                                                                                         | —                                                                                                                                                                 |

Not part of this file, and **kept as they are** (they belong to the trait's general vocabulary, not to
candle): `EmbeddingRole` (`ai/traits.rs:27-47`, enum at `:33`; every caller still passes it), `EmbeddingPooling::LastToken`
(`traits.rs:94,105`; `Cls` has no producer either), and `EmbeddingDescriptor.instruction_template`
(`traits.rs:132`). On disk only the fingerprint _hash_ is stored (`vector_store.rs:97,144`,
`vector_planes.rs:72`, `reverse_visit_map.rs:77`). The variant therefore never needs to deserialize, so
keeping or dropping it is safe for data. Keep it.

The `AnyEmbeddingProvider::Candle` arm lives in `embedding_external.rs`: variant `:442-451`, match arms
`:476,485,493`, doc text `:437-438,454-457,465`, test `:844-862`. All of it goes.

Consent flow: the gate is `consented: bool` on `ensure_model_downloaded` (`EC:450-454`). The only caller is
the worker's static download, which passes `true` because pressing "Download" is the consent
(`vault-worker/src/intelligence/model_download.rs:13-21,169`). Cancellation: `ModelDownloadProgress::cancelled`
is checked between files (`EC:460`) and inside each file (`EC:523`), driven by the worker's global flag
(`model_download.rs:35-48`). Both move unchanged.

### Test disposition (`EC:1369-2056`)

Ratings are from `docs/plan/test-audit-2026-10/vault-core-ai.md:873-906`.

| Test (line)                                                                                                                                                       | Rating                           | Disposition                                                                              |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------- | ---------------------------------------------------------------------------------------- |
| `model_dir_for_repo_flattens_slash_into_one_segment` (1474)                                                                                                       | D                                | Move                                                                                     |
| `verify_file_sha256_accepts_match_and_rejects_mismatch` (1535)                                                                                                    | D                                | Move                                                                                     |
| `verify_file_sha256_errors_on_missing_file` (1546)                                                                                                                | C                                | Delete (audit)                                                                           |
| `leak_manifest` helper (1552)                                                                                                                                     | —                                | Move                                                                                     |
| `model_is_present_and_verified_requires_all_files_and_matching_digests` (1563)                                                                                    | D                                | Move                                                                                     |
| `model_is_loadable_requires_marker_and_present_nonempty_files` (1585). Rename the `w.gguf` fixture name.                                                          | D                                | Move                                                                                     |
| `stub_manifest`, `RecordingProgress` (1810-1838). Rename the `weights.gguf` fixture name.                                                                         | —                                | Move                                                                                     |
| `ensure_download_*` (1841, 1852, 1868, 1885, 1902, 1925, 1940)                                                                                                    | D                                | Move                                                                                     |
| `download_needed_is_false_only_for_present_and_matching_file` (1960), `noop_progress_defaults_are_inert` (1977)                                                   | C                                | Delete (audit)                                                                           |
| `StreamRecorder`, `stream_to_file_*` (1988-2056)                                                                                                                  | D                                | Move                                                                                     |
| 1375-1411 helpers; 1414, 1427, 1437, 1443, 1450, 1463, 1469, 1486, 1494, 1505, 1518, 1524, 1608, 1640, 1665, 1676, 1694, 1703, 1709, 1728, 1749, 1781, 1790, 1801 | C/D, all candle                  | Delete                                                                                   |
| `select_embedding_provider_routes_external_arm` (1763)                                                                                                            | C (tests the cfg(test) selector) | Delete (audit). The selector's external routing is proved through backfill/search tests. |

Outside `EC`: `embedding_external.rs:844-862` (`any_provider_dispatches_candle_variant`) goes;
`ai/tests.rs:3023-3024` asserts that the candle sentinel passes through `normalize_local_base_url`; change it
to `STATIC_INAPP_BASE_URL` (same no-`://` shape).
`src-tauri/crates/vault-core/tests/candle_embedding_e2e.rs` (280 lines, env-gated S1 benchmark and
LM Studio parity) is deleted in full.

---

## 2. Every reference across the workspace

Searches used: `git ls-files | xargs grep -i` for `candle`, `qwen`, `gguf`, `gpu_enabled|gpuEnabled`,
`metal`, `heavy tier`, `LastToken|last-token`, `Q8_0`, `hf_hub|hf-hub`, `tokenizers::`, `candle_core`,
plus every symbol `ai.rs:110-118` re-exports.

### 2.1 Rust, vault-core `src/` (outside `EC`)

| Location                                                                                              | What it does                                                                                                      | Action                                                                                                              |
| ----------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `ai.rs:18`                                                                                            | `mod embedding_candle;`                                                                                           | Replace with the new modules (§3)                                                                                   |
| `ai.rs:110-118`                                                                                       | `pub use self::embedding_candle::{…}` (25 names)                                                                  | Keep the shared names (from the new module) and drop the candle ones                                                |
| `lib.rs:66-95`                                                                                        | Crate-root re-exports of the same names                                                                           | Same                                                                                                                |
| `ai/embedding_external.rs:15`                                                                         | Header: "in-app candle inference (W-AI-4b adds…)"                                                                 | Reword                                                                                                              |
| `ai/embedding_external.rs:26`                                                                         | `use super::embedding_candle::CandleEmbeddingProvider`                                                            | Delete                                                                                                              |
| `ai/embedding_external.rs:195-198, 238, 251`                                                          | Comments that say candle is coming in 4b                                                                          | Reword                                                                                                              |
| `ai/embedding_external.rs:432-497`                                                                    | `AnyEmbeddingProvider` with the `Candle` variant and arms                                                         | Drop the variant; move the enum (§3 step 2)                                                                         |
| `ai/embedding_external.rs:844-862`                                                                    | Candle dispatch test                                                                                              | Delete                                                                                                              |
| `ai/embedding_static.rs:13-14, 19, 27-30, 45, 58-60, 66, 74, 118, 143, 166, 363, 412, 540, 565, 1122` | Doc text about candle's helpers and candle's selector                                                             | Reword (and fix the `degrade_static_to_external` claim, §3 related item C)                                          |
| `ai/embedding_static.rs:32, 34`                                                                       | `use super::embedding_candle::{ModelFile, model_dir_for_repo}`                                                    | Point at the new module                                                                                             |
| `ai/embedding_static.rs:177, 590`                                                                     | `super::embedding_candle::model_is_present_and_verified` / `model_is_loadable`                                    | Point at the new module                                                                                             |
| `ai/embedding_static.rs:421-454`                                                                      | `candle_core::safetensors::load`, `DType::F32`, `Device::Cpu` (static matrix loader)                              | Keep, or replace (§3 decision A)                                                                                    |
| `ai/indexing/backfill.rs:157-159`                                                                     | `let gpu_enabled = config.ai.gpu_enabled;` with Metal comment                                                     | Delete                                                                                                              |
| `ai/indexing/backfill.rs:258, 609`                                                                    | `gpu_enabled` passed into and declared on `run_embedding_backfill`                                                | Delete                                                                                                              |
| `ai/indexing/backfill.rs:613-617`                                                                     | Comment, and the call to `embedding_candle::select_embedding_provider(paths, provider, gpu_enabled)`              | Call the moved selector; no gpu argument                                                                            |
| `ai/indexing/backfill.rs:733, 833`                                                                    | Comments naming candle = LastToken                                                                                | Reword                                                                                                              |
| `ai/indexing/backfill.rs:31`                                                                          | "bounded heavy-tier working set" doc                                                                              | Reword or remove with related item B                                                                                |
| `ai/search.rs:377-380`                                                                                | Comment and `embedding_candle::select_embedding_provider(paths, provider, false)` for the fingerprint-drift check | Call the moved selector                                                                                             |
| `ai/provider.rs:75`                                                                                   | `normalize_local_base_url` doc mentions the candle sentinel                                                       | Reword                                                                                                              |
| `ai/provider.rs:172, 179-180`                                                                         | `embedding_descriptor_for` comments that say candle is coming                                                     | Reword                                                                                                              |
| `ai/traits.rs:29, 135, 144`                                                                           | Docs: "Qwen3 family", "in-app candle"                                                                             | Reword; no code change                                                                                              |
| `ai/tests.rs:3023-3024`                                                                               | Sentinel passthrough assertion                                                                                    | Use `STATIC_INAPP_BASE_URL`                                                                                         |
| `ai/tests_native/indexing.rs:522`                                                                     | Comment "(not the candle/external stub dim)"                                                                      | Reword                                                                                                              |
| `ai/working_set.rs:23-24, 69, 160`                                                                    | "heavy tier" docs (the selector is shared with content fetch)                                                     | Reword                                                                                                              |
| `ai/visit_content_map.rs:6`                                                                           | "search/heavy-tier resolve…" doc                                                                                  | Reword                                                                                                              |
| `models/intelligence.rs:321, 346`                                                                     | Docs: "external / candle provider"                                                                                | Reword                                                                                                              |
| `models/intelligence.rs:532-541`                                                                      | `AiSettings.gpu_enabled` (`#[serde(default)]`)                                                                    | Delete field                                                                                                        |
| `models/intelligence.rs:683-684`                                                                      | Default `gpu_enabled: false`                                                                                      | Delete                                                                                                              |
| `models/intelligence.rs:988, 995, 999`                                                                | `ReembedScope` docs: "GPU/heavy-tier settings UI"                                                                 | Reword (related item B)                                                                                             |
| `models/intelligence.rs:1254`                                                                         | `ModelDownloadProgressEvent` doc links `crate::ModelDownloadProgress`                                             | Still valid after the move (re-export)                                                                              |
| `models/intelligence.rs:2533-2535, 2538-2547`                                                         | gpu default assertion, `ai_settings_gpu_enabled_round_trips_when_present`                                         | Delete the assertion and the test. Replace with "an old config carrying `gpuEnabled: true` still parses" if wanted. |

**Keep (look like candle hits, but are not):** `ai/fingerprint.rs:150-154, 249-292` (golden-vector test data:
model name strings, `LastToken`); `tests/lmstudio_embedding_e2e.rs:13,32` and
`tests/lmstudio_agent_e2e.rs:66,263` (`text-embedding-qwen3-embedding-0.6b` is the LM Studio _external_
model); `visit_taxonomy/sites/international.rs` (a `qwen` site rule).

### 2.2 Rust, other crates

| Location                                                                                 | What it does                                                                                                   | Action                                                                                             |
| ---------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `vault-worker/src/intelligence/model_download.rs:1-21`                                   | Header says "multi-hundred-MB GGUF"; notes the candle command was removed                                      | Reword (static only)                                                                               |
| `vault-worker/src/intelligence/model_download.rs:27-29, 55, 140, 162, 169, 262-270, 292` | Uses `ModelFile`, `ModelDownloadProgress`, `ensure_model_downloaded`, `model_dir_for_repo` from the crate root | No change if `lib.rs` keeps re-exporting them. Rename the `weights.gguf` fixture at `:263`.        |
| `vault-worker/Cargo.toml:7-11`                                                           | `metal = ["vault-core/metal"]` passthrough                                                                     | Delete                                                                                             |
| `vault-metal-backend/` (`Cargo.toml` 29 lines, `src/lib.rs` 13 lines, no code)           | Exists only to turn on candle's `metal` feature on macOS                                                       | Delete the crate                                                                                   |
| `src-tauri/Cargo.toml:8-17`                                                              | Desktop `metal` feature forwarding to worker/core                                                              | Delete                                                                                             |
| `src-tauri/Cargo.toml:31`                                                                | Workspace member `crates/vault-metal-backend`                                                                  | Delete                                                                                             |
| `src-tauri/Cargo.toml:45-49`                                                             | Workspace deps `candle-core`, `candle-nn`, `candle-transformers` 0.10.2                                        | Drop nn and transformers; core per decision A                                                      |
| `src-tauri/Cargo.toml:53-55`                                                             | `hf-hub` 0.5.0 (`ureq` feature)                                                                                | Delete. **No Rust code uses `hf_hub`**; the download already calls `ureq` directly (`EC:592-604`). |
| `src-tauri/Cargo.toml:56-60`                                                             | `ureq` 3.3.0, with a comment saying it was promoted from hf-hub                                                | Keep (the moved download uses it); reword the comment                                              |
| `src-tauri/Cargo.toml:79-88`                                                             | `tokenizers` 0.22, with a comment about candle-core forcing `onig`                                             | Keep the dep. If decision A drops candle-core, the `onig` caveat goes too: rewrite the comment.    |
| `src-tauri/crates/vault-core/Cargo.toml:7-43`                                            | Feature docs and `metal = ["dep:vault-metal-backend"]`                                                         | Delete                                                                                             |
| `vault-core/Cargo.toml:50-54`                                                            | `candle-core/nn/transformers`                                                                                  | As for the workspace                                                                               |
| `vault-core/Cargo.toml:60-61`                                                            | `hf-hub`                                                                                                       | Delete                                                                                             |
| `vault-core/Cargo.toml:81-84, 86-88`                                                     | `tokenizers`, `ureq`, with comments about the candle forward pass and hf-hub                                   | Keep; reword the comments                                                                          |
| `vault-core/Cargo.toml:99-107`                                                           | macOS-only optional `vault-metal-backend` dep                                                                  | Delete                                                                                             |
| `src-tauri/Cargo.lock` (17 candle lines)                                                 | —                                                                                                              | Regenerated by a build on Modal                                                                    |

Reverse dependencies in `Cargo.lock` (parsed from the lock file):

- `candle-nn` ← candle-transformers, vault-core, vault-metal-backend.
- `candle-transformers` ← vault-core, vault-metal-backend.
- `hf-hub` ← vault-core only. It also brings in `indicatif`, and `rand` through hf-hub.
- `tokenizers` ← candle-core, vault-core. `onig`/`onig_sys` ← tokenizers; the feature comes from candle-core.
- `safetensors` (0.4.5 and 0.7.0) ← candle-core, candle-nn, `ug`.
- `ureq` ← hf-hub, `libsodium-sys-stable`, vault-core.

The `deny.toml:4-26` ignores are not candle-specific: `paste` still comes in through `stronghold_engine` and
`rand` through stronghold, turso and nanoid. But `unused-ignored-advisory = "deny"` (`deny.toml:29`), so
`cargo deny check` must run on the new graph.

### 2.3 Tauri app crate (`src-tauri/src`)

- `commands/ai_chat.rs:62` is a doc comment: "reuses the same SHA-256-verified download path as the heavy
  tier". Reword it.
- There are no candle commands left. `download_ai_embedding_model` was removed in 2026-10 (`CHANGELOG.md:2586`,
  `desktop-command-surface.md:153-157`). The static commands stay unchanged: `lib.rs:275-276`,
  `commands/ai_chat.rs:66-80`, `worker_bridge/intelligence/ai.rs:199-209`,
  `dev_ipc_bridge/dispatch.rs:576-583,1154-1164`.

### 2.4 Frontend (`src/`)

| Location                                     | What it does                                                                     | Action         |
| -------------------------------------------- | -------------------------------------------------------------------------------- | -------------- |
| `src/lib/types/intelligence.ts:179-185`      | `AiSettings.gpuEnabled?` with Metal doc                                          | Delete         |
| `src/lib/types/intelligence.ts:252`          | Comment: "static tier and the heavy candle tier emit on this single channel"     | Reword         |
| `src/lib/backend-client/intelligence.ts:155` | Comment: "(static or heavy tier; …share one cancel flag)"                        | Reword         |
| `src/lib/types/intelligence.ts:616-618`      | `ReembedScope` includes `'working-set'`; nothing in the frontend uses that value | Related item B |

There is no settings UI, hook, or component for candle or GPU. v0.3.0's `src/pages/settings/ai-gpu-section.tsx`
no longer exists (it is still cited in `BACKLOG.md:693`).

### 2.5 i18n

None. `git ls-files src/lib/i18n | xargs grep -i 'metal|gpu|qwen|candle|heavy|transformer|gguf'` matches only
`format.ts:94-95` (a byte-size `1024`). No en, zh-CN, or zh-TW copy needs to change.

### 2.6 E2E, scripts, CI, Modal

- `tests/e2e/`: no candle references.
- `scripts/`: none. `scripts/gen_static_parity_fixture.py` is static-only.
- `.github/workflows/*.yml`, `.modal-offload.toml`: none (no features flag, no model cache).
- `package.json:30` runs `lint:rust` with `--all-features`. Today that turns on `metal` (inert on Linux, real
  on macOS). After removal the flag only covers `devtools-bridge` and `test-fixtures`; no edit is needed.

### 2.7 Ignore files and artifacts

- `.gitignore:17-24`: comment says "(candle ~1.2 GB, static potion ~512 MB)"; patterns
  `artifacts/candle-models/`, `**/candle-models/`. Remove the candle lines and keep the static ones.
- `.prettierignore:3-6`: `artifacts/candle-models`, `**/candle-models`. Remove.
- `artifacts/benchmarks/2026-06-21-wai4b-candle-s1.md`: **keep**. It is the measured evidence (1.25 docs/s
  on CPU) for removing the tier.

### 2.8 Docs

Live docs that describe candle as shipped. **Must fix:**

| Location                                           | Says                                                                                   | Fix                                                                         |
| -------------------------------------------------- | -------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `docs/features/intelligence.md:7`                  | "in-app（candle Qwen3）+ external embedding" shipped                                   | Change to the static model plus external                                    |
| `docs/features/intelligence.md:71`                 | Vector similarity runs on in-app (candle Qwen3) or external                            | Same                                                                        |
| `docs/features/intelligence.md:384`                | Already says candle was removed. This contradicts `:7` and `:71`.                      | Keep; align the others with it                                              |
| `docs/features/intelligence-current-state.md:8`    | "in-app（candle Qwen3）+ external embedding"                                           | Fix                                                                         |
| `docs/architecture/tech-stack.md:27`               | "model2vec static base + candle Qwen3 quality tier … Metal GPU tier"                   | Static only; drop Metal                                                     |
| `docs/architecture/tech-stack.md:31`               | "in-app candle" under AI inference                                                     | Fix                                                                         |
| `docs/architecture/tech-stack.md:49`               | "in-app embedding（candle）"                                                           | Fix                                                                         |
| `docs/architecture/tech-stack.md:54`               | "雙 `EmbeddingProvider`… in-app candle Qwen3"                                          | Change to static plus external                                              |
| `docs/architecture/ai-security-posture.md:293-303` | "The GPU tier is an opt-in reproducible-build feature…" (Metal, `vault-metal-backend`) | Delete the bullet                                                           |
| `docs/architecture/ai-security-posture.md:366-372` | "A Metal CI lane must re-run `cargo deny`…"                                            | Delete                                                                      |
| `docs/architecture/data-model.md:433`              | "in-app candle embedding 下載的模型權重"                                               | Change to the static model's weights                                        |
| `docs/plan/program/repo-baseline.md:36, 174, 192`  | Giant-file rows for `embedding_candle.rs`                                              | Remove the rows; add the new module only if it is ≥1000 lines (it won't be) |
| `docs/plan/BACKLOG.md:721, 723`                    | "external/candle embedding transport seam"                                             | External only                                                               |
| `docs/plan/BACKLOG.md:693`                         | Cites `src/pages/settings/ai-gpu-section.tsx` (gone)                                   | Fix the stale path                                                          |

Historical or accepted plan docs. Add a dated note; do **not** rewrite (AGENTS.md principle 7):

- `docs/plan/program/ai-redesign-2026/02-architecture-decisions.md:15, 73, 91, 138, 188, 209`: D2, candle
  as the in-app engine.
- `05-embedding-retrieval-revision.md:5, 14, 42, 47, 49, 57-59, 67`: three tiers, Qwen3 working set, Metal.
- `README.md:3`, `03-implementation-plan.md:10, 32, 72, 97`, `04-current-state-and-execution.md:3, 48, 61,
71, 73, 78, 92, 115-119, 141`, `01-research-findings-2026.md:70-74, 172-180`, and the
  `research-appendix/*`: research and history; leave them alone.

Append-only or point-in-time (leave alone):

- `docs/plan/CHANGELOG.md:2297, 2330-2335, 2376, 2484, 2491, 2586`.
- `docs/plan/test-audit-2026-10/vault-core-ai.md:53, 222-231, 262-267, 873-906` and
  `vault-core-intelligence.md:286, 770-777`.
- `docs/plan/test-audit-2026-10/README.md:84` lists the candle tier as "a user decision first". Mark it
  decided.
- `docs/plan/STATUS.md:95-96`: tick it off at close.

---

## 3. Removal plan

### Ordered steps

1. **Record the decision.** Write a short ADR, `docs/architecture/decisions/013-remove-candle-embedding-tier.md`.
   Cover:
   - Why: CPU throughput of 1.25 docs/s (S1 artifact); no UI entry; no download command since 2026-10; the
     user's decision of 2026-10-09.
   - What remains: static by default, plus external providers.
   - What it supersedes: D2/C.1 in `02-architecture-decisions.md` and the Qwen3 and Metal tiers in
     `05-…revision.md` §tier/§7.
     Add a one-line dated "superseded by ADR-013" note at the top of those two docs, and a row in the
     index `docs/architecture/decisions/README.md` (the numbers already skip 008, so 013 is the next free
     one). Principle 7 asks for a trade-off record. The user's consent is in `STATUS.md:96`.

2. **New module `ai/model_files.rs`** (or `model_download.rs`). Move everything marked "Shared → move" in
   §1: `EC:91-105, 166-172, 296-628`, about 360 production lines, plus the moved tests (about 330 lines).
   The module stays well under 1000 lines.
   - Header. Responsibilities: model manifest entry, on-disk layout under `models_dir`, SHA-256 integrity,
     the consent-gated streaming download with cancel and progress. Not responsible for: any engine,
     choosing which model.
   - Reword `EC:452` and `EC:461` to "model {repo}" (no "candle").
   - In tests, rename the `*.gguf` fixture names (`EC:1588, 1813`; worker `model_download.rs:263`).
   - `embedding_static.rs:32,34,177,590` import from it.
   - `ai.rs` re-exports the same public names. `read_model.rs` (`use super::*`), the worker and
     `static_embedding_e2e.rs` then compile unchanged.

3. **New module `ai/embedding_engine.rs`.**
   - Move `AnyEmbeddingProvider` and its `EmbeddingProvider` impl (`embedding_external.rs:432-497`) into it,
     without `Candle`.
   - Add one `select_embedding_provider(paths, runtime) -> Result<AnyEmbeddingProvider>`:
     `runtime_uses_static` → `select_static_embedding_provider`, else `External`. There is no
     `gpu_enabled` parameter and no cfg twin; the static stub keeps its own twin at
     `embedding_static.rs:191-201`.
   - Why a new module: `embedding_external.rs` then holds only the external adapter, which is what its
     header says. Together with related item C it also ends the `embedding_external` ↔ `embedding_static`
     import cycle (`embedding_external.rs:27` ↔ `embedding_static.rs:35`). The move alone does not:
     `degrade_static_to_external` (`embedding_static.rs:148-162`) still builds an `ExternalEmbeddingProvider`
     until item C collapses it.
   - Update `backfill.rs:613-617` and `search.rs:377-380`.

4. **Remove `gpu_enabled`.** `backfill.rs:157-159, 258, 609`; `models/intelligence.rs:532-541, 683-684,
2533-2547`; `src/lib/types/intelligence.ts:179-185`.

5. **Delete** `EC` (what is left after step 2), `tests/candle_embedding_e2e.rs`, and
   `embedding_external.rs:844-862`. Remove `mod embedding_candle` (`ai.rs:18`) and the candle names from
   `ai.rs:110-118` and `lib.rs:66-95`. Fix `ai/tests.rs:3023-3024`.

6. **Comments and docstrings** listed in §2.1 to §2.4 (Rust and TS). No behaviour change.

7. **Cargo.**
   - Delete `candle-nn`, `candle-transformers`, `hf-hub`; every `metal` feature (desktop, worker, core); the
     `vault-metal-backend` crate and its workspace member.
   - Then decision A for `candle-core`.
   - Update the dependency comments (`src-tauri/Cargo.toml:45-60, 79-88`; `vault-core/Cargo.toml:50-61,
81-88`).

8. **Ignore files** (§2.7). **Docs** (§2.8). Then STATUS, CHANGELOG and `repo-baseline.md`.

9. **Gate on Modal, never locally.**
   - `touch` the edited `.rs` files first (stale-build memory).
   - Run `bun run check`, `bun run check:slow` (supply-chain: `cargo deny check` and `audit:rust` on the
     smaller graph), and `bun run lint:rust`.
   - If decision A replaces the loader, also run the env-gated `PATHKEEP_STATIC_PARITY=1` and
     `PATHKEEP_STATIC_S1=1` tests (`tests/static_embedding_e2e.rs`) on Modal. They are the only real-model
     proof that the static loader is unchanged.
   - Run `offload ps` at the end.

### Decision A: `candle-core` (one is recommended)

- **Recommended: replace it with the `safetensors` crate.** Rewrite `load_static_matrix`
  (`embedding_static.rs:416-469`) on `safetensors::SafeTensors::deserialize`.
  - Read the `[vocab, dim]` tensor as F32 bytes. Reject any other dtype: the default potion export is f32
    (`embedding_static.rs:401`). A hand-written f16/bf16 widening is the alternative if wider support is
    wanted.
  - Supply chain: Hugging Face maintains `safetensors` (a known org), and it is already in the lock through
    candle-core, so no new author enters the graph. Pin `0.7.0`, the version candle-core and candle-nn
    0.10.2 resolve today. The lock's other copy, `0.4.5`, is only reached through `ug` and leaves with
    candle.
  - What goes with candle-core: the C `onig_sys` build that candle-core forces
    (`src-tauri/Cargo.toml:82-86`), `gemm*`, `ug*`, `float8`, `rand_distr`, and part of `half`. `tokenizers`
    becomes pure-Rust `fancy-regex`.
  - Risk: the potion Unigram tokenizer must still load without the `onig` feature. The env-gated static
    parity test proves it (step 9).
- **Alternative: keep `candle-core` only for the static loader.** The diff is smaller, but a GPU-capable ML
  framework stays in the build for one file read, and so does the `onig` C build.

### Migration concerns

| Concern                                                            | Evidence                                                                                                                                                                                                                                                             | Handling                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Saved provider with `base_url = "candle:in-app"`                   | No shipped UI ever wrote it: `git grep candle v0.3.0 -- src/` finds only a comment, and there was no candle preset. A hand-typed or dev-bridge row would, after removal, go to `External` and fail at send with the endpoint `candle:in-app/embeddings`.             | **Recommended: no migration code.** No shipped build can produce the row. If one exists, it fails at send with a transport error. That is the same way any base URL without a scheme fails today: `normalize_local_base_url` accepts and returns scheme-less input (`provider.rs:71-72, 80-82`; test `ai/tests.rs:2998`), but nothing adds a scheme before reqwest. The endpoint is a plain `format!("{trimmed}/embeddings")` (`embedding_external.rs:240-242`), and `url` parsing either rejects `127.0.0.1:1234/…` or reads `host:` as a non-http scheme. (That last point comes from the `url` crate's rules; it was not run.) A general `http(s)://` check in `validate_provider` (`provider.rs:318-343`) would turn that into a config-time error. It is a separate fix for every scheme-less base, not part of this change. **Alternative:** in `merge_embedding_providers` (`models/intelligence.rs:325-341`), drop rows whose `base_url` is the old sentinel, so that `ensure_embedding_provider_default` (`:351-358`) falls back to `static-in-app`. This keeps a legacy constant in `models/` forever for a row nobody can have. |
| `gpuEnabled` in saved `AiSettings`                                 | v0.3.0's GPU section persisted it. `AiSettings` is `#[serde(rename_all = "camelCase", default)]` (`models/intelligence.rs:468-471`). No `deny_unknown_fields` anywhere in `models/`.                                                                                 | Nothing to do. An unknown key is ignored on load and dropped on the next save.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Vectors embedded by a candle provider                              | Stores are keyed by provider id and model (`vector_store.rs:114`; `backfill.rs:220-224`). Only the fingerprint hash is persisted (`vector_store.rs:97`). No sweep of stores for providers missing from config was found (grep of `vector_store.rs`, `ai/indexing/`). | No user can have one (concern 1). If a dev install has one, the files are rebuildable derived data in `derived/vectors/`. If the row were rerouted to External, the descriptor would differ (pooling `Unknown` vs `LastToken`, no instruction), so search reports `ConfigDriftFingerprint` and falls back to lexical (`search.rs:385-393`). No code needed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Downloaded candle weights (`models_dir/<Qwen…GGUF>`, about 640 MB) | Only the never-called command or the env tests could create them.                                                                                                                                                                                                    | Nothing to do. `wipe.rs:272` removes all of `models_dir`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| IPC contract                                                       | `AiSettings.gpuEnabled` was optional in TS and never read. The progress event (`MODEL_DOWNLOAD_PROGRESS_EVENT`) and the static commands are unchanged. `ReembedScope` is unchanged (item B).                                                                         | Desktop contract (`src/lib/ipc/bridge.ts`) untouched.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Settings keys or ledger                                            | No settings key besides `gpuEnabled`. The provider-keyed AI index ledger has no candle rows.                                                                                                                                                                         | —                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |

### Related decisions found during the review (kept separate on purpose)

- **A.** The candle-core choice, above.
- **B. `ReembedScope::WorkingSet`.** It was added for the heavy tier: `models/intelligence.rs:988-1001`,
  `backfill.rs:31-60` (`working_set_canonical_urls`), `working_set.rs:160-185`
  (`estimate_reembed` working-set arm). Its tests are `tests_native/working_set.rs:194-260` and
  `models/intelligence.rs:2561`. The frontend never sends `'working-set'` (`use-semantic-index.ts:143`
  defaults to `'full'`; `ai-section.tsx:137-138` estimates `'full'`). Without a heavy tier it re-embeds a
  subset with the same engine. The selector `select_working_set` stays, because content fetch uses it
  (`online-work-rows.tsx:65`). Recommendation: remove the scope variant in the same STATUS item, as a
  separate commit, because it changes the IPC enum.
- **C. `degrade_static_to_external`** (`embedding_static.rs:148-162`). Its external arm can't be reached:
  it is only called with the static sentinel (audit `vault-core-ai.md:264-267`). The `Static` variant doc
  promises a fallback "to candle/External" (`embedding_external.rs:456`). Collapse it to the honest
  "not downloaded" error and fix the doc.
- **D. `cfg(any(test, coverage))` twins.** These are `select_static_embedding_provider` stub
  (`embedding_static.rs:191-201`) and stub `fetch_one_file` (`EC:612-628`, after the move). They are STATUS
  item 4's "test doubles in production". Move them as they are now; delete them in that work.
- **E. A configured static model can't be verified.** `select_static_embedding_provider` verifies against
  `DEFAULT_STATIC_MODEL_FILES` whatever `default_model` repo is configured (`embedding_static.rs:175-181`,
  also `read_model.rs:292`). So "a future static model is a config change" (`embedding_static.rs:60-61`) is
  not true. This also blocks a small fixture model for E2E (§4).

---

## 4. E2E coverage check

Specs that touch embeddings or semantic search:

- `tests/e2e/backup.spec.ts:138-171` sets `semanticIndexEnabled: true` and `autoIndexAfterBackup: true`
  with `jobQueuePaused: true`. It asserts only the backup-run line "New pages are queued for semantic
  search…". No index is built and no vector is produced.
- `tests/e2e/history-tools.spec.ts:339-346` opens `?mode=semantic` with semantic search off and asserts
  "Semantic search is off. These are full-text results instead." and the `· Full text` label.
- No other spec mentions semantic search, embedding, `static-in-app`, or the model download (grep over
  `tests/`).

Below E2E:

- `ai/tests_native/indexing.rs:~480-540` builds an index and runs `semantic_search_history` through the
  static engine. Under `cfg(test)` that is the stub (`embedding_static.rs:191-201`), not the real matrix
  and tokenizer. The pipeline under it (Vault, `.pkvec`/`.pkmap`/planes, cursors) is real.
- The real static engine (download, verify, safetensors load, tokenizer, pooling) runs only in the
  env-gated `tests/static_embedding_e2e.rs:99,151`, which `bun run check` never runs.
- `tests/lmstudio_embedding_e2e.rs` covers the real external path, also env-gated, and needs LM Studio.

**Conclusion.** Removing candle takes away no E2E coverage. No E2E exercised candle, and no unit test
outside `EC` and `embedding_external.rs:844-862` uses the candle stub. But nothing in the gate proves
semantic search works with the default provider, or with any provider, before or after this change.

Gaps:

1. **The real static build plus query in the app** (onboarding → download → index → `?mode=semantic`
   returns a page by meaning). Two things block it: the 512 MB download from Hugging Face in E2E, and
   item E, which means a small fixture model can't pass verification. Options:
   - fix E (verify against the configured repo's own manifest) and ship a small fixture model plus its
     manifest with the test data; or
   - cache the real potion files on the Modal volume and seed `models_dir` before the spec.
2. **External path through a fake OpenAI-compatible `/v1/embeddings` server.** This is already planned in
   `STATUS.md:98` (item 6: "Ask（假的 OpenAI 相容 server）… 語意索引建置中斷再續"). It also proves the
   moved selector's external branch end to end.
3. **Static model download UX** (progress, cancel, failure message). Not covered. Note that
   `semantic-row.tsx:108` shows the raw backend `event.message` (English) when a download fails. That is an
   i18n gap whatever happens to candle.
4. **Decision A regression proof.** If the safetensors loader is rewritten, the only check is the env-gated
   parity test. Run it once on Modal as part of step 9, until gap 1 exists.
