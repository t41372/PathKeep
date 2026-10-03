/** Ask screens that have no conversation yet: the starter and the setup prompt. */
import { CornerDownRight } from 'lucide-react'
import { Link } from 'react-router-dom'
import { BrandMark } from '@/components/app/brand-mark'
import { Button } from '@/components/ui/button'
import { useI18n } from '@/lib/i18n'

export function StarterEmpty({
  remoteProvider,
  onPick,
}: {
  /** Name of the online service in use, or null when the model is local. */
  remoteProvider: string | null
  onPick: (question: string) => void
}) {
  const { t } = useI18n()
  const examples = [
    t('ask.empty.example1'),
    t('ask.empty.example2'),
    t('ask.empty.example3'),
    t('ask.empty.example4'),
  ]
  return (
    <div className="mx-auto flex max-w-[600px] flex-col items-center gap-[18px] px-8 pt-[16vh] pb-8 text-center animate-rise">
      <BrandMark className="size-10" />
      <h1 className="text-[22px] font-semibold tracking-[-0.01em]">
        {t('ask.empty.title')}
      </h1>
      <p className="max-w-[440px] text-[13px] leading-[1.55] text-muted-foreground">
        {remoteProvider
          ? t('ask.empty.bodyRemote', { provider: remoteProvider })
          : t('ask.empty.bodyLocal')}
      </p>
      <div
        className="flex w-full flex-col gap-1.5"
        role="group"
        aria-label={t('ask.empty.examplesLabel')}
      >
        {examples.map((question) => (
          <button
            key={question}
            type="button"
            onClick={() => onPick(question)}
            className="flex items-center gap-2.5 rounded-[10px] border bg-card px-3.5 py-3 text-left transition-colors hover:bg-popover"
          >
            <CornerDownRight className="size-3.5 shrink-0 text-muted-foreground" />
            {question}
          </button>
        ))}
      </div>
    </div>
  )
}

/** Shown instead of the chat when no usable assistant provider is set up. */
export function AssistantSetup() {
  const { t } = useI18n()
  return (
    <div className="flex flex-1 items-center justify-center p-8">
      <div className="flex max-w-[440px] flex-col items-center gap-4 text-center animate-rise">
        <BrandMark className="size-10" />
        <h1 className="text-[22px] font-semibold tracking-[-0.01em]">
          {t('ask.setup.title')}
        </h1>
        <p className="text-[13px] leading-[1.6] text-muted-foreground">
          {t('ask.setup.body')}
        </p>
        <Button asChild>
          <Link to="/settings/ai">{t('ask.setup.action')}</Link>
        </Button>
        <p className="text-xs text-muted-foreground">
          {t('ask.setup.optional')}
        </p>
      </div>
    </div>
  )
}
