/**
 * What ⌘K shows of Agent conversations for the words typed so far: the six
 * most recent conversations (not put away) and five projects with nothing
 * typed; with words, the conversations whose title matches (put-away ones
 * too) and the projects whose name does (`data-spaces.md` §6).
 *
 * Responsibilities: one read per set of words, each with its own request
 * id, so the backend stops the read the next keystroke made pointless
 * (`cancel_agent_read`), and the last answer stays on screen until the
 * next arrives. Read only while the kind is on or paused and its store
 * opens. A refusal that only means "not now" (locked, off, its store
 * closed, a read called off) is no error worth showing.
 * Not responsible for: drawing (`palette-results.tsx`).
 */
import { keepPreviousData, useQuery } from '@tanstack/react-query'
import { agentChatsClient } from '@/lib/backend-client/agent-chats'
import { agentPaletteClient } from '@/lib/backend-client/agent-palette'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { queryKeys } from '@/lib/query'
import { useAgentKind } from '@/lib/queries/agent-backup'

const QUIET = new Set([
  'lock-required',
  'kind-off',
  'kind-unavailable',
  'cancelled',
  'archive-busy',
])

export function useAgentPalette(term: string) {
  const { readable } = useAgentKind()
  const query = useQuery({
    queryKey: [...queryKeys.archiveData, 'agent-palette', term],
    queryFn: ({ signal }) => {
      const requestId = crypto.randomUUID()
      // Unused once the words change or the palette closes: stop the read.
      signal.addEventListener('abort', () => {
        void agentChatsClient.cancel(requestId).catch(() => undefined)
      })
      return agentPaletteClient.query(term, requestId)
    },
    enabled: readable,
    placeholderData: keepPreviousData,
    // Every opening reads afresh: a backup or a rename may have come since.
    staleTime: 0,
    gcTime: 60_000,
  })
  const code = query.isError ? commandErrorCode(query.error) : null
  return {
    data: readable ? query.data : undefined,
    /** Nothing to show yet for any words. */
    loading: readable && query.isPending && query.fetchStatus !== 'idle',
    /** Rows from earlier words, while the answer for these is on its way. */
    stale: query.isPlaceholderData,
    failed: readable && query.isError && !QUIET.has(code ?? ''),
  }
}
