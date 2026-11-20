/**
 * The conversation reader (`agent-history.md` §4), beside the list and as
 * a full page: the header, find in this conversation, the conversation
 * (or its raw records), the outline of what you said, the info panel,
 * subagents in a side sheet and Rename. Opens at the start, or from a
 * search at the occurrence it was opened at; "Jump to the end" goes to the
 * last row.
 *
 * Responsible for: the reader's own state (raw or not, info, outline, the
 * version of a prompt picked, open subagents), its states (loading, not
 * found, failed), ⌘F and Esc (find first, then the reader). Not
 * responsible for: what a row looks like, reading windows or finding (the
 * sibling files).
 */
import { MessagesSquare, TriangleAlert } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty'
import { Skeleton } from '@/components/ui/skeleton'
import { useChatMarks } from '@/features/agent-chats/chat-marks'
import type { RecordAt } from '@/features/agent-chats/chats-params'
import { useInsightsFormat } from '@/features/insights/zone'
import type { ChatChildCard } from '@/lib/backend-client/agent-chats'
import { useI18n } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { queryClient, queryKeys } from '@/lib/query'
import {
  useChatChildren,
  useChatHeader,
  useChatInfo,
} from '@/lib/queries/agent-chats'
import { spacePath } from '@/lib/spaces'
import { FindBar } from './find-bar'
import { FindTargetContext } from './find-target'
import { InfoPanel } from './info-panel'
import { MoreMenu } from './more-menu'
import { Outline } from './outline'
import { RawView } from './raw-view'
import { ReaderPrefsContext, useLoadedReaderPrefs } from './reader-prefs'
import { ReaderHeader } from './reader-header'
import { RenameDialog } from './rename-dialog'
import { ResumeMenu } from './resume-menu'
import { SubagentSheet } from './subagent-sheet'
import { Transcript, type TranscriptControl } from './transcript'
import { useReaderFind } from './use-reader-find'

function ReaderSkeleton() {
  const { t } = useI18n()
  return (
    <div
      role="status"
      aria-label={t('agentReader.loading')}
      className="flex flex-1 flex-col"
    >
      <div className="flex flex-col gap-2 border-b px-5 pt-5 pb-3">
        <Skeleton className="h-5 w-2/3" />
        <Skeleton className="h-3.5 w-1/2" />
      </div>
      <div className="mx-auto flex w-full max-w-[760px] flex-col gap-4 px-6 pt-6">
        <Skeleton className="h-10 w-2/5 self-end rounded-[18px]" />
        <Skeleton className="h-3.5 w-full" />
        <Skeleton className="h-3.5 w-4/5" />
        <Skeleton className="h-3.5 w-3/5" />
      </div>
    </div>
  )
}

function ReaderMessage({
  title,
  body,
  failed,
  onRetry,
}: {
  title: string
  body: string
  failed?: boolean
  onRetry?: () => void
}) {
  const { t } = useI18n()
  return (
    <Empty className="animate-rise flex-1">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          {failed ? <TriangleAlert /> : <MessagesSquare />}
        </EmptyMedia>
        <EmptyTitle className="text-base">{title}</EmptyTitle>
        <EmptyDescription className="break-words">{body}</EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        {onRetry ? (
          <Button variant="outline" size="sm" onClick={onRetry}>
            {t('common.retry')}
          </Button>
        ) : (
          <Button asChild size="sm" variant="outline">
            <Link to={spacePath('agent', 'chats')}>
              {t('agentReader.backToList')}
            </Link>
          </Button>
        )}
      </EmptyContent>
    </Empty>
  )
}

function isTyping(target: EventTarget | null) {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable ||
      ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName))
  )
}

