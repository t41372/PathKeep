/**
 * The press on "Start saving": turns Agent conversations on with the
 * reviewed preview and the ticked tools' folders, which starts the first
 * save (`enable_agent_history`).
 *
 * Responsibilities: waiting for the choose step's own saves (each one
 * changes the settings token); sending the folders of the ticked tools that
 * can be read; trying once more with a fresh token when the settings
 * changed meanwhile and the kind is still off; showing the kind as on at
 * once, so the switcher has the space before the backend's event arrives;
 * remembering the started save for the dialog. A preview that expired or no
 * longer matches the folders comes back as the reason to look again.
 * Not responsible for: drawing the progress (`run-step.tsx`).
 */
import { notifyManager } from '@tanstack/react-query'
import { useState } from 'react'
import {
  agentBackupClient,
  type EnableResult,
} from '@/lib/backend-client/agent-backup'
import {
  dataKindsClient,
  dataKindsErrorCode,
  needsAgentSetup,
  type AgentToolId,
  type DataKindList,
  type PreviewReport,
} from '@/lib/backend-client/data-kinds'
import { queryClient, queryKeys } from '@/lib/query'
import { inToolOrder } from '@/features/data-kinds/tools'
import { rememberFirstSave, type ShownSave } from './first-save'
import { isFound } from './preview-model'

/** Why the folders have to be looked at again before saving. */
export type RestartReason = 'expired' | 'changed' | 'policy' | 'empty'

export type StartOutcome =
  | { kind: 'started'; save: ShownSave }
  | { kind: 'restart'; reason: RestartReason }
  /** The kind was turned on somewhere else meanwhile. */
  | { kind: 'elsewhere' }
  | { kind: 'failed'; error: unknown }

function restartReason(error: unknown): RestartReason | null {
  switch (dataKindsErrorCode(error)) {
    case 'preview-expired':
      return 'expired'
    case 'root-changed':
      return 'changed'
    case 'policy-changed':
      return 'policy'
    case 'source-selection-empty':
    case 'enable-preview-required':
    case 'scope-denied':
      return 'empty'
    default:
      return null
  }
}

async function send(
  report: PreviewReport,
  rootIds: string[],
  readGit: boolean,
): Promise<EnableResult> {
  const settings = await queryClient.fetchQuery({
    queryKey: queryKeys.agentSettings,
    queryFn: dataKindsClient.agentSettings,
    staleTime: 0,
  })
  return agentBackupClient.enable({
    requestId: crypto.randomUUID(),
    previewId: report.previewId,
    selectedRootIds: rootIds,
    readGit,
    expectedPolicyToken: settings.policyToken,
  })
}

/** Whether the kind still waits to be turned on, or for its folders to be chosen, here. */
async function stillOff() {
  const list = await queryClient.fetchQuery({
    queryKey: queryKeys.dataKinds,
    queryFn: dataKindsClient.list,
    staleTime: 0,
  })
  const row = list.kinds.find((item) => item.kind === 'agent-history')
  return row?.state === 'off' || needsAgentSetup(row)
}

function showOn(result: EnableResult) {
  queryClient.setQueryData<DataKindList>(queryKeys.dataKinds, (list) =>
    list
      ? {
          policyToken: result.policyToken,
          kinds: list.kinds.map((row) =>
            row.kind === 'agent-history'
              ? { ...row, state: 'on', hasBackupSource: true }
              : row,
          ),
        }
      : list,
  )
  for (const queryKey of [
    queryKeys.dataKinds,
    queryKeys.agentSettings,
    queryKeys.kindTasks,
  ])
    void queryClient.invalidateQueries({ queryKey })
}

export function useStartSaving() {
  const [starting, setStarting] = useState(false)

  const start = async ({
    report,
    ticked,
    readGit,
    flush,
  }: {
    report: PreviewReport
    ticked: ReadonlySet<AgentToolId>
    readGit: boolean
    /** The choose step's saves still on their way; the last one's error. */
    flush: () => Promise<unknown>
  }): Promise<StartOutcome> => {
    const rows = report.rows.filter(
      (row) => isFound(row) && row.readable && ticked.has(row.tool),
    )
    const tools = inToolOrder(rows.map((row) => row.tool))
    setStarting(true)
    try {
      const unsaved = await flush()
      if (unsaved !== null) return { kind: 'failed', error: unsaved }
      let result: EnableResult
      try {
        result = await send(
          report,
          rows.map((row) => row.rootId),
          readGit,
        )
      } catch (error) {
        if (dataKindsErrorCode(error) !== 'policy-changed') throw error
        // The settings changed between reading the token and the press,
        // perhaps by this dialog's own last tick. Still off: once more.
        if (!(await stillOff())) return { kind: 'elsewhere' }
        result = await send(
          report,
          rows.map((row) => row.rootId),
          readGit,
        )
      }
      const save: ShownSave = {
        taskId: result.firstSave.taskId,
        runRef: result.firstSave.runRef,
        tools,
      }
      // Together: the dialog must never see the kind on without its save,
      // nor the save while the kind still reads off.
      notifyManager.batch(() => {
        showOn(result)
        rememberFirstSave(save)
      })
      return { kind: 'started', save }
    } catch (error) {
      const reason = restartReason(error)
      return reason ? { kind: 'restart', reason } : { kind: 'failed', error }
    } finally {
      setStarting(false)
    }
  }

  return { starting, start }
}
