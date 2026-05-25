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
})
