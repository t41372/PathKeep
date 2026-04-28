/**
 * This test file protects the render-only Settings updater section.
 *
 * Why this file exists:
 * - Updater lifecycle actions are owned by the route hook, but the section owns the visible buttons and progress review.
 * - Strict coverage should catch lost button wiring without requiring a full Settings route render.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Keep assertions on user-visible updater state and handler calls.
 */

import userEvent from '@testing-library/user-event'
import { render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import type { SettingsSectionNavItem } from './section-nav-items'
import { UpdaterSection, type UpdaterSectionState } from './updater-section'

const navItem: SettingsSectionNavItem = {
  id: 'settings-updater',
  icon: 'system_update',
  key: 'updater',
  label: 'Updates',
}

const state = (
  patch: Partial<UpdaterSectionState> = {},
): UpdaterSectionState => ({
  buildInfo: {
    productName: 'PathKeep',
    version: '0.1.0',
    gitCommitShort: 'abc123',
    gitCommitFull: 'abc123456',
    gitDirty: false,
  },
  pendingUpdate: {
    currentVersion: '0.1.0',
    version: '0.2.0',
    notes: 'Important fix.',
    publishedAt: '2026-04-25T10:00:00.000Z',
    downloadUrl: 'https://example.com/latest.json',
  },
  updateAvailability: {
    supported: true,
    available: true,
    currentVersion: '0.1.0',
    version: '0.2.0',
    checkedAt: '2026-04-25T10:01:00.000Z',
    notes: 'Important fix.',
    publishedAt: '2026-04-25T10:00:00.000Z',
    error: null,
    downloadUrl: 'https://example.com/latest.json',
  },
  updateInstallState: {
    phase: 'installed',
    version: '0.2.0',
    downloadedBytes: 50,
    contentLength: 100,
    message: 'Ready to restart.',
  },
  onCheckForUpdates: vi.fn(),
  onDownloadAndInstallUpdate: vi.fn(),
  onOpenReleasePage: vi.fn(),
  onRelaunchForUpdate: vi.fn(),
  ...patch,
})

describe('UpdaterSection', () => {
  test('renders update progress and forwards every updater action', async () => {
    const user = userEvent.setup()
    const sectionState = state()
    render(
      <I18nProvider>
        <UpdaterSection navItem={navItem} state={sectionState} />
      </I18nProvider>,
    )

    expect(screen.getByText('Ready to restart.')).toBeVisible()
    expect(screen.getByText('Downloaded 50 B of 100 B.')).toBeVisible()
    expect(screen.getByText('Important fix.')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Check now' }))
    await user.click(
      screen.getByRole('button', { name: 'Download and install' }),
    )
    await user.click(screen.getByRole('button', { name: 'Restart now' }))
    await user.click(screen.getByRole('button', { name: 'Open release page' }))

    expect(sectionState.onCheckForUpdates).toHaveBeenCalledTimes(1)
    expect(sectionState.onDownloadAndInstallUpdate).toHaveBeenCalledTimes(1)
    expect(sectionState.onRelaunchForUpdate).toHaveBeenCalledTimes(1)
    expect(sectionState.onOpenReleasePage).toHaveBeenCalledTimes(1)
  })

  test('renders unavailable version and zero-byte download progress fallbacks', () => {
    render(
      <I18nProvider>
        <UpdaterSection
          navItem={navItem}
          state={state({
            buildInfo: null,
            updateAvailability: {
              supported: true,
              available: true,
              currentVersion: '0.1.0',
              version: null,
              checkedAt: '2026-04-25T10:01:00.000Z',
              notes: null,
              publishedAt: null,
              error: null,
              downloadUrl: null,
            },
            updateInstallState: {
              phase: 'downloading',
              version: '0.2.0',
              downloadedBytes: null,
              contentLength: 100,
              message: null,
            },
          })}
        />
      </I18nProvider>,
    )

    expect(screen.getAllByText('Not available')).toHaveLength(4)
    expect(screen.getByText('Downloaded 0 B of 100 B.')).toBeVisible()
  })
})
