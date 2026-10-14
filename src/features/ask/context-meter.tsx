/**
 * How much of the model's context the next request takes, beside the send
 * button, and a plain line when it no longer fits.
 *
 * The numbers are the backend's own estimate (`context-usage.ts`), so the
 * meter turns red exactly when the backend would refuse to send. Not
 * responsible for asking the service for its context (`ask-page.tsx`).
 */
import { contextSize } from '@/features/ai-setup/providers'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { contextLevel } from './context-usage'

/** Token counts the way the model list shows sizes: 2.8K, 32K. */
function tokens(count: number) {
  if (count < 1024) return String(count)
  if (count < 10 * 1024) return `${(count / 1024).toFixed(1)}K`
  return contextSize(count)
}

export function ContextMeter({
  used,
  contextLength,
  service,
}: {
  used: number
  contextLength: number | null
  service: string
}) {
  const { t } = useI18n()
  const level = contextLevel(used, contextLength)
  const label = contextLength
    ? t('ask.composer.context', {
        used: tokens(used),
        limit: contextSize(contextLength),
      })
    : t('ask.composer.contextUnknown', { used: tokens(used) })
  const title = contextLength
    ? t('ask.composer.contextTitle', {
        used: used.toLocaleString(),
        limit: contextLength.toLocaleString(),
      })
    : t('ask.composer.contextUnknownTitle', {
        used: used.toLocaleString(),
        service,
      })
  return (
    <span
      title={title}
      aria-label={title}
      className={cn(
        'shrink-0 font-mono text-[11px] tabular-nums',
        level === 'over'
          ? 'text-destructive'
          : level === 'near'
            ? 'text-brand'
            : 'text-muted-foreground',
      )}
    >
      {label}
    </span>
  )
}

/** Said above the composer when the conversation is close to, or past, the model's context. */
export function ContextWarning({
  used,
  contextLength,
}: {
  used: number
  contextLength: number | null
}) {
  const { t } = useI18n()
  const level = contextLevel(used, contextLength)
  if (!contextLength || (level !== 'near' && level !== 'over')) return null
  return (
    <p
      role={level === 'over' ? 'alert' : 'status'}
      className={cn(
        'text-xs leading-5',
        level === 'over' ? 'text-destructive' : 'text-brand',
      )}
    >
      {t(
        level === 'over'
          ? 'ask.composer.contextOver'
          : 'ask.composer.contextNear',
        { limit: contextSize(contextLength) },
      )}
    </p>
  )
}
