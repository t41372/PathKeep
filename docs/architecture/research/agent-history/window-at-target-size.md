# Window reads at 264 MB

Starting revision `5e290623`. The reviewer's Linux baseline used four vCPUs and an 8 GiB limit. Cold p95 grew from 18.9 to 222.6 ms across the plain source, from 36.1 to 593.3 ms across the multiframe source, and from 34.6 to 2333.2 ms across the single frame. The largest scan progress gaps were 206, 199 and 285 ms respectively.

## Why the cost grew

A timed release profile on arm64 macOS used the same 23,812-record corpus. In the old `RawReader::load_chunk`, the database call alone took 1.46 ms on the first plain slice, 41.71 ms for the slice containing the middle window and 95.90 ms for the slice containing the tail. The corresponding first tail fetch on multiframe took 96.10 ms. These are individual instrumented observations.

The engine plan for the predecessor lookup used the manifest's ordinal index on `revision_id=?`, looked up the candidate payloads by hash, and used a sorter for `ORDER BY byte_offset DESC`. Each late lookup could sort earlier blobs, including when a native decoder had restarted at the correct frame. Sequential scans paid that cost repeatedly.

The revised reader resolves retention and the current manifest, seeks `ah_revision_chunks_offset` with `revision_id=? AND byte_offset<=?`, then gets only the selected payload by hash. These reads share one evidence snapshot. Both changes matter on this engine. Splitting the SQL alone still selected the ordinal index. Forcing the offset index on the old join still produced a sorter. A real-engine plan test rejects either regression. Archive migrations and slice formats are unchanged.

`read_window` starts a native decoder at the frame recorded in the locator. Its `skip` must produce and discard the decoded bytes between that restart and the requested record. At the multiframe tail that prefix was 793,996 bytes. At the single-frame tail it was 261,787,912 bytes. The latter profile spent 0.16 ms on outline lookup and delivered its first row at 547.35 ms. Whole-window decoded work was 264,000,000 bytes.

## Release measurements after the fix

The command was `AH_BYTES=264000000 AH_RECORDS=23812 AH_SAMPLES=40 cargo run --release -p agent-history --example window_measurement`, run on arm64 macOS using the synthetic-file cache sweep. Each entry below is cold p95 / p99 in milliseconds for a 200-record window. Middle begins at 11,806 and tail begins at 23,612. The rendered candidate loads the outline plus independently compressed pages from a disposable encrypted database. It reconstructs full text and part ranges. Exact raw expansion continues to use the evidence reader.

| Form         | Evidence head | Evidence middle |   Evidence tail | Rendered head | Rendered middle | Rendered tail | Scan ms | Longest progress gap ms |
| ------------ | ------------: | --------------: | --------------: | ------------: | --------------: | ------------: | ------: | ----------------------: |
| Plain        | 21.06 / 23.97 |   18.73 / 20.21 |   17.71 / 18.48 |   3.65 / 3.86 |     4.11 / 4.21 |   3.54 / 3.72 | 1565.51 |                   17.62 |
| Multiframe   | 16.38 / 17.32 |   16.60 / 17.12 |   17.15 / 18.96 |   3.39 / 3.60 |     3.75 / 3.80 |   3.62 / 4.56 |  659.13 |                   12.97 |
| Single frame | 17.77 / 18.44 | 349.79 / 381.34 | 570.84 / 592.42 |   6.54 / 7.05 |     6.45 / 6.66 |   3.42 / 3.72 |  618.42 |                   12.50 |

The raw oracle checked all bytes, absolute marker positions, IDs and text. The rendered oracle independently generated and checked the complete message text, part ranges and outline address. Both paths used separate cold trials. The [complete output](validation/window-target-macos.jsonl) includes warm and during-backup distributions.

## Storage decision

For each identical decoded body, the outline-only fixture was 200,704 bytes after checkpoint. With rendered pages it was 70,606,848 bytes. The increment was **70,406,144 bytes**, or **266.69 MB per decimal GB of decoded source**. The native single-frame transport in the profile occupied 70,946,444 bytes, so the extra derived data was about **992 MB per GB of native single-frame input** on this corpus. Compression ratios on other transcripts will differ.

Building the rendered pages in a separate pass took 2.99 s plain, 1.76 s multiframe and 1.71 s single-frame. Appending a 609-byte native message rewrote one tail page starting at ordinal 23,810 and submitted 19,979 compressed bytes. The three observed append costs were 0.85, 0.79 and 0.78 ms. These are incremental render/commit costs. They exclude source capture, immutable publication and garbage collection.

Selective publication would add pages for sources with excessive native-frame replay. Plain sources and short independent frames could keep serving windows directly. Publishing pages for all sources would give every rendered read the same lookup path and charge the measured space and append cost on all covered sources. Keeping evidence-only storage preserves the single-frame replay cost for cold middle and tail reads. Treat the native frame's decoded extent as the deciding input. A file with multiple large frames can also have expensive replay.

The experiment does not select or install a production representation. A storage decision would also need renderer/publication versioning, giant-part continuations, retention and archive lifecycle coverage. The 100 ms observations above apply to this target-size macOS run. The revised Linux path awaits verification.

## Regression checks

Each run applied one break in an isolated copy and executed the library, binary, example and integration tests with `--no-fail-fast`. The explicitly named `cli_e2e` target and desktop E2E were excluded. Each compiling mutation produced 313 passes and one failing test. The restored checkout produced 314 passes, including exact scenario J. Formatting and all-target strict Clippy passed.

| Deliberate break                                 | Catching test                                                                    |
| ------------------------------------------------ | -------------------------------------------------------------------------------- |
| Accept mismatched revision identity              | `selection_skips_a_large_plain_gap_and_checks_revision_identity`                 |
| Leave the reader token active during maintenance | `maintenance_cancels_a_reader_waiting_in_its_sink`                               |
| Classify stored-slice damage as I/O              | `archive::damage_tests::damaged_slice_is_distinct_from_invalid_native_transport` |
| Interpret a clipped preview                      | `preview_continues_without_interpreting_a_partial_object`                        |
| Accept row 201                                   | `rejects_row_201_before_delivering_any_window_rows`                              |
| Feed the BOM to the first JSON parser            | `first_bom_record_retains_the_prefix_only_in_exact_bytes`                        |
| Disable the pre-read retention check             | `expired_record_rejects_even_an_empty_final_continuation`                        |