export function Reader({
  id,
  beside,
  find = null,
  at = null,
  onFind,
  onClose,
  onTrashed,
  onOpenProject,
}: {
  id: string
  /** Beside the list (outline off, a close button) rather than a full page. */
  beside: boolean
  /** Opened from a search: find these words, at this record. */
  find?: string | null
  at?: RecordAt | null
  /** The words find looks for changed (`null`: find closed). */
  onFind?: (find: string | null) => void
  onClose: () => void
  /** It went to the trash from here. */
  onTrashed: () => void
  /** The project in the header or the info panel was clicked. */
  onOpenProject: (project: string) => void
}) {
  const { t } = useI18n()
  const f = useInsightsFormat()
  const marks = useChatMarks()
  const header = useChatHeader(id)
  // Settings › Agent conversations: folded tool calls, thinking shown.
  const loadedPrefs = useLoadedReaderPrefs()
  const [raw, setRaw] = useState(false)
  const [info, setInfo] = useState(false)
  const [outline, setOutline] = useState(!beside)
  const [menuOpen, setMenuOpen] = useState(false)
  const [renaming, setRenaming] = useState(false)
  const [line, setLine] = useState<{ leaf: number; prompt: string } | null>(
    null,
  )
  const [sheet, setSheet] = useState<ChatChildCard[]>([])
  const [topTurn, setTopTurn] = useState<number | null>(null)
  const control = useRef<TranscriptControl | null>(null)
  const publication = header.data?.publication
  const children = useChatChildren(id, publication)
  const cards = useMemo(
    () => children.data?.pages.flatMap((page) => page.children) ?? [],
    [children.data],
  )
  const latestLeaf = children.data?.pages[0]?.leaves.latestRecorded ?? null
  // The tool's file, for "Show in Finder": read when the menu opens.
  const details = useChatInfo(id, menuOpen || info)

  const refreshHeader = useCallback(
    () =>
      // The header alone: its new publication brings new windows.
      void queryClient.invalidateQueries({
        queryKey: queryKeys.agentChat(id),
        exact: true,
      }),
    [id],
  )

  const pickLine = useCallback(
    (next: number) => setLine({ leaf: next, prompt: '' }),
    [],
  )
  const finder = useReaderFind({
    header: header.data,
    leaf: line?.leaf ?? null,
    rootCards: cards,
    find,
    at,
    onStale: refreshHeader,
    onFind,
    onSheet: setSheet,
    onLine: pickLine,
    onRaw: setRaw,
  })
  const { show: showFind, close: closeFind, open: findOpen } = finder

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return
      if (
        (event.metaKey || event.ctrlKey) &&
        !event.altKey &&
        event.key.toLowerCase() === 'f'
      ) {
        event.preventDefault()
        showFind()
        return
      }
      if (event.key !== 'Escape' || isTyping(event.target)) return
      if (document.querySelector('[role="dialog"], [role="menu"]')) return
      if (findOpen) closeFind()
      else onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose, showFind, closeFind, findOpen])

  if (!header.data || !loadedPrefs.ready) {
    if (header.isError) {
      return commandErrorCode(header.error) === 'conversation_not_found' ? (
        <ReaderMessage
          title={t('agentReader.notFound')}
          body={t('agentReader.notFoundBody')}
        />
      ) : (
        <ReaderMessage
          failed
          title={t('agentReader.failed')}
          body={header.error.message}
          onRetry={() => void header.refetch()}
        />
      )
    }
    return <ReaderSkeleton />
  }

  const data = header.data
  const leaf = line?.leaf ?? null
  const scrollTo = (rank: number, align: 'start' | 'center' | 'end') =>
    requestAnimationFrame(() => control.current?.scrollToRank(rank, align))

  return (
    <section
      aria-label={t('agentReader.region')}
      data-reader=""
      className="flex min-h-0 min-w-0 flex-1 flex-col"
    >
      <ReaderHeader
        header={data}
        f={f}
        beside={beside}
        raw={raw}
        info={info}
        marks={marks}
        resume={<ResumeMenu header={data} />}
        more={
          <MoreMenu
            header={data}
            beside={beside}
            outline={outline}
            raw={raw}
            leaf={leaf}
            sourcePath={details.data?.sourcePath}
            marks={marks}
            onOpenChange={setMenuOpen}
            onOutline={setOutline}
            onRaw={setRaw}
            onEnd={() => {
              setRaw(false)
              requestAnimationFrame(() => control.current?.scrollToEnd())
            }}
            onRename={() => setRenaming(true)}
            onTrashed={onTrashed}
            onFind={showFind}
            onStale={refreshHeader}
          />
        }
        onInfo={() => setInfo((open) => !open)}
        onClose={onClose}
        onOpenProject={onOpenProject}
        onRaw={setRaw}
      />
      {finder.open && sheet.length === 0 && <FindBar find={finder} />}
      <ReaderPrefsContext.Provider value={loadedPrefs.prefs}>
        <FindTargetContext.Provider value={finder.target}>
          <div className="flex min-h-0 flex-1">
            {raw ? (
              <RawView
                header={data}
                f={f}
                startRank={finder.rawRank}
                onStale={refreshHeader}
                onSee={(rank) => {
                  setLine(null)
                  setRaw(false)
                  scrollTo(rank, 'center')
                }}
              />
            ) : (
              <>
                <Transcript
                  key={data.publication}
                  id={id}
                  publication={data.publication}
                  leaf={leaf}
                  knownTotal={data.visibleRecords}
                  subagents={cards}
                  f={f}
                  control={control}
                  switchedPrompt={line?.prompt ?? null}
                  onPickVersion={(next, prompt) => {
                    setLine(next === latestLeaf ? null : { leaf: next, prompt })
                  }}
                  onOpenChild={(child) => setSheet([child])}
                  onTopTurn={setTopTurn}
                  onStale={refreshHeader}
                  scrollRequest={finder.rootScroll}
                />
                {outline && (
                  <Outline
                    id={id}
                    publication={data.publication}
                    leaf={leaf}
                    promptCount={data.promptCount}
                    topTurn={topTurn}
                    onPick={(rank) => scrollTo(rank, 'start')}
                    onStale={refreshHeader}
                  />
                )}
              </>
            )}
            {info && (
              <InfoPanel header={data} f={f} onOpenProject={onOpenProject} />
            )}
          </div>
          <SubagentSheet
            stack={sheet}
            f={f}
            onStack={setSheet}
            findBar={
              finder.open ? <FindBar find={finder} className="pr-12" /> : null
            }
            onEscape={(event) => {
              if (!finder.open) return
              event.preventDefault()
              closeFind()
            }}
            scrollRequest={finder.sheetScroll}
          />
        </FindTargetContext.Provider>
      </ReaderPrefsContext.Provider>
      <RenameDialog
        summary={data.summary}
        marks={marks}
        open={renaming}
        onOpenChange={setRenaming}
      />
    </section>
  )
}
