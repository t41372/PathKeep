/**
 * Neutral review-row and code-preview grammar shared across Settings, Jobs,
 * Schedule, Audit, and intelligence-derived workbench surfaces.
 *
 * Why this file exists:
 * - M11 promotes review chrome out of intelligence-only ownership so routes
 *   with PME / diagnostics stories stop hand-rolling the same row shell.
 */

import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'

export interface ReviewCopyFeedback {
  key: string
  tone: 'success' | 'error'
}

export interface ReviewTargetLink {
  href: string
  key: string
  label: string
}

export function ReviewSection({
  active = false,
  children,
  className,
  headerMeta,
  title,
}: {
  active?: boolean
  children: ReactNode
  className?: string
  headerMeta?: ReactNode
  title: ReactNode
}) {
  return (
    <div
      className={`result-row${active ? ' result-row--active' : ''}${
        className ? ` ${className}` : ''
      }`}
    >
      <div className="result-row__header">
        <strong>{title}</strong>
        {headerMeta}
      </div>
      {children}
    </div>
  )
}

export function ReviewCopyStatus({
  copyFeedback,
  copyKey,
  errorMessage,
  successMessage,
}: {
  copyFeedback: ReviewCopyFeedback | null
  copyKey: string
  errorMessage: string
  successMessage: string
}) {
  if (copyFeedback?.key !== copyKey) {
    return null
  }

  return (
    <p
      className={
        copyFeedback.tone === 'success'
          ? 'dashboard-next-action'
          : 'inline-error'
      }
      role="status"
    >
      {copyFeedback.tone === 'success' ? successMessage : errorMessage}
    </p>
  )
}

export function ReviewCodePreview({
  actions,
  code,
  copyFeedback,
  copyKey,
  copyLabel,
  defaultOpen = true,
  errorMessage,
  onCopy,
  successMessage,
  title,
  titleMeta,
}: {
  actions?: ReactNode
  code: string
  copyFeedback: ReviewCopyFeedback | null
  copyKey: string
  copyLabel: string
  defaultOpen?: boolean
  errorMessage: string
  onCopy: (key: string, payload: string) => void
  successMessage: string
  title: ReactNode
  titleMeta?: ReactNode
}) {
  const codePanel = (
    <>
      <pre className="code-block">
        <code>{code}</code>
      </pre>
      <div className="code-actions">
        <button
          className="btn-tiny"
          type="button"
          onClick={() => {
            void onCopy(copyKey, code)
          }}
        >
          {copyLabel}
        </button>
        {actions}
      </div>
    </>
  )

  return (
    <ReviewSection headerMeta={titleMeta} title={title}>
      {defaultOpen ? (
        <div className="code-panel">{codePanel}</div>
      ) : (
        <details className="code-panel">
          <summary className="code-panel__summary">{title}</summary>
          {codePanel}
        </details>
      )}
      <ReviewCopyStatus
        copyFeedback={copyFeedback}
        copyKey={copyKey}
        errorMessage={errorMessage}
        successMessage={successMessage}
      />
    </ReviewSection>
  )
}

export function ReviewTargetLinksRow({
  fallback,
  label,
  primaryHref,
  primaryLabel,
  secondaryLinks = [],
}: {
  fallback?: ReactNode
  label: ReactNode
  primaryHref?: string | null
  primaryLabel?: ReactNode
  secondaryLinks?: ReviewTargetLink[]
}) {
  if (!primaryHref && secondaryLinks.length === 0 && !fallback) {
    return null
  }

  return (
    <div className="config-row">
      <span className="config-label">{label}</span>
      <span className="config-value">
        {primaryHref && primaryLabel ? (
          <Link className="intelligence-link" to={primaryHref}>
            {primaryLabel}
          </Link>
        ) : (
          (fallback ?? null)
        )}
        {secondaryLinks.length > 0 ? (
          <span className="settings-output-chip-list">
            {secondaryLinks.map((target) => (
              <Link key={target.key} className="chip-button" to={target.href}>
                {target.label}
              </Link>
            ))}
          </span>
        ) : null}
      </span>
    </div>
  )
}
