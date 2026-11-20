/**
 * Carries out the wizard's choices once the user presses "Start first
 * backup": create the archive, store the password, install the schedule, run
 * the first backup of each chosen kind of data, then set up AI.
 *
 * Responsible for: the order of those stages, resuming from the one that
 * failed, and letting the user skip a stage that is not essential. In the
 * first backup, each chosen kind runs as its own task, side by side, and
 * ends on its own: one that fails says why while the others finish; the
 * stage fails (with retry and skip, as every stage) only when none of them
 * could run, and a retry runs only the ones that failed. The Agent save can
 * be left to go on in the background, as in the enable dialog: setup stops
 * waiting for it and finishes its other stages. When the answer to turning
 * the kind on is lost while the save it started goes on, setup takes that
 * save instead of turning the kind on again.
 * Creating the archive saves the final kinds and Agent tools first
 * (`save_onboarding_choices`): the backend applies the last ones saved as
 * the new archive's states, so they must be the ones shown.
 * Not responsible for: rendering, or deciding the choices (see `draft.ts`).
 *
 * Nothing here runs before the user starts it, so going back and changing a
 * choice costs nothing (Preview → Manual → Execute).
 */
import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useRef, useState } from 'react'
import { scheduleCopy } from '@/features/backup/schedule-copy'
import { useBackupRunner } from '@/app/backup-runner'
import { useAiProviders } from '@/features/ai-setup/use-ai-providers'
import { useSemanticIndex } from '@/features/ai-setup/use-semantic-index'
import { frequencyHours } from '@/features/backup/schedule-actions'
import {
  forgetFirstSave,
  type ShownSave,
} from '@/features/enable-agent/first-save'
import { useStartSaving } from '@/features/enable-agent/use-start-saving'
import type { ArchiveRunDetail } from '@/lib/backend-client/agent-backup'
import { archiveClient } from '@/lib/backend-client/archive'
import {
  dataKindsClient,
  dataKindsErrorCode,
  type AgentToolId,
  type DataKindId,
} from '@/lib/backend-client/data-kinds'
import { scheduleClient } from '@/lib/backend-client/schedule'
import { securityClient } from '@/lib/backend-client/security'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { isFullDiskAccessError } from '@/lib/ipc/command-error'
import { queryKeys } from '@/lib/query'
import { currentSnapshot, useSaveConfig } from '@/lib/queries/app'
import {
  BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID,
  type BackupReport,
} from '@/lib/types'
import { waitForNextPaint } from '@/lib/wait-for-next-paint'
import { agentStartFailure } from './agent-words'
import { choicesToSave, type Draft } from './draft'
import {
  adoptFirstSave,
  turnBrowsingOff,
  waitForRunEnd,
  walkTools,
} from './first-agent-save'

export type Stage = 'archive' | 'keychain' | 'schedule' | 'backup' | 'ai'

/** Stages the user may skip after a failure. The archive is the only must. */
export const skippable: ReadonlySet<Stage> = new Set([
  'keychain',
  'schedule',
  'backup',
  'ai',
])

/** Browsing history's first backup: the shared backup runner's run. */
export type BrowsingRun =
  | { status: 'running' }
  | { status: 'ended'; report: BackupReport }
  | { status: 'failed'; message: string }

/**
 * Agent conversations' first save: walking the chosen tools and turning the
 * kind on (`preparing`), then the save, a background task, until it ends.
 * A failure after the save started keeps it, so a retry waits for the same
 * save instead of turning the kind on twice. `continued`: the person let it
 * go on in the background; it is still running, and nothing here waits.
 */
export type AgentRun =
  | { status: 'preparing' }
  | { status: 'saving'; save: ShownSave }
  | { status: 'continued'; save: ShownSave }
  | { status: 'ended'; save: ShownSave; detail: ArchiveRunDetail }
  | { status: 'failed'; message: string; save?: ShownSave }

export interface KindRuns {
  browsing?: BrowsingRun
  agent?: AgentRun
}

export interface FinishState {
  status: 'idle' | 'running' | 'failed' | 'done'
  current: Stage | null
  /** Finished or skipped. */
  completed: Stage[]
  skipped: Stage[]
  error: { stage: Stage; message: string } | null
  /** The first backup of each chosen kind, once it started. */
  runs: KindRuns
  /** Only Agent conversations were chosen, and browsing could not be turned off. */
  browsingStillOn: boolean
}

/** What the stages need beyond the draft: the choices the sources step settled. */
export interface FinishChoices {
  kinds: DataKindId[]
  /** The ticked Agent tools PathKeep can read. */
  agentTools: AgentToolId[]
}

const idle: FinishState = {
  status: 'idle',
  current: null,
  completed: [],
  skipped: [],
  error: null,
  runs: {},
  browsingStillOn: false,
}

export function stagesFor(
  draft: Draft,
  options: { keychainAvailable: boolean; scheduleSupported: boolean },
): Stage[] {
  const stages: Stage[] = ['archive']
  if (draft.encrypt && draft.keychain && options.keychainAvailable) {
    stages.push('keychain')
  }
  if (draft.frequency !== 'off' && options.scheduleSupported) {
    stages.push('schedule')
  }
  stages.push('backup')
  if (draft.ai !== 'off') stages.push('ai')
  return stages
}

