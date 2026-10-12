/**
 * `useSearchParamsUpdate`: changes to the URL's search parameters that build
 * on what the URL holds now.
 *
 * A unit test because the bug is a race the E2E cannot set up on demand
 * (`quality-matrix.md` §什麼時候才寫 unit test): React Router renders a
 * navigation as a transition, and a click handled before it commits saw the
 * parameters of the last render. Choosing "All time" and then a profile on
 * Insights wrote the profile onto the old range, undoing "All time" (seen in
 * the gate, 2 runs in 6 on a busy machine).
 *
 * Ways it can fail:
 * - a second change before the screen renders the first undoes the first;
 * - a handler kept from an earlier render writes onto that render's URL;
 * - parameters the change does not touch are lost;
 * - a change adds a history entry when it should replace the current one.
 */
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { createHashRouter, RouterProvider } from 'react-router-dom'
import { afterEach, describe, expect, it } from 'vitest'
import { useSearchParamsUpdate } from './search-params'

type Update = ReturnType<typeof useSearchParamsUpdate>[1]

// Lets `act` flush React's work, as a test renderer would.
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })

let unmount = () => {}
afterEach(() => unmount())

/** A page at `#/page?<search>` whose updater each render hands out. */
function mount(search: string) {
  window.location.hash = `#/page?${search}`
  const updates: Update[] = []
  function Page() {
    updates.push(useSearchParamsUpdate()[1])
    return null
  }
  const router = createHashRouter([{ path: '/page', element: <Page /> }])
  const root = createRoot(document.createElement('div'))
  act(() => root.render(<RouterProvider router={router} />))
  unmount = () => act(() => root.unmount())
  return { updates, router }
}

const search = () => new URLSearchParams(window.location.hash.split('?')[1])

const set =
  (name: string, value: string | null) => (current: URLSearchParams) => {
    if (value === null) current.delete(name)
    else current.set(name, value)
    return current
  }

describe('useSearchParamsUpdate', () => {
  it('keeps a change made before the screen rendered the one before it', () => {
    const { updates } = mount('range=custom&start=2026-03-26')
    const update = updates.at(-1)!
    act(() => {
      update(set('range', null))
      update(set('start', null))
      update(set('profile', 'chrome:Profile 1'))
    })
    expect(Object.fromEntries(search())).toEqual({
      profile: 'chrome:Profile 1',
    })
  })

  it('builds on the URL now, also from a handler of an earlier render', () => {
    const { updates } = mount('range=custom')
    const first = updates.at(-1)!
    act(() => first(set('range', '7d')))
    expect(updates.length).toBeGreaterThan(1)
    act(() => first(set('profile', 'firefox:a')))
    expect(Object.fromEntries(search())).toEqual({
      range: '7d',
      profile: 'firefox:a',
    })
  })

  it('leaves the parameters it does not change', () => {
    const { updates } = mount('q=rust&view=list')
    act(() => updates.at(-1)!(set('view', 'timeline')))
    expect(Object.fromEntries(search())).toEqual({
      q: 'rust',
      view: 'timeline',
    })
  })

  it('replaces the current history entry', () => {
    const { updates, router } = mount('view=list')
    act(() => updates.at(-1)!(set('view', 'timeline')))
    expect(router.state.historyAction).toBe('REPLACE')
  })
})
