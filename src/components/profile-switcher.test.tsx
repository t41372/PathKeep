/**
 * @file profile-switcher.test.tsx
 * @description Focused interaction coverage for the shared shell profile switcher.
 * @module components
 *
 * ## Responsibilities
 * - Verify the profile switcher exposes selected browser profiles through an accessible listbox.
 * - Protect keyboard navigation, outside-click dismissal, and stale stored-profile recovery.
 * - Keep browser-profile labels tied to the app snapshot rather than raw profile ids.
 *
 * ## Not responsible for
 * - Re-testing the sidebar layout or route navigation around the switcher.
 * - Re-testing browser icon asset rendering beyond the switcher's accessible behavior.
 *
 * ## Dependencies
 * - Uses the shipped i18n provider, profile-scope provider, and shell-data context.
 *
 * ## Performance notes
 * - Builds a minimal shell snapshot fixture so the suite stays fast while still exercising the real component.
 */

import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, test, vi } from 'vitest'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../app/shell-data-context'
import { backend } from '../lib/backend-client'
import { backendTestHarness } from '../lib/backend'
import { I18nProvider } from '../lib/i18n'
import { ProfileScopeProvider } from '../lib/profile-scope'
import { ProfileScopeContext } from '../lib/profile-scope-context'
import type { AppSnapshot, BrowserProfile } from '../lib/types'
import { ProfileSwitcher } from './profile-switcher'

