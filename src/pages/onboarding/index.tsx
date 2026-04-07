import { useEffect, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { PermissionGate } from '../../components/primitives/permission-gate'
import { backend } from '../../lib/backend'
import { formatBytes } from '../../lib/format'
import type { SchedulePlan } from '../../lib/types'

const dueAfterOptions = [6, 12, 24, 72]
const scheduleOptions = [1, 6, 12]

interface SchedulePreviewState {
  requestKey: string | null
  plan: SchedulePlan | null
  error: string | null
}

export function OnboardingPage() {
  const navigate = useNavigate()
  const {
    busyAction,
    error,
    loading,
    saveConfig,
    initializeArchive,
    runBackup,
    snapshot,
  } = useShellData()
  const [scheduleState, setScheduleState] = useState<SchedulePreviewState>({
    requestKey: null,
    plan: null,
    error: null,
  })
  const [selectedFileIndex, setSelectedFileIndex] = useState(0)
  const [masterPassword, setMasterPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [rememberKey, setRememberKey] = useState(false)
  const [localError, setLocalError] = useState<string | null>(null)
  const schedulePreviewKey = snapshot
    ? `${snapshot.config.dueAfterHours}:${snapshot.config.scheduleCheckIntervalHours}`
    : null

  useEffect(() => {
    if (!schedulePreviewKey) {
      return
    }

    let cancelled = false
    const loadSchedule = async () => {
      try {
        const plan = await backend.previewSchedule()
        if (cancelled) {
          return
        }
        setScheduleState({
          requestKey: schedulePreviewKey,
          plan,
          error: null,
        })
        setSelectedFileIndex(0)
      } catch (nextError) {
        if (!cancelled) {
          setScheduleState({
            requestKey: schedulePreviewKey,
            plan: null,
            error:
              nextError instanceof Error
                ? nextError.message
                : 'PathKeep could not preview the native schedule.',
          })
        }
      }
    }

    void loadSchedule()

    return () => {
      cancelled = true
    }
  }, [schedulePreviewKey])

  if (loading && !snapshot) {
    return (
      <section
        className="page-shell onboarding-page"
        data-testid="onboarding-page"
      >
        <LoadingState label="Loading onboarding decisions" />
      </section>
    )
  }

  if (error && !snapshot) {
    return (
      <section
        className="page-shell onboarding-page"
        data-testid="onboarding-page"
      >
        <ErrorState
          title="Onboarding data is unavailable"
          description={error}
        />
      </section>
    )
  }

  if (!snapshot) {
    return (
      <section
        className="page-shell onboarding-page"
        data-testid="onboarding-page"
      >
        <EmptyState
          description="PathKeep needs a local app snapshot before it can preview storage, profiles, and security choices."
          eyebrow="ONBOARDING"
          title="Archive decisions are not ready yet"
        />
      </section>
    )
  }

  const chromiumProfiles = snapshot.browserProfiles.filter(
    (profile) => profile.browserFamily === 'chromium' && profile.historyExists,
  )
  const unsupportedProfiles = snapshot.browserProfiles.filter(
    (profile) => profile.browserFamily !== 'chromium' && profile.historyExists,
  )
  const currentConfig = snapshot.config
  const selectedChromiumCount = snapshot.config.selectedProfileIds.filter(
    (profileId) =>
      profileId.startsWith('chrome:') || profileId.startsWith('arc:'),
  ).length
  const scheduleLoading =
    Boolean(schedulePreviewKey) &&
    scheduleState.requestKey !== schedulePreviewKey
  const schedulePlan =
    scheduleState.requestKey === schedulePreviewKey ? scheduleState.plan : null
  const scheduleError =
    scheduleState.requestKey === schedulePreviewKey ? scheduleState.error : null
  const selectedGeneratedFile =
    schedulePlan?.generatedFiles[selectedFileIndex] ?? null

  async function updateConfig(
    updater: (current: typeof currentConfig) => typeof currentConfig,
  ) {
    setLocalError(null)
    await saveConfig(updater(currentConfig))
  }

  async function handleFirstBackup() {
    setLocalError(null)

    if (selectedChromiumCount === 0) {
      setLocalError(
        'Select at least one Chromium profile before the first backup.',
      )
      return
    }

    const encrypted = currentConfig.archiveMode === 'Encrypted'
    if (encrypted) {
      if (!masterPassword.trim()) {
        setLocalError(
          'Encrypted mode needs a master password before initialization.',
        )
        return
      }

      if (masterPassword !== confirmPassword) {
        setLocalError(
          'The confirmation password does not match the master password.',
        )
        return
      }
    }

    try {
      if (!currentConfig.initialized) {
        await initializeArchive(
          currentConfig,
          encrypted ? masterPassword : null,
        )

        if (encrypted && rememberKey) {
          await backend.keyringStoreDatabaseKey(masterPassword)
        }
      }

      await runBackup()
      void navigate('/')
    } catch (nextError) {
      setLocalError(
        nextError instanceof Error
          ? nextError.message
          : 'PathKeep could not finish the first backup flow.',
      )
    }
  }

  return (
    <section
      className="page-shell onboarding-page"
      data-testid="onboarding-page"
    >
      <div className="onboarding-hero shell-panel shell-panel--accent">
        <div className="panel-header">
          <span className="panel-title">ONBOARDING / SETUP</span>
          <span className="panel-action">Preview, manual, then execute</span>
        </div>
        <div className="panel-body onboarding-hero__body">
          <div>
            <h2>Onboarding / Setup</h2>
            <p>
              Set the first archive boundary before PathKeep touches history.
              Storage path, browser scope, security mode, and schedule artifacts
              all stay inspectable before the first mutating run.
            </p>
          </div>
          <div className="onboarding-summary-grid">
            <article className="pme-column">
              <span className="mono-kicker">Storage</span>
              <p>{snapshot.directories.archiveDatabasePath}</p>
            </article>
            <article className="pme-column">
              <span className="mono-kicker">Profiles ready</span>
              <p>
                {selectedChromiumCount} Chromium profiles selected for backup.
              </p>
            </article>
            <article className="pme-column">
              <span className="mono-kicker">Security</span>
              <p>
                {snapshot.config.archiveMode === 'Encrypted'
                  ? 'Encrypted archive with an explicit unlock step.'
                  : 'Plaintext archive with no key material required.'}
              </p>
            </article>
          </div>
        </div>
      </div>

      <div className="onboarding-workflow-grid">
        <section className="shell-panel">
          <div className="panel-header">
            <span className="panel-title">1 · STORAGE</span>
            <span className="panel-action">Local-first by default</span>
          </div>
          <div className="panel-body stack-list">
            <article className="list-item">
              <strong>Archive database</strong>
              <span className="mono-support">
                {snapshot.directories.archiveDatabasePath}
              </span>
            </article>
            <article className="list-item">
              <strong>Audit manifests</strong>
              <span className="mono-support">
                {snapshot.directories.manifestsDir}
              </span>
            </article>
            <article className="list-item">
              <strong>Snapshot checkpoints</strong>
              <span className="mono-support">
                {snapshot.directories.rawSnapshotsDir}
              </span>
            </article>
            <article className="list-item">
              <strong>Current archive footprint</strong>
              <span className="mono-support">
                {formatBytes(snapshot.config.initialized ? 146_800_640 : 0)}
              </span>
            </article>
          </div>
        </section>

        <section className="shell-panel">
          <div className="panel-header">
            <span className="panel-title">2 · BROWSER DETECTION</span>
            <span className="panel-action">
              Profiles are the backup boundary
            </span>
          </div>
          <div className="panel-body stack-list">
            {chromiumProfiles.map((profile) => {
              const selected = snapshot.config.selectedProfileIds.includes(
                profile.profileId,
              )

              return (
                <label
                  key={profile.profileId}
                  className="choice-row"
                  data-active={selected}
                >
                  <input
                    checked={selected}
                    type="checkbox"
                    onChange={() => {
                      const nextSelected = selected
                        ? snapshot.config.selectedProfileIds.filter(
                            (value) => value !== profile.profileId,
                          )
                        : [
                            ...snapshot.config.selectedProfileIds,
                            profile.profileId,
                          ]

                      void updateConfig((current) => ({
                        ...current,
                        selectedProfileIds: nextSelected,
                      }))
                    }}
                  />
                  <div>
                    <strong>{profile.browserName}</strong>
                    <span className="mono-support">
                      {profile.profileName} · {profile.profileId}
                    </span>
                    <p>
                      {profile.userName ?? 'Local profile'} ·{' '}
                      {profile.browserVersion ?? 'Version unknown'}
                    </p>
                  </div>
                </label>
              )
            })}
            {unsupportedProfiles.length > 0 ? (
              <PermissionGate
                detail={`Detected ${unsupportedProfiles.length} non-Chromium source${
                  unsupportedProfiles.length > 1 ? 's' : ''
                }. They stay visible here, but PathKeep will not back them up during M1.`}
                eyebrow="MANUAL REVIEW"
                title="Firefox and Safari stay preview-only for this milestone"
              />
            ) : null}
          </div>
        </section>
      </div>

      <div className="onboarding-workflow-grid">
        <section className="shell-panel">
          <div className="panel-header">
            <span className="panel-title">3 · SECURITY CHOICE</span>
            <span className="panel-action">No silent high-risk defaults</span>
          </div>
          <div className="panel-body stack-list">
            <label
              className="choice-row"
              data-active={snapshot.config.archiveMode === 'Encrypted'}
            >
              <input
                checked={snapshot.config.archiveMode === 'Encrypted'}
                name="archive-mode"
                type="radio"
                onChange={() => {
                  void updateConfig((current) => ({
                    ...current,
                    archiveMode: 'Encrypted',
                  }))
                }}
              />
              <div>
                <strong>Encrypted archive</strong>
                <span className="mono-support">
                  Requires a master password before the first backup.
                </span>
                <p>
                  Forgetting the password means losing access to the archive.
                  The key can optionally live in the native keyring for
                  convenience.
                </p>
              </div>
            </label>
            <label
              className="choice-row"
              data-active={snapshot.config.archiveMode === 'Plaintext'}
            >
              <input
                checked={snapshot.config.archiveMode === 'Plaintext'}
                name="archive-mode"
                type="radio"
                onChange={() => {
                  void updateConfig((current) => ({
                    ...current,
                    archiveMode: 'Plaintext',
                  }))
                }}
              />
              <div>
                <strong>Plaintext archive</strong>
                <span className="mono-support">
                  No password gate. The database stays readable on disk.
                </span>
                <p>
                  Pick this only if the device storage is already protected and
                  the archive path is acceptable in cleartext.
                </p>
              </div>
            </label>

            {snapshot.config.archiveMode === 'Encrypted' ? (
              <div className="security-form-grid">
                <label className="field-stack">
                  <span className="mono-kicker">MASTER PASSWORD</span>
                  <input
                    autoComplete="new-password"
                    type="password"
                    value={masterPassword}
                    onChange={(event) => setMasterPassword(event.target.value)}
                  />
                </label>
                <label className="field-stack">
                  <span className="mono-kicker">CONFIRM PASSWORD</span>
                  <input
                    autoComplete="new-password"
                    type="password"
                    value={confirmPassword}
                    onChange={(event) => setConfirmPassword(event.target.value)}
                  />
                </label>
                <label className="checkbox-row">
                  <input
                    checked={rememberKey}
                    type="checkbox"
                    onChange={(event) => setRememberKey(event.target.checked)}
                  />
                  <span>
                    Store the unlock key in the native keyring after
                    initialization.
                  </span>
                </label>
              </div>
            ) : null}
          </div>
        </section>

        <section className="shell-panel">
          <div className="panel-header">
            <span className="panel-title">4 · SCHEDULE PREVIEW</span>
            <span className="panel-action">Preview only in onboarding</span>
          </div>
          <div className="panel-body stack-list">
            <article className="list-item">
              <strong>Backup interval</strong>
              <div className="segmented-row">
                {dueAfterOptions.map((hours) => (
                  <button
                    key={hours}
                    className={`chip-button ${
                      snapshot.config.dueAfterHours === hours
                        ? 'chip-button--active'
                        : ''
                    }`}
                    type="button"
                    onClick={() => {
                      void updateConfig((current) => ({
                        ...current,
                        dueAfterHours: hours,
                      }))
                    }}
                  >
                    {hours}h
                  </button>
                ))}
              </div>
            </article>
            <article className="list-item">
              <strong>Wake-up cadence</strong>
              <div className="segmented-row">
                {scheduleOptions.map((hours) => (
                  <button
                    key={hours}
                    className={`chip-button ${
                      snapshot.config.scheduleCheckIntervalHours === hours
                        ? 'chip-button--active'
                        : ''
                    }`}
                    type="button"
                    onClick={() => {
                      void updateConfig((current) => ({
                        ...current,
                        scheduleCheckIntervalHours: hours,
                      }))
                    }}
                  >
                    {hours}h
                  </button>
                ))}
              </div>
            </article>

            {scheduleLoading ? (
              <LoadingState label="Rendering native schedule preview" />
            ) : scheduleError ? (
              <ErrorState
                title="Schedule preview is unavailable"
                description={scheduleError}
              />
            ) : schedulePlan ? (
              <>
                <article className="list-item">
                  <strong>{schedulePlan.platform} timer plan</strong>
                  <span className="mono-support">
                    {schedulePlan.manualSteps[0] ??
                      'Manual instructions stay visible before PathKeep applies anything.'}
                  </span>
                </article>
                <div className="generated-file-tabs">
                  {schedulePlan.generatedFiles.map((file, index) => (
                    <button
                      key={file.relativePath}
                      className={`chip-button ${
                        selectedFileIndex === index ? 'chip-button--active' : ''
                      }`}
                      type="button"
                      onClick={() => setSelectedFileIndex(index)}
                    >
                      {file.relativePath}
                    </button>
                  ))}
                </div>
                {selectedGeneratedFile ? (
                  <article className="code-panel">
                    <div className="row-between">
                      <strong>{selectedGeneratedFile.purpose}</strong>
                      <span className="mono-support">
                        {selectedGeneratedFile.relativePath}
                      </span>
                    </div>
                    <pre>{selectedGeneratedFile.contents}</pre>
                  </article>
                ) : null}
              </>
            ) : null}
          </div>
        </section>
      </div>

      <section className="shell-panel shell-panel--accent">
        <div className="panel-header">
          <span className="panel-title">5 · FIRST BACKUP READY</span>
          <span className="panel-action">Manual backup stays explicit</span>
        </div>
        <div className="panel-body onboarding-final-panel">
          <div>
            <h2>
              {snapshot.config.initialized
                ? 'The archive exists. You can run another manual backup now.'
                : 'Initialize the archive and run the first manual backup.'}
            </h2>
            <p>
              PathKeep will only touch the profiles selected above. Native
              schedule installation still stays manual-first after this backup.
            </p>
            {localError ? (
              <p className="inline-error" role="alert">
                {localError}
              </p>
            ) : null}
          </div>
          <div className="utility-block__actions">
            <Link className="ghost-button" to="/">
              Skip for now
            </Link>
            <button
              className="primary-button"
              type="button"
              onClick={() => {
                void handleFirstBackup()
              }}
            >
              {busyAction ?? 'Initialize + run first backup'}
            </button>
          </div>
        </div>
      </section>
    </section>
  )
}
