/**
 * @file route-state.test.tsx
 * @description URL-state regression coverage for shared Intelligence routes.
 * @module pages/intelligence
 *
 * ## Responsibilities
 * - Verify the route query contract for presets, custom ranges, profile scope, and promoted focus.
 * - Keep route-state mutations covered without mounting the full Intelligence page.
 *
 * ## Not responsible for
 * - Re-testing Core Intelligence backend payload loading.
 * - Re-testing entity route rendering.
 *
 * ## Dependencies
 * - Uses React Router memory history and the shared profile-scope context.
 *
 * ## Performance notes
 * - Hook-level tests avoid loading heavy Intelligence sections.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { MemoryRouter } from 'react-router-dom'
import { afterEach, describe, expect, test, vi } from 'vitest'
import { ProfileScopeContext } from '../../lib/profile-scope-context'
import { useIntelligenceRouteState } from './route-state'

function wrapperFor(
  initialEntry: string,
  activeProfileId: string | null = 'chrome:Default',
) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <MemoryRouter initialEntries={[initialEntry]}>
        <ProfileScopeContext.Provider
          value={{
            activeProfileId,
            setActiveProfileId: vi.fn(),
          }}
        >
          {children}
        </ProfileScopeContext.Provider>
      </MemoryRouter>
    )
  }
}

describe('useIntelligenceRouteState', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  test('falls back from incomplete custom ranges while preserving profile scope', () => {
    const { result } = renderHook(() => useIntelligenceRouteState(), {
      wrapper: wrapperFor(
        '/intelligence?range=custom&start=2026-04-01&focusType=path-flow&focusId=flow-1',
      ),
    })

    expect(result.current.preset).toBe('custom')
    expect(result.current.dateRange.end).toBeTruthy()
    expect(result.current.effectiveProfileId).toBe('chrome:Default')
    expect(result.current.profileScopeLabel).toBe('Default')
    expect(result.current.focus).toEqual({
      focusType: 'path-flow',
      focusId: 'flow-1',
    })
  })

  test('updates preset and custom range query state', async () => {
    const { result } = renderHook(() => useIntelligenceRouteState(), {
      wrapper: wrapperFor(
        '/intelligence?range=custom&start=2026-04-01&end=2026-04-30&profileId=safari%3AWork',
      ),
    })

    expect(result.current.explicitProfileId).toBe('safari:Work')
    expect(result.current.profileScopeLabel).toBe('Work')

    act(() => {
      result.current.setPreset('week')
    })
    await waitFor(() => expect(result.current.preset).toBe('week'))
    expect(result.current.withCurrentRouteSearch()).toBe(
      '?range=week&profileId=safari%3AWork',
    )

    act(() => {
      result.current.setCustomRange({
        start: '2026-04-10',
        end: '2026-04-12',
      })
    })
    await waitFor(() => expect(result.current.preset).toBe('custom'))
    expect(result.current.dateRange).toEqual({
      start: '2026-04-10',
      end: '2026-04-12',
    })
  })

  test('builds route search strings with explicit override semantics', () => {
    const { result } = renderHook(() => useIntelligenceRouteState(), {
      wrapper: wrapperFor('/intelligence?range=day', null),
    })

    expect(result.current.effectiveProfileId).toBeNull()
    expect(result.current.profileScopeLabel).toBeNull()
    expect(
      result.current.withCurrentRouteSearch({
        dateRange: { start: '2026-04-01', end: '2026-04-30' },
        preset: 'custom',
        profileId: 'chrome:Default',
        focus: {
          focusType: 'compare-set',
          focusId: 'compare-1',
        },
      }),
    ).toBe(
      '?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault&focusType=compare-set&focusId=compare-1',
    )
    expect(
      result.current.withCurrentRouteSearch({
        profileId: null,
        focus: null,
      }),
    ).toBe('?range=day')
  })

  test('parses all-time scope and keeps entity deep-link params without custom dates', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-04-25T12:00:00'))

    const { result } = renderHook(() => useIntelligenceRouteState(), {
      wrapper: wrapperFor(
        '/intelligence?range=all&start=2025-01-01&end=2025-12-31&profileId=chrome%3ADefault&focusType=compare-set&focusId=compare-1',
        null,
      ),
    })

    expect(result.current.preset).toBe('all')
    expect(result.current.dateRange).toEqual({
      start: '1900-01-01',
      end: '2026-04-25',
    })
    expect(result.current.withCurrentRouteSearch()).toBe(
      '?range=all&profileId=chrome%3ADefault&focusType=compare-set&focusId=compare-1',
    )

    vi.useRealTimers()

    act(() => {
      result.current.setCustomRange({
        start: '2026-04-10',
        end: '2026-04-12',
      })
    })
    await waitFor(() => expect(result.current.preset).toBe('custom'))

    act(() => {
      result.current.setPreset('all')
    })
    await waitFor(() => expect(result.current.preset).toBe('all'))
    expect(result.current.withCurrentRouteSearch({ focus: null })).toBe(
      '?range=all&profileId=chrome%3ADefault',
    )
  })

  test('normalizes invalid range links and keeps custom preset mutation branches explicit', async () => {
    const { result } = renderHook(() => useIntelligenceRouteState(), {
      wrapper: wrapperFor(
        '/intelligence?range=decade&start=2026-04-01&end=2026-04-30',
        null,
      ),
    })

    expect(result.current.preset).toBe('custom')
    expect(result.current.dateRange).toEqual({
      start: '2026-04-01',
      end: '2026-04-30',
    })
    expect(result.current.withCurrentRouteSearch({ profileId: null })).toBe(
      '?range=custom&start=2026-04-01&end=2026-04-30',
    )

    act(() => {
      result.current.setPreset('custom')
    })
    await waitFor(() => expect(result.current.preset).toBe('custom'))
    expect(result.current.dateRange).toEqual({
      start: '2026-04-01',
      end: '2026-04-30',
    })
  })
})