describe('ProfileSwitcher', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  test('selects profiles, supports keyboard movement, and closes on outside click', async () => {
    const user = userEvent.setup()
    const snapshot = await createSnapshot()

    renderProfileSwitcher(snapshot)

    const trigger = screen.getByRole('button', {
      name: 'Switch profile scope. Current: All profiles',
    })
    expect(trigger).not.toHaveAttribute('aria-controls')
    expect(trigger.querySelector('.profile-switcher__caret')).toHaveTextContent(
      '▾',
    )
    await user.click(trigger)
    expect(trigger).toHaveAttribute('aria-controls', 'profile-scope-listbox')
    expect(trigger.querySelector('.profile-switcher__caret')).toHaveTextContent(
      '▴',
    )

    const listbox = screen.getByRole('listbox', {
      name: 'Switch profile scope',
    })
    expect(listbox).toBeVisible()
    expect(screen.getByRole('option', { name: 'All profiles' })).toHaveClass(
      'profile-switcher__option',
      'profile-switcher__option--active',
    )
    expect(
      screen.getByRole('option', { name: 'All profiles' }),
    ).toHaveAttribute('aria-selected', 'true')
    expect(
      screen.getByRole('option', { name: 'Personal research' }),
    ).toBeVisible()
    expect(
      screen
        .getByRole('option', { name: 'Personal research' })
        .className.trim(),
    ).toBe('profile-switcher__option')
    expect(
      screen.getByRole('option', { name: 'Personal research' }),
    ).toHaveAttribute('aria-selected', 'false')
    expect(screen.getByRole('option', { name: 'Work' })).toBeVisible()

    await waitFor(() => {
      expect(screen.getByRole('option', { name: 'All profiles' })).toHaveFocus()
    })
    await user.keyboard('{ArrowDown}')
    expect(
      screen.getByRole('option', { name: 'Personal research' }),
    ).toHaveFocus()
    await user.keyboard('{End}')
    expect(screen.getByRole('option', { name: 'Work' })).toHaveFocus()
    await user.keyboard('{Home}')
    expect(screen.getByRole('option', { name: 'All profiles' })).toHaveFocus()

    await user.click(screen.getByRole('option', { name: 'Personal research' }))
    expect(
      screen.getByRole('button', {
        name: 'Switch profile scope. Current: Personal research',
      }),
    ).toBeVisible()
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    expect(
      screen
        .getByRole('button', {
          name: 'Switch profile scope. Current: Personal research',
        })
        .querySelector('.profile-switcher__caret'),
    ).toHaveTextContent('▾')

    await user.click(
      screen.getByRole('button', {
        name: 'Switch profile scope. Current: Personal research',
      }),
    )
    expect(screen.getByRole('listbox')).toBeVisible()
    expect(
      screen.getByRole('option', { name: 'Personal research' }),
    ).toHaveClass('profile-switcher__option--active')
    expect(
      screen.getByRole('option', { name: 'Personal research' }),
    ).toHaveAttribute('aria-selected', 'true')
    await user.click(screen.getByRole('button', { name: 'outside target' }))
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  test('updates profile options when the shell snapshot changes without remounting', async () => {
    const user = userEvent.setup()
    const initialSnapshot = await createSnapshot({
      selectedProfileIds: ['chrome:Default'],
      browserProfiles: [
        createBrowserProfile({
          profileId: 'chrome:Default',
          browserFamily: 'chromium',
          browserName: 'Google Chrome',
          profileName: 'Personal research',
          profilePath:
            '/Users/test/Library/Application Support/Google/Chrome/Default',
          historyPath:
            '/Users/test/Library/Application Support/Google/Chrome/Default/History',
          historyFileName: 'History',
        }),
        createBrowserProfile({
          profileId: 'safari:Work',
          browserFamily: 'safari',
          browserName: 'Safari',
          profileName: 'Work',
          profilePath: '/Users/test/Library/Safari',
          historyPath: '/Users/test/Library/Safari/History.db',
          historyFileName: 'History.db',
        }),
      ],
    })
    const updatedSnapshot = await createSnapshot({
      selectedProfileIds: ['safari:Work'],
      browserProfiles: [
        createBrowserProfile({
          profileId: 'safari:Work',
          browserFamily: 'safari',
          browserName: 'Safari',
          profileName: 'Client work',
          profilePath: '/Users/test/Library/Safari',
          historyPath: '/Users/test/Library/Safari/History.db',
          historyFileName: 'History.db',
        }),
      ],
    })

    const view = renderProfileSwitcher(initialSnapshot)
    view.rerender(
      profileSwitcherTree(
        updatedSnapshot,
        <ProfileScopeProvider>
          <ProfileSwitcher />
        </ProfileScopeProvider>,
      ),
    )

    await user.click(
      screen.getByRole('button', {
        name: 'Switch profile scope. Current: All profiles',
      }),
    )

    expect(
      screen.queryByRole('option', { name: 'Personal research' }),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByRole('option', { name: 'Work' }),
    ).not.toBeInTheDocument()
    expect(screen.getByRole('option', { name: 'Client work' })).toBeVisible()
  })

  test('clears an active profile when selected profiles change after mount', async () => {
    const setActiveProfileId = vi.fn()
    const initialSnapshot = await createSnapshot({
      selectedProfileIds: ['chrome:Default'],
    })
    const updatedSnapshot = await createSnapshot({
      selectedProfileIds: ['safari:Work'],
    })

    const view = renderProfileSwitcherShell(
      initialSnapshot,
      <ProfileScopeContext.Provider
        value={{ activeProfileId: 'chrome:Default', setActiveProfileId }}
      >
        <ProfileSwitcher />
      </ProfileScopeContext.Provider>,
    )
    expect(setActiveProfileId).not.toHaveBeenCalled()

    view.rerender(
      profileSwitcherTree(
        updatedSnapshot,
        <ProfileScopeContext.Provider
          value={{ activeProfileId: 'chrome:Default', setActiveProfileId }}
        >
          <ProfileSwitcher />
        </ProfileScopeContext.Provider>,
      ),
    )

    await waitFor(() => {
      expect(setActiveProfileId).toHaveBeenCalledWith(null)
    })
  })

  test('focuses the active profile option when the dropdown opens', async () => {
    const snapshot = await createSnapshot()

    renderProfileSwitcherWithProfileScope(snapshot, 'safari:Work')

    fireEvent.keyDown(
      screen.getByRole('button', {
        name: 'Switch profile scope. Current: Work',
      }),
      { key: 'ArrowDown' },
    )

    await waitFor(() => {
      expect(screen.getByRole('option', { name: 'Work' })).toHaveFocus()
    })
  })

  test('attaches dropdown document listeners and animation frames only while open', async () => {
    const addSpy = vi.spyOn(document, 'addEventListener')
    const removeSpy = vi.spyOn(document, 'removeEventListener')
    const requestAnimationFrameSpy = vi.spyOn(window, 'requestAnimationFrame')
    const cancelAnimationFrameSpy = vi.spyOn(window, 'cancelAnimationFrame')
    const user = userEvent.setup()
    const snapshot = await createSnapshot()

    renderProfileSwitcher(snapshot)

    expect(addSpy.mock.calls.some(([type]) => type === 'mousedown')).toBe(false)
    expect(addSpy.mock.calls.some(([type]) => type === 'keydown')).toBe(false)
    expect(requestAnimationFrameSpy).not.toHaveBeenCalled()

    await user.click(
      screen.getByRole('button', {
        name: 'Switch profile scope. Current: All profiles',
      }),
    )

    expect(addSpy.mock.calls.some(([type]) => type === 'mousedown')).toBe(true)
    expect(addSpy.mock.calls.some(([type]) => type === 'keydown')).toBe(true)
    expect(requestAnimationFrameSpy).toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: 'outside target' }))

    expect(removeSpy.mock.calls.some(([type]) => type === 'mousedown')).toBe(
      true,
    )
    expect(removeSpy.mock.calls.some(([type]) => type === 'keydown')).toBe(true)
    expect(cancelAnimationFrameSpy).toHaveBeenCalled()
  })

  test('keeps option keys distinct across normal and malformed profile ids', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
    const user = userEvent.setup()
    const normalSnapshot = await createSnapshot()

    const normalView = renderProfileSwitcher(normalSnapshot)
    await user.click(
      screen.getByRole('button', {
        name: 'Switch profile scope. Current: All profiles',
      }),
    )
    expect(screen.getAllByRole('option')).toHaveLength(3)
    normalView.unmount()

    const malformedSnapshot = await createSnapshot({
      selectedProfileIds: [''],
      browserProfiles: [],
    })

    renderProfileSwitcher(malformedSnapshot)
    await user.click(
      screen.getByRole('button', {
        name: 'Switch profile scope. Current: All profiles',
      }),
    )

    expect(screen.getAllByRole('option')).toHaveLength(2)
    expect(
      consoleError.mock.calls.some((call) =>
        call.some((part) =>
          String(part).includes('Encountered two children with the same key'),
        ),
      ),
    ).toBe(false)
  })

  test('recovers when stored profile scope no longer exists in selected profiles', async () => {
    window.localStorage.setItem('pathkeep.profile-scope', 'safari:Old')
    const snapshot = await createSnapshot({
      selectedProfileIds: ['chrome:Default'],
    })

    renderProfileSwitcher(snapshot)

    await waitFor(() => {
      expect(
        screen.getByRole('button', {
          name: 'Switch profile scope. Current: All profiles',
        }),
      ).toBeVisible()
    })
    expect(window.localStorage.getItem('pathkeep.profile-scope')).toBeNull()
  })

  test('opens from trigger arrows and closes from keyboard escape paths', async () => {
    const user = userEvent.setup()
    const snapshot = await createSnapshot()

    renderProfileSwitcher(snapshot)

    const trigger = screen.getByRole('button', {
      name: 'Switch profile scope. Current: All profiles',
    })
    trigger.focus()
    await user.keyboard('{ArrowDown}')

    expect(
      screen.getByRole('listbox', {
        name: 'Switch profile scope',
      }),
    ).toBeVisible()
    await waitFor(() => {
      expect(screen.getByRole('option', { name: 'All profiles' })).toHaveFocus()
    })

    await user.keyboard('{ArrowUp}')
    expect(screen.getByRole('option', { name: 'Work' })).toHaveFocus()

    await user.keyboard('{Escape}')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    expect(trigger).toHaveFocus()

    await user.keyboard('{ArrowUp}')
    expect(
      screen.getByRole('listbox', {
        name: 'Switch profile scope',
      }),
    ).toBeVisible()
    act(() => {
      document.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }),
      )
    })
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  test('falls back to profile id labels when selected profiles are not discovered', async () => {
    const user = userEvent.setup()
    window.localStorage.setItem('pathkeep.profile-scope', 'arc:Research')
    const snapshot = await createSnapshot({
      browserProfiles: [
        createBrowserProfile({
          profileId: 'chrome:Other',
          browserFamily: 'chromium',
          browserName: 'Google Chrome',
          profileName: 'Other',
          profilePath:
            '/Users/test/Library/Application Support/Google/Chrome/Profile 1',
          historyPath:
            '/Users/test/Library/Application Support/Google/Chrome/Profile 1/History',
          historyFileName: 'History',
        }),
      ],
      selectedProfileIds: ['arc:Research'],
    })

    renderProfileSwitcher(snapshot)

    expect(
      screen.getByRole('button', {
        name: 'Switch profile scope. Current: Research',
      }),
    ).toBeVisible()

    await user.click(
      screen.getByRole('button', {
        name: 'Switch profile scope. Current: Research',
      }),
    )

    expect(screen.getByRole('option', { name: 'Research' })).toBeVisible()
  })

  test('uses raw profile labels when discovery metadata is not loaded yet', async () => {
    const user = userEvent.setup()
    const baseSnapshot = await createSnapshot()
    const snapshot = {
      ...baseSnapshot,
      config: {
        ...baseSnapshot.config,
        selectedProfileIds: ['chrome:Default'],
      },
      browserProfiles: undefined,
    } as unknown as AppSnapshot

    renderProfileSwitcherWithProfileScope(snapshot, 'chrome:Default')

    const trigger = screen.getByRole('button', {
      name: 'Switch profile scope. Current: Default',
    })
    await user.click(trigger)

    expect(screen.getByRole('option', { name: 'Default' })).toBeVisible()
  })

  test('renders only the archive-wide option when the snapshot has no selected profile list yet', async () => {
    const user = userEvent.setup()
    const baseSnapshot = await createSnapshot()
    const snapshot = {
      ...baseSnapshot,
      config: {
        ...baseSnapshot.config,
        selectedProfileIds: undefined,
      },
      browserProfiles: undefined,
    } as unknown as AppSnapshot

    renderProfileSwitcher(snapshot)

    const trigger = screen.getByRole('button', {
      name: 'Switch profile scope. Current: All profiles',
    })
    fireEvent.keyDown(trigger, { key: 'Enter' })
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()

    await user.click(trigger)
    expect(screen.getAllByRole('option')).toHaveLength(1)
    expect(screen.getByRole('option', { name: 'All profiles' })).toBeVisible()
  })

  test('keeps fallback focus stable when the active profile is outside the visible options', async () => {
    const snapshot = await createSnapshot({
      selectedProfileIds: ['chrome:Default'],
    })

    renderProfileSwitcherWithProfileScope(snapshot, 'safari:Archived')

    const trigger = screen.getByRole('button', {
      name: 'Switch profile scope. Current: Archived',
    })
    fireEvent.keyDown(trigger, { key: 'ArrowDown' })

    await waitFor(() => {
      expect(screen.getByRole('option', { name: 'All profiles' })).toHaveFocus()
    })

    const allProfilesOption = screen.getByRole('option', {
      name: 'All profiles',
    })
    fireEvent.keyDown(allProfilesOption, { key: 'Tab' })
    expect(allProfilesOption).toHaveFocus()
  })
})

