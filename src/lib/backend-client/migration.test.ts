/**
 * @file migration.test.ts
 * @description Pins the hand-copied data-migration error prefixes to the Rust literals.
 * @module lib/backend-client
 *
 * ## Responsibilities
 * - Lock `IMPORT_SOURCE_KEY_REQUIRED_PREFIX` / `IMPORT_SOURCE_KEY_INVALID_PREFIX` to exact literals.
 *
 * ## Not responsible for
 * - Exercising the migration IPC commands themselves.
 * - Rendering the Settings data-migration panel.
 *
 * ## Dependencies
 * - Depends only on the migration backend-client module.
 *
 * ## Performance notes
 * - Constant assertions only; no IPC, no fixtures.
 *
 * ## Cross-language contract
 * These two literals are duplicated in
 * `src-tauri/crates/vault-core/src/migration.rs`, which pins them from the Rust
 * side in `import_source_key_error_prefixes_stay_the_literals_the_shell_pins`.
 * Both tests write the strings out longhand on purpose: there is no runtime
 * bridge between the languages, so the only way a prefix drift can be caught is
 * for each side to fail independently. If you change one, change both.
 */

import { describe, expect, test } from 'vitest'
import {
  IMPORT_SOURCE_KEY_INVALID_PREFIX,
  IMPORT_SOURCE_KEY_REQUIRED_PREFIX,
} from './migration'

describe('data-migration error prefixes', () => {
  test('match the Rust constants in vault-core/src/migration.rs', () => {
    expect(IMPORT_SOURCE_KEY_REQUIRED_PREFIX).toBe(
      'source_archive_key required',
    )
    expect(IMPORT_SOURCE_KEY_INVALID_PREFIX).toBe('source_archive_key invalid')
  })
})
