/**
 * Tests for Settings → Data migration.
 *
 * Covers:
 * - Idle render: both action tiles + intro copy.
 * - Export happy path: save-dialog returns a path, backend.exportAppData
 *   resolves → success banner shows the bundle path / size / file count.
 * - Export error path: dialog cancel returns idle; thrown error renders
 *   the error banner.
 * - Import preview happy path: open-dialog → preview rendered with
 *   schema-migration descriptor + overwrite warning + exclusion list.
 * - Import apply happy path: confirm → success banner with applied
 *   migrations + bak notice.
 * - Import apply error path: preview stays, error banner appears inside
 *   the preview panel.
 * - Cancel button returns to idle.
 *
 * The Tauri dialog plugin is mocked at the module level so the lazy
 * dynamic imports inside the component resolve to spies the test owns.
 */

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '@/lib/i18n'
import { backend } from '@/lib/backend-client'
import type {
  ExportedBundle,
  ImportPreview,
  ImportResult,
} from '@/lib/backend-client/migration'
import { DataMigrationSection } from './data-migration-section'

const dialogSaveMock = vi.fn<(...args: unknown[]) => Promise<unknown>>()
const dialogOpenMock = vi.fn<(...args: unknown[]) => Promise<unknown>>()

vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: (...args: unknown[]): Promise<unknown> => dialogSaveMock(...args),
  open: (...args: unknown[]): Promise<unknown> => dialogOpenMock(...args),
}))

const NAV_ITEM = {
  id: 'settings-migration',
  icon: 'download' as const,
  key: 'migration' as const,
  label: 'Data migration',
}

function renderSection() {
  return render(
    <I18nProvider>
      <DataMigrationSection navItem={NAV_ITEM} />
    </I18nProvider>,
  )
}

function mockBundle(overrides: Partial<ExportedBundle> = {}): ExportedBundle {
  return {
    bundlePath: '/tmp/pathkeep-export-2026-05-25.pathkeep',
    bytesWritten: 4_194_304,
    manifest: {
      formatVersion: 1,
      appVersion: '0.3.0',
      archiveSchemaVersion: 13,
      archiveMode: 'plaintext',
      exportedAt: '2026-05-25T01:00:00Z',
      exporterHostname: 'source-machine',
      files: [
        { path: 'config/config.json', sha256: 'abc', sizeBytes: 1024 },
        {
          path: 'archive/history-vault.sqlite',
          sha256: 'def',
          sizeBytes: 4_193_280,
        },
      ],
    },
    ...overrides,
  }
}

function mockPreview(overrides: Partial<ImportPreview> = {}): ImportPreview {
  const base = mockBundle().manifest
  return {
    manifest: base,
    schemaUpToDate: true,
    migrationsToApply: [],
    bytesToExtract: 4_194_304,
    willOverwriteExisting: true,
    exclusionNotes: [
      {
        path: 'vault.hold',
        reason: 'Stronghold App Lock secrets stay on the source machine.',
      },
      { path: 'schedule/', reason: 'Platform-specific scheduler artifacts.' },
    ],
    ...overrides,
  }
}

function mockResult(overrides: Partial<ImportResult> = {}): ImportResult {
  return {
    manifest: mockBundle().manifest,
    migrationsApplied: [],
    finalSchemaVersion: 13,
    preservedPreviousAsBak: true,
    ...overrides,
  }
}

