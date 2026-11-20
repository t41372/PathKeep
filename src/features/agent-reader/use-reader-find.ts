/**
 * The reader's find (`agent-history.md` §4.4, §4.6): the bar's text, the
 * query it finds, which occurrence is current, and going there: into the
 * subagents on its path (the side sheet), to its record on the line shown
 * (a Pi branch through it when it is on another branch; the raw records
 * when it is on no line), with its fold opened and its words lit.
 *
 * Opened from a search (`find`, `at`), it finds the search's words and
 * goes to the occurrence at that record as soon as it arrives.
 *
 * Responsible for: that state and those moves. Not responsible for: the
 * stream (`use-find.ts`), painting (`find-paint.ts`) or drawing the bar.
 */
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type MutableRefObject,
} from 'react'
import {
  agentChatsClient,
  type ChatChildCard,
  type ChatHeader,
} from '@/lib/backend-client/agent-chats'
import type { MatchAddress } from '@/lib/backend-client/agent-search'
import { toast } from 'sonner'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { queryClient, queryKeys } from '@/lib/query'
import { CHILDREN_PAGE } from '@/lib/queries/agent-chats'
import { writeAt, type RecordAt } from '@/features/agent-chats/chats-params'
import { clearFind, paintFind } from './find-paint'
import type { FindTarget } from './find-target'
import type { ScrollRequest } from './transcript'
import { partToken, useFind, type FindState } from './use-find'

const DEBOUNCE_MS = 280

/** Where a record is on a line: its rank, or `null` when it is not on it. */
async function rankOf(
  address: MatchAddress,
  view: 'visible' | 'raw',
  leaf: number | null,
  onStale: () => void,
) {
  const line = leaf === null ? 'current' : `leaf:${leaf}`
  try {
    return await queryClient.fetchQuery({
      queryKey: [
        ...queryKeys.agentChatRank(
          address.conversationId,
          address.publication,
          `${view}:${line}`,
          address.sourceId,
          address.ordinal,
        ),
      ],
      queryFn: async () =>
        (
          await agentChatsClient.window({
            id: address.conversationId,
            publication: address.publication,
            view,
            ...(leaf === null ? {} : { leaf }),
            anchor: {
              kind: 'record',
              sourceId: address.sourceId,
              ordinal: address.ordinal,
            },
            before: 0,
            limit: 1,
          })
        ).first,
      staleTime: Infinity,
      retry: false,
    })
  } catch (error) {
    if (commandErrorCode(error) === 'stale_cursor') onStale()
    return null
  }
}

/** The cards of the subagents on `path`, outermost first. */
async function stackOf(path: string[], rootCards: ChatChildCard[]) {
  const stack: ChatChildCard[] = []
  let cards = rootCards
  for (const [depth, id] of path.entries()) {
    let card = cards.find((item) => item.id === id)
    if (!card) {
      // A deeper level: read its parent's subagents until it turns up.
      const parent = depth === 0 ? null : path[depth - 1]
      if (parent) {
        const header = await agentChatsClient.header(parent)
        let after: string | undefined
        do {
          const page = await agentChatsClient.branches({
            id: parent,
            publication: header.publication,
            limit: CHILDREN_PAGE,
            ...(after ? { childrenAfter: after } : {}),
          })
          card = page.children.find((item) => item.id === id)
          after = page.childrenAfter ?? undefined
        } while (!card && after)
      }
    }
    if (!card) {
      // Not listed under its parent: show it from its own header.
      const header = await agentChatsClient.header(id)
      card = { ...header.summary, launch: null, agentType: null, model: null }
    }
    stack.push(card)
    cards = []
  }
  return stack
}

export interface ReaderFind {
  open: boolean
  draft: string
  /** What is being found; `null` before anything is sent. */
  query: string | null
  state: FindState
  /** Index into `state.matches.items`. */
  current: number | null
  /** Changes when the bar should take focus and select its text (⌘F). */
  focusNonce: number
  /** The last `focusNonce` a bar selected its text for: drawn again (in the sheet), it only takes focus. */
  selectedRef: MutableRefObject<number>
  target: FindTarget | null
  rootScroll: ScrollRequest | null
  sheetScroll: SheetScroll | null
  /** The raw records at this rank: the occurrence is on no line. */
  rawRank: number | null
  show: () => void
  type: (text: string) => void
  composing: (on: boolean) => void
  next: () => void
  previous: () => void
  close: () => void
}