export function useFinishSetup(
  draft: Draft,
  stages: Stage[],
  choices: FinishChoices,
) {
  const { t, preference } = useI18n()
  const client = useQueryClient()
  const runner = useBackupRunner()
  const saving = useStartSaving()
  const semantic = useSemanticIndex()
  const providers = useAiProviders()
  const save = useSaveConfig()
  const [state, setState] = useState<FinishState>(idle)
  const busy = useRef(false)
  // The kinds' runs outlive one attempt: a retry reads what the last one left.
  const runs = useRef<KindRuns>({})
  // Aborted by "Keep going in the background": stops waiting for the save.
  const watch = useRef(new AbortController())

  const setRun = useCallback((patch: KindRuns) => {
    runs.current = { ...runs.current, ...patch }
    const next = runs.current
    setState((current) => ({ ...current, runs: next }))
  }, [])

  const runBrowsing = useCallback(async () => {
    setRun({ browsing: { status: 'running' } })
    const outcome = await runner.run({ toast: false })
    if (!outcome) {
      setRun({
        browsing: {
          status: 'failed',
          message: t('onboarding.done.alreadyRunning'),
        },
      })
    } else if (!outcome.ok) {
      setRun({
        browsing: {
          status: 'failed',
          message: isFullDiskAccessError(outcome.error)
            ? t('onboarding.done.fullDiskAccess')
            : describeError(outcome.error, 'backup'),
        },
      })
    } else {
      setRun({ browsing: { status: 'ended', report: outcome.report } })
    }
  }, [runner, setRun, t])

  /** Walks the chosen tools again and turns the kind on with them. */
  const startAgent = useCallback(async (): Promise<ShownSave> => {
    for (let attempt = 0; ; attempt += 1) {
      const report = await walkTools(choices.agentTools)
      const outcome = await saving.start({
        report,
        ticked: new Set(choices.agentTools),
        readGit: true,
        flush: () => Promise.resolve(null),
      })
      if (outcome.kind === 'started') return outcome.save
      if (outcome.kind === 'failed') throw outcome.error
      if (outcome.kind === 'elsewhere')
        throw new Error(t('enableAgent.choose.turnedOnElsewhere'))
      // The folders changed between the walk and the press: look once more.
      if (outcome.reason === 'empty')
        throw new Error(t('onboarding.agent.noneReadable'))
      if (attempt > 0) throw new Error(t('onboarding.agent.changed'))
    }
  }, [choices.agentTools, saving, t])

  /** The save `enable` started, when its answer never came back; `null` for none. */
  const adopt = useCallback(
    async (): Promise<ShownSave | null> =>
      adoptFirstSave().then(
        (save) => save && { ...save, tools: choices.agentTools },
        () => null,
      ),
    [choices.agentTools],
  )

  const runAgent = useCallback(async () => {
    const previous = runs.current.agent
    let started =
      previous && 'save' in previous && previous.save
        ? previous.save
        : undefined
    if (!started) {
      setRun({ agent: { status: 'preparing' } })
      try {
        // A retry after a lost answer finds the save already going.
        started = (await adopt()) ?? (await startAgent())
      } catch (error) {
        // The answer may be what failed, not the start: look once more.
        const going = await adopt()
        if (!going) {
          setRun({
            agent: { status: 'failed', message: agentStartFailure(t, error) },
          })
          return
        }
        started = going
      }
      if (!choices.kinds.includes('browsing')) {
        const off = await turnBrowsingOff().catch(() => false)
        if (!off) setState((current) => ({ ...current, browsingStillOn: true }))
      }
    }
    const save = started
    if (watch.current.signal.aborted) {
      setRun({ agent: { status: 'continued', save } })
      return
    }
    setRun({ agent: { status: 'saving', save } })
    try {
      const detail = await waitForRunEnd(save.runRef, watch.current.signal)
      if (!detail) {
        // Still running; Background tasks and the enable dialog show it.
        setRun({ agent: { status: 'continued', save } })
        return
      }
      // Its end is shown here; the enable dialog has nothing left to show.
      forgetFirstSave()
      setRun({ agent: { status: 'ended', save, detail } })
    } catch (error) {
      setRun({
        agent: {
          status: 'failed',
          save,
          message: describeError(error, 'get_archive_run'),
        },
      })
    }
  }, [adopt, choices.kinds, setRun, startAgent, t])

  /** Lets the Agent save go on without setup waiting for it, as the enable dialog does. */
  const keepGoingInBackground = useCallback(() => {
    watch.current.abort()
  }, [])

  const runFirstBackups = useCallback(async () => {
    const work: Promise<void>[] = []
    if (
      choices.kinds.includes('browsing') &&
      runs.current.browsing?.status !== 'ended'
    )
      work.push(runBrowsing())
    const agent = runs.current.agent?.status
    if (
      choices.kinds.includes('agent-history') &&
      agent !== 'ended' &&
      agent !== 'continued'
    )
      work.push(runAgent())
    await Promise.all(work)
    const failures = choices.kinds.flatMap((kind) => {
      const run =
        kind === 'browsing' ? runs.current.browsing : runs.current.agent
      return run?.status === 'failed' ? [{ kind, message: run.message }] : []
    })
    // Each kind that ran says how it went; the stage fails only when none could.
    if (failures.length < choices.kinds.length) return
    throw new Error(
      failures.length === 1
        ? failures[0].message
        : failures
            .map(({ kind, message }) =>
              t('onboarding.done.kindFailed', {
                kind: t(
                  kind === 'browsing'
                    ? 'spaces.browsing.name'
                    : 'spaces.agent.name',
                ),
                reason: message,
              }),
            )
            .join(' '),
    )
  }, [choices.kinds, runAgent, runBrowsing, t])

  const runStage = useCallback(
    async (stage: Stage) => {
      switch (stage) {
        case 'archive': {
          // Creating the archive applies the last choices saved: these.
          await dataKindsClient
            .saveOnboardingChoices(
              choicesToSave(choices.kinds, choices.agentTools),
            )
            .catch((error: unknown) => {
              // A retry after an answer that never came: it exists already.
              if (dataKindsErrorCode(error) !== 'archive-already-initialized')
                throw error
            })
          const config = currentSnapshot(client).config
          const snapshot = await archiveClient.initializeArchive(
            {
              ...config,
              // Without browsing, the saved choice stays as it is: an empty
              // one would be refused while browsing is the only kind on.
              selectedProfileIds: choices.kinds.includes('browsing')
                ? draft.selectedProfileIds
                : config.selectedProfileIds,
              dueAfterHours:
                draft.frequency === 'off'
                  ? config.dueAfterHours
                  : frequencyHours[draft.frequency],
              archiveMode: draft.encrypt ? 'Encrypted' : 'Plaintext',
              rememberDatabaseKeyInKeyring: stages.includes('keychain'),
              preferredLanguage: preference,
            },
            draft.encrypt ? draft.password : null,
          )
          client.setQueryData(queryKeys.snapshot, snapshot)
          if (!snapshot.config.initialized || !snapshot.archiveStatus.unlocked)
            throw new Error(t('onboarding.done.notOpened'))
          return
        }
        case 'keychain':
          await securityClient.storeDatabaseKey(draft.password)
          return
        case 'schedule': {
          // Same inputs as the plan shown on the schedule step: the saved
          // interval and this app's paths.
          const plan = await scheduleClient.previewInstall()
          const result = await scheduleClient.applyInstall(plan)
          if (!result.applied)
            throw new Error(scheduleCopy(t, result.message) ?? result.message)
          await client.invalidateQueries({ queryKey: queryKeys.schedule })
          return
        }
        case 'backup':
          await runFirstBackups()
          return
        case 'ai':
          if (draft.ai === 'provider') {
            await providers.add(draft.provider)
          } else {
            // Downloads a large model, so it carries on in the background;
            // Settings → AI shows its progress.
            void semantic.enable(BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID)
          }
          return
      }
    },
    [
      choices.agentTools,
      choices.kinds,
      client,
      draft,
      preference,
      providers,
      runFirstBackups,
      semantic,
      stages,
      t,
    ],
  )

  const runFrom = useCallback(
    async (completed: Stage[]) => {
      if (busy.current) return
      busy.current = true
      setState((current) => ({ ...current, status: 'running', error: null }))
      // Let the running state paint before the backend starts working.
      await waitForNextPaint()
      const done = [...completed]
      try {
        for (const stage of stages) {
          if (done.includes(stage)) continue
          setState((current) => ({ ...current, current: stage }))
          try {
            await runStage(stage)
          } catch (error) {
            setState((current) => ({
              ...current,
              status: 'failed',
              error: { stage, message: describeError(error, stage) },
            }))
            return
          }
          done.push(stage)
          setState((current) => ({ ...current, completed: [...done] }))
        }
        setState((current) => ({ ...current, status: 'done', current: null }))
      } finally {
        busy.current = false
      }
    },
    [runStage, stages],
  )

  const start = useCallback(
    () => void runFrom(state.completed),
    [runFrom, state.completed],
  )

  /** Moves past a failed stage the user can live without. */
  const skip = useCallback(async () => {
    const stage = state.error?.stage
    if (!stage || !skippable.has(stage)) return
    if (stage === 'keychain') {
      // The archive was created expecting the keychain; tell it not to.
      try {
        await save.mutateAsync((config) => ({
          ...config,
          rememberDatabaseKeyInKeyring: false,
        }))
      } catch (error) {
        setState((current) => ({
          ...current,
          error: { stage, message: describeError(error, 'save_config') },
        }))
        return
      }
    }
    setState((current) => ({
      ...current,
      skipped: [...current.skipped, stage],
    }))
    await runFrom([...state.completed, stage])
  }, [runFrom, save, state.completed, state.error])

  return { state, start, skip, keepGoingInBackground }
}
