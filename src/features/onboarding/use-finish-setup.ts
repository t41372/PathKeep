/**
 * Carries out the wizard's choices once the user presses "Start first
 * backup": create the archive, store the password, install the schedule, run
 * the first backup, then set up AI.
 *
 * Responsible for: the order of those stages, resuming from the one that
 * failed, and letting the user skip a stage that is not essential.
 * Not responsible for: rendering, or deciding the choices (see `draft.ts`).
 *
 * Nothing here runs before the user starts it, so going back and changing a
 * choice costs nothing (Preview → Manual → Execute).
 */
import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useRef, useState } from 'react'
import { useBackupRunner } from '@/app/backup-runner'
import { useAiProviders } from '@/features/ai-setup/use-ai-providers'
import { useSemanticIndex } from '@/features/ai-setup/use-semantic-index'
import { frequencyHours } from '@/features/backup/schedule-actions'
import { archiveClient } from '@/lib/backend-client/archive'
import { scheduleClient } from '@/lib/backend-client/schedule'
import { securityClient } from '@/lib/backend-client/security'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { isFullDiskAccessError } from '@/lib/ipc/command-error'
import { queryKeys } from '@/lib/query'
import { currentSnapshot, useSaveConfig } from '@/lib/queries/app'
import type { BackupReport } from '@/lib/types'
import { waitForNextPaint } from '@/lib/wait-for-next-paint'
import type { Draft } from './draft'

export type Stage = 'archive' | 'keychain' | 'schedule' | 'backup' | 'ai'

/** Stages the user may skip after a failure. The archive is the only must. */
export const skippable: ReadonlySet<Stage> = new Set([
  'keychain',
  'schedule',
  'backup',
  'ai',
])

export interface FinishState {
  status: 'idle' | 'running' | 'failed' | 'done'
  current: Stage | null
  /** Finished or skipped. */
  completed: Stage[]
  skipped: Stage[]
  error: { stage: Stage; message: string } | null
  report: BackupReport | null
}

const idle: FinishState = {
  status: 'idle',
  current: null,
  completed: [],
  skipped: [],
  error: null,
  report: null,
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

export function useFinishSetup(draft: Draft, stages: Stage[]) {
  const { t, preference } = useI18n()
  const client = useQueryClient()
  const runner = useBackupRunner()
  const semantic = useSemanticIndex()
  const providers = useAiProviders()
  const save = useSaveConfig()
  const [state, setState] = useState<FinishState>(idle)
  const busy = useRef(false)

  const runStage = useCallback(
    async (stage: Stage) => {
      switch (stage) {
        case 'archive': {
          const config = currentSnapshot(client).config
          const snapshot = await archiveClient.initializeArchive(
            {
              ...config,
              selectedProfileIds: draft.selectedProfileIds,
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
          if (!result.applied) throw new Error(result.message)
          await client.invalidateQueries({ queryKey: queryKeys.schedule })
          return
        }
        case 'backup': {
          const outcome = await runner.run({ toast: false })
          if (!outcome) throw new Error(t('onboarding.done.alreadyRunning'))
          if (!outcome.ok) throw outcome.error
          setState((current) => ({ ...current, report: outcome.report }))
          return
        }
        case 'ai':
          if (draft.ai === 'provider') {
            await providers.add(draft.provider)
          } else {
            // Downloads a large model, so it carries on in the background;
            // Settings → AI shows its progress.
            void semantic.enable()
          }
          return
      }
    },
    [client, draft, preference, providers, runner, semantic, stages, t],
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
              error: {
                stage,
                message:
                  stage === 'backup' && isFullDiskAccessError(error)
                    ? t('onboarding.done.fullDiskAccess')
                    : describeError(error, stage),
              },
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
    [runStage, stages, t],
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

  return { state, start, skip }
}
