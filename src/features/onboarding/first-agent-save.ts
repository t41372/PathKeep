/**
 * The steps of the Agent conversations' first save that onboarding runs once
 * the archive exists: walking the chosen tools' folders again (a walk made
 * before the archive existed cannot be handed to it), waiting for the save
 * to end, and turning browsing history off when only Agent conversations
 * were chosen.
 *
 * Responsibilities: those three, each as one promise. Not responsible for:
 * turning the kind on (`enable-agent/use-start-saving.ts`), the order of
 * the setup stages (`use-finish-setup.ts`) or any wording.
 */
import {
  agentBackupClient,
  KIND_BACKUP_FINISHED_EVENT,
  type ArchiveRunDetail,
  type KindBackupFinishedEvent,
} from '@/lib/backend-client/agent-backup'
import {
  dataKindsClient,
  dataKindsErrorCode,
  type AgentToolId,
  type DataKindList,
  type PreviewReport,
} from '@/lib/backend-client/data-kinds'
import type { AgentPreviewStream } from '@/lib/ipc/agent-preview-stream'
import { listenDesktopEvent } from '@/lib/ipc/events'
import { queryClient, queryKeys } from '@/lib/query'
import { WalkStopped } from './agent-words'

/**
 * Walks only `tools`' folders (the unticked tools are not looked at) and
 * resolves with the report, whose ticket the save needs.
 */
export async function walkTools(tools: AgentToolId[]): Promise<PreviewReport> {
  let stream: AgentPreviewStream | undefined
  try {
    return await new Promise<PreviewReport>((resolve, reject) => {
      dataKindsClient
        .previewAgentSources({ purpose: 'enable', tools }, (frame) => {
          if (frame.type === 'finished') resolve(frame.payload)
          else if (frame.type === 'error')
            reject(new WalkStopped(frame.payload.code))
          else if (frame.type === 'cancelled')
            reject(new WalkStopped(frame.payload.code))
        })
        .then((started) => {
          stream = started
        }, reject)
    })
  } finally {
    // An ended walk has nothing to stop; one that failed to deliver is.
    stream?.close()
  }
}

/** How often to read the run again, in case its end event went missing. */
const CHECK_MS = 3_000
/** Reads that may fail in a row (a brief lock, a busy archive) before giving up. */
const READ_TRIES = 5

/** Resolves with the run's first page once it has ended. */
export async function waitForRunEnd(runRef: string): Promise<ArchiveRunDetail> {
  let wake = () => {}
  const stop = await listenDesktopEvent<KindBackupFinishedEvent>(
    KIND_BACKUP_FINISHED_EVENT,
    (event) => {
      if (event.runRef === runRef) wake()
    },
  )
  try {
    let failures = 0
    for (;;) {
      try {
        const detail = await agentBackupClient.run(runRef)
        failures = 0
        if (detail.result !== 'running') return detail
      } catch (error) {
        failures += 1
        if (failures >= READ_TRIES) throw error
      }
      await new Promise<void>((resolve) => {
        const timer = setTimeout(resolve, CHECK_MS)
        wake = () => {
          clearTimeout(timer)
          resolve()
        }
      })
    }
  } finally {
    stop()
  }
}

/**
 * Turns browsing history off once Agent conversations are on, for a setup
 * that chose only them. Tries once more with a fresh token if the choices
 * changed meanwhile; resolves to whether browsing is off.
 */
export async function turnBrowsingOff(): Promise<boolean> {
  for (let attempt = 0; attempt < 2; attempt += 1) {
    const list = await dataKindsClient.list()
    const browsing = list.kinds.find((row) => row.kind === 'browsing')
    if (browsing?.state === 'off') return true
    try {
      const next = await dataKindsClient.setState(
        'browsing',
        'off',
        list.policyToken,
      )
      queryClient.setQueryData<DataKindList>(queryKeys.dataKinds, {
        kinds: next.kinds,
        policyToken: next.policyToken,
      })
      return true
    } catch (error) {
      if (dataKindsErrorCode(error) !== 'policy-changed') return false
    }
  }
  return false
}
