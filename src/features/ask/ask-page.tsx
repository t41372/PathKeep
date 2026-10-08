/**
 * Ask: chat with an assistant that searches the local archive.
 *
 * Responsibilities: opening, starting and deleting conversations, wiring the
 * list, transcript and composer together, and the setup prompt when no AI
 * service is set up (saved conversations stay readable then). Streaming lives in `use-chat.ts`; saved conversations in
 * `conversations.ts`. Not responsible for configuring providers (Settings → AI).
 */
import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef, useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { toast } from 'sonner'
import { Skeleton } from '@/components/ui/skeleton'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import {
  isLocalProvider,
  providerLabel,
  usableAssistantProvider,
} from '../ai-setup/providers'
import { AssistantTurn } from './assistant-turn'
import { fromAgentMessage } from './chat-model'
import { Composer } from './composer'
import { ConversationList } from './conversation-list'
import { conversationsKey, useConversations } from './conversations'
import { AssistantSetup, StarterEmpty } from './empty-states'
import { useChat } from './use-chat'
import { useStickToBottom } from './use-stick-to-bottom'

export default function AskPage() {
  const snapshot = useSnapshot()
  const provider = usableAssistantProvider(snapshot.config.ai)
  // Without a service there is no one to ask, but saved conversations are the user's and stay
  // readable: after an upgrade, or with AI turned off, they are still here.
  return (
    <AskChat
      providerId={provider?.id ?? null}
      label={provider ? providerLabel(provider) : ''}
      remoteName={provider && !isLocalProvider(provider) ? provider.name : null}
      systemPrompt={snapshot.config.ai.assistantSystemPrompt}
    />
  )
}

function AskChat({
  providerId,
  label,
  remoteName,
  systemPrompt,
}: {
  /** `null` when no AI service is set up: conversations can be read, not continued. */
  providerId: string | null
  label: string
  remoteName: string | null
  systemPrompt: string
}) {
  const { t } = useI18n()
  const client = useQueryClient()
  const conversations = useConversations()
  const [params, setParams] = useSearchParams()
  const [opening, setOpening] = useState(false)
  const openToken = useRef(0)
  const { scrollRef, contentRef, onScroll, pin } = useStickToBottom()

  const chat = useChat({
    providerId: providerId ?? '',
    systemPrompt,
    titleFor: (id) => conversations.data?.find((item) => item.id === id)?.title,
    onSaved: () =>
      void client.invalidateQueries({ queryKey: conversationsKey }),
  })
  const { open, startNew, send } = chat

  const startFresh = useCallback(() => {
    openToken.current += 1
    setOpening(false)
    startNew()
  }, [startNew])

  const openConversation = useCallback(
    async (id: string) => {
      const token = ++openToken.current
      setOpening(true)
      try {
        const detail = await intelligenceClient.loadConversation(id)
        if (token !== openToken.current) return
        if (!detail) throw new Error('missing')
        open(id, detail.messages.map(fromAgentMessage))
        pin()
      } catch {
        if (token === openToken.current) toast.error(t('ask.loadFailed'))
      } finally {
        if (token === openToken.current) setOpening(false)
      }
    },
    [open, pin, t],
  )

  // The command palette links to /ask?new=1.
  useEffect(() => {
    if (params.get('new') !== '1') return
    startFresh()
    setParams({}, { replace: true })
  }, [params, setParams, startFresh])

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'n') {
        event.preventDefault()
        startFresh()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [startFresh])

  const submit = (text: string) => {
    pin()
    send(text)
  }

  const firstQuestion =
    chat.turns.find((turn) => turn.role === 'user')?.content ?? ''
  const lastIndex = chat.turns.length - 1

  return (
    <div className="flex min-w-0 flex-1">
      <ConversationList
        conversations={conversations}
        activeId={chat.conversationId}
        draftTitle={firstQuestion.slice(0, 60)}
        onSelect={(id) => {
          if (id !== chat.conversationId) void openConversation(id)
        }}
        onNew={startFresh}
        onDeleted={(id) => {
          if (id === chat.conversationId) startFresh()
        }}
      />
      <div className="flex min-w-0 flex-1 flex-col">
        <div
          ref={scrollRef}
          onScroll={onScroll}
          className="flex-1 overflow-y-auto"
        >
          <div ref={contentRef}>
            {opening ? (
              <OpeningSkeleton label={t('ask.loadingConversation')} />
            ) : chat.turns.length === 0 ? (
              providerId ? (
                <StarterEmpty remoteProvider={remoteName} onPick={submit} />
              ) : (
                <AssistantSetup />
              )
            ) : (
              <div className="mx-auto flex max-w-[760px] flex-col gap-5 px-8 py-7">
                {chat.turns.map((turn, index) =>
                  turn.role === 'user' ? (
                    <div
                      key={turn.id}
                      className="max-w-[80%] self-end rounded-[18px] bg-user-bubble px-4 py-2.5 leading-normal whitespace-pre-wrap text-user-bubble-foreground animate-rise"
                    >
                      {turn.content}
                    </div>
                  ) : (
                    <AssistantTurn
                      key={turn.id}
                      turn={turn}
                      canRetry={
                        providerId !== null &&
                        index === lastIndex &&
                        !chat.streaming
                      }
                      onRetry={chat.retry}
                    />
                  ),
                )}
              </div>
            )}
          </div>
        </div>
        {providerId ? (
          <Composer
            streaming={chat.streaming}
            providerLabel={label}
            onSend={submit}
            onStop={chat.stop}
            focusKey={chat.conversationId}
          />
        ) : (
          chat.turns.length > 0 && (
            <p className="border-t px-8 py-4 text-center text-[13px] text-muted-foreground">
              {t('ask.setup.toContinue')}{' '}
              <Link to="/settings/ai" className="text-foreground underline">
                {t('ask.setup.action')}
              </Link>
            </p>
          )
        )}
      </div>
    </div>
  )
}

function OpeningSkeleton({ label }: { label: string }) {
  return (
    <div
      role="status"
      aria-label={label}
      className="mx-auto flex max-w-[760px] flex-col gap-4 px-8 py-7"
    >
      <Skeleton className="h-10 w-1/2 self-end rounded-[18px]" />
      <Skeleton className="h-5 w-40" />
      <Skeleton className="h-24 rounded-xl" />
    </div>
  )
}
