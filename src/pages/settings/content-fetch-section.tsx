/**
 * Settings → Site content (content-fetch consent) section (W-ENRICH-1, 06 §6).
 *
 * This is the consent boundary for the only PathKeep feature that reaches out to
 * the sites the user visited. It surfaces:
 * - A master `enabled` switch (HARD-DEFAULT-OFF — reflects the backend).
 * - A network-policy disclosure (what a host learns / what is never sent /
 *   offline-first / per-host rate limit) so the user opts in with full sight of
 *   the egress, not a black box.
 * - Per-extractor toggles (GitHub repo metadata, generic page summary), plus an
 *   honest "limited / unavailable" note for video captions and X.
 * - A per-domain blocklist.
 * - A small live activity status + an "enrich top pages" prime action.
 *
 * ## Responsibilities
 * - Own the content-fetch settings draft (loaded via the typed wrapper), apply
 *   the user's consent edits through `set_content_fetch_settings`, and re-sync
 *   shell state on success.
 * - Frame every egress-enabling control with PME clarity; never silently enable
 *   network access.
 *
 * ## Not responsible for
 * - Detail-panel enrichment rendering (that lives in the explorer surfaces).
 * - The offline title-normalization plugin (governed separately by
 *   `enrichmentEnabled` in the derived-state section).
 *
 * ## Performance notes
 * - One settings read on mount, writes only on user edits (all-auto-save). No
 *   polling, no per-render fan-out. The blocklist textarea keeps a local draft
 *   while typing and auto-saves on blur, off the keystroke hot path.
 */

import { useCallback, useEffect, useMemo, useState } from 'react'
import { useI18n } from '@/lib/i18n'
import { useShellData } from '@/app/shell-data-context'
import { backend } from '@/lib/backend-client'
import { describeError } from '@/lib/errors'
import { hasDesktopCommandTransport } from '@/lib/runtime'
import { PaperCard, PaperCardBody, PaperCardHeader } from '@/components/cards'
import { Button } from '@/components/ui/button'
import { StatusCallout } from '@/components/primitives/status-callout'
import type { ContentFetchSettings } from '@/lib/types'
import { cn } from '@/lib/cn'
import { Field, Toggle } from './paper-form-primitives'
import { SettingsSavedChip } from './settings-saved-feedback'
import { useSavedFeedback } from './use-saved-feedback'
import {
  CONTENT_FETCH_EXTRACTOR_GENERIC_READABLE,
  CONTENT_FETCH_EXTRACTOR_GITHUB_REPO,
  applyContentFetchExtractorToggle,
  applyContentFetchMasterToggle,
  buildContentFetchDomainRules,
  domainRulesToText,
  extractorEnabled,
} from './content-fetch-helpers'

export interface ContentFetchSectionProps {
  anchorId?: string
}

const PRIME_WORKING_SET_LIMIT = 500

