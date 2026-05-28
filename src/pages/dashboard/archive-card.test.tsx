/**
 * Coverage test for the dashboard archive card.
 *
 * ## Responsibilities
 * - Exercise Export (→ /audit) and Reveal (→ /maintenance) click handlers so
 *   the uncovered onClick branches reach navigate().
 * - Cover the Plaintext / Encrypted label branch and the chain-verified vs
 *   awaiting-first-run branch.
 */

import { describe, expect, test, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import type * as ReactRouter from 'react-router-dom'
import { I18nProvider } from '@/lib/i18n'
import { DashboardArchiveCard } from './archive-card'
import { dashboardT } from '../../app/index-tests/test-helpers'

const baseStorage = {
  archiveDatabaseBytes: 1024,
  sourceEvidenceDatabaseBytes: 0,
  searchDatabaseBytes: 512,
  intelligenceDatabaseBytes: 256,
  manifestBytes: 64,
  snapshotBytes: 128,
  exportBytes: 0,
  stagingBytes: 0,
  quarantineBytes: 0,
  semanticSidecarBytes: 0,
  intelligenceBlobBytes: 0,
}

const navigateMock = vi.fn()
vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof ReactRouter>('react-router-dom')
  return {
    ...actual,
    useNavigate: () => navigateMock,
  }
})

function renderCard(props: Parameters<typeof DashboardArchiveCard>[0]) {
  return render(
    <I18nProvider>
      <MemoryRouter>
        <DashboardArchiveCard {...props} />
      </MemoryRouter>
    </I18nProvider>,
  )
}

describe('DashboardArchiveCard', () => {
  test('exports route to /audit and reveals route to /maintenance', async () => {
    navigateMock.mockReset()
    const user = userEvent.setup()
    renderCard({
      databasePath: '/tmp/archive.db',
      archiveMode: 'Encrypted',
      totalBytes: 2048,
      storage: baseStorage,
      latestManifestHash: 'abcdef1234567890',
    })

    await user.click(
      screen.getByRole('button', { name: dashboardT('archiveExport') }),
    )
    expect(navigateMock).toHaveBeenLastCalledWith('/audit')

    await user.click(
      screen.getByRole('button', { name: dashboardT('archiveReveal') }),
    )
    expect(navigateMock).toHaveBeenLastCalledWith('/maintenance')
  })

  test('falls back to placeholders when path is empty and chain is awaiting first run', () => {
    navigateMock.mockReset()
    renderCard({
      databasePath: '',
      archiveMode: 'Plaintext',
      totalBytes: 0,
      storage: baseStorage,
      latestManifestHash: null,
    })

    expect(screen.getByText('~/PathKeep/archive.db')).toBeInTheDocument()
    expect(screen.getByText('----…----')).toBeInTheDocument()
  })
})