async function createSnapshot({
  selectedProfileIds = ['chrome:Default', 'safari:Work'],
  browserProfiles,
}: {
  selectedProfileIds?: string[]
  browserProfiles?: BrowserProfile[]
} = {}): Promise<AppSnapshot> {
  backendTestHarness.reset()
  const snapshot = await backend.getAppSnapshot()

  return {
    ...snapshot,
    config: {
      ...snapshot.config,
      selectedProfileIds,
    },
    browserProfiles: browserProfiles ?? [
      createBrowserProfile({
        profileId: 'chrome:Default',
        browserFamily: 'chromium',
        browserName: 'Google Chrome',
        profileName: 'Personal research',
        profilePath:
          '/Users/test/Library/Application Support/Google/Chrome/Default',
        historyPath:
          '/Users/test/Library/Application Support/Google/Chrome/Default/History',
        historyFileName: 'History',
      }),
      createBrowserProfile({
        profileId: 'safari:Work',
        browserFamily: 'safari',
        browserName: 'Safari',
        profileName: 'Work',
        profilePath: '/Users/test/Library/Safari',
        historyPath: '/Users/test/Library/Safari/History.db',
        historyFileName: 'History.db',
      }),
    ],
  }
}

function createBrowserProfile(
  profile: Pick<
    BrowserProfile,
    | 'browserFamily'
    | 'browserName'
    | 'historyFileName'
    | 'historyPath'
    | 'profileId'
    | 'profileName'
    | 'profilePath'
  >,
): BrowserProfile {
  return {
    ...profile,
    userName: null,
    faviconsPath: null,
    historyExists: true,
    historyReadable: true,
    accessIssue: null,
    browserVersion: null,
    historyBytes: 12_000,
    faviconsBytes: 2_000,
    supportingBytes: 14_000,
    retentionBoundary: {
      kind:
        profile.browserFamily === 'safari' ? 'macos-safari' : 'browser-managed',
      localDays: profile.browserFamily === 'safari' ? 365 : 90,
    },
  }
}