export function ContentFetchSection({
  anchorId = 'content-fetch',
}: ContentFetchSectionProps) {
  const { t } = useI18n()
  const { refreshAppData } = useShellData()
  const { visible: savedVisible, flash } = useSavedFeedback()
  const desktop = hasDesktopCommandTransport()

  const [settings, setSettings] = useState<ContentFetchSettings | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [domainsDraft, setDomainsDraft] = useState('')
  const [priming, setPriming] = useState(false)
  const [primeSummary, setPrimeSummary] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    void (async () => {
      try {
        const next = await backend.getContentFetchSettings()
        if (cancelled) return
        setSettings(next)
        setDomainsDraft(domainRulesToText(next.domains))
        setLoadError(null)
      } catch (error) {
        if (cancelled) return
        setLoadError(describeError(error, 'get_content_fetch_settings'))
      }
    })()
    return () => {
      cancelled = true
    }
  }, [])

  // Persist a full settings draft, optimistically reflecting it so the consent
  // controls stay responsive, then re-syncing on the backend's truth. On
  // failure we roll back to the prior settings and surface an honest error —
  // an egress switch must never read "on" when the write did not land.
  const persist = useCallback(
    async (next: ContentFetchSettings) => {
      const previous = settings
      setSettings(next)
      setSaving(true)
      setSaveError(null)
      try {
        await backend.setContentFetchSettings(next)
        await refreshAppData()
        setSettings(next)
        // Quiet "Saved" confirmation on a landed write — the page is all-auto-save.
        flash()
      } catch (error) {
        setSettings(previous)
        setSaveError(describeError(error, 'set_content_fetch_settings'))
      } finally {
        setSaving(false)
      }
    },
    [flash, refreshAppData, settings],
  )

  const onToggleMaster = useCallback(
    (nextEnabled: boolean) => {
      if (!settings) return
      void persist(applyContentFetchMasterToggle(settings, nextEnabled))
    },
    [persist, settings],
  )

  const onToggleExtractor = useCallback(
    (extractorId: string, nextEnabled: boolean) => {
      // Unreachable while settings are null: the extractor toggles are disabled
      // until the master switch is on, which requires loaded settings. Kept for
      // type-safe access below.
      /* v8 ignore next */
      if (!settings) return
      void persist(
        applyContentFetchExtractorToggle(settings, extractorId, nextEnabled),
      )
    },
    [persist, settings],
  )

  // Per-domain blocklist auto-saves on blur (the page is all-auto-save). It is a
  // free-text textarea so it edits a local draft while typing — keeping the
  // backend write off the keystroke hot path — and only persists when focus
  // leaves and the canonicalized rules differ from what's saved, so a blur with
  // no edit never fires a redundant write or a misleading "Saved".
  const onCommitDomains = useCallback(() => {
    // The textarea only renders once settings load, so blur can't fire before
    // then; this guard is for type-safe access and is unreachable via the UI.
    /* v8 ignore next */
    if (!settings) return
    const nextDomains = buildContentFetchDomainRules(domainsDraft)
    if (
      domainRulesToText(nextDomains) === domainRulesToText(settings.domains)
    ) {
      return
    }
    void persist({ ...settings, domains: nextDomains })
  }, [domainsDraft, persist, settings])

  const onPrime = useCallback(async () => {
    setPriming(true)
    setPrimeSummary(null)
    try {
      const count = await backend.enqueueContentFetchWorkingSet(
        PRIME_WORKING_SET_LIMIT,
      )
      setPrimeSummary(
        count > 0
          ? t('settings.contentFetchPrimeSummary', { count: String(count) })
          : t('settings.contentFetchPrimeNone'),
      )
    } catch (error) {
      setPrimeSummary(describeError(error, 'enqueue_content_fetch_working_set'))
    } finally {
      setPriming(false)
    }
  }, [t])

  const domainsChanged = useMemo(() => {
    if (!settings) return false
    return domainsDraft !== domainRulesToText(settings.domains)
  }, [domainsDraft, settings])

  const masterOn = settings?.enabled ?? false

  if (!desktop) {
    // Browser-preview has no fetch backend; show the section honestly disabled
    // rather than pretending consent can be granted in the preview.
    return (
      <PaperCard testId="settings-content-fetch-section" id={anchorId}>
        <PaperCardHeader title={t('settings.contentFetchTitle')} />
        <PaperCardBody>
          <StatusCallout
            tone="info"
            title={t('settings.contentFetchTitle')}
            body={t('settings.contentFetchUnavailable')}
          />
        </PaperCardBody>
      </PaperCard>
    )
  }

  return (
    <PaperCard testId="settings-content-fetch-section" id={anchorId}>
      <PaperCardHeader
        title={t('settings.contentFetchTitle')}
        right={<SettingsSavedChip visible={savedVisible} />}
      />
      <PaperCardBody>
        <p className="text-ink-muted m-0 mb-4 font-serif text-[13.5px] leading-[1.55] italic">
          {t('settings.contentFetchIntro')}
        </p>

        {loadError ? (
          <div className="mb-4">
            <StatusCallout
              tone="warning"
              title={t('settings.contentFetchTitle')}
              body={loadError}
            />
          </div>
        ) : null}

        <Field
          label={t('settings.contentFetchMasterLabel')}
          help={t('settings.contentFetchMasterHelp')}
        >
          <Toggle
            value={masterOn}
            onChange={onToggleMaster}
            onLabel={t('settings.contentFetchMasterOn')}
            offLabel={t('settings.contentFetchMasterOff')}
            testId="content-fetch-master-toggle"
          />
        </Field>

        {/*
          The network-policy disclosure is intentionally always visible — even
          while fetching is off — so the user can read exactly what opting in
          means before they flip the switch, not after.
        */}
        <div className="my-4" data-testid="content-fetch-disclosure">
          <StatusCallout
            tone="info"
            title={t('settings.contentFetchDisclosureTitle')}
            body={t('settings.contentFetchDisclosureBody')}
          />
          <ul className="text-ink-muted mt-2 flex list-none flex-col gap-1.5 p-0 font-sans text-[12px] leading-[1.5]">
            <li>{t('settings.contentFetchDisclosureNotSent')}</li>
            <li>{t('settings.contentFetchDisclosureOffline')}</li>
            <li>{t('settings.contentFetchDisclosureRateLimit')}</li>
          </ul>
        </div>

        <Field
          label={t('settings.contentFetchExtractorsLabel')}
          help={t('settings.contentFetchExtractorsHelp')}
        >
          <div className="flex flex-col gap-2">
            <ExtractorToggle
              label={t('settings.contentFetchExtractorGithubRepo')}
              hint={t('settings.contentFetchExtractorGithubRepoHint')}
              enabled={extractorEnabled(
                settings,
                CONTENT_FETCH_EXTRACTOR_GITHUB_REPO,
              )}
              disabled={!masterOn}
              onLabel={t('settings.contentFetchExtractorOn')}
              offLabel={t('settings.contentFetchExtractorOff')}
              onChange={(next) =>
                onToggleExtractor(CONTENT_FETCH_EXTRACTOR_GITHUB_REPO, next)
              }
              testId="content-fetch-extractor-github-repo"
            />
            <ExtractorToggle
              label={t('settings.contentFetchExtractorGenericReadable')}
              hint={t('settings.contentFetchExtractorGenericReadableHint')}
              enabled={extractorEnabled(
                settings,
                CONTENT_FETCH_EXTRACTOR_GENERIC_READABLE,
              )}
              disabled={!masterOn}
              onLabel={t('settings.contentFetchExtractorOn')}
              offLabel={t('settings.contentFetchExtractorOff')}
              onChange={(next) =>
                onToggleExtractor(
                  CONTENT_FETCH_EXTRACTOR_GENERIC_READABLE,
                  next,
                )
              }
              testId="content-fetch-extractor-generic-readable"
            />
          </div>
        </Field>

        <div className="my-4" data-testid="content-fetch-limited">
          <StatusCallout
            tone="info"
            title={t('settings.contentFetchLimitedTitle')}
            body={t('settings.contentFetchLimitedBody')}
          />
        </div>

        <Field
          label={t('settings.contentFetchDomainsLabel')}
          help={t('settings.contentFetchDomainsHelp')}
        >
          <textarea
            value={domainsDraft}
            onChange={(event) => setDomainsDraft(event.target.value)}
            onBlur={onCommitDomains}
            rows={3}
            placeholder={t('settings.contentFetchDomainsPlaceholder')}
            data-testid="content-fetch-domains-input"
            className={cn(
              'border-border-default rounded-paper bg-paper text-ink w-full resize-y border px-3 py-2 font-mono text-[11.5px]',
              'focus:border-accent focus:outline-none',
            )}
          />
          {!domainsChanged ? (
            <p className="text-ink-faint mt-2 font-mono text-[10.5px]">
              {t('settings.contentFetchDomainsEmpty')}
            </p>
          ) : null}
        </Field>

        <Field label={t('settings.contentFetchStatusLabel')}>
          <p
            className="text-ink-muted m-0 font-mono text-[11.5px]"
            data-testid="content-fetch-status"
          >
            {!masterOn
              ? t('settings.contentFetchStatusOff')
              : settings && settings.storedRecords > 0
                ? t('settings.contentFetchStatusSummary', {
                    stored: String(settings.storedRecords),
                    queued: String(settings.queuedJobs),
                    running: String(settings.runningJobs),
                    failed: String(settings.failedJobs),
                  })
                : t('settings.contentFetchStatusEmpty')}
          </p>
        </Field>

        <Field
          label={t('settings.contentFetchPrimeLabel')}
          help={t('settings.contentFetchPrimeHelp')}
        >
          <div className="flex flex-wrap items-center gap-2">
            <Button
              type="button"
              variant="accent"
              onClick={() => void onPrime()}
              disabled={!masterOn || priming}
              data-testid="content-fetch-prime"
            >
              {priming
                ? t('settings.contentFetchPriming')
                : t('settings.contentFetchPrimeAction')}
            </Button>
            {primeSummary ? (
              <span
                className="text-ink-faint font-mono text-[10.5px]"
                data-testid="content-fetch-prime-summary"
              >
                {primeSummary}
              </span>
            ) : null}
          </div>
        </Field>

        {saving ? (
          <p
            className="text-ink-faint mt-2 font-mono text-[10.5px]"
            data-testid="content-fetch-saving"
          >
            {t('settings.contentFetchSaving')}
          </p>
        ) : null}
        {saveError ? (
          <p
            role="alert"
            className="text-error mt-2 font-mono text-[10.5px]"
            data-testid="content-fetch-save-error"
          >
            {t('settings.contentFetchSaveError')}
          </p>
        ) : null}
      </PaperCardBody>
    </PaperCard>
  )
}