describe('DataMigrationSection', () => {
  beforeEach(() => {
    dialogSaveMock.mockReset()
    dialogOpenMock.mockReset()
    vi.restoreAllMocks()
  })

  test('renders both action tiles with localized labels', () => {
    renderSection()
    expect(screen.getByTestId('settings-migration')).toBeInTheDocument()
    expect(screen.getByTestId('settings-migration-export')).toBeInTheDocument()
    expect(screen.getByTestId('settings-migration-import')).toBeInTheDocument()
    expect(screen.getByText(/DATA MIGRATION/i)).toBeInTheDocument()
  })

  test('export happy path renders success banner with bundle path / size / file count', async () => {
    dialogSaveMock.mockResolvedValue('/tmp/pk.pathkeep')
    const bundle = mockBundle({ bundlePath: '/tmp/pk.pathkeep' })
    vi.spyOn(backend, 'exportAppData').mockResolvedValue(bundle)

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-export'))

    await waitFor(() =>
      expect(screen.getByTestId('settings-migration-exported')).toBeVisible(),
    )
    const banner = screen.getByTestId('settings-migration-exported')
    expect(banner).toHaveTextContent('/tmp/pk.pathkeep')
    expect(banner).toHaveTextContent('2') // file count
    expect(backend.exportAppData).toHaveBeenCalledWith('/tmp/pk.pathkeep')
  })

  test('export dialog cancel returns to idle without firing the backend', async () => {
    dialogSaveMock.mockResolvedValue(null)
    const spy = vi.spyOn(backend, 'exportAppData')

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-export'))

    await waitFor(() =>
      expect(
        screen.queryByTestId('settings-migration-exported'),
      ).not.toBeInTheDocument(),
    )
    expect(spy).not.toHaveBeenCalled()
  })

  test('export error path renders the error banner with the underlying message', async () => {
    dialogSaveMock.mockResolvedValue('/tmp/pk.pathkeep')
    vi.spyOn(backend, 'exportAppData').mockRejectedValue(new Error('disk full'))

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-export'))

    await waitFor(() =>
      expect(
        screen.getByTestId('settings-migration-export-error'),
      ).toBeVisible(),
    )
    expect(
      screen.getByTestId('settings-migration-export-error'),
    ).toHaveTextContent('disk full')
  })

  test('export error path surfaces the raw rejection when the dialog plugin throws a plain string', async () => {
    // Regression: when the Tauri dialog plugin (or any non-Error throw)
    // bubbled out of `save()`, the section used to fall back to an i18n
    // "unknown reason" banner — leaving the user with no idea what went
    // wrong. The describeError helper must surface the raw string instead.
    dialogSaveMock.mockRejectedValue(
      'dialog.save not allowed by the app capabilities',
    )

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-export'))

    await waitFor(() =>
      expect(
        screen.getByTestId('settings-migration-export-error'),
      ).toBeVisible(),
    )
    expect(
      screen.getByTestId('settings-migration-export-error'),
    ).toHaveTextContent('dialog.save not allowed by the app capabilities')
  })

  test('export error path surfaces a Tauri-style {message} object verbatim', async () => {
    dialogSaveMock.mockResolvedValue('/tmp/pk.pathkeep')
    vi.spyOn(backend, 'exportAppData').mockRejectedValue({
      kind: 'Io',
      message: 'permission denied: /Users/yt/PathKeep/exports',
    })

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-export'))

    await waitFor(() =>
      expect(
        screen.getByTestId('settings-migration-export-error'),
      ).toBeVisible(),
    )
    expect(
      screen.getByTestId('settings-migration-export-error'),
    ).toHaveTextContent('permission denied: /Users/yt/PathKeep/exports')
  })

  test('import preview renders schema descriptor + overwrite warning + exclusions', async () => {
    dialogOpenMock.mockResolvedValue('/tmp/bundle.pathkeep')
    const preview = mockPreview({
      schemaUpToDate: false,
      migrationsToApply: [13, 14],
    })
    vi.spyOn(backend, 'previewAppDataImport').mockResolvedValue(preview)

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-import'))

    await waitFor(() =>
      expect(screen.getByTestId('settings-migration-preview')).toBeVisible(),
    )
    expect(
      screen.getByTestId('settings-migration-preview-overwrite-warning'),
    ).toBeInTheDocument()
    const preview_panel = screen.getByTestId('settings-migration-preview')
    expect(preview_panel).toHaveTextContent('/tmp/bundle.pathkeep')
    expect(preview_panel).toHaveTextContent('will apply 2 forward migration')
    expect(preview_panel).toHaveTextContent('vault.hold')
  })

  test('import apply happy path renders success banner with migrations + bak notice', async () => {
    dialogOpenMock.mockResolvedValue('/tmp/bundle.pathkeep')
    vi.spyOn(backend, 'previewAppDataImport').mockResolvedValue(mockPreview())
    vi.spyOn(backend, 'applyAppDataImport').mockResolvedValue(
      mockResult({ migrationsApplied: [13], preservedPreviousAsBak: true }),
    )

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-import'))
    await waitFor(() =>
      expect(screen.getByTestId('settings-migration-preview')).toBeVisible(),
    )
    await userEvent.click(screen.getByTestId('settings-migration-confirm'))

    await waitFor(() =>
      expect(screen.getByTestId('settings-migration-applied')).toBeVisible(),
    )
    const banner = screen.getByTestId('settings-migration-applied')
    expect(banner).toHaveTextContent('schema v13')
    expect(banner).toHaveTextContent('Previous project preserved')
    expect(backend.applyAppDataImport).toHaveBeenCalledWith(
      '/tmp/bundle.pathkeep',
      { confirmOverwrite: true },
    )
  })

  test('apply error keeps the preview mounted and renders inline error', async () => {
    dialogOpenMock.mockResolvedValue('/tmp/bundle.pathkeep')
    vi.spyOn(backend, 'previewAppDataImport').mockResolvedValue(mockPreview())
    vi.spyOn(backend, 'applyAppDataImport').mockRejectedValue(
      new Error('staging swap failed: ENOSPC'),
    )

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-import'))
    await waitFor(() =>
      expect(screen.getByTestId('settings-migration-preview')).toBeVisible(),
    )
    await userEvent.click(screen.getByTestId('settings-migration-confirm'))

    await waitFor(() =>
      expect(
        screen.getByTestId('settings-migration-apply-error'),
      ).toBeVisible(),
    )
    // Preview panel must still be mounted so the user can retry or cancel.
    expect(screen.getByTestId('settings-migration-preview')).toBeVisible()
    expect(
      screen.getByTestId('settings-migration-apply-error'),
    ).toHaveTextContent('ENOSPC')
  })

  test('cancel returns to idle and unmounts the preview panel', async () => {
    dialogOpenMock.mockResolvedValue('/tmp/bundle.pathkeep')
    vi.spyOn(backend, 'previewAppDataImport').mockResolvedValue(mockPreview())

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-import'))
    await waitFor(() =>
      expect(screen.getByTestId('settings-migration-preview')).toBeVisible(),
    )
    await userEvent.click(screen.getByTestId('settings-migration-cancel'))

    await waitFor(() =>
      expect(
        screen.queryByTestId('settings-migration-preview'),
      ).not.toBeInTheDocument(),
    )
  })

  test('import dialog cancel returns to idle without firing the backend', async () => {
    // L118-120: when the Tauri open() resolves to `null` (user dismissed
    // the picker), the section must reset to idle so the import tile
    // becomes clickable again — not freeze in the "previewing" spinner.
    dialogOpenMock.mockResolvedValue(null)
    const previewSpy = vi.spyOn(backend, 'previewAppDataImport')

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-import'))

    await waitFor(() =>
      expect(
        screen.queryByTestId('settings-migration-preview'),
      ).not.toBeInTheDocument(),
    )
    expect(previewSpy).not.toHaveBeenCalled()
    // Idle means the import tile is re-enabled and no preview/error
    // banner is rendered.
    expect(
      screen.queryByTestId('settings-migration-preview-error'),
    ).not.toBeInTheDocument()
    expect(screen.getByTestId('settings-migration-import')).not.toBeDisabled()
  })

  test('import dialog returns an empty string → still treated as cancel', async () => {
    // L118 covers the `!selected.trim()` branch — some platforms return
    // an empty path string instead of null when the picker is dismissed.
    dialogOpenMock.mockResolvedValue('   ')
    const previewSpy = vi.spyOn(backend, 'previewAppDataImport')

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-import'))

    await waitFor(() =>
      expect(
        screen.getByTestId('settings-migration-import'),
      ).not.toBeDisabled(),
    )
    expect(previewSpy).not.toHaveBeenCalled()
  })

  test('apply success with no forward migrations renders the "none" descriptor', async () => {
    // HTML L1237 branch: `migrationsApplied.length === 0` → the i18n
    // string `migrationAppliedNoMigrations` ('none') is rendered in
    // place of the comma-joined migration list. Also exercises the
    // `preservedPreviousAsBak: false` branch so `{bakNotice}` resolves
    // to '' and gets trimmed away.
    dialogOpenMock.mockResolvedValue('/tmp/bundle.pathkeep')
    vi.spyOn(backend, 'previewAppDataImport').mockResolvedValue(mockPreview())
    vi.spyOn(backend, 'applyAppDataImport').mockResolvedValue(
      mockResult({
        migrationsApplied: [],
        preservedPreviousAsBak: false,
      }),
    )

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-import'))
    await waitFor(() =>
      expect(screen.getByTestId('settings-migration-preview')).toBeVisible(),
    )
    await userEvent.click(screen.getByTestId('settings-migration-confirm'))

    await waitFor(() =>
      expect(screen.getByTestId('settings-migration-applied')).toBeVisible(),
    )
    const banner = screen.getByTestId('settings-migration-applied')
    expect(banner).toHaveTextContent('none')
    // bakNotice path went to '' so the .bak preservation copy must be
    // absent from the success banner.
    expect(banner).not.toHaveTextContent('Previous project preserved')
  })

  test('handleApplyImport retries after an apply error so the user can fix and re-submit', async () => {
    // After Codex C4, an apply error keeps the preview panel mounted on
    // purpose so the user can change the source archive key (when the
    // bundle is encrypted) and click Confirm again. The retry must
    // re-invoke `applyAppDataImport`. The previous behaviour (no-op on
    // any non-`previewed` phase) was the wrong contract — it forced the
    // user to cancel and re-pick the bundle on every typo.
    dialogOpenMock.mockResolvedValue('/tmp/bundle.pathkeep')
    vi.spyOn(backend, 'previewAppDataImport').mockResolvedValue(mockPreview())
    const applySpy = vi
      .spyOn(backend, 'applyAppDataImport')
      .mockRejectedValueOnce(new Error('boom'))
      .mockResolvedValueOnce(mockResult())

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-import'))
    await waitFor(() =>
      expect(screen.getByTestId('settings-migration-preview')).toBeVisible(),
    )
    await userEvent.click(screen.getByTestId('settings-migration-confirm'))
    await waitFor(() =>
      expect(
        screen.getByTestId('settings-migration-apply-error'),
      ).toBeVisible(),
    )
    expect(applySpy).toHaveBeenCalledTimes(1)

    // Click confirm a second time from the `applyError` phase — the
    // backend is invoked again and on success the panel transitions to
    // the `applied` banner.
    await userEvent.click(screen.getByTestId('settings-migration-confirm'))
    await waitFor(() =>
      expect(screen.getByTestId('settings-migration-applied')).toBeVisible(),
    )
    expect(applySpy).toHaveBeenCalledTimes(2)
  })

  test('preview renders without exporter hostname suffix when manifest omits it', async () => {
    // L342 branch: `exporterHostname` falsy → the ` · {host}` suffix is
    // skipped. Same component now also exercises the
    // `willOverwriteExisting: false` branch on L376 so the overwrite
    // warning is absent.
    dialogOpenMock.mockResolvedValue('/tmp/bundle.pathkeep')
    const baseManifest = mockBundle().manifest
    vi.spyOn(backend, 'previewAppDataImport').mockResolvedValue(
      mockPreview({
        willOverwriteExisting: false,
        manifest: { ...baseManifest, exporterHostname: '' },
      }),
    )

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-import'))
    await waitFor(() =>
      expect(screen.getByTestId('settings-migration-preview')).toBeVisible(),
    )
    const preview_panel = screen.getByTestId('settings-migration-preview')
    expect(preview_panel).toHaveTextContent('2026-05-25T01:00:00Z')
    expect(preview_panel).not.toHaveTextContent('source-machine')
    expect(
      screen.queryByTestId('settings-migration-preview-overwrite-warning'),
    ).not.toBeInTheDocument()
  })

  test('preview error renders the error banner without opening the preview panel', async () => {
    dialogOpenMock.mockResolvedValue('/tmp/bundle.pathkeep')
    vi.spyOn(backend, 'previewAppDataImport').mockRejectedValue(
      new Error('not a valid bundle'),
    )

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-import'))

    await waitFor(() =>
      expect(
        screen.getByTestId('settings-migration-preview-error'),
      ).toBeVisible(),
    )
    expect(
      screen.queryByTestId('settings-migration-preview'),
    ).not.toBeInTheDocument()
  })

  test('encrypted preview renders the source-key prompt and gates Confirm on a non-empty key', async () => {
    // Codex C4 frontend gate: the source-key input appears only when
    // the manifest reports `archiveMode === "encrypted"`. Confirm is
    // disabled until the user types a non-empty key, so a click can't
    // bounce off the backend with "source_archive_key required."
    dialogOpenMock.mockResolvedValue('/tmp/encrypted.pathkeep')
    vi.spyOn(backend, 'previewAppDataImport').mockResolvedValue(
      mockPreview({
        manifest: {
          ...mockBundle().manifest,
          archiveMode: 'encrypted',
        },
      }),
    )
    const applySpy = vi
      .spyOn(backend, 'applyAppDataImport')
      .mockResolvedValue(mockResult())

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-import'))
    await waitFor(() =>
      expect(
        screen.getByTestId('settings-migration-source-key-prompt'),
      ).toBeVisible(),
    )

    const confirm = screen.getByTestId('settings-migration-confirm')
    expect(confirm).toBeDisabled()
    expect(applySpy).not.toHaveBeenCalled()

    const input = screen.getByTestId('settings-migration-source-key-input')
    expect(input).toHaveAttribute('type', 'password')

    await userEvent.type(input, 'source-machine-key')
    expect(confirm).toBeEnabled()

    await userEvent.click(confirm)
    await waitFor(() => expect(applySpy).toHaveBeenCalledTimes(1))
    expect(applySpy).toHaveBeenCalledWith('/tmp/encrypted.pathkeep', {
      confirmOverwrite: true,
      sourceArchiveKey: 'source-machine-key',
    })
  })

  test('plaintext bundle import does not render the source-key prompt and omits sourceArchiveKey', async () => {
    // Regression guard: the default plaintext path must keep working.
    // `bundleIsEncrypted` only flips on the literal `"encrypted"`
    // value so an unknown mode falls back to plaintext rather than
    // demanding a key the bundle does not need.
    dialogOpenMock.mockResolvedValue('/tmp/plaintext.pathkeep')
    vi.spyOn(backend, 'previewAppDataImport').mockResolvedValue(mockPreview())
    const applySpy = vi
      .spyOn(backend, 'applyAppDataImport')
      .mockResolvedValue(mockResult())

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-import'))
    await waitFor(() =>
      expect(screen.getByTestId('settings-migration-preview')).toBeVisible(),
    )

    expect(
      screen.queryByTestId('settings-migration-source-key-prompt'),
    ).not.toBeInTheDocument()
    await userEvent.click(screen.getByTestId('settings-migration-confirm'))
    await waitFor(() => expect(applySpy).toHaveBeenCalledTimes(1))
    expect(applySpy).toHaveBeenCalledWith('/tmp/plaintext.pathkeep', {
      confirmOverwrite: true,
      sourceArchiveKey: undefined,
    })
  })

  test('source-key-required error swaps the apply banner copy and keeps the panel mounted for retry', async () => {
    // Backend signals `source_archive_key required` when the bundle is
    // encrypted but no key was passed. The frontend matches on the
    // typed prefix and renders the dedicated "Source archive key
    // required" copy instead of the generic "Import failed" banner.
    dialogOpenMock.mockResolvedValue('/tmp/encrypted.pathkeep')
    vi.spyOn(backend, 'previewAppDataImport').mockResolvedValue(
      mockPreview({
        manifest: {
          ...mockBundle().manifest,
          archiveMode: 'encrypted',
        },
      }),
    )
    // Reject with the typed prefix exactly the way the Rust side
    // formats it. The test pins the contract by string-matching the
    // constant — if the prefix ever drifts on the Rust side the
    // catalogue test will catch the missing locale, and this test
    // pins the JS-side detection.
    vi.spyOn(backend, 'applyAppDataImport').mockRejectedValue(
      new Error(
        'source_archive_key required: the imported bundle was encrypted on the source machine.',
      ),
    )

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-import'))
    await waitFor(() =>
      expect(
        screen.getByTestId('settings-migration-source-key-prompt'),
      ).toBeVisible(),
    )
    await userEvent.type(
      screen.getByTestId('settings-migration-source-key-input'),
      'whatever',
    )
    await userEvent.click(screen.getByTestId('settings-migration-confirm'))

    await waitFor(() =>
      expect(
        screen.getByTestId('settings-migration-apply-error'),
      ).toBeVisible(),
    )
    const banner = screen.getByTestId('settings-migration-apply-error')
    expect(banner).toHaveTextContent('Source archive key required')
    // Preview panel + source-key input still mounted so the user can
    // edit the key and click confirm again.
    expect(screen.getByTestId('settings-migration-preview')).toBeInTheDocument()
    expect(
      screen.getByTestId('settings-migration-source-key-input'),
    ).toBeInTheDocument()
  })

  test('source-key-invalid error renders the "wrong key" copy distinct from the generic banner', async () => {
    // Distinct typed prefix → distinct copy. UX needs to tell the user
    // "key wrong, try again" not "import failed for an unknown reason."
    dialogOpenMock.mockResolvedValue('/tmp/encrypted.pathkeep')
    vi.spyOn(backend, 'previewAppDataImport').mockResolvedValue(
      mockPreview({
        manifest: {
          ...mockBundle().manifest,
          archiveMode: 'encrypted',
        },
      }),
    )
    vi.spyOn(backend, 'applyAppDataImport').mockRejectedValue(
      new Error(
        'source_archive_key invalid: the supplied key does not decrypt the archive.',
      ),
    )

    renderSection()
    await userEvent.click(screen.getByTestId('settings-migration-import'))
    await waitFor(() =>
      expect(
        screen.getByTestId('settings-migration-source-key-prompt'),
      ).toBeVisible(),
    )
    await userEvent.type(
      screen.getByTestId('settings-migration-source-key-input'),
      'wrong-key',
    )
    await userEvent.click(screen.getByTestId('settings-migration-confirm'))

    await waitFor(() =>
      expect(
        screen.getByTestId('settings-migration-apply-error'),
      ).toBeVisible(),
    )
    const banner = screen.getByTestId('settings-migration-apply-error')
    expect(banner).toHaveTextContent('Source archive key is incorrect')
  })
})
