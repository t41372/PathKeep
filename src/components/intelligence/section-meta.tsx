/**
 * Shared evidence/freshness review drawer for Core Intelligence sections.
 *
 * Why this file exists:
 * - `/intelligence` should reuse one honest grammar for freshness, source tables, module ownership,
 *   and degraded-state review instead of hand-rolling copy per section.
 * - Settings still owns rebuild/clear mutations, so this component stays read-only and review-focused.
 *
 * Main declarations:
 * - `IntelligenceSectionMeta`
 *
 * Source-of-truth notes:
 * - Keep labels aligned with the runtime terminology already used in Settings and Jobs.
 * - Keep section metadata behavior aligned with `docs/design/screens-and-nav.md` and
 *   `docs/features/intelligence-current-state.md`.
 */

import {
  useEffect,
  useId,
  useRef,
  useState,
  type FocusEvent as ReactFocusEvent,
} from 'react'
import { formatDateTime } from '../../lib/format'
import type { CoreIntelligenceSectionMeta } from '../../lib/core-intelligence'
import { deterministicModuleLabel } from '../../lib/intelligence-runtime'
import { useI18n } from '../../lib/i18n/hooks'

interface IntelligenceSectionMetaProps {
  meta: CoreIntelligenceSectionMeta
  scopeLabel: string
}

function isDateRangeWindow(window: unknown): window is {
  kind: 'date-range'
  dateRange: { start: string; end: string }
} {
  return (
    typeof window === 'object' &&
    window !== null &&
    (window as { kind?: unknown }).kind === 'date-range' &&
    typeof (window as { dateRange?: unknown }).dateRange === 'object' &&
    (window as { dateRange?: unknown }).dateRange !== null &&
    typeof (window as { dateRange: { start?: unknown } }).dateRange.start ===
      'string' &&
    typeof (window as { dateRange: { end?: unknown } }).dateRange.end ===
      'string'
  )
}

function isCalendarDayHistoryWindow(window: unknown): window is {
  kind: 'calendar-day-history'
  referenceDate: string
} {
  return (
    typeof window === 'object' &&
    window !== null &&
    (window as { kind?: unknown }).kind === 'calendar-day-history' &&
    typeof (window as { referenceDate?: unknown }).referenceDate === 'string'
  )
}

function sectionStateLabel(
  state: CoreIntelligenceSectionMeta['state'],
  t: (key: string) => string,
  settingsT: (key: string) => string,
) {
  switch (state) {
    case 'ready':
      return settingsT('deterministicModuleReady')
    case 'stale':
      return settingsT('deterministicModuleStale')
    case 'disabled':
      return settingsT('deterministicModuleDisabled')
    case 'degraded':
      return t('sectionMetaStateDegraded')
  }
}

function formatWindow(
  window: unknown,
  t: (key: string, vars?: Record<string, string | number>) => string,
  commonT: (key: string) => string,
) {
  if (isDateRangeWindow(window)) {
    return t('sectionMetaWindowDateRange', {
      start: window.dateRange.start,
      end: window.dateRange.end,
    })
  }

  if (isCalendarDayHistoryWindow(window)) {
    return t('sectionMetaWindowCalendarDayHistory', {
      date: window.referenceDate,
    })
  }

  return commonT('notAvailable')
}

/**
 * Renders the shared review drawer for one section envelope.
 */
