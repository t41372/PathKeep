/**
 * Shared review-row and code-preview chrome for Settings intelligence workbench surfaces.
 *
 * Why this file exists:
 * - External outputs and trusted local-host review should share one
 *   presentational grammar instead of hand-rolling another Settings-only shell.
 */

import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'

export interface WorkbenchCopyFeedback {
  key: string
  tone: 'success' | 'error'
}

export interface WorkbenchTargetLink {
  href: string
  key: string
  label: string
}

export function WorkbenchReviewSection({
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

export function WorkbenchCopyStatus({
  copyFeedback,
  copyKey,
  errorMessage,
  successMessage,
}: {
  copyFeedback: WorkbenchCopyFeedback | null
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

export function WorkbenchCodePreview({
  actions,
  code,
  copyFeedback,
  copyKey,
  copyLabel,
  errorMessage,
  onCopy,
  successMessage,
  title,
  titleMeta,
}: {
  actions?: ReactNode
  code: string
  copyFeedback: WorkbenchCopyFeedback | null
  copyKey: string
  copyLabel: string
  errorMessage: string
  onCopy: (key: string, payload: string) => void
  successMessage: string
  title: ReactNode
  titleMeta?: ReactNode
}) {
  return (
    <WorkbenchReviewSection headerMeta={titleMeta} title={title}>
      <div className="code-panel">
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
      </div>
      <WorkbenchCopyStatus
        copyFeedback={copyFeedback}
        copyKey={copyKey}
        errorMessage={errorMessage}
        successMessage={successMessage}
      />
    </WorkbenchReviewSection>
  )
}

export function WorkbenchTargetLinksRow({
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
  secondaryLinks?: WorkbenchTargetLink[]
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
