# Native backup at a million visits (2026-11-01)

What a backup costs with the identity and read policy in
[backup-integrity.md](../../backup-integrity.md), on a generated Chrome history. These numbers
replace the earlier report on this work, whose raw outputs are gone. The files beside this one
are the raw `/usr/bin/time -l` outputs of each run.

**Input.** One Chrome profile written by
`archive::ingest::scale_measure::measure_native_backup` (phase `source`): 1,000,000 visits over
the last 90 days, 200,000 URLs, every second visit opened by the one before, 1,000 downloads,
10,000 searches. About 11,100 visits fall in the last 24 hours. Synthetic only; every browser,
keychain and scheduler path points into the measurement folder.

**Archive.** Encrypted (AES, the app's key path), opened by `Vault::open`, favicons off, the real
`run_backup` entry point. Release build, each run its own process.

**Machine.** Apple M5 Max (18 cores, 64 GB), macOS 27.2, warm file cache. Not the 4-core,
8 GB target machine; times there will be longer.

| Run                                                                      |  Backup | Peak RSS | Peak footprint | Result                         |
| ------------------------------------------------------------------------ | ------: | -------: | -------------: | ------------------------------ |
| First backup, new archive (every row)                                    | 156.7 s | 1003 MiB |        466 MiB | 1,000,000 visits, 200,000 URLs |
| Next backup, nothing new (rows after the last, 24 h again)               |  0.62 s |  112 MiB |         72 MiB | 0 new                          |
| Weekly read of every row, nothing new                                    |  32.4 s |  476 MiB |        411 MiB | 0 new                          |
| Version 1 archive of the same input, made by the code before this change | 192.4 s |  964 MiB |        484 MiB | 1,000,000 visits               |
| Open after the update (migrations 2–16, opener backfill)                 |   4.0 s |        — |              — | —                              |
| First backup after the update (every row; gives each visit its time)     |  38.4 s |  463 MiB |        303 MiB | 0 new, still 1,000,000 visits  |
| Next backup after that                                                   |  0.64 s |  114 MiB |         73 MiB | 0 new                          |

After the update's first backup, all 1,000,000 visits have their browser time and the 500,000
that name an opener have it by archive id (`*-9-counts.txt`; their `open_seconds` field printed
the opener count by mistake, fixed in the code since).

`before-review-fixes/` holds the same three fresh runs from before the last review's fixes
(164.7 s, 0.64 s, 33.3 s), for comparison.

Measured before the second review's two fixes. One adds a comparison per visit read (a time
past year 9999 is not the newest), the other a bound on the check for new visits under old ids
(still one probe of the browser's time index); the import preview fix is not on a backup's path.
They were not measured again.

Not measured here: 14.4M visits, the target machine, a cold file cache, a profile Sync wrote
newest-first (openers after their children), a live browser holding the file.

To run again:

```sh
cargo test --release --manifest-path src-tauri/Cargo.toml -p vault-core --lib --no-run
PATHKEEP_SCALE_ROOT=<empty folder> PATHKEEP_SCALE_PHASE=source PATHKEEP_SCALE_ROWS=1000000 \
  /usr/bin/time -l <test binary> --ignored --exact archive::ingest::scale_measure::measure_native_backup --nocapture
# then PATHKEEP_SCALE_PHASE=backup (and =age before a backup, for the weekly read)
```