interface ExtractorToggleProps {
  label: string
  hint: string
  enabled: boolean
  disabled: boolean
  onLabel: string
  offLabel: string
  onChange: (next: boolean) => void
  testId: string
}

function ExtractorToggle({
  label,
  hint,
  enabled,
  disabled,
  onLabel,
  offLabel,
  onChange,
  testId,
}: ExtractorToggleProps) {
  return (
    <div
      className={cn(
        'flex items-start justify-between gap-3',
        disabled && 'opacity-60',
      )}
    >
      <div className="min-w-0">
        <div className="text-ink font-sans text-[12.5px] font-medium">
          {label}
        </div>
        <div className="text-ink-faint mt-0.5 font-mono text-[10px] leading-[1.4]">
          {hint}
        </div>
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={enabled}
        aria-label={label}
        disabled={disabled}
        onClick={() => onChange(!enabled)}
        data-testid={testId}
        className={cn(
          'border-border-default rounded-paper inline-flex shrink-0 items-center gap-2 border px-3 py-1.5 font-sans text-[12px] transition-colors',
          enabled
            ? 'border-accent bg-accent-soft text-accent-text'
            : 'text-ink-muted hover:border-ink-muted hover:bg-hover',
          disabled && 'cursor-not-allowed',
        )}
      >
        <span
          className={cn(
            'inline-block h-3 w-3 rounded-full',
            enabled ? 'bg-accent' : 'bg-ink-faint',
          )}
        />
        {enabled ? onLabel : offLabel}
      </button>
    </div>
  )
}
