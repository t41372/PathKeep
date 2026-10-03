/**
 * The collapsible "Thinking · searched the archive N times" part of an
 * assistant turn: the model's reasoning, then each tool call and what it
 * returned. Shown so the user can see how an answer was reached.
 */
import { ChevronDown, ChevronRight, LoaderCircle } from 'lucide-react'
import { memo, useState } from 'react'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import type { ChatTurn, ToolStep } from './chat-model'

function oneLine(text: string, max: number) {
  const flat = text.replace(/\s+/g, ' ').trim()
  return flat.length > max ? `${flat.slice(0, max)}…` : flat
}

function Step({ step }: { step: ToolStep }) {
  const { t } = useI18n()
  return (
    <div className="flex flex-col gap-1">
      <code className="font-mono text-xs break-all text-foreground/80">
        {step.name}({oneLine(step.arguments, 240)})
      </code>
      {step.pending ? (
        <span className="flex items-center gap-1.5 font-mono text-xs">
          <LoaderCircle className="size-3 animate-spin" />
          {t('ask.turn.toolPending')}
        </span>
      ) : (
        <code
          className={cn(
            'font-mono text-xs break-all',
            step.isError && 'text-destructive',
          )}
        >
          → {step.isError ? `${t('ask.turn.toolFailed')}: ` : ''}
          {oneLine(step.result ?? '', 280)}
        </code>
      )}
      {step.codeSource && (
        <details className="text-xs">
          <summary className="cursor-pointer select-none">
            {t('ask.turn.code')}
          </summary>
          <pre className="mt-1 max-h-48 overflow-auto rounded-md bg-background/60 p-2 font-mono">
            {step.codeSource}
          </pre>
        </details>
      )}
    </div>
  )
}

export const ThinkingBlock = memo(function ThinkingBlock({
  turn,
}: {
  turn: ChatTurn
}) {
  const { t } = useI18n()
  const [userOpen, setUserOpen] = useState<boolean | null>(null)
  const streaming = turn.status === 'streaming'
  const searches = turn.tools.length
  const hasReasoning = turn.reasoning.trim().length > 0

  if (!hasReasoning && searches === 0) return null

  const open = userOpen ?? (streaming && turn.content === '')
  const label =
    !streaming && searches === 0
      ? t('ask.turn.thinking')
      : searches > 0
        ? hasReasoning
          ? t('ask.turn.thinkingSearched', { count: searches })
          : t('ask.turn.searchedOnly', { count: searches })
        : t('ask.turn.thinking')
  const Chevron = open ? ChevronDown : ChevronRight

  return (
    <Collapsible
      open={open}
      onOpenChange={setUserOpen}
      className="flex flex-col gap-2"
    >
      <CollapsibleTrigger className="flex items-center gap-1.5 self-start text-[13px] text-muted-foreground hover:text-foreground">
        <Chevron className="size-3.5" />
        {label}
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div className="flex flex-col gap-2.5 rounded-[10px] bg-muted px-3.5 py-3 text-[13px] text-muted-foreground">
          {hasReasoning && (
            <p className="whitespace-pre-wrap">{turn.reasoning.trim()}</p>
          )}
          {turn.tools.map((step) => (
            <Step key={step.id} step={step} />
          ))}
        </div>
      </CollapsibleContent>
    </Collapsible>
  )
})
