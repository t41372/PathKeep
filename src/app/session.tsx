/**
 * Boot sequence and session state.
 *
 * Responsibilities:
 * - Ask first whether this build may open the archive at all
 *   (`get_archive_access`). A refused or damaged archive, and a desktop
 *   started for recovery only, go to the `access` screen before anything
 *   else is read: in that mode only the recovery commands answer.
 * - Decide which full-screen state the app is in otherwise: locked, archive
 *   locked, recovery, upgrade, onboarding or ready.
 * - Unlock (app passcode, Touch ID or archive password), lock, and idle
 *   auto-lock.
 * - Seed the query cache with the snapshot so screens never fetch it again.
 *
 * Not responsible for: rendering those states (see `app/shell/`), or any
 * per-screen data.
 */
import { useQuery } from '@tanstack/react-query'
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import { appClient } from '@/lib/backend-client/app'
import { archiveClient } from '@/lib/backend-client/archive'
import {
  archiveAccessClient,
  type ArchiveAccess,
} from '@/lib/backend-client/archive-access'
import { securityClient } from '@/lib/backend-client/security'
import { parseRecoveryReport } from './recovery-report'
import { describeError } from '@/lib/errors'
import {
  CommandInvokeError,
  ERROR_CODE_ARCHIVE_PASSWORD_WRONG,
  isArchiveClosedError,
  isLockRequiredError,
} from '@/lib/ipc/command-error'
import { useI18n } from '@/lib/i18n'
import { queryClient, queryKeys } from '@/lib/query'
import type {
  AppBuildInfo,
  AppSnapshot,
  ArchiveRecoveryReport,
  ArchiveUpgradeAssessment,
} from '@/lib/types'
import { prefetchInsightsZone } from '@/features/insights/zone'
import { announceWipeNotices } from './wipe-notices'

export type SessionPhase =
  | 'booting'
  | 'error'
  | 'access'
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
  /** Why the archive was not opened, for the `access` screen. */
  access: ArchiveAccess | null
  /**
   * The package the archive was just prepared into for PathKeep 0.5.0. The
   * app is recovery-only from then on, so it leaves the ordinary screens.
   */
  prepared: string | null
  /** Called once preparing for PathKeep 0.5.0 succeeded. */
  showPrepared: (packageFolder: string) => Promise<void>
  refresh: () => Promise<void>
  /** Resolves to false when there is no passcode, so nothing was locked. */
  lock: () => Promise<boolean>
  unlockApp: (passcode: string) => Promise<void>
  /** Shows the system Touch ID prompt; the backend refuses it when Settings turned it off. */
  unlockWithTouchId: () => Promise<void>
  unlockArchive: (password: string, remember: boolean) => Promise<void>
  /** Called by onboarding, upgrade and recovery once the archive is usable. */
  enter: (snapshot: AppSnapshot) => void
  /**
   * Boots again from nothing, as on a fresh launch. Used after all data was
   * deleted, which leaves the app uninitialized and so lands on onboarding.
   */
  restart: () => Promise<void>
}

const SessionContext = createContext<SessionValue | null>(null)

