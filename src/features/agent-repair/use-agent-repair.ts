/**
 * The conversation repair as a sequence the dialog draws: check (the
 * backend prepares a repair and holds it for 15 minutes), choose where
 * conversations come back from when the damage allows a choice, repair, and
 * the result.
 *
 * Rules it keeps:
 * - Only an explicit request starts a check; choosing a copy does not, so a
 *   keyboard walking the list doesn't start one check per row.
 * - The backend holds one prepared repair. A later successful check
 *   replaces it; a failed one leaves it; stopping a check discards it too
 *   (`cancel_maintenance` does both).
 * - A repair that fails has used up its check, whatever the reason: the next
 *   step is always a new check.
 * - A copy's password is asked for only when its check says the copy needs
 *   one (a copy under the archive's own key needs none).
 *
 * Not responsible for: drawing (`agent-repair-dialog.tsx` and its parts),
 * the words for errors (`repair-errors.ts`).
 */
import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef, useState } from 'react'
import { refreshAfterArchiveChange } from '@/app/backup-runner'
import {
  agentRepairClient,
  type RepairPreview,
  type RepairResult,
  type RepairSource,
} from '@/lib/backend-client/agent-repair'
import { queryKeys } from '@/lib/query'
import { waitForNextPaint } from '@/lib/wait-for-next-paint'
import { repairErrorIs } from './repair-errors'

/** Where conversations come back from: what is here, or a listed copy. */
export type SourceChoice = { kind: 'local' } | { kind: 'copy'; path: string }

export const LOCAL: SourceChoice = { kind: 'local' }

export function sameSource(a: SourceChoice, b: SourceChoice) {
  return a.kind === 'local'
    ? b.kind === 'local'
    : b.kind === 'copy' && a.path === b.path
}

export type RepairPhase = 'checking' | 'ready' | 'running' | 'done'

interface Prepared {
  preview: RepairPreview
  source: SourceChoice
  request: RepairSource
}

export interface RepairFailure {
  error: unknown
  during: 'check' | 'repair'
  source: SourceChoice
}

export interface RepairDone {
  result: RepairResult
  preview: RepairPreview
}

/** Lists that show what a repair changes: copies, disk use, the kinds and the Agent pages. */
async function refreshAfterRepair(client: ReturnType<typeof useQueryClient>) {
  await Promise.all([
    refreshAfterArchiveChange(),
    client.invalidateQueries({ queryKey: ['recovery-snapshots'] }),
    client.invalidateQueries({ queryKey: ['retention-preview'] }),
    client.invalidateQueries({ queryKey: queryKeys.agentSettings }),
    client.invalidateQueries({ queryKey: queryKeys.kindTasks }),
  ])
}

export function useAgentRepair() {
  const client = useQueryClient()
  const [phase, setPhase] = useState<RepairPhase>('checking')
  const [chosen, setChosen] = useState<SourceChoice>(LOCAL)
  const [prepared, setPrepared] = useState<Prepared | null>(null)
  const [failure, setFailure] = useState<RepairFailure | null>(null)
  const [stopped, setStopped] = useState(false)
  const stopping = useRef(false)
  /** Shows where conversations can come back from: the conversation files themselves are damaged. */
  const [canChoose, setCanChoose] = useState(false)
  /** Copies whose check asked for their own password. */
  const [asked, setAsked] = useState<ReadonlySet<string>>(new Set())
  const [passwords, setPasswords] = useState<Record<string, string>>({})
  const [done, setDone] = useState<RepairDone | null>(null)
  /**
   * False once the dialog is gone. Closing lets go of what the backend holds
   * (`cancel_maintenance`); a check asked for just before must not start
   * after that and leave a new one held.
   */
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])

  const requestFor = useCallback(
    (source: SourceChoice): RepairSource => {
      if (source.kind === 'local') return {}
      const password = passwords[source.path]
      return asked.has(source.path) && password
        ? { copyPath: source.path, copyPassword: password }
        : { copyPath: source.path }
    },
    [asked, passwords],
  )

  const check = useCallback(
    async (source: SourceChoice) => {
      setPhase('checking')
      setFailure(null)
      setStopped(false)
      stopping.current = false
      // The progress paints before the backend starts reading.
      await waitForNextPaint()
      if (!mounted.current) return
      const request = requestFor(source)
      try {
        const preview = await agentRepairClient.preview(request)
        setPrepared({ preview, source, request })
        if (preview.action !== 'rebuild') setCanChoose(true)
      } catch (error) {
        if (stopping.current) {
          // Stopping also let go of any repair the backend held.
          setPrepared(null)
          setStopped(true)
        } else {
          setFailure({ error, during: 'check', source })
          if (
            source.kind === 'copy' &&
            repairErrorIs(error, 'archive-password-required')
          )
            setAsked((now) => new Set(now).add(source.path))
          if (repairErrorIs(error, 'conversation-pair-unverified'))
            setCanChoose(true)
          if (repairErrorIs(error, 'conversation-repair-healthy'))
            void client.invalidateQueries({ queryKey: queryKeys.dataKinds })
        }
      } finally {
        setPhase('ready')
      }
    },
    [client, requestFor],
  )

  const repair = useCallback(async () => {
    if (!prepared || !sameSource(prepared.source, chosen)) return
    const { preview, request, source } = prepared
    setPhase('running')
    setFailure(null)
    setStopped(false)
    stopping.current = false
    await waitForNextPaint()
    if (!mounted.current) return
    try {
      const result = await agentRepairClient.repair(request, preview.token)
      setPrepared(null)
      setDone({ result, preview })
      setPhase('done')
      await refreshAfterRepair(client).catch(() => undefined)
    } catch (error) {
      // The check is used up either way; the next step is a new one.
      setPrepared(null)
      if (stopping.current) setStopped(true)
      else setFailure({ error, during: 'repair', source })
      if (repairErrorIs(error, 'conversation-repair-healthy'))
        void client.invalidateQueries({ queryKey: queryKeys.dataKinds })
      setPhase('ready')
    }
  }, [chosen, client, prepared])

  /** Marks the stop the person asked for, so it reads as a stop, not a failure. */
  const markStopping = useCallback(() => {
    stopping.current = true
  }, [])

  const setPassword = useCallback((path: string, value: string) => {
    setPasswords((now) => ({ ...now, [path]: value }))
  }, [])

  const planMatches = prepared !== null && sameSource(prepared.source, chosen)

  return {
    phase,
    chosen,
    choose: setChosen,
    prepared: planMatches ? prepared.preview : null,
    /** The backend holds a check or one is running: closing should let go of it. */
    holds: phase === 'checking' || (phase === 'ready' && prepared !== null),
    failure,
    stopped,
    canChoose,
    asked,
    passwords,
    setPassword,
    done,
    check,
    repair,
    markStopping,
  }
}

export type AgentRepairFlow = ReturnType<typeof useAgentRepair>
