# Agent backup preview growth estimate

The enable preview can inspect native file sizes before the person grants capture permission. It measures ordinary files and native zstd files separately. The estimate covers the first capture of these sources into an initialized archive.

On Linux x86-64, in release mode, the fixture generated 16 sessions per tool with 96 messages per session, alternating prompts and replies. Each message contains deterministic mixed text and identifiers. Ordinary sources use Claude Code, Codex, Pi and DeepSeek. The compressed batch uses Codex and DeepSeek. Each sample starts with an empty initialized search index and closed databases. The fixture subtracts that baseline from the sizes of the same closed roles after a real backup and verifies the number of published roots. File sizes are logical bytes. Native zstd input is measured on disk before decoding.

| Archive mode | Input       | Native bytes | Evidence growth | Read-model growth | Search growth |
| ------------ | ----------- | -----------: | --------------: | ----------------: | ------------: |
| Plaintext    | Ordinary    |    8,056,480 |       2,289,664 |        33,222,656 |     3,468,324 |
| Encrypted    | Ordinary    |    8,056,480 |       2,297,856 |        33,271,808 |     3,468,884 |
| Plaintext    | Native zstd |      909,780 |       1,171,456 |        16,097,280 |     2,083,403 |
| Encrypted    | Native zstd |      909,780 |       1,171,456 |        16,121,856 |     2,083,859 |

The table above gives the first run in bytes. A second fresh capture used the same inputs. Mean growth across both runs, in bytes per on-disk source byte:

| Archive mode | Input       | Evidence | Read model | Search |
| ------------ | ----------- | -------: | ---------: | -----: |
| Plaintext    | Ordinary    |   0.2842 |     4.1250 | 0.4307 |
| Encrypted    | Ordinary    |   0.2852 |     4.1326 | 0.4313 |
| Plaintext    | Native zstd |   1.2876 |    17.6733 | 2.2855 |
| Encrypted    | Native zstd |   1.2876 |    17.7139 | 2.2887 |

Each coefficient is the mean role growth per native input byte, rounded up to the nearest thousandth. Multiply each input class by its coefficient before adding and rounding the role estimate. Actual growth depends on compression, duplication, record density, attachments and index vocabulary. The estimate excludes transient writer and staging space.

From `src-tauri`, run `cargo test --release -p agent-history --test preview_growth -- --ignored --nocapture --test-threads=1`. [Measurement output](validation/preview-growth-linux.jsonl)
