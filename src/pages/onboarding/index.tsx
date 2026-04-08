import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { BrandMark } from '../../components/brand-mark'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { backend } from '../../lib/backend'
import type { SchedulePlan } from '../../lib/types'

const stepLabels = [
  'Welcome',
  'Browsers',
  'Storage',
  'Security',
  'Schedule',
  'Ready',
]
const dueAfterOptions = [6, 12, 24, 72]

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
  const [step, setStep] = useState(0)
  const [masterPassword, setMasterPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [rememberKey, setRememberKey] = useState(false)
  const [localError, setLocalError] = useState<string | null>(null)
  const [schedulePlan, setSchedulePlan] = useState<SchedulePlan | null>(null)
  const [schedulePreviewLoading, setSchedulePreviewLoading] = useState(false)
  const [schedulePreviewError, setSchedulePreviewError] = useState<
    string | null
  >(null)

  useEffect(() => {
    if (step === 4 && snapshot) {
      let cancelled = false
      setSchedulePreviewLoading(true)
      setSchedulePreviewError(null)
      void backend
        .previewSchedule()
        .then((plan) => {
          if (!cancelled) setSchedulePlan(plan)
        })
        .catch((nextError) => {
          if (!cancelled) {
            setSchedulePlan(null)
            setSchedulePreviewError(
              nextError instanceof Error
                ? nextError.message
                : 'PathKeep could not preview the native schedule yet.',
            )
          }
        })
        .finally(() => {
          if (!cancelled) {
            setSchedulePreviewLoading(false)
          }
        })
      return () => {
        cancelled = true
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step, snapshot?.config.dueAfterHours])

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

  const currentConfig = snapshot.config
  const readableProfiles = snapshot.browserProfiles.filter(
    (p) => p.historyExists,
  )
  const attentionProfiles = snapshot.browserProfiles.filter(
    (p) => !p.historyExists,
  )
  const selectedCount = snapshot.config.selectedProfileIds.filter((id) =>
    snapshot.browserProfiles.some(
      (profile) => profile.profileId === id && profile.historyExists,
    ),
  ).length

  async function updateConfig(
    updater: (c: typeof currentConfig) => typeof currentConfig,
  ) {
    setLocalError(null)
    await saveConfig(updater(currentConfig))
  }

  async function handleFinish() {
    setLocalError(null)
    if (selectedCount === 0) {
      setLocalError(
        'Select at least one readable browser profile before the first backup.',
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
    } catch (e) {
      setLocalError(
        e instanceof Error
          ? e.message
          : 'PathKeep could not finish the first backup flow.',
      )
    }
  }

  function browserIconClass(profileId: string) {
    if (profileId.startsWith('chrome:')) return 'chrome'
    if (profileId.startsWith('arc:')) return 'arc'
    if (profileId.startsWith('firefox:')) return 'firefox'
    if (profileId.startsWith('safari:')) return 'safari'
    return ''
  }

  function browserIconLetter(profileId: string) {
    if (profileId.startsWith('chrome:')) return 'C'
    if (profileId.startsWith('arc:')) return 'A'
    if (profileId.startsWith('firefox:')) return 'F'
    if (profileId.startsWith('safari:')) return 'S'
    return '?'
  }

  return (
    <section data-testid="onboarding-page">
      {/* Stepper Bar */}
      {step > 0 && (
        <div className="onboarding-stepper">
          <div className="stepper-track">
            {stepLabels.map((label, i) => (
              <div key={label} style={{ display: 'contents' }}>
                <div
                  className={`stepper-step ${i < step ? 'completed' : ''} ${i === step ? 'active' : ''} ${i < step ? 'clickable' : ''}`}
                  onClick={() => {
                    if (i < step) setStep(i)
                  }}
                >
                  <div className="stepper-dot">
                    <span className="stepper-check">✓</span>
                    <span className="stepper-num">{i}</span>
                  </div>
                  <span className="stepper-label">{label}</span>
                </div>
                {i < stepLabels.length - 1 && (
                  <div
                    className={`stepper-line ${i < step ? 'completed' : ''} ${i === step - 1 ? 'active' : ''}`}
                  />
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* STEP 0: WELCOME */}
      {step === 0 && (
        <div className="welcome-hero">
          <div className="welcome-logo">
            <BrandMark alt="" />
          </div>
          <h1 className="welcome-title">PATHKEEP</h1>
          <p className="welcome-version mono">
            v0.1.0-alpha · Tauri desktop app
          </p>
          <p className="welcome-tagline">
            Your browsing history is yours.
            <br />
            Archive it, search it, understand it.
          </p>

          <div className="welcome-features">
            <div className="welcome-feature">
              <div className="feature-icon">↓</div>
              <div className="feature-text">
                <div className="feature-title">AUTOMATIC BACKUP</div>
                <div className="feature-desc">
                  Incrementally back up Chromium, Firefox, and Safari history
                  into a local SQLite archive. Safari may still require Full
                  Disk Access before PathKeep can stage `History.db`.
                </div>
              </div>
            </div>
            <div className="welcome-feature">
              <div className="feature-icon">◎</div>
              <div className="feature-text">
                <div className="feature-title">FULL-TEXT SEARCH</div>
                <div className="feature-desc">
                  FTS5-powered search across millions of records. Find any page
                  you ever visited, even years later.
                </div>
              </div>
            </div>
            <div className="welcome-feature">
              <div className="feature-icon">◈</div>
              <div className="feature-text">
                <div className="feature-title">INTELLIGENT INSIGHTS</div>
                <div className="feature-desc">
                  Intelligence is optional and comes after the archive
                  foundation. M1 focuses on backup, audit, search, export, and
                  trustworthy local operation.
                </div>
              </div>
            </div>
          </div>

          <div className="welcome-trust">
            <div className="trust-item">
              <span className="trust-icon">⊘</span>
              <span>Local-first — data never leaves your machine</span>
            </div>
            <div className="trust-item">
              <span className="trust-icon">⊞</span>
              <span>Open-source — GPL v3 licensed, audit the code</span>
            </div>
            <div className="trust-item">
              <span className="trust-icon">⚙</span>
              <span>Built with Tauri + Rust + SQLite</span>
            </div>
          </div>

          <button
            className="btn-primary btn-lg"
            type="button"
            onClick={() => setStep(1)}
          >
            Begin Setup →
          </button>
        </div>
      )}

      {/* STEP 1: BROWSER DETECTION */}
      {step === 1 && (
        <div className="ob-panel-container">
          <div className="ob-header">
            <div className="crosshair-mark">+</div>
            <h2 className="ob-title">Browser Detection</h2>
            <p className="ob-desc">
              We scanned your system and found the following browser profiles.
              Select which ones to include in automatic backups.
            </p>
          </div>

          <div className="ob-scan-status">
            <div className="status-dot status-ok" />
            <span className="mono">
              Scan complete · {snapshot.browserProfiles.length} profiles
              detected · {selectedCount} selected for backup
            </span>
          </div>

          <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
            <div className="panel-header">
              <span className="panel-title">DETECTED PROFILES</span>
              <span className="panel-action">
                {snapshot.browserProfiles.length} found
              </span>
            </div>
            <div className="panel-body" style={{ padding: 0 }}>
              <div className="profile-list">
                {[...readableProfiles, ...attentionProfiles].map((profile) => {
                  const selected = snapshot.config.selectedProfileIds.includes(
                    profile.profileId,
                  )
                  return (
                    <div
                      key={profile.profileId}
                      className="profile-item"
                      onClick={() => {
                        const nextSelected = selected
                          ? snapshot.config.selectedProfileIds.filter(
                              (v) => v !== profile.profileId,
                            )
                          : [
                              ...snapshot.config.selectedProfileIds,
                              profile.profileId,
                            ]
                        void updateConfig((c) => ({
                          ...c,
                          selectedProfileIds: nextSelected,
                        }))
                      }}
                      style={{ cursor: 'pointer' }}
                    >
                      <div className={`checkbox ${selected ? 'active' : ''}`}>
                        {selected ? '✓' : ''}
                      </div>
                      <div
                        className={`browser-icon ${browserIconClass(profile.profileId)}`}
                      >
                        {browserIconLetter(profile.profileId)}
                      </div>
                      <div className="profile-info">
                        <div className="profile-name">
                          {profile.browserName} / {profile.profileName}
                        </div>
                        <div className="profile-path dim mono">
                          {profile.profilePath}
                        </div>
                      </div>
                      <div className="profile-detection">
                        <span
                          className={`status-badge ${
                            profile.historyExists
                              ? 'status-completed'
                              : 'status-pending'
                          }`}
                        >
                          {profile.historyExists
                            ? 'HISTORY FOUND'
                            : 'ACTION REQUIRED'}
                        </span>
                        <span
                          className="mono dim"
                          style={{
                            fontSize: '10px',
                            marginTop: '2px',
                            display: 'block',
                          }}
                        >
                          {profile.historyExists
                            ? `${profile.browserVersion ?? 'Version unknown'} · ${profile.browserFamily} engine`
                            : profile.browserFamily === 'safari'
                              ? 'Grant Full Disk Access so PathKeep can read Safari History.db.'
                              : `PathKeep could not read ${profile.historyFileName} at this location yet.`}
                        </span>
                      </div>
                    </div>
                  )
                })}
              </div>
            </div>
          </div>

          {snapshot.browserProfiles.some(
            (profile) => profile.browserFamily !== 'chromium',
          ) && (
            <div className="ob-info-box">
              <span className="info-icon">ℹ</span>
              <span className="info-text">
                <strong>Firefox</strong> now lands in the same backup flow as
                Chromium. <strong>Safari</strong> supports baseline history
                ingest, but the file may stay unreadable until Full Disk Access
                is granted on macOS.
              </span>
            </div>
          )}

          <div className="ob-actions">
            <button
              className="btn-secondary"
              type="button"
              onClick={() => setStep(0)}
            >
              ← Back
            </button>
            <button
              className="btn-primary"
              type="button"
              onClick={() => setStep(2)}
            >
              Continue →
            </button>
          </div>
        </div>
      )}

      {/* STEP 2: STORAGE */}
      {step === 2 && (
        <div className="ob-panel-container">
          <div className="ob-header">
            <div className="crosshair-mark">+</div>
            <h2 className="ob-title">Storage Location</h2>
            <p className="ob-desc">
              PathKeep stores everything locally. Here&apos;s the directory
              layout the archive will use.
            </p>
          </div>

          <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
            <div className="panel-header">
              <span className="panel-title">ARCHIVE ROOT</span>
              <span className="panel-action">Local-first</span>
            </div>
            <div className="panel-body">
              <div
                className="storage-path-display"
                style={{ marginBottom: 'var(--space-4)' }}
              >
                <span className="storage-path-field">
                  {snapshot.directories.appRoot}
                </span>
              </div>

              <div className="dir-tree">
                <div className="dir-item">
                  <span className="dir-icon">📁</span>
                  <span>{snapshot.directories.appRoot}</span>
                </div>
                <div className="dir-item indent">
                  <span className="dir-icon">🗄</span>
                  <span>archive/history-vault.sqlite</span>
                </div>
                <div className="dir-item indent">
                  <span className="dir-icon">📋</span>
                  <span>audit/manifests/</span>
                </div>
                <div className="dir-item indent">
                  <span className="dir-icon">📸</span>
                  <span>raw-snapshots/</span>
                </div>
                <div className="dir-item indent">
                  <span className="dir-icon">📤</span>
                  <span>exports/</span>
                </div>
                <div className="dir-item indent">
                  <span className="dir-icon">⚙</span>
                  <span>config.json</span>
                </div>
              </div>
            </div>
          </div>

          <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
            <div className="panel-header">
              <span className="panel-title">SIZE ESTIMATES</span>
              <span className="panel-action">Projected</span>
            </div>
            <div className="panel-body">
              <div className="estimate-grid">
                <div className="estimate-item">
                  <span className="estimate-label">Archive database</span>
                  <span className="estimate-value">~140 MB</span>
                </div>
                <div className="estimate-item">
                  <span className="estimate-label">Manifest ledger</span>
                  <span className="estimate-value">~375 KB</span>
                </div>
                <div className="estimate-item">
                  <span className="estimate-label">Raw snapshots</span>
                  <span className="estimate-value">~1.2 MB</span>
                </div>
                <div className="estimate-item highlight">
                  <span className="estimate-label">Total estimated</span>
                  <span className="estimate-value">~142 MB</span>
                </div>
              </div>
            </div>
          </div>

          <div className="ob-actions">
            <button
              className="btn-secondary"
              type="button"
              onClick={() => setStep(1)}
            >
              ← Back
            </button>
            <button
              className="btn-primary"
              type="button"
              onClick={() => setStep(3)}
            >
              Continue →
            </button>
          </div>
        </div>
      )}

      {/* STEP 3: SECURITY */}
      {step === 3 && (
        <div className="ob-panel-container">
          <div className="ob-header">
            <div className="crosshair-mark">+</div>
            <h2 className="ob-title">Security</h2>
            <p className="ob-desc">
              Choose whether to encrypt the archive at rest. This decision is
              visible before PathKeep writes any data.
            </p>
          </div>

          <div
            aria-label="Archive encryption mode"
            className="security-options"
            role="radiogroup"
          >
            <div
              className={`security-option ${currentConfig.archiveMode === 'Encrypted' ? 'selected' : ''}`}
            >
              <button
                aria-checked={currentConfig.archiveMode === 'Encrypted'}
                aria-label="Select encrypted mode"
                className="security-option-trigger"
                disabled={busyAction !== null}
                role="radio"
                type="button"
                onClick={() =>
                  void updateConfig((c) => ({ ...c, archiveMode: 'Encrypted' }))
                }
              >
                <div className="option-header">
                  <div
                    className={`option-radio ${currentConfig.archiveMode === 'Encrypted' ? 'selected' : ''}`}
                  />
                  <div className="option-title-row">
                    <span className="option-title">🔒 Encrypted</span>
                    <span className="tag tag-sm tag-backup">RECOMMENDED</span>
                  </div>
                </div>
              </button>
              <div className="option-body">
                <p className="option-desc">
                  SQLCipher AES-256 encryption at rest. Requires a master
                  password on each unlock.
                </p>
                {currentConfig.archiveMode === 'Encrypted' && (
                  <div className="security-form">
                    <div className="form-field">
                      <label className="field-label">MASTER PASSWORD</label>
                      <input
                        className="form-input"
                        type="password"
                        autoComplete="new-password"
                        value={masterPassword}
                        onChange={(e) => setMasterPassword(e.target.value)}
                        placeholder="Enter master password"
                      />
                    </div>
                    <div className="form-field">
                      <label className="field-label">CONFIRM PASSWORD</label>
                      <input
                        className="form-input"
                        type="password"
                        autoComplete="new-password"
                        value={confirmPassword}
                        onChange={(e) => setConfirmPassword(e.target.value)}
                        placeholder="Confirm password"
                      />
                    </div>
                    <label className="form-checkbox-row">
                      <input
                        type="checkbox"
                        checked={rememberKey}
                        onChange={(e) => setRememberKey(e.target.checked)}
                      />
                      <span>Store in native keyring for auto-unlock</span>
                    </label>
                  </div>
                )}
              </div>
            </div>

            <div
              className={`security-option ${currentConfig.archiveMode === 'Plaintext' ? 'selected' : ''}`}
            >
              <button
                aria-checked={currentConfig.archiveMode === 'Plaintext'}
                aria-label="Select plaintext mode"
                className="security-option-trigger"
                disabled={busyAction !== null}
                role="radio"
                type="button"
                onClick={() =>
                  void updateConfig((c) => ({ ...c, archiveMode: 'Plaintext' }))
                }
              >
                <div className="option-header">
                  <div
                    className={`option-radio ${currentConfig.archiveMode === 'Plaintext' ? 'selected' : ''}`}
                  />
                  <div className="option-title-row">
                    <span className="option-title">📄 Plaintext</span>
                  </div>
                </div>
              </button>
              <div className="option-body">
                <p className="option-desc">
                  No encryption. The database stays readable on disk. Choose
                  only if your system storage is already protected.
                </p>
                {currentConfig.archiveMode === 'Plaintext' && (
                  <div className="plaintext-tradeoffs">
                    <div className="tradeoff-row tradeoff-pro">
                      ✓ No password to remember
                    </div>
                    <div className="tradeoff-row tradeoff-pro">
                      ✓ Easier to inspect with external tools
                    </div>
                    <div className="tradeoff-row tradeoff-con">
                      ✗ Browsing history visible to anyone with file access
                    </div>
                    <div className="tradeoff-row tradeoff-con">
                      ✗ Cannot upgrade to encrypted later without rekey
                    </div>
                  </div>
                )}
              </div>
            </div>
          </div>

          <div className="warning-box">
            <span className="warning-icon">⚠</span>
            <span className="warning-text">
              <strong>Password recovery is not possible.</strong> If you choose
              encrypted mode and forget the master password, the archive data
              cannot be recovered. PathKeep does not store passwords or have any
              backdoor.
            </span>
          </div>

          <div className="ob-actions">
            <button
              className="btn-secondary"
              type="button"
              onClick={() => setStep(2)}
            >
              ← Back
            </button>
            <button
              className="btn-primary"
              type="button"
              onClick={() => setStep(4)}
            >
              Continue →
            </button>
          </div>
        </div>
      )}

      {/* STEP 4: SCHEDULE */}
      {step === 4 && (
        <div className="ob-panel-container">
          <div className="ob-header">
            <div className="crosshair-mark">+</div>
            <h2 className="ob-title">Backup Schedule</h2>
            <p className="ob-desc">
              How often should PathKeep check for new browsing history? This
              controls the due-after interval.
            </p>
          </div>

          <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
            <div className="panel-header">
              <span className="panel-title">BACKUP INTERVAL</span>
              <span className="panel-action">Select hours between checks</span>
            </div>
            <div className="panel-body">
              <div className="interval-chips">
                {dueAfterOptions.map((hours) => (
                  <button
                    key={hours}
                    className={`interval-chip ${currentConfig.dueAfterHours === hours ? 'active' : ''}`}
                    type="button"
                    onClick={() =>
                      void updateConfig((c) => ({ ...c, dueAfterHours: hours }))
                    }
                  >
                    {hours}h
                  </button>
                ))}
              </div>
            </div>
          </div>

          {schedulePreviewLoading ? (
            <LoadingState label="Previewing native schedule artifacts" />
          ) : null}

          {schedulePreviewError ? (
            <p className="inline-error" role="alert">
              {schedulePreviewError}
            </p>
          ) : null}

          {schedulePlan && (
            <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
              <div className="panel-header">
                <span className="panel-title">SCHEDULE PREVIEW</span>
                <span className="panel-action">{schedulePlan.platform}</span>
              </div>
              <div className="panel-body">
                <div className="manual-steps">
                  {schedulePlan.manualSteps.map((s, i) => (
                    <div key={i} className="manual-step">
                      <span className="step-num-inline">{i + 1}.</span>
                      <span>{s}</span>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )}

          <div className="ob-actions">
            <button
              className="btn-secondary"
              type="button"
              onClick={() => setStep(3)}
            >
              ← Back
            </button>
            <button
              className="btn-primary"
              type="button"
              onClick={() => setStep(5)}
            >
              Continue →
            </button>
          </div>
        </div>
      )}

      {/* STEP 5: READY */}
      {step === 5 && (
        <div className="ob-panel-container">
          <div className="ob-header">
            <div className="crosshair-mark">+</div>
            <h2 className="ob-title">Ready to Initialize</h2>
            <p className="ob-desc">
              Review your configuration below. When ready, initialize the
              archive and run the first backup.
            </p>
          </div>

          <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
            <div className="panel-header">
              <span className="panel-title">CONFIGURATION SUMMARY</span>
              <span className="panel-action">Review before init</span>
            </div>
            <div className="panel-body">
              <div className="summary-config">
                <div className="config-row">
                  <span className="config-label">Profiles</span>
                  <span className="config-value">
                    {selectedCount} readable profiles selected
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">Storage</span>
                  <span className="config-value">
                    {snapshot.directories.appRoot}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">Encryption</span>
                  <span className="config-value">
                    {currentConfig.archiveMode}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">Schedule</span>
                  <span className="config-value">
                    Every {currentConfig.dueAfterHours}h
                  </span>
                </div>
              </div>
            </div>
          </div>

          <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
            <div className="panel-header">
              <span className="panel-title">INITIALIZATION STEPS</span>
              <span className="panel-action">What happens next</span>
            </div>
            <div className="panel-body">
              <div className="init-steps">
                <div className="init-step">
                  <span className="init-num">1.</span>
                  <div className="init-info">
                    <span className="init-action">
                      Create the archive database
                    </span>
                    <span className="init-detail">
                      SQLite +{' '}
                      {currentConfig.archiveMode === 'Encrypted'
                        ? 'SQLCipher encryption'
                        : 'plaintext mode'}
                    </span>
                  </div>
                </div>
                <div className="init-step">
                  <span className="init-num">2.</span>
                  <div className="init-info">
                    <span className="init-action">
                      Write the config and audit manifests
                    </span>
                    <span className="init-detail">
                      First manifest starts the hash chain
                    </span>
                  </div>
                </div>
                <div className="init-step">
                  <span className="init-num">3.</span>
                  <div className="init-info">
                    <span className="init-action">Run the first backup</span>
                    <span className="init-detail">
                      Ingest history from {selectedCount} selected profile
                      {selectedCount !== 1 ? 's' : ''}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>

          {localError && (
            <p className="inline-error" role="alert">
              {localError}
            </p>
          )}

          <div className="ob-actions">
            <button
              className="btn-secondary"
              type="button"
              onClick={() => setStep(4)}
            >
              ← Back
            </button>
            <button
              className="btn-primary btn-lg"
              type="button"
              disabled={busyAction !== null}
              onClick={() => {
                void handleFinish()
              }}
            >
              {busyAction ?? 'Initialize + First Backup →'}
            </button>
          </div>
        </div>
      )}
    </section>
  )
}
