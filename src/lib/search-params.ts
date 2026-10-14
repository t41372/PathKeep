/**
 * Changes to the URL's search parameters that build on what the URL holds
 * now, for screens that keep their state there (Insights' scope, History's
 * filters).
 *
 * React Router hands `setSearchParams(previous => …)` the parameters of the
 * last render, and renders a navigation as a transition. A click handled
 * before that transition commits (the screen is busy drawing the first
 * change) wrote onto the old URL: choosing "All time" and then a profile on
 * Insights kept the profile and lost "All time". The router writes the URL
 * as it navigates, so the current parameters are read from it instead.
 *
 * Responsible for: reading the current parameters and replacing the URL.
 * Not responsible for: what the parameters mean.
 */
import { useCallback } from 'react'
import { useSearchParams } from 'react-router-dom'

/**
 * The search part of the URL as it is now. Both of PathKeep's routers are
 * hash routers, so it is in the hash.
 */
function currentSearch() {
  const hash = window.location.hash
  const query = hash.indexOf('?')
  return new URLSearchParams(query < 0 ? '' : hash.slice(query + 1))
}

/**
 * The URL's search parameters, and a function that changes them in place:
 * `change` gets the parameters the URL holds when it is called and returns the
 * new ones. The change replaces the current history entry.
 */
export function useSearchParamsUpdate() {
  const [params, setParams] = useSearchParams()
  const update = useCallback(
    (change: (current: URLSearchParams) => URLSearchParams) =>
      setParams(change(currentSearch()), { replace: true }),
    [setParams],
  )
  return [params, update] as const
}
