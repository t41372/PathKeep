/**
 * The choose step's ticks (which tools) and the `.git` switch, kept as the
 * same device settings the Agent backup page edits (`update_agent_sources`),
 * so closing the dialog and opening it again shows what was chosen.
 *
 * Responsibilities: what is shown (the stored choice among the tools that
 * can be read here, every readable tool when nothing is stored), saving
 * each change in order with the current settings token, trying once more
 * after the settings changed elsewhere, and saying when a save failed.
 * Each save sends every readable tool's tick, not only the one changed:
 * the stored set starts empty, so a single "off" would change nothing.
 * `flush` waits for the saves on their way, so the save step starts from
 * the stored choice and its token.
 *
 * Not responsible for: turning the kind on (the save step).
 */
import { useRef, useState } from 'react'
import {
  dataKindsClient,
  dataKindsErrorCode,
  type AgentSettings,
  type AgentToolId,
} from '@/lib/backend-client/data-kinds'
import { queryClient, queryKeys } from '@/lib/query'
import { useAgentSettings } from '@/lib/queries/data-kinds'
import type { ToolChoice } from './preview-model'

interface Choice {
  ticked: ReadonlySet<AgentToolId>
  readGit: boolean
}

async function currentToken() {
  const cached = queryClient.getQueryData<AgentSettings>(
    queryKeys.agentSettings,
  )
  if (cached) return cached.policyToken
  const fresh = await queryClient.fetchQuery({
    queryKey: queryKeys.agentSettings,
    queryFn: dataKindsClient.agentSettings,
  })
  return fresh.policyToken
}

async function store(choice: Choice, tickable: readonly AgentToolId[]) {
  const toolChanges = Object.fromEntries(
    tickable.map((tool) => [tool, choice.ticked.has(tool)]),
  )
  const send = async (expectedToken: string) =>
    dataKindsClient.updateAgentSources({
      expectedToken,
      readGit: choice.readGit,
      toolChanges,
    })
  let saved: AgentSettings
  try {
    saved = await send(await currentToken())
  } catch (error) {
    if (dataKindsErrorCode(error) !== 'policy-changed') throw error
    // Changed elsewhere meanwhile; this save carries every tick, so sending
    // it again with the new token loses nothing.
    const fresh = await queryClient.fetchQuery({
      queryKey: queryKeys.agentSettings,
      queryFn: dataKindsClient.agentSettings,
    })
    saved = await send(fresh.policyToken)
  }
  queryClient.setQueryData(queryKeys.agentSettings, saved)
}

export function useAgentChoices(tools: readonly ToolChoice[]) {
  const settings = useAgentSettings()
  const [edit, setEdit] = useState<Choice | null>(null)
  const [saveError, setSaveError] = useState<unknown>(null)
  const latest = useRef<Choice | null>(null)
  const saving = useRef<Promise<void> | null>(null)
  /** The same as `saveError`, readable right after a save without a render. */
  const lastError = useRef<unknown>(null)

  const tickable = tools
    .filter((tool) => tool.readable)
    .map((tool) => tool.tool)
  const policy = settings.data?.policy
  const stored = policy
    ? policy.enabledTools.filter((tool) => tickable.includes(tool))
    : []
  // Nothing chosen yet on this computer: every readable tool, as the
  // spec's "all ticked". Chosen, every tool unticked stays unticked. A
  // backend that does not say falls back to "something is stored".
  const chosen = settings.data?.toolsChosen ?? stored.length > 0
  const shown: Choice = edit ?? {
    ticked: new Set(chosen ? stored : tickable),
    readGit: policy?.readGit ?? true,
  }

  const persist = (choice: Choice) => {
    latest.current = choice
    if (saving.current) return saving.current
    const run = async () => {
      try {
        // Changes made during a save go out right after it, newest only.
        let next: Choice | null = latest.current
        while (next) {
          latest.current = null
          await store(next, tickable)
          next = latest.current
        }
        lastError.current = null
        setSaveError(null)
      } catch (error) {
        lastError.current = error
        setSaveError(error)
        throw error
      } finally {
        saving.current = null
      }
    }
    saving.current = run()
    return saving.current
  }

  const change = (next: Choice) => {
    setEdit(next)
    persist(next).catch(() => undefined)
  }

  return {
    /** `true` until the stored choice is known. */
    loading: settings.isPending,
    ticked: shown.ticked,
    readGit: shown.readGit,
    setTicked: (tool: AgentToolId, on: boolean) => {
      const ticked = new Set(shown.ticked)
      if (on) ticked.add(tool)
      else ticked.delete(tool)
      change({ ticked, readGit: shown.readGit })
    },
    setReadGit: (readGit: boolean) => change({ ticked: shown.ticked, readGit }),
    saveError,
    retrySave: () => void persist(shown).catch(() => undefined),
    /** Waits until every change is stored; the last save's error, or null. */
    flush: async (): Promise<unknown> => {
      if (saving.current) await saving.current.catch(() => undefined)
      return lastError.current
    },
  }
}
