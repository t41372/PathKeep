/**
 * Product-grade route error boundary for shell routes.
 *
 * Why this file exists:
 * - Shell routes should never fall back to React Router's raw developer error page in front of users.
 * - A shared boundary keeps Explorer, Intelligence, and Jobs on the same recovery grammar.
 *
 * Main declarations:
 * - `ShellRouteErrorBoundary`
 */

import { useMemo } from 'react'
import { Link, isRouteErrorResponse, useRouteError } from 'react-router-dom'
import { ErrorState } from '../components/primitives/error-state'
import { useI18n } from '../lib/i18n/hooks'

function routeErrorMessage(error: unknown) {
  if (isRouteErrorResponse(error)) {
    if (typeof error.data === 'string' && error.data.trim()) {
      return error.data
    }
    return `${error.status} ${error.statusText}`.trim()
  }
  if (error instanceof Error) {
    return error.stack ?? error.message
  }
  if (typeof error === 'string') {
    return error
  }
  return null
}

/**
 * Renders a user-facing route error state instead of the default React Router developer page.
 */
export function ShellRouteErrorBoundary() {
  const error = useRouteError()
  const { ns } = useI18n()
  const commonT = ns('common')
  const jobsT = ns('jobs')
  const details = useMemo(() => routeErrorMessage(error), [error])

  return (
    <section className="page-shell" data-testid="shell-route-error-boundary">
      <ErrorState
        eyebrow={commonT('routeRenderErrorEyebrow')}
        title={commonT('routeRenderErrorTitle')}
        description={commonT('routeRenderErrorBody')}
        action={
          <div className="utility-block__actions">
            <button
              className="btn-secondary"
              type="button"
              onClick={() => {
                if (typeof window !== 'undefined') {
                  window.location.reload()
                }
              }}
            >
              {commonT('routeRenderErrorRetry')}
            </button>
            <Link className="btn-secondary" to="/jobs">
              {jobsT('openJobs')}
            </Link>
            <Link className="btn-secondary" to="/">
              {commonT('routeRenderErrorOverview')}
            </Link>
          </div>
        }
      />

      {details ? (
        <details className="panel" style={{ marginTop: 'var(--space-4)' }}>
          <summary className="panel-header" style={{ cursor: 'pointer' }}>
            <span className="panel-title">
              {commonT('routeRenderErrorDetails')}
            </span>
          </summary>
          <div className="panel-body">
            <pre className="code-block">
              <code>{details}</code>
            </pre>
          </div>
        </details>
      ) : null}
    </section>
  )
}
