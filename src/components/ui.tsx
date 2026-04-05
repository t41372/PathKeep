import type { ReactNode } from 'react'
import type { TakeoutPreviewEntry } from '../lib/types'
import type { ResolvedLanguage } from '../lib/i18n'
import { formatDateTime } from '../lib/format'

// ---------------------------------------------------------------------------
// Glyph — Material Symbols icon wrapper
// ---------------------------------------------------------------------------

export function Glyph({
  icon,
  filled = false,
}: {
  icon: string
  filled?: boolean
}) {
  return (
    <span
      className={`material-symbols-outlined glyph ${filled ? 'filled' : ''}`}
    >
      {icon}
    </span>
  )
}

// ---------------------------------------------------------------------------
// Surface — bordered card with optional header actions
// ---------------------------------------------------------------------------

export function Surface({
  eyebrow,
  title,
  icon,
  actions,
  children,
}: {
  eyebrow: string
  title: string
  icon: string
  actions?: ReactNode
  children: ReactNode
}) {
  return (
    <section className="surface">
      <header className="surfaceHeader">
        <div className="surfaceTitle">
          <span className="surfaceIcon">
            <Glyph icon={icon} />
          </span>
          <div>
            <p className="sectionEyebrow">{eyebrow}</p>
            {title ? <h3>{title}</h3> : null}
          </div>
        </div>
        {actions ? <div className="surfaceActions">{actions}</div> : null}
      </header>
      <div className="surfaceBody">{children}</div>
    </section>
  )
}

// ---------------------------------------------------------------------------
// FieldBlock — label + form control wrapper
// ---------------------------------------------------------------------------

export function FieldBlock({
  label,
  control,
  children,
}: {
  label: string
  control?: ReactNode
  children?: ReactNode
}) {
  return (
    <label className="fieldBlock">
      <span className="fieldLabel">{label}</span>
      {control ?? children}
    </label>
  )
}

// ---------------------------------------------------------------------------
// ToggleRow — label + checkbox toggle
// ---------------------------------------------------------------------------

export function ToggleRow({
  checked,
  label,
  onChange,
}: {
  checked: boolean
  label: string
  onChange: (checked: boolean) => void
}) {
  return (
    <label className="toggleRow">
      <span>{label}</span>
      <input
        checked={checked}
        type="checkbox"
        onChange={(event) => onChange(event.target.checked)}
      />
    </label>
  )
}

// ---------------------------------------------------------------------------
// DataRow — key/value display row
// ---------------------------------------------------------------------------

export function DataRow({
  label,
  value,
  children,
}: {
  label: string
  value?: ReactNode
  children?: ReactNode
}) {
  return (
    <div className="dataRow">
      <dt>{label}</dt>
      <dd>{value ?? children}</dd>
    </div>
  )
}

// ---------------------------------------------------------------------------
// PathRow — path display with open/copy actions
// ---------------------------------------------------------------------------

export function PathRow({
  label,
  value,
  actions,
  onOpen,
  onCopy,
}: {
  label: string
  value: string
  actions?: ReactNode
  onOpen?: () => void
  onCopy?: () => void
}) {
  const actionBar =
    actions ??
    (onOpen || onCopy ? (
      <>
        {onOpen && (
          <button className="ghostButton" type="button" onClick={onOpen}>
            <Glyph icon="folder_open" />
          </button>
        )}
        {onCopy && (
          <button className="ghostButton" type="button" onClick={onCopy}>
            <Glyph icon="content_copy" />
          </button>
        )}
      </>
    ) : null)
  return (
    <div className="pathRow">
      <FieldBlock
        label={label}
        control={
          <input readOnly aria-label={label} type="text" value={value} />
        }
      />
      {actionBar ? <div className="pathActions">{actionBar}</div> : null}
    </div>
  )
}

// ---------------------------------------------------------------------------
// EmptyState — dashed box placeholder
// ---------------------------------------------------------------------------

export function EmptyState({
  children,
  icon,
  message,
}: {
  children?: ReactNode
  icon?: string
  message?: string
}) {
  return (
    <div className="emptyState">
      {icon && <Glyph icon={icon} />}
      {message && <p>{message}</p>}
      {children}
    </div>
  )
}

// ---------------------------------------------------------------------------
// InfoStat — compact stat chip with label + bold value
// ---------------------------------------------------------------------------

export function InfoStat({
  label,
  value,
}: {
  label: string
  value: ReactNode
}) {
  return (
    <div className="infoStat">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  )
}

// ---------------------------------------------------------------------------
// StatusTag — tinted inline badge
// ---------------------------------------------------------------------------

