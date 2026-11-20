/**
 * The two reader preferences of Settings › Agent conversations
 * (`agent-history.md` §7 "閱讀器"), as the rows read them: whether each
 * turn's tool calls start as one line, and whether recorded thinking is
 * shown at all. The reader provides them once for the conversation and
 * its subagent sheet.
 */
import { createContext, useContext, useMemo } from 'react'
import {
  READER_DEFAULTS,
  useViewerPreferences,
} from '@/lib/queries/agent-viewer'

export interface ReaderPrefs {
  collapseToolCalls: boolean
  showThinking: boolean
}

export const ReaderPrefsContext = createContext<ReaderPrefs>({
  collapseToolCalls: READER_DEFAULTS.readerCollapseToolCalls,
  showThinking: READER_DEFAULTS.readerShowThinking,
})

export function useReaderPrefs() {
  return useContext(ReaderPrefsContext)
}

/**
 * The saved preferences, or the defaults when they could not be read.
 * `ready` is false only while they are first being read, so the reader
 * does not draw thinking it would then take away.
 */
export function useLoadedReaderPrefs() {
  const query = useViewerPreferences()
  const collapse =
    query.data?.readerCollapseToolCalls ??
    READER_DEFAULTS.readerCollapseToolCalls
  const thinking =
    query.data?.readerShowThinking ?? READER_DEFAULTS.readerShowThinking
  const prefs = useMemo<ReaderPrefs>(
    () => ({ collapseToolCalls: collapse, showThinking: thinking }),
    [collapse, thinking],
  )
  return { prefs, ready: !(query.isPending && query.fetchStatus !== 'idle') }
}
