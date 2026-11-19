/**
 * The enable dialog's three steps, `See what it reads` → `Choose` → `Save`,
 * with the current one marked for sight and for screen readers.
 */
import { Check } from 'lucide-react'
import { cn } from '@/lib/cn'
import { useI18n, type MessageKey } from '@/lib/i18n'

const STEPS: MessageKey[] = [
  'enableAgent.steps.preview',
  'enableAgent.steps.choose',
  'enableAgent.steps.save',
]

export function Steps({ at }: { at: number }) {
  const { t } = useI18n()
  return (
    <ol
      aria-label={t('enableAgent.steps.label')}
      className="flex flex-wrap items-center gap-2 text-xs"
    >
      {STEPS.map((key, index) => {
        const label = t(key)
        const done = index < at
        return (
          <li
            key={key}
            aria-current={index === at ? 'step' : undefined}
            className="flex items-center gap-2"
          >
            <span
              aria-hidden
              className={cn(
                'flex size-5 items-center justify-center rounded-full border text-[11px] font-medium tabular transition-colors duration-200',
                done && 'border-transparent bg-foreground text-background',
                index === at && 'border-foreground text-foreground',
                index > at && 'text-muted-foreground',
              )}
            >
              {done ? <Check className="size-3" /> : index + 1}
            </span>
            <span
              className={cn(
                'transition-colors duration-200',
                index === at
                  ? 'font-medium text-foreground'
                  : 'text-muted-foreground',
              )}
            >
              {done ? (
                <>
                  <span className="sr-only">
                    {t('enableAgent.steps.done', { step: label })}
                  </span>
                  <span aria-hidden>{label}</span>
                </>
              ) : (
                label
              )}
            </span>
            {index < STEPS.length - 1 && (
              <span aria-hidden className="h-px w-6 bg-border" />
            )}
          </li>
        )
      })}
    </ol>
  )
}