export function StatusTag({
  tone,
  children,
}: {
  tone: 'info' | 'success' | 'danger' | 'neutral'
  children: ReactNode
}) {
  return <span className={`statusTag ${tone}`}>{children}</span>
}

// ---------------------------------------------------------------------------
// PreviewEntryList — Takeout preview rows
// ---------------------------------------------------------------------------

export function PreviewEntryList({
  entries,
  language,
}: {
  entries: TakeoutPreviewEntry[]
  language: ResolvedLanguage
}) {
  return (
    <div className="previewList">
      {entries.map((entry) => (
        <article
          className="previewEntry"
          key={`${entry.sourcePath}:${entry.sourceVisitId}`}
        >
          <div className="previewMeta">
            <span>{formatDateTime(entry.visitedAt, language)}</span>
            <StatusTag tone={entry.status === 'imported' ? 'success' : 'info'}>
              {entry.status}
            </StatusTag>
          </div>
          <strong>{entry.title || entry.url}</strong>
          <p>{entry.url}</p>
          <small>
            {entry.sourcePath} · #{entry.sourceVisitId}
          </small>
        </article>
      ))}
    </div>
  )
}

// ---------------------------------------------------------------------------
// WorkflowStep type + OperationWorkflow — step-by-step process UI
// ---------------------------------------------------------------------------

export type WorkflowStep = {
  id: string
  title: string
  status: 'pending' | 'complete'
  summary: string
  reason: string
  files?: string[]
  commands?: string[]
  checklist?: string[]
  actions?: ReactNode
}

export function OperationWorkflow({
  actionLabel,
  labels,
  language,
  onCopy,
  steps,
}: {
  actionLabel: string
  labels: {
    why: string
    files: string
    commands: string
    checklist: string
    copy: string
    current: string
    complete: string
    pending: string
  }
  language: ResolvedLanguage
  onCopy: (value: string) => Promise<void>
  steps: WorkflowStep[]
}) {
  const currentIndex = steps.findIndex((step) => step.status !== 'complete')

  return (
    <ol className="workflowList" aria-label={actionLabel}>
      {steps.map((step, index) => {
        const displayStatus =
          step.status === 'complete'
            ? 'complete'
            : currentIndex === index
              ? 'current'
              : 'pending'

        return (
          <li className={`workflowStep ${displayStatus}`} key={step.id}>
            <div className="workflowMarker">
              <span>{index + 1}</span>
            </div>
            <div className="workflowCard">
              <div className="workflowHeader">
                <div>
                  <p className="sectionEyebrow">
                    {displayStatus === 'complete'
                      ? labels.complete
                      : displayStatus === 'current'
                        ? labels.current
                        : labels.pending}
                  </p>
                  <h3>{step.title}</h3>
                </div>
                <StatusTag
                  tone={
                    displayStatus === 'complete'
                      ? 'success'
                      : displayStatus === 'current'
                        ? 'info'
                        : 'neutral'
                  }
                >
                  {displayStatus === 'complete'
                    ? labels.complete
                    : displayStatus === 'current'
                      ? labels.current
                      : labels.pending}
                </StatusTag>
              </div>
              <p className="workflowSummary">{step.summary}</p>
              <div className="workflowSection">
                <strong>{labels.why}</strong>
                <p>{step.reason}</p>
              </div>
              {step.files?.length ? (
                <div className="workflowSection">
                  <strong>{labels.files}</strong>
                  <div className="artifactList">
                    {step.files.map((file) => (
                      <article className="artifactCard compactCard" key={file}>
                        <strong>{file}</strong>
                      </article>
                    ))}
                  </div>
                </div>
              ) : null}
              {step.commands?.length ? (
                <div className="workflowSection">
                  <strong>{labels.commands}</strong>
                  <div className="generatedList">
                    {step.commands.map((command) => (
                      <article className="codeArtifact" key={command}>
                        <div className="artifactHeader">
                          <strong>
                            {formatDateTime(new Date().toISOString(), language)}
                          </strong>
                          <button
                            className="ghostButton"
                            type="button"
                            onClick={() => void onCopy(command)}
                          >
                            {labels.copy}
                          </button>
                        </div>
                        <pre>{command}</pre>
                      </article>
                    ))}
                  </div>
                </div>
              ) : null}
              {step.checklist?.length ? (
                <div className="workflowSection">
                  <strong>{labels.checklist}</strong>
                  <ol className="stepList">
                    {step.checklist.map((item) => (
                      <li key={item}>{item}</li>
                    ))}
                  </ol>
                </div>
              ) : null}
              {step.actions ? (
                <div className="workflowActions">{step.actions}</div>
              ) : null}
            </div>
          </li>
        )
      })}
    </ol>
  )
}
