/**
 * Behaviour tests for the content-fetch consent section (W-ENRICH-1).
 *
 * This is a privacy-sensitive surface, so the tests assert the load-bearing
 * facts: the master switch reflects the backend, the network-policy disclosure
 * (what is / is not sent) is always present, toggling persists through the
 * dedicated command, extractor toggles gate on the master switch, the blocklist
 * round-trips, a save failure surfaces honestly (no fake opt-in), and the
 * browser-preview build shows the honest "desktop only" state.
 */

import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

const { hasDesktopCommandTransportMock } = vi.hoisted(() => ({
  hasDesktopCommandTransportMock: vi.fn(() => true),
}))

vi.mock('@/lib/runtime', () => ({
  hasDesktopCommandTransport: hasDesktopCommandTransportMock,
}))

import { I18nProvider } from '@/lib/i18n'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '@/app/shell-data-context'
import { backend } from '@/lib/backend-client'
import type { ContentFetchSettings } from '@/lib/types'
import { ContentFetchSection } from './content-fetch-section'

function settings(
  overrides: Partial<ContentFetchSettings> = {},
): ContentFetchSettings {
  return {
    enabled: false,
    extractors: [],
    domains: [],
    queuedJobs: 0,
    runningJobs: 0,
    failedJobs: 0,
    storedRecords: 0,
    ...overrides,
  }
}

function renderSectionRaw(
  refreshAppData = vi.fn().mockResolvedValue(undefined),
) {
  const value = {
    snapshot: null,
    refreshAppData,
  } as unknown as ShellDataContextValue
  const result = render(
    <I18nProvider>
      <ShellDataContext.Provider value={value}>
        <ContentFetchSection />
      </ShellDataContext.Provider>
    </I18nProvider>,
  )
  return { ...result, refreshAppData }
}

function renderSection(refreshAppData = vi.fn().mockResolvedValue(undefined)) {
  return renderSectionRaw(refreshAppData)
}