export function IntelligenceSectionMeta({
  meta,
  scopeLabel,
}: IntelligenceSectionMetaProps) {
  const { language, ns } = useI18n()
  const t = ns('intelligence')
  const settingsT = ns('settings')
  const commonT = ns('common')
  const malformedWindow =
    !isDateRangeWindow(meta.window) && !isCalendarDayHistoryWindow(meta.window)
  const effectiveState = malformedWindow ? 'degraded' : meta.state
  const moduleSummary = meta.moduleIds.length
    ? meta.moduleIds
        .map((moduleId) => deterministicModuleLabel(moduleId, settingsT))
        .join(', ')
    : t('sectionMetaDirectRead')
  const notes = malformedWindow
    ? [t('sectionMetaMetadataFallback'), ...meta.notes]
    : meta.notes
  const rootRef = useRef<HTMLDivElement>(null)
  const [open, setOpen] = useState(false)
  const [pinned, setPinned] = useState(false)
  const panelId = useId()

  useEffect(() => {
    if (!open) {
      return
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') {
        return
      }

      setPinned(false)
      setOpen(false)
    }

    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [open])

  useEffect(() => {
    if (!pinned) {
      return
    }

    const handlePointerDown = (event: MouseEvent | TouchEvent) => {
      const target = event.target
      if (!(target instanceof Node) || rootRef.current?.contains(target)) {
        return
      }

      setPinned(false)
      setOpen(false)
    }

    document.addEventListener('mousedown', handlePointerDown)
    document.addEventListener('touchstart', handlePointerDown)
    return () => {
      document.removeEventListener('mousedown', handlePointerDown)
      document.removeEventListener('touchstart', handlePointerDown)
    }
  }, [pinned])

  const handleFocusCapture = (event: ReactFocusEvent<HTMLDivElement>) => {
    if (!rootRef.current?.contains(event.relatedTarget as Node | null)) {
      setOpen(true)
    }
  }

  const handleBlurCapture = (event: ReactFocusEvent<HTMLDivElement>) => {
    if (
      rootRef.current?.contains(event.relatedTarget as Node | null) ||
      pinned
    ) {
      return
    }

    setOpen(Boolean(rootRef.current?.matches(':hover')))
  }

  return (
    <div
      className="intelligence-section-meta"
      data-open={open}
      data-pinned={pinned}
      data-testid={`intelligence-section-meta-${meta.sectionId}`}
      ref={rootRef}
      onBlurCapture={handleBlurCapture}
      onFocusCapture={handleFocusCapture}
      onMouseEnter={() => {
        // Stryker disable next-line ConditionalExpression: when pinned is true, the panel is already open; calling setOpen(true) again is equivalent.
        if (!pinned) {
          setOpen(true)
        }
      }}
      onMouseLeave={() => {
        if (!pinned && !rootRef.current?.contains(document.activeElement)) {
          setOpen(false)
        }
      }}
    >
      <div className="intelligence-section-meta__summary">
        <button
          aria-controls={panelId}
          aria-expanded={open}
          aria-label={t(
            open ? 'sectionMetaClosePanelAria' : 'sectionMetaOpenPanelAria',
          )}
          className="intelligence-section-meta__trigger"
          data-testid={`intelligence-section-meta-trigger-${meta.sectionId}`}
          type="button"
          onClick={() => {
            if (pinned) {
              setPinned(false)
              setOpen(false)
              return
            }

            setPinned(true)
            setOpen(true)
          }}
        >
          <span className="intelligence-section-meta__summary-title">
            {t('sectionMetaTitle')}
          </span>
        </button>
        <span
          className={`status-badge intelligence-section-meta__state intelligence-section-meta__state--${effectiveState}`}
        >
          {sectionStateLabel(effectiveState, t, settingsT)}
        </span>
      </div>

      {open ? (
        <div
          className="intelligence-section-meta__panel"
          data-testid={`intelligence-section-meta-panel-${meta.sectionId}`}
          id={panelId}
        >
          <div className="intelligence-section-meta__panel-header">
            <span className="intelligence-section-meta__panel-title">
              {t('sectionMetaTitle')}
            </span>
            <span
              className={`status-badge intelligence-section-meta__state intelligence-section-meta__state--${effectiveState}`}
            >
              {sectionStateLabel(effectiveState, t, settingsT)}
            </span>
          </div>

          <div className="intelligence-section-meta__panel-body">
            <div className="intelligence-section-meta__grid">
              <div className="intelligence-section-meta__row">
                <span className="intelligence-section-meta__label">
                  {t('sectionMetaGeneratedAt')}
                </span>
                <span className="intelligence-section-meta__value mono-support">
                  {meta.generatedAt
                    ? (formatDateTime(meta.generatedAt, language) ??
                      meta.generatedAt)
                    : commonT('notAvailable')}
                </span>
              </div>
              <div className="intelligence-section-meta__row">
                <span className="intelligence-section-meta__label">
                  {t('sectionMetaScope')}
                </span>
                <span className="intelligence-section-meta__value">
                  {scopeLabel}
                </span>
              </div>
              <div className="intelligence-section-meta__row">
                <span className="intelligence-section-meta__label">
                  {t('sectionMetaWindow')}
                </span>
                <span className="intelligence-section-meta__value mono-support">
                  {formatWindow(meta.window, t, commonT)}
                </span>
              </div>
              <div className="intelligence-section-meta__row">
                <span className="intelligence-section-meta__label">
                  {t('sectionMetaModules')}
                </span>
                <span className="intelligence-section-meta__value">
                  {moduleSummary}
                </span>
              </div>
              <div className="intelligence-section-meta__row">
                <span className="intelligence-section-meta__label">
                  {t('sectionMetaSourceTables')}
                </span>
                <span className="intelligence-section-meta__value mono-support">
                  {meta.sourceTables.length
                    ? meta.sourceTables.join(', ')
                    : commonT('notAvailable')}
                </span>
              </div>
              <div className="intelligence-section-meta__row">
                <span className="intelligence-section-meta__label">
                  {t('sectionMetaEnrichment')}
                </span>
                <span className="intelligence-section-meta__value">
                  {meta.includesEnrichment
                    ? t('sectionMetaEnrichmentEnabled')
                    : t('sectionMetaEnrichmentDisabled')}
                </span>
              </div>
              {meta.stateReason ? (
                <div className="intelligence-section-meta__row">
                  <span className="intelligence-section-meta__label">
                    {t('sectionMetaStateReason')}
                  </span>
                  <span className="intelligence-section-meta__value">
                    {meta.stateReason}
                  </span>
                </div>
              ) : null}
            </div>

            {notes.length ? (
              <div className="intelligence-section-meta__notes">
                <span className="intelligence-section-meta__label">
                  {t('sectionMetaNotes')}
                </span>
                {notes.map((note) => (
                  <p key={`${meta.sectionId}-${note}`} className="mono-support">
                    {note}
                  </p>
                ))}
              </div>
            ) : null}
          </div>
        </div>
      ) : null}
    </div>
  )
}
