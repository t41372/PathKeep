# Native v1 archive fixtures

These archives contain synthetic browser history only. They were produced by the real
`run_backup` entry point at `ff89355071f04d1559be83e542f6ae3c4ee27df2`, before the repair.
`healthy.tar.gz` contains a 50-step Chrome navigation chain, a second Chrome profile,
and a Firefox profile. Its `project-root/expected-before-upgrade.json` records the
actual old-version navigation response and run result before opening it in the update.

`generate.rs` is the source of the isolated fixture generator. Run it as a temporary
Cargo binary depending on `vault-core` and `vault-store` from a checkout of that base
revision, with all browser, keyring, project-root and OS-sandbox environment overrides
pointing at a generated `pathkeep-rescue-native-v1-*` directory. The recorded production
of these fixtures used only generated browser folders and isolated keyring / scheduler
roots; the input paths and hashes were retained. This is a fact about those runs, not an
automatic guarantee provided by the generator.

Manual-tool limitation: the current generator checks the project-path name and required
override presence, but does not canonicalize all overrides or verify containment in one
owned fixture namespace. Every browser, project, keyring and OS-sandbox path must be checked
before use. Complete the canonical namespace guard before running this generator again.
It is a manually compiled research binary, not an automatic Cargo or Playwright target.

The E2E runner extracts a new copy into a temporary directory on every run. The
fixture retains the old absolute provenance strings; live discovery is redirected to
the extracted synthetic browsers. No source path is followed to inspect real data.

`damaged.tar.gz` preserves the baseline E2E archive after a real backup reported success with zero new visits despite seven new native rows. The additional baseline generator run records its actual damaged navigation before upgrade; it does not repair any old URL association. The recovery scenario checks the missing new event set and idempotence, with no authorization to repair old misbindings.