/** A rank in one subagent's transcript (the sheet shows one at a time). */
export interface SheetScroll extends ScrollRequest {
  id: string
}

type Wanted = { kind: 'first' } | { kind: 'at'; at: RecordAt } | null

export function useReaderFind({
  header,
  leaf,
  rootCards,
  find,
  at,
  onStale,
  onFind,
  onSheet,
  onLine,
  onRaw,
}: {
  header: ChatHeader | undefined
  leaf: number | null
  rootCards: ChatChildCard[]
  /** From a search: find these words… */
  find: string | null
  /** …and go to the occurrence at this record. */
  at: RecordAt | null
  onStale: () => void
  /** The words found changed (or `null`: closed), for the address. */
  onFind?: (find: string | null) => void
  onSheet: (stack: ChatChildCard[]) => void
  onLine: (leaf: number) => void
  onRaw: (raw: boolean) => void
}): ReaderFind {
  const { t } = useI18n()
  const id = header?.summary.id ?? ''
  const [open, setOpen] = useState(find !== null)
  const [draft, setDraft] = useState(find ?? '')
  const [query, setQuery] = useState<string | null>(find)
  const [current, setCurrent] = useState<number | null>(null)
  const [wanted, setWanted] = useState<Wanted>(
    find === null ? null : at ? { kind: 'at', at } : { kind: 'first' },
  )
  const [focusNonce, setFocusNonce] = useState(0)
  const [target, setTarget] = useState<FindTarget | null>(null)
  const [rootScroll, setRootScroll] = useState<ScrollRequest | null>(null)
  const [sheetScroll, setSheetScroll] = useState<SheetScroll | null>(null)
  const [rawRank, setRawRank] = useState<number | null>(null)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const isComposing = useRef(false)
  const moves = useRef(0)
  const revealed = useRef<number | null>(null)
  const selectedRef = useRef(0)

  // Another search hit in this same conversation: its words, its record.
  // The words this bar sent coming back through the address are not one.
  const askedKey = `${find ?? ''}\n${at ? writeAt(at) : ''}`
  const [asked, setAsked] = useState(askedKey)
  const [sent, setSent] = useState<string | null | undefined>(undefined)
  if (asked !== askedKey) {
    setAsked(askedKey)
    if (find !== null && !(find === sent && at === null)) {
      setOpen(true)
      setDraft(find)
      setQuery(find)
      setCurrent(null)
      setWanted(at ? { kind: 'at', at } : { kind: 'first' })
    }
  }

  const state = useFind(id, header ? query : null, onStale)
  const items = state.matches.items

  const commit = useCallback(
    (text: string) => {
      if (timer.current) clearTimeout(timer.current)
      timer.current = null
      const next = text.trim() ? text : null
      setQuery(next)
      setCurrent(null)
      setTarget(null)
      setWanted(next === null ? null : { kind: 'first' })
      setSent(next)
      onFind?.(next)
    },
    [onFind],
  )

  const go = useCallback(
    async (index: number) => {
      const match = items[index]
      if (!match || !header) return
      const move = ++moves.current
      setCurrent(index)
      setTarget({ occurrence: match.occurrence, token: partToken(match) })
      revealed.current = null
      try {
        if (match.path.length > 0) {
          const stack = await stackOf(match.path, rootCards)
          if (move !== moves.current) return
          onRaw(false)
          onSheet(stack)
          if (match.address) {
            const rank = await rankOf(match.address, 'visible', null, onStale)
            if (move === moves.current && rank !== null)
              setSheetScroll({
                id: match.address.conversationId,
                rank,
                nonce: move,
              })
          }
          return
        }
        onSheet([])
        if (!match.address) return
        let rank = await rankOf(match.address, 'visible', leaf, onStale)
        if (rank === null && header.summary.harness === 'pi') {
          // Another branch of a Pi conversation: the line through this record.
          onLine(match.address.ordinal)
          rank = await rankOf(
            match.address,
            'visible',
            match.address.ordinal,
            onStale,
          )
        }
        if (move !== moves.current) return
        if (rank === null) {
          // On no line (an earlier source, unresolved parentage): the raw records.
          const raw = await rankOf(match.address, 'raw', null, onStale)
          if (move !== moves.current) return
          setRawRank(raw)
          onRaw(true)
          return
        }
        setRawRank(null)
        onRaw(false)
        setRootScroll({ rank, nonce: move })
      } catch (error) {
        // It stays current and counted; only showing it failed.
        if (move === moves.current)
          toast.error(t('agentReader.find.cantShow'), {
            description: describeError(error),
          })
      }
    },
    [items, header, rootCards, leaf, onStale, onSheet, onLine, onRaw, t],
  )

  // The occurrence asked for, once it has arrived (or the first, once the
  // record asked for turned out to have none).
  useEffect(() => {
    if (!wanted || items.length === 0) return
    let index = 0
    if (wanted.kind === 'at') {
      index = items.findIndex(
        (match) =>
          match.address?.sourceId === wanted.at.sourceId &&
          match.address.ordinal === wanted.at.ordinal,
      )
      if (index < 0) {
        if (state.phase !== 'done') return
        index = 0
      }
    }
    // eslint-disable-next-line react-hooks/set-state-in-effect -- moving there once the occurrence exists
    setWanted(null)
    void go(index)
  }, [wanted, items, state.phase, go])

  // Light what is drawn now, and again whenever the page changes.
  useEffect(() => {
    if (!open || items.length === 0) {
      clearFind()
      return
    }
    let frame = 0
    const paint = () => {
      frame = 0
      const range = paintFind(
        [document.body],
        state.matches,
        current === null ? null : items[current],
      )
      if (range && current !== null && revealed.current !== current) {
        revealed.current = current
        range.startContainer.parentElement?.scrollIntoView({
          block: 'center',
        })
      }
    }
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(paint)
    }
    schedule()
    const observer = new MutationObserver(schedule)
    observer.observe(document.body, {
      childList: true,
      subtree: true,
      characterData: true,
    })
    return () => {
      observer.disconnect()
      if (frame) cancelAnimationFrame(frame)
    }
  }, [open, items, current, state.matches])

  useEffect(() => () => clearFind(), [])

  const show = useCallback(() => {
    setOpen(true)
    setFocusNonce((n) => n + 1)
  }, [])

  const type = useCallback(
    (text: string) => {
      setDraft(text)
      if (isComposing.current) return
      if (timer.current) clearTimeout(timer.current)
      timer.current = setTimeout(() => commit(text), DEBOUNCE_MS)
    },
    [commit],
  )

  const composing = useCallback((on: boolean) => {
    isComposing.current = on
    if (on && timer.current) clearTimeout(timer.current)
  }, [])

  const step = useCallback(
    (by: number) => {
      if ((draft.trim() || null) !== query) {
        commit(draft)
        return
      }
      if (items.length === 0) return
      const next =
        current === null
          ? by > 0
            ? 0
            : items.length - 1
          : (current + by + items.length) % items.length
      void go(next)
    },
    [draft, query, items.length, current, commit, go],
  )
  const next = useCallback(() => step(1), [step])
  const previous = useCallback(() => step(-1), [step])

  const close = useCallback(() => {
    if (timer.current) clearTimeout(timer.current)
    moves.current += 1
    setOpen(false)
    setQuery(null)
    setDraft('')
    setCurrent(null)
    setTarget(null)
    setWanted(null)
    setSent(null)
    clearFind()
    onFind?.(null)
  }, [onFind])

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current)
    },
    [],
  )

  return {
    open,
    draft,
    query,
    state,
    current,
    focusNonce,
    selectedRef,
    target,
    rootScroll,
    sheetScroll,
    rawRank,
    show,
    type,
    composing,
    next,
    previous,
    close,
  }
}
