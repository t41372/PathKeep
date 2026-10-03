/**
 * Boot sequence and session state.
 *
 * Responsibilities:
 * - Decide which full-screen state the app is in: locked, archive locked,
 *   recovery, upgrade, onboarding or ready.
 * - Unlock (app passcode or archive password), lock, and idle auto-lock.
 * - Seed the query cache with the snapshot so screens never fetch it again.
 *
 * Not responsible for: rendering those states (see `app/shell/`), or any
 * per-screen data.
 */
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import { appClient } from '@/lib/backend-client/app'
import { archiveClient } from '@/lib/backend-client/archive'
import { securityClient } from '@/lib/backend-client/security'
import { describeError } from '@/lib/errors'
import { isLockRequiredError } from '@/lib/ipc/command-error'
import { useI18n } from '@/lib/i18n'
import { queryClient, queryKeys } from '@/lib/query'
import type {
  AppBuildInfo,
  AppSnapshot,
  ArchiveRecoveryReport,
  ArchiveUpgradeAssessment,
} from '@/lib/types'

export type SessionPhase =
  | 'booting'
  | 'error'
  | 'app-locked'
  | 'archive-locked'
  | 'recovery'
  | 'upgrade'
  | 'onboarding'
  | 'ready'

interface SessionValue {
  phase: SessionPhase
  error: string | null
  buildInfo: AppBuildInfo | null
  recovery: ArchiveRecoveryReport | null
  upgrade: ArchiveUpgradeAssessment | null
  refresh: () => Promise<void>
  lock: () => Promise<void>
  unlockApp: (passcode: string) => Promise<void>
  unlockArchive: (password: string, remember: boolean) => Promise<void>
  /** Called by onboarding, upgrade and recovery once the archive is usable. */
  enter: (snapshot: AppSnapshot) => void
}

const SessionContext = createContext<SessionValue | null>(null)

const RECOVERY_PREFIX = 'archive_recovery_required: '

function parseRecoveryReport(error: unknown): ArchiveRecoveryReport | null {
  const message =
    error instanceof Error
      ? error.message
      : typeof error === 'string'
        ? error
        : ''
  const at = message.indexOf(RECOVERY_PREFIX)
  if (at < 0) return null
  try {
    return JSON.parse(
      message.slice(at + RECOVERY_PREFIX.length),
    ) as ArchiveRecoveryReport
  } catch {
    return null
  }
}

/** A plaintext archive that is configured but will not open is damaged. */
function needsLaunchRecovery(snapshot: AppSnapshot) {
  const status = snapshot.archiveStatus
  return (
    status.initialized &&
    !status.unlocked &&
    !status.encrypted &&
    Boolean(status.warning)
  )
}

function canAutoUnlock(snapshot: AppSnapshot) {
  return (
    snapshot.archiveStatus.encrypted &&
    !snapshot.archiveStatus.unlocked &&
    snapshot.config.rememberDatabaseKeyInKeyring &&
    snapshot.keyringStatus.available &&
    snapshot.keyringStatus.storedSecret
  )
}

