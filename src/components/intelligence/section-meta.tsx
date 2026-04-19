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

  return (
    <details
      className="intelligence-section-meta"
      open={effectiveState !== 'ready'}
      data-testid={`intelligence-section-meta-${meta.sectionId}`}
    >
      <summary className="intelligence-section-meta__summary">
        <span className="intelligence-section-meta__summary-title">
          {t('sectionMetaTitle')}
        </span>
        <span
          className={`status-badge intelligence-section-meta__state intelligence-section-meta__state--${effectiveState}`}
        >
          {sectionStateLabel(effectiveState, t, settingsT)}
        </span>
      </summary>

      <div className="intelligence-section-meta__grid">
        <div className="intelligence-section-meta__row">
          <span className="intelligence-section-meta__label">
            {t('sectionMetaGeneratedAt')}
          </span>
          <span className="intelligence-section-meta__value mono-support">
            {meta.generatedAt
              ? (formatDateTime(meta.generatedAt, language) ?? meta.generatedAt)
              : commonT('notAvailable')}
          </span>
        </div>
        <div className="intelligence-section-meta__row">
          <span className="intelligence-section-meta__label">
            {t('sectionMetaScope')}
          </span>
          <span className="intelligence-section-meta__value">{scopeLabel}</span>
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
    </details>
  )
}