/** States the ordinary screens can't start from: the archive was not opened. */
function refused(access: ArchiveAccess) {
  return (
    access.recoveryOnly ||
    access.state === 'refusedRead' ||
    access.state === 'refusedWrite' ||
    access.state === 'needsRecovery'
  )
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
  const { setPreference, preference, t } = useI18n()
  const tRef = useRef(t)
  useLayoutEffect(() => {
    tRef.current = t
  }, [t])
  const [phase, setPhase] = useState<SessionPhase>('booting')
  const [error, setError] = useState<string | null>(null)
  const [buildInfo, setBuildInfo] = useState<AppBuildInfo | null>(null)
  const [recovery, setRecovery] = useState<ArchiveRecoveryReport | null>(null)
  const [upgrade, setUpgrade] = useState<ArchiveUpgradeAssessment | null>(null)
  const [access, setAccess] = useState<ArchiveAccess | null>(null)
  const [prepared, setPrepared] = useState<string | null>(null)
  const triedKeyring = useRef(false)
  // What the upgrade check found; 'pending' until the upgrade screen enters
  // the converted archive.
  const upgradeDue = useRef<'unknown' | 'pending' | 'none'>('unknown')
  const preferenceRef = useRef(preference)
  useLayoutEffect(() => {
    preferenceRef.current = preference
  }, [preference])

  const enter = useCallback(
    (snapshot: AppSnapshot) => {
      upgradeDue.current = 'none'
      queryClient.setQueryData(queryKeys.snapshot, snapshot)
      if (snapshot.config.initialized) prefetchInsightsZone()
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

  // One pass through the gates below; `refresh` runs them one at a time.
  const gate = useCallback(async () => {
    setError(null)
    try {
      // First of all: may this build open the archive? When it may not, the
      // desktop answers only the recovery commands, so nothing below may run.
      const archiveAccess = await archiveAccessClient.get()
      if (refused(archiveAccess)) {
        setBuildInfo(await appClient.getBuildInfo())
        // App Lock still guards the recovery actions when its status can be
        // read; with the settings file missing or unreadable it can't be.
        const lockStatus = await appClient.getLockStatus().catch(() => null)
        if (lockStatus)
          queryClient.setQueryData(queryKeys.lockStatus, lockStatus)
        if (lockStatus?.locked) return toLocked()
        setAccess(archiveAccess)
        setPhase('access')
        return
      }
      setAccess(null)

      const [lockStatus, info] = await Promise.all([
        appClient.getLockStatus(),
        appClient.getBuildInfo(),
      ])
      setBuildInfo(info)
      queryClient.setQueryData(queryKeys.lockStatus, lockStatus)
      if (lockStatus.locked) return toLocked()

      // An archive still in the old format can't open, so neither unlocking nor
      // the keychain can help yet: convert it first. Cheap when nothing is due.
      // A pass after the check found an upgrade goes back to it. It used to
      // remember only that it had checked, so a second pass (React runs the
      // first effect twice in development; an unlock runs another) went on,
      // found an encrypted archive it could not open and asked for the
      // archive password instead (WORK-UPGRADE-LOCKED-FLAKE).
      if (upgradeDue.current === 'pending') {
        setPhase('upgrade')
        return
      }
      if (upgradeDue.current === 'unknown') {
        // The old app's crash journals are finished first; one it cannot
        // finish needs the recovery screen.
        let assessment: ArchiveUpgradeAssessment
        try {
          assessment = await archiveClient.assessArchiveUpgrade()
        } catch (assessError) {
          const report = parseRecoveryReport(assessError)
          if (report) {
            setRecovery(report)
            setPhase('recovery')
            return
          }
          // Not knowing whether the archive is in the old format, the gates
          // below would ask for a password it may not have: show the error,
          // and check again next time.
          throw assessError
        }
        if (assessment.pending) {
          upgradeDue.current = 'pending'
          queryClient.setQueryData(
            queryKeys.snapshot,
            await appClient.getSnapshot(),
          )
          setUpgrade(assessment)
          setPhase('upgrade')
          return
        }
        upgradeDue.current = 'none'
      }

      let snapshot = await appClient.getSnapshot()

      if (!triedKeyring.current && canAutoUnlock(snapshot)) {
        triedKeyring.current = true
        try {
          await appClient.unlockArchiveFromKeychain()
          void appClient.reconcileArchiveEncryption().catch(() => undefined)
          snapshot = await appClient.getSnapshot()
        } catch {
          // No stored key, or keychain access was denied: ask for the password.
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

      enter(snapshot)
      // Said once, after the archive is open: a "Delete all data" that was
      // undone at startup, files it moved aside, a disk worth checking.
      void archiveAccessClient
        .takeWipeNotices()
        .then((notices) => announceWipeNotices(tRef.current, notices))
        .catch(() => undefined)
    } catch (bootError) {
      if (isLockRequiredError(bootError)) return toLocked()
      setError(describeError(bootError, 'boot'))
      setPhase('error')
    }
  }, [enter, toLocked])

  // A call while a pass runs is answered by one more pass after it, so what
  // it waits for reflects what changed before it was made (an unlock).
  const running = useRef<Promise<void> | null>(null)
  const again = useRef(false)
  const refresh = useCallback(() => {
    if (running.current) {
      again.current = true
      return running.current
    }
    const passes = async () => {
      do {
        again.current = false
        await gate()
      } while (again.current)
    }
    running.current = passes().finally(() => {
      running.current = null
    })
    return running.current
  }, [gate])

  useEffect(() => {
    void refresh()
  }, [refresh])

  // A read refused because the archive closed under this window goes back
  // through the gates above (keychain, then password or passcode) instead of
  // leaving every screen empty. Mutations report their own errors.
  const regating = useRef(false)
  useEffect(() => {
    if (phase !== 'ready') return
    return queryClient.getQueryCache().subscribe((event) => {
      if (event.type !== 'updated' || event.action.type !== 'error') return
      if (regating.current || !isArchiveClosedError(event.action.error)) return
      regating.current = true
      triedKeyring.current = false
      queryClient.clear()
      setPhase('booting')
      void refresh().finally(() => {
        regating.current = false
      })
    })
  }, [phase, refresh])

  const lock = useCallback(async () => {
    const status = await appClient.lockAppSession('manual')
    queryClient.setQueryData(queryKeys.lockStatus, status)
    // Without a passcode the backend leaves the session unlocked. Showing the
    // lock screen anyway would ask for a passcode that does not exist.
    if (!status.locked) return false
    toLocked()
    return true
  }, [toLocked])

  const unlockApp = useCallback(
    async (passcode: string) => {
      await appClient.unlockAppSession({ passcode, useBiometric: false })
      await refresh()
    },
    [refresh],
  )

  const unlockWithTouchId = useCallback(async () => {
    await appClient.unlockAppSession({ passcode: null, useBiometric: true })
    await refresh()
  }, [refresh])

  const unlockArchive = useCallback(
    async (password: string, remember: boolean) => {
      await appClient.setSessionDatabaseKey(password)
      const snapshot = await appClient.getSnapshot()
      if (!snapshot.archiveStatus.unlocked) {
        await appClient.clearSessionDatabaseKey()
        throw new CommandInvokeError('wrong-password', {
          code: ERROR_CODE_ARCHIVE_PASSWORD_WRONG,
        })
      }
      if (remember) {
        await securityClient.storeDatabaseKey(password).catch(() => undefined)
      }
      void appClient.reconcileArchiveEncryption().catch(() => undefined)
      await refresh()
    },
    [refresh],
  )

  const showPrepared = useCallback(
    async (packageFolder: string) => {
      // Every cached read described an archive this app no longer opens.
      queryClient.clear()
      setPrepared(packageFolder)
      setPhase('booting')
      await refresh()
    },
    [refresh],
  )

  const restart = useCallback(async () => {
    // Nothing cached may survive: it describes an archive that is gone.
    queryClient.clear()
    triedKeyring.current = false
    upgradeDue.current = 'unknown'
    setRecovery(null)
    setUpgrade(null)
    setPhase('booting')
    await refresh()
  }, [refresh])

  useIdleLock(phase === 'ready', toLocked)

  const value = useMemo<SessionValue>(
    () => ({
      phase,
      error,
      buildInfo,
      recovery,
      upgrade,
      access,
      prepared,
      showPrepared,
      refresh,
      lock,
      unlockApp,
      unlockWithTouchId,
      unlockArchive,
      enter,
      restart,
    }),
    [
      phase,
      error,
      buildInfo,
      recovery,
      upgrade,
      access,
      prepared,
      showPrepared,
      refresh,
      lock,
      unlockApp,
      unlockWithTouchId,
      unlockArchive,
      enter,
      restart,
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
  // Subscribes to the cached snapshot without ever fetching it, so a timeout
  // changed in Settings takes effect at once instead of after a relaunch.
  const { data: status } = useQuery({
    queryKey: queryKeys.snapshot,
    queryFn: appClient.getSnapshot,
    enabled: false,
    staleTime: Infinity,
    select: (snapshot: AppSnapshot) => snapshot.appLockStatus,
  })
  const minutes = status?.enabled ? status.idleTimeoutMinutes : 0

  useEffect(() => {
    if (!active || !minutes || minutes <= 0) return

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
        .then((status) => status.locked && onLocked())
        .catch(() => undefined)
    }, 15_000)
    return () => {
      window.clearInterval(timer)
      events.forEach((name) => window.removeEventListener(name, touch))
    }
  }, [active, minutes, onLocked])
}

export function useSession() {
  const value = useContext(SessionContext)
  if (!value) throw new Error('useSession must be used inside SessionProvider')
  return value
}