function renderProfileSwitcher(snapshot: AppSnapshot) {
  return renderProfileSwitcherShell(
    snapshot,
    <ProfileScopeProvider>
      <ProfileSwitcher />
    </ProfileScopeProvider>,
  )
}

function renderProfileSwitcherWithProfileScope(
  snapshot: AppSnapshot,
  activeProfileId: string | null,
) {
  return renderProfileSwitcherShell(
    snapshot,
    <ProfileScopeContext.Provider
      value={{ activeProfileId, setActiveProfileId: vi.fn() }}
    >
      <ProfileSwitcher />
    </ProfileScopeContext.Provider>,
  )
}

function renderProfileSwitcherShell(
  snapshot: AppSnapshot,
  profileSwitcher: ReactNode,
) {
  return render(profileSwitcherTree(snapshot, profileSwitcher))
}

function profileSwitcherTree(
  snapshot: AppSnapshot,
  profileSwitcher: ReactNode,
) {
  const shellValue = createShellDataValue(snapshot)

  return (
    <I18nProvider>
      <ShellDataContext.Provider value={shellValue}>
        <button type="button">outside target</button>
        {profileSwitcher}
      </ShellDataContext.Provider>
    </I18nProvider>
  )
}

function createShellDataValue(snapshot: AppSnapshot): ShellDataContextValue {
  return {
    buildInfo: null,
    appLockStatus: snapshot.appLockStatus,
    snapshot,
    dashboard: null,
    loading: false,
    busyAction: null,
    busyOverlay: null,
    error: null,
    notice: null,
    refreshKey: 0,
    refreshAppData: vi.fn().mockResolvedValue(undefined),
    refreshRuntimeStatus: vi.fn().mockResolvedValue({
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: null,
    }),
    saveConfig: vi.fn().mockResolvedValue(snapshot),
    initializeArchive: vi.fn().mockResolvedValue(snapshot),
    runBackup: vi.fn().mockRejectedValue(new Error('not implemented')),
    setAppLockPasscode: vi.fn().mockRejectedValue(new Error('not implemented')),
    clearAppLockPasscode: vi
      .fn()
      .mockRejectedValue(new Error('not implemented')),
    lockAppSession: vi.fn().mockRejectedValue(new Error('not implemented')),
    unlockAppSession: vi.fn().mockRejectedValue(new Error('not implemented')),
    clearNotice: vi.fn(),
    errorKind: null,
    clearError: vi.fn(),
  }
}