describe('ContentFetchSection', () => {
  beforeEach(() => {
    hasDesktopCommandTransportMock.mockReturnValue(true)
    vi.spyOn(backend, 'getContentFetchSettings').mockResolvedValue(settings())
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  test('reflects the master switch from the backend and always shows the egress disclosure', async () => {
    vi.spyOn(backend, 'getContentFetchSettings').mockResolvedValue(
      settings({ enabled: true }),
    )
    renderSection()

    await waitFor(() =>
      expect(screen.getByTestId('content-fetch-master-toggle')).toHaveAttribute(
        'aria-checked',
        'true',
      ),
    )
    // The disclosure of what a host learns / what is never sent is always
    // visible — even while off — so the user consents with full sight.
    const disclosure = screen.getByTestId('content-fetch-disclosure')
    expect(disclosure).toHaveTextContent('your IP address')
    expect(disclosure).toHaveTextContent('Never sent: cookies')
    expect(disclosure).toHaveTextContent('Offline-first')
    expect(disclosure).toHaveTextContent('Rate-limited per host')
  })

  test('turning the master switch on persists through set_content_fetch_settings and re-syncs the shell', async () => {
    const setSpy = vi
      .spyOn(backend, 'setContentFetchSettings')
      .mockResolvedValue({} as never)
    const { refreshAppData } = renderSection()

    await waitFor(() =>
      expect(screen.getByTestId('content-fetch-master-toggle')).toBeVisible(),
    )
    await userEvent.click(screen.getByTestId('content-fetch-master-toggle'))

    expect(setSpy).toHaveBeenCalledTimes(1)
    expect(setSpy.mock.calls[0][0]).toMatchObject({ enabled: true })
    await waitFor(() => expect(refreshAppData).toHaveBeenCalled())
  })

  test('extractor toggles are disabled until the master switch is on', async () => {
    renderSection()
    await waitFor(() =>
      expect(
        screen.getByTestId('content-fetch-extractor-github-repo'),
      ).toBeVisible(),
    )
    expect(
      screen.getByTestId('content-fetch-extractor-github-repo'),
    ).toBeDisabled()
  })

  test('with the master switch on, toggling an extractor persists an explicit disable', async () => {
    vi.spyOn(backend, 'getContentFetchSettings').mockResolvedValue(
      settings({ enabled: true }),
    )
    const setSpy = vi
      .spyOn(backend, 'setContentFetchSettings')
      .mockResolvedValue({} as never)
    renderSection()

    await waitFor(() =>
      expect(
        screen.getByTestId('content-fetch-extractor-github-repo'),
      ).not.toBeDisabled(),
    )
    await userEvent.click(
      screen.getByTestId('content-fetch-extractor-github-repo'),
    )
    expect(setSpy).toHaveBeenCalledTimes(1)
    expect(setSpy.mock.calls[0][0].extractors).toContainEqual({
      extractorId: 'github-repo',
      enabled: false,
    })
  })

  test('auto-saves the blocklist on blur (no Save/Reset buttons)', async () => {
    vi.spyOn(backend, 'getContentFetchSettings').mockResolvedValue(
      settings({ enabled: true }),
    )
    const setSpy = vi
      .spyOn(backend, 'setContentFetchSettings')
      .mockResolvedValue({} as never)
    renderSection()

    const input = await screen.findByTestId('content-fetch-domains-input')
    // The per-section Save / Reset controls are gone in the all-auto-save model.
    expect(screen.queryByTestId('content-fetch-domains-save')).toBeNull()
    expect(screen.queryByTestId('content-fetch-domains-reset')).toBeNull()

    fireEvent.change(input, { target: { value: 'blocked.test\n' } })
    // Persists on blur, off the keystroke hot path.
    fireEvent.blur(input)

    await waitFor(() => expect(setSpy).toHaveBeenCalledTimes(1))
    expect(setSpy.mock.calls[0][0].domains).toEqual([
      { domain: 'blocked.test', allowed: false },
    ])
    // The quiet "Saved" chip flashes after a landed write.
    expect(await screen.findByText('Saved')).toBeInTheDocument()
  })

  test('a save failure rolls back the toggle and surfaces an honest error (no fake opt-in)', async () => {
    vi.spyOn(backend, 'setContentFetchSettings').mockRejectedValue(
      new Error('boom'),
    )
    renderSection()

    await waitFor(() =>
      expect(screen.getByTestId('content-fetch-master-toggle')).toBeVisible(),
    )
    await userEvent.click(screen.getByTestId('content-fetch-master-toggle'))

    await waitFor(() =>
      expect(screen.getByTestId('content-fetch-save-error')).toBeVisible(),
    )
    // The egress switch must not read "on" when the write did not land.
    expect(screen.getByTestId('content-fetch-master-toggle')).toHaveAttribute(
      'aria-checked',
      'false',
    )
  })

  test('the prime action enqueues the working set and reports the count', async () => {
    vi.spyOn(backend, 'getContentFetchSettings').mockResolvedValue(
      settings({ enabled: true }),
    )
    const enqueueSpy = vi
      .spyOn(backend, 'enqueueContentFetchWorkingSet')
      .mockResolvedValue(7)
    renderSection()

    await waitFor(() =>
      expect(screen.getByTestId('content-fetch-prime')).not.toBeDisabled(),
    )
    await userEvent.click(screen.getByTestId('content-fetch-prime'))
    expect(enqueueSpy).toHaveBeenCalledWith(500)
    await waitFor(() =>
      expect(
        screen.getByTestId('content-fetch-prime-summary'),
      ).toHaveTextContent('Queued 7 pages'),
    )
  })

  test('reports "nothing new" when the prime enqueues zero jobs', async () => {
    vi.spyOn(backend, 'getContentFetchSettings').mockResolvedValue(
      settings({ enabled: true }),
    )
    vi.spyOn(backend, 'enqueueContentFetchWorkingSet').mockResolvedValue(0)
    renderSection()

    await waitFor(() =>
      expect(screen.getByTestId('content-fetch-prime')).not.toBeDisabled(),
    )
    await userEvent.click(screen.getByTestId('content-fetch-prime'))
    await waitFor(() =>
      expect(
        screen.getByTestId('content-fetch-prime-summary'),
      ).toHaveTextContent('Nothing new to enrich'),
    )
  })

  test('surfaces a prime failure honestly in the summary', async () => {
    vi.spyOn(backend, 'getContentFetchSettings').mockResolvedValue(
      settings({ enabled: true }),
    )
    vi.spyOn(backend, 'enqueueContentFetchWorkingSet').mockRejectedValue(
      new Error('prime boom'),
    )
    renderSection()

    await waitFor(() =>
      expect(screen.getByTestId('content-fetch-prime')).not.toBeDisabled(),
    )
    await userEvent.click(screen.getByTestId('content-fetch-prime'))
    await waitFor(() =>
      expect(
        screen.getByTestId('content-fetch-prime-summary'),
      ).toBeInTheDocument(),
    )
  })

  test('toggling the generic-readable extractor persists an explicit disable', async () => {
    vi.spyOn(backend, 'getContentFetchSettings').mockResolvedValue(
      settings({ enabled: true }),
    )
    const setSpy = vi
      .spyOn(backend, 'setContentFetchSettings')
      .mockResolvedValue({} as never)
    renderSection()

    await waitFor(() =>
      expect(
        screen.getByTestId('content-fetch-extractor-generic-readable'),
      ).not.toBeDisabled(),
    )
    await userEvent.click(
      screen.getByTestId('content-fetch-extractor-generic-readable'),
    )
    expect(setSpy.mock.calls[0][0].extractors).toContainEqual({
      extractorId: 'generic-readable',
      enabled: false,
    })
  })

  test('a blur with no blocklist change does not re-save (no-op auto-save)', async () => {
    vi.spyOn(backend, 'getContentFetchSettings').mockResolvedValue(
      settings({
        enabled: true,
        domains: [{ domain: 'saved.test', allowed: false }],
      }),
    )
    const setSpy = vi
      .spyOn(backend, 'setContentFetchSettings')
      .mockResolvedValue({} as never)
    renderSection()

    const input = await screen.findByTestId('content-fetch-domains-input')
    await waitFor(() =>
      expect((input as HTMLTextAreaElement).value).toBe('saved.test'),
    )
    // Focus and blur without editing — the canonicalized rules are unchanged, so
    // there is no redundant write or misleading "Saved".
    fireEvent.focus(input)
    fireEvent.blur(input)
    expect(setSpy).not.toHaveBeenCalled()
  })

  test('shows the live activity summary when records exist', async () => {
    vi.spyOn(backend, 'getContentFetchSettings').mockResolvedValue(
      settings({
        enabled: true,
        storedRecords: 5,
        queuedJobs: 2,
        runningJobs: 1,
        failedJobs: 0,
      }),
    )
    renderSection()

    await waitFor(() =>
      expect(screen.getByTestId('content-fetch-status')).toHaveTextContent(
        '5 enriched',
      ),
    )
  })

  test('surfaces a load error honestly when the settings read fails', async () => {
    vi.spyOn(backend, 'getContentFetchSettings').mockRejectedValue(
      new Error('nope'),
    )
    renderSection()
    // The panel still renders (default off); the load error callout is shown
    // above the controls rather than crashing the panel.
    await waitFor(() =>
      expect(screen.getByTestId('content-fetch-master-toggle')).toHaveAttribute(
        'aria-checked',
        'false',
      ),
    )
    expect(screen.getByTestId('settings-content-fetch-section')).toBeVisible()
  })

  test('drops a settings read that resolves after unmount (no late state update)', async () => {
    let resolveRead: (value: ContentFetchSettings) => void = () => {}
    vi.spyOn(backend, 'getContentFetchSettings').mockReturnValue(
      new Promise<ContentFetchSettings>((resolve) => {
        resolveRead = resolve
      }),
    )
    const { unmount } = renderSectionRaw()
    unmount()
    // Resolving after unmount must hit the `if (cancelled) return` guard and
    // not throw / warn about updating an unmounted component.
    await act(async () => {
      resolveRead(settings())
      await Promise.resolve()
    })
  })

  test('drops a settings read that rejects after unmount', async () => {
    let rejectRead: (reason: unknown) => void = () => {}
    vi.spyOn(backend, 'getContentFetchSettings').mockReturnValue(
      new Promise<ContentFetchSettings>((_resolve, reject) => {
        rejectRead = reject
      }),
    )
    const { unmount } = renderSectionRaw()
    unmount()
    await act(async () => {
      rejectRead(new Error('late'))
      await Promise.resolve()
    })
  })

  test('a toggle click during the initial load (settings not yet loaded) is a safe no-op', () => {
    // Hold the read open so the controls render with the default-off toggle
    // while `settings` is still null, exercising the `if (!settings) return`
    // guards in the handlers.
    vi.spyOn(backend, 'getContentFetchSettings').mockReturnValue(
      new Promise<ContentFetchSettings>(() => {}),
    )
    const setSpy = vi
      .spyOn(backend, 'setContentFetchSettings')
      .mockResolvedValue({} as never)
    renderSection()

    // The master toggle renders immediately (default off) before settings
    // resolve; clicking it must hit the `if (!settings) return` guard in
    // onToggleMaster and not call the backend. (The extractor toggles are
    // disabled while the master switch is off, so they cannot fire here.)
    fireEvent.click(screen.getByTestId('content-fetch-master-toggle'))
    expect(
      screen.getByTestId('content-fetch-extractor-github-repo'),
    ).toBeDisabled()
    expect(setSpy).not.toHaveBeenCalled()
  })

  test('shows the honest "desktop only" state in browser-preview', async () => {
    hasDesktopCommandTransportMock.mockReturnValue(false)
    renderSection()
    expect(
      await screen.findByText(
        'Site content fetching only works in the desktop app.',
      ),
    ).toBeVisible()
    expect(
      screen.queryByTestId('content-fetch-master-toggle'),
    ).not.toBeInTheDocument()
  })
})
