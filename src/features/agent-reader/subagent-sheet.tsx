/**
 * A subagent opened from its card (`agent-history.md` §4.5): a side sheet
 * with its title, what it was (type, model, time, tokens), a line on how
 * PathKeep keeps it, and its own transcript drawn like the conversation's.
 * A subagent it sent is a card too; opening one replaces the sheet's
 * content, with a way back to the one before.
 */
import { ChevronLeft } from 'lucide-react'
import { useCallback, useMemo, useRef } from 'react'
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet'
import { Skeleton } from '@/components/ui/skeleton'
import type { ChatChildCard } from '@/lib/backend-client/agent-chats'
import { useI18n, type Formatters } from '@/lib/i18n'
import { queryClient, queryKeys } from '@/lib/query'
import { useChatChildren, useChatHeader } from '@/lib/queries/agent-chats'
import { durationText, tokensText } from './reader-words'
import { Transcript, type TranscriptControl } from './transcript'

function ChildTranscript({
  child,
  f,
  onOpenChild,
}: {
  child: ChatChildCard
  f: Formatters
  onOpenChild: (child: ChatChildCard) => void
}) {
  const header = useChatHeader(child.id)
  const children = useChatChildren(child.id, header.data?.publication)
  const cards = useMemo(
    () => children.data?.pages.flatMap((page) => page.children) ?? [],
    [children.data],
  )
  const control = useRef<TranscriptControl | null>(null)
  const refresh = useCallback(
    () =>
      void queryClient.invalidateQueries({
        queryKey: queryKeys.agentChat(child.id),
        exact: true,
      }),
    [child.id],
  )
  if (!header.data)
    return (
      <div className="flex flex-col gap-2 px-6 py-4">
        <Skeleton className="h-3.5 w-4/5" />
        <Skeleton className="h-3.5 w-3/5" />
      </div>
    )
  return (
    <Transcript
      id={child.id}
      publication={header.data.publication}
      leaf={null}
      knownTotal={header.data.visibleRecords}
      subagents={cards}
      f={f}
      control={control}
      switchedPrompt={null}
      onPickVersion={() => {}}
      onOpenChild={onOpenChild}
      onStale={refresh}
    />
  )
}

export function SubagentSheet({
  stack,
  f,
  onStack,
}: {
  /** The subagents opened, outermost first; empty when closed. */
  stack: ChatChildCard[]
  f: Formatters
  onStack: (stack: ChatChildCard[]) => void
}) {
  const { t } = useI18n()
  const child = stack.at(-1)
  const parent = stack.length > 1 ? stack.at(-2) : undefined
  const meta = child
    ? [
        child.agentType,
        child.model,
        child.agentMs === null ? null : durationText(t, child.agentMs),
        child.tokensTotal === null ? null : tokensText(t, f, child.tokensTotal),
      ]
        .filter(Boolean)
        .join(' · ')
    : ''
  return (
    <Sheet
      open={stack.length > 0}
      onOpenChange={(open) => !open && onStack([])}
    >
      <SheetContent className="w-[600px] gap-0 sm:max-w-[600px]">
        {child && (
          <>
            <SheetHeader className="gap-1.5 border-b pr-12">
              {parent && (
                <button
                  type="button"
                  onClick={() => onStack(stack.slice(0, -1))}
                  className="flex items-center gap-1 self-start text-[12.5px] text-muted-foreground hover:text-foreground"
                >
                  <ChevronLeft className="size-3.5" aria-hidden />
                  {t('agentReader.sheet.back', {
                    title: parent.title?.trim() || t('agentReader.untitled'),
                  })}
                </button>
              )}
              <SheetTitle className="leading-snug">
                {child.title?.trim() || t('agentReader.untitled')}
              </SheetTitle>
              <SheetDescription>
                {meta
                  ? t('agentReader.sheet.meta', { parts: meta })
                  : t('agentReader.rows.subagent')}
              </SheetDescription>
              <p className="text-[12px] text-muted-foreground">
                {t('agentReader.sheet.explain')}
              </p>
            </SheetHeader>
            <div className="flex min-h-0 flex-1 flex-col">
              <ChildTranscript
                key={child.id}
                child={child}
                f={f}
                onOpenChild={(next) => onStack([...stack, next])}
              />
            </div>
          </>
        )}
      </SheetContent>
    </Sheet>
  )
}
