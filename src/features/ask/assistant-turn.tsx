/** One assistant answer: reasoning and tool trace, text, sources and actions. */
import { Check, Copy, History, RotateCcw, TriangleAlert } from 'lucide-react'
import { memo, useState } from 'react'
import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import type { ChatTurn } from './chat-model'
import { Markdown } from './markdown'
import { historyLink, uniqueCitations } from './citations'
import { Sources } from './sources'
import { ThinkingBlock } from './thinking-block'

export const AssistantTurn = memo(function AssistantTurn({
  turn,
  canRetry,
  onRetry,
}: {
  turn: ChatTurn
  canRetry: boolean
  onRetry: () => void
}) {
  const { t } = useI18n()
  const [copied, setCopied] = useState(false)
  const streaming = turn.status === 'streaming'
  const citations = uniqueCitations(turn.citations)
  const waiting =
    streaming &&
    turn.content === '' &&
    turn.tools.length === 0 &&
    !turn.reasoning
  const failed = turn.status === 'error'
  const incomplete = turn.status === 'cancelled'

  const copy = async () => {
    await navigator.clipboard.writeText(turn.content)
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1500)
  }

  return (
    <div className="flex animate-rise flex-col gap-2.5">
      <ThinkingBlock turn={turn} />

      {waiting && (
        <div className="flex items-center gap-2 text-[13px] text-muted-foreground">
          <span className="size-1.5 animate-pulse rounded-full bg-brand" />
          {t('ask.turn.working')}
        </div>
      )}

      {turn.content !== '' && (
        <div className="rounded-xl bg-popover px-[18px] py-3.5 shadow-card">
          <Markdown content={turn.content} streaming={streaming} />
        </div>
      )}

      {turn.notes.map((note) => (
        <p key={note.code} className="text-[13px] text-muted-foreground">
          {t(`ask.turn.notes.${note.code}`)}
        </p>
      ))}

      {failed && (
        <div
          role="alert"
          className="flex items-start gap-2.5 rounded-xl border border-destructive/30 bg-destructive/5 px-4 py-3 text-[13px]"
        >
          <TriangleAlert className="mt-0.5 size-4 shrink-0 text-destructive" />
          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <span className="font-medium">{t('ask.turn.failedTitle')}</span>
            <span className="break-words text-muted-foreground">
              {turn.error || t('ask.turn.interrupted')}
            </span>
          </div>
        </div>
      )}

      {incomplete && (
        <p className="text-[13px] text-muted-foreground">
          {t('ask.turn.cancelled')}
        </p>
      )}

      {citations.length > 0 && !streaming && <Sources citations={citations} />}

      {!streaming && (
        <div
          className={cn(
            'flex flex-wrap items-center gap-1.5',
            failed && 'order-last',
          )}
        >
          {citations.length > 0 && (
            <Button variant="outline" size="sm" asChild>
              <Link to={historyLink(citations[0])}>
                <History />
                {t('ask.turn.openInHistory')}
              </Link>
            </Button>
          )}
          {turn.content !== '' && (
            <Button variant="ghost" size="sm" onClick={() => void copy()}>
              {copied ? <Check /> : <Copy />}
              {copied ? t('ask.turn.copied') : t('ask.turn.copy')}
            </Button>
          )}
          {canRetry && turn.status !== 'done' && (
            <Button variant="ghost" size="sm" onClick={onRetry}>
              <RotateCcw />
              {t('ask.turn.retry')}
            </Button>
          )}
          {turn.usage && (
            <span className="ml-auto font-mono text-[11px] text-muted-foreground">
              {t('ask.turn.tokens', {
                prompt: turn.usage.promptTokens,
                completion: turn.usage.completionTokens,
              })}
            </span>
          )}
        </div>
      )}
    </div>
  )
})
