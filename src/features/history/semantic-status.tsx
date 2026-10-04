/**
 * The line under History's results that says what semantic results come
 * from, or, when semantic search was asked for but cannot run, why these are
 * full-text results instead. The link goes to Settings → AI; nothing here
 * turns anything on.
 */
import { Info, Sparkles } from 'lucide-react'
import { Link } from 'react-router-dom'
import { useFormat, useI18n } from '@/lib/i18n'
import type { SemanticInfo } from './semantic-info'

const settingsLink =
  'shrink-0 rounded text-foreground underline-offset-2 outline-none hover:underline focus-visible:ring-2 focus-visible:ring-ring/50'

export function SemanticNote({
  info,
  active,
}: {
  info: SemanticInfo
  /** Semantic results are on screen (otherwise: asked for, but fell back). */
  active: boolean
}) {
  const { t } = useI18n()
  const format = useFormat()

  if (!active) {
    return (
      <p
        role="status"
        className="flex flex-wrap items-center gap-x-1.5 px-7 pb-1.5 text-xs text-muted-foreground"
      >
        <Info className="size-3.5 shrink-0" aria-hidden />
        <span>
          {t(`historySearch.semantic.reason.${info.reason ?? 'off'}`)}{' '}
          {t('historySearch.semantic.fellBack')}
        </span>
        <Link to="/settings/ai" className={settingsLink}>
          {t('historySearch.semantic.openSettings')}
        </Link>
      </p>
    )
  }

  const parts = [
    info.provider &&
      t('historySearch.semantic.using', { provider: info.provider }),
    t('historySearch.semantic.indexed', { count: info.indexed }),
    info.lastIndexedAt &&
      t('historySearch.semantic.updated', {
        when: format.relative(info.lastIndexedAt),
      }),
  ].filter(Boolean)

  return (
    <p className="flex flex-wrap items-center gap-x-1.5 px-7 pb-1.5 text-xs text-muted-foreground">
      <Sparkles className="size-3.5 shrink-0 text-brand" aria-hidden />
      <span>{parts.join(' · ')}</span>
      {info.stale && (
        <>
          <span>· {t('historySearch.semantic.stale')}</span>
          <Link to="/settings/ai" className={settingsLink}>
            {t('historySearch.semantic.rebuild')}
          </Link>
        </>
      )}
    </p>
  )
}
