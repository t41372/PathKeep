/**
 * The card on browsing's Home that tells someone already using PathKeep it
 * can keep their Agent conversations too (`data-spaces.md` §3.1).
 *
 * Responsibilities: showing it only while Agent conversations are off, at
 * least one tool's folder is on this computer and the card was not put
 * away on this device; naming exactly the tools found; opening the preview
 * or putting the card away for good. Finding the tools only checks that
 * their folders exist: nothing in them is opened or listed.
 * Not responsible for: the preview itself (`features/enable-agent/`).
 */
import { Bot, X } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { useI18n } from '@/lib/i18n'
import { useDataKinds, useDetectedSources } from '@/lib/queries/data-kinds'
import { kindStateOf } from '@/lib/spaces'
import { useOpenEnableAgent } from '@/features/enable-agent/links'
import { foundAgentTools } from './presence'
import { ToolBadge } from './tool-badge'

/** Put away on this device only, like the theme: another computer still shows it. */
const DISMISSED_KEY = 'pathkeep-agent-intro-dismissed'

function readDismissed() {
  try {
    return localStorage.getItem(DISMISSED_KEY) === '1'
  } catch {
    return false
  }
}

function rememberDismissed() {
  try {
    localStorage.setItem(DISMISSED_KEY, '1')
  } catch {
    // Put away for this window at least.
  }
}

export function AgentIntroCard() {
  const { t } = useI18n()
  const openEnable = useOpenEnableAgent()
  const [dismissed, setDismissed] = useState(readDismissed)
  const kinds = useDataKinds()
  const off = kindStateOf(kinds.data, 'agent-history') === 'off'
  const detected = useDetectedSources(['agent-history'], off && !dismissed)
  const found = detected.data ? foundAgentTools(detected.data) : []

  if (dismissed || !off || found.length === 0) return null

  const dismiss = () => {
    rememberDismissed()
    setDismissed(true)
  }

  return (
    <section
      aria-labelledby="agent-intro-title"
      className="relative flex animate-rise gap-4 rounded-xl border border-brand/25 bg-brand-soft p-5"
    >
      <span className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-card shadow-card">
        <Bot aria-hidden className="size-5" strokeWidth={1.75} />
      </span>
      <div className="flex min-w-0 flex-col gap-2 pr-6">
        <h2 id="agent-intro-title" className="text-[15px] font-semibold">
          {t('dataKinds.intro.title')}
        </h2>
        <p className="max-w-[640px] text-[13px] leading-relaxed text-muted-foreground">
          {t('dataKinds.intro.body')}
        </p>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[13px]">
          <span>{t('dataKinds.intro.found')}</span>
          <ul
            aria-label={t('dataKinds.intro.found')}
            className="flex flex-wrap items-center gap-x-3 gap-y-1"
          >
            {found.map((tool) => (
              <li key={tool}>
                <ToolBadge id={tool} />
              </li>
            ))}
          </ul>
        </div>
        <p className="text-xs text-muted-foreground">
          {t('dataKinds.intro.onlyChecked')}
        </p>
        <div className="mt-1 flex flex-wrap gap-2">
          <Button onClick={openEnable}>{t('dataKinds.intro.preview')}</Button>
          <Button variant="ghost" onClick={dismiss}>
            {t('dataKinds.intro.later')}
          </Button>
        </div>
      </div>
      <button
        type="button"
        aria-label={t('dataKinds.intro.dismiss')}
        onClick={dismiss}
        className="absolute top-3 right-3 rounded-md p-1 text-muted-foreground transition-colors hover:bg-card hover:text-foreground"
      >
        <X aria-hidden className="size-4" />
      </button>
    </section>
  )
}