export function SessionProvider({ children }: { children: ReactNode }) {
  const { setPreference, preference } = useI18n()
  const [phase, setPhase] = useState<SessionPhase>('booting')
  const [error, setError] = useState<string | null>(null)
  const [buildInfo, setBuildInfo] = useState<AppBuildInfo | null>(null)
  const [recovery, setRecovery] = useState<ArchiveRecoveryReport | null>(null)
  const [upgrade, setUpgrade] = useState<ArchiveUpgradeAssessment | null>(null)
  const triedKeyring = useRef(false)
  const upgradeChecked = useRef(false)
  const preferenceRef = useRef(preference)
  preferenceRef.current = preference

  const enter = useCallback(
    (snapshot: AppSnapshot) => {
      queryClient.setQueryData(queryKeys.snapshot, snapshot)
      // The config holds the language for the scheduler and worker. An explicit
      // choice there wins; 'system' (the default) leaves the local choice alone.
      const configured = snapshot.config.preferredLanguage
      if (
        configured &&
        configured !== 'system' &&
        configured !== preferenceRef.current
      ) {
        setPreference(configured)
      }
      setPhase(snapshot.config.initialized ? 'ready' : 'onboarding')
    },
    [setPreference],
  )

  const toLocked = useCallback(() => {
    // Drop every cached read so nothing from the archive stays in memory.
    queryClient.clear()
    setPhase('app-locked')
  }, [])

  const refresh = useCallback(async () => {
    setError(null)
    try {
      const [lockStatus, info] = await Promise.all([
        appClient.getLockStatus(),
        appClient.getBuildInfo(),
      ])
      setBuildInfo(info)
      queryClient.setQueryData(queryKeys.lockStatus, lockStatus)
      if (lockStatus.locked) return toLocked()

      let snapshot = await appClient.getSnapshot()

      if (!triedKeyring.current && canAutoUnlock(snapshot)) {
        triedKeyring.current = true
        try {
          const key = await securityClient.getDatabaseKey()
          if (key) {
            await appClient.setSessionDatabaseKey(key)
            void appClient.reconcileArchiveEncryption().catch(() => undefined)
            snapshot = await appClient.getSnapshot()
          }
        } catch {
          // Keychain access can be denied; fall back to the password prompt.
        }
      }

      queryClient.setQueryData(queryKeys.snapshot, snapshot)

      if (
        snapshot.archiveStatus.encrypted &&
        !snapshot.archiveStatus.unlocked
      ) {
        setPhase('archive-locked')
        return
      }

      if (needsLaunchRecovery(snapshot)) {
        try {
          snapshot = await archiveClient.initializeArchive(snapshot.config)
        } catch (initError) {
          const report = parseRecoveryReport(initError)
          if (!report) throw initError
          setRecovery(report)
          setPhase('recovery')
          return
        }
      }

      if (
        !upgradeChecked.current &&
        snapshot.config.initialized &&
        snapshot.archiveStatus.unlocked
      ) {
        upgradeChecked.current = true
        const assessment = await archiveClient
          .assessArchiveUpgrade()
          .catch(() => null)
        if (assessment?.pending) {
          queryClient.setQueryData(queryKeys.snapshot, snapshot)
          setUpgrade(assessment)
          setPhase('upgrade')
          return
        }
      }

      enter(snapshot)
    } catch (bootError) {
      if (isLockRequiredError(bootError)) return toLocked()
      setError(describeError(bootError, 'boot'))
      setPhase('error')
    }
  }, [enter, toLocked])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const lock = useCallback(async () => {
    await appClient.lockAppSession('manual')
    toLocked()
  }, [toLocked])

  const unlockApp = useCallback(
    async (passcode: string) => {
      await appClient.unlockAppSession({ passcode })
      await refresh()
    },
    [refresh],
  )

  const unlockArchive = useCallback(
    async (password: string, remember: boolean) => {
      await appClient.setSessionDatabaseKey(password)
      const snapshot = await appClient.getSnapshot()
      if (!snapshot.archiveStatus.unlocked) {
        await appClient.clearSessionDatabaseKey()
        throw new Error('wrong-password')
      }
      if (remember) {
        await securityClient.storeDatabaseKey(password).catch(() => undefined)
      }
      void appClient.reconcileArchiveEncryption().catch(() => undefined)
      await refresh()
    },
    [refresh],
  )

  useIdleLock(phase === 'ready', toLocked)

  const value = useMemo<SessionValue>(
    () => ({
      phase,
      error,
      buildInfo,
      recovery,
      upgrade,
      refresh,
      lock,
      unlockApp,
      unlockArchive,
      enter,
    }),
    [
      phase,
      error,
      buildInfo,
      recovery,
      upgrade,
      refresh,
      lock,
      unlockApp,
      unlockArchive,
      enter,
    ],
  )

  return (
    <SessionContext.Provider value={value}>{children}</SessionContext.Provider>
  )
}

/**
 * Locks after the configured idle time. Activity only resets a timestamp;
 * a single coarse interval checks it, so mouse movement costs nothing.
 */
function useIdleLock(active: boolean, onLocked: () => void) {
  useEffect(() => {
    if (!active) return
    const status = queryClient.getQueryData<AppSnapshot>(
      queryKeys.snapshot,
    )?.appLockStatus
    const minutes = status?.enabled ? status.idleTimeoutMinutes : 0
    if (!minutes || minutes <= 0) return

    let lastActivity = Date.now()
    const touch = () => {
      lastActivity = Date.now()
    }
    const events = ['pointerdown', 'pointermove', 'keydown', 'wheel'] as const
    events.forEach((name) =>
      window.addEventListener(name, touch, { passive: true }),
    )
    const timer = window.setInterval(() => {
      if (Date.now() - lastActivity < minutes * 60_000) return
      window.clearInterval(timer)
      void appClient
        .lockAppSession('idle-timeout')
        .then(onLocked)
        .catch(() => undefined)
    }, 15_000)
    return () => {
      window.clearInterval(timer)
      events.forEach((name) => window.removeEventListener(name, touch))
    }
  }, [active, onLocked])
}

export function useSession() {
  const value = useContext(SessionContext)
  if (!value) throw new Error('useSession must be used inside SessionProvider')
  return value
}
