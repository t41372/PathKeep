import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { BrandMark } from '../../components/brand-mark'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { useI18n } from '../../lib/i18n'
import { backend } from '../../lib/backend'
import { browserRetentionMeta } from '../../lib/browser-retention'
import { formatBytes } from '../../lib/format'
import { estimateOnboardingStorage } from '../../lib/onboarding-estimates'
import type { SchedulePlan } from '../../lib/types'

const stepKeys = [
  'stepWelcome',
  'stepBrowsers',
  'stepStorage',
  'stepSecurity',
  'stepSchedule',
  'stepReady',
] as const
const dueAfterOptions = [6, 12, 24, 72]

export function OnboardingPage() {
  const navigate = useNavigate()
  const {
    buildInfo,
    busyAction,
    error,
    loading,
    saveConfig,
    initializeArchive,
    runBackup,
    snapshot,
  } = useShellData()
  const { language, t, ns } = useI18n('onboarding')
  const commonT = ns('common')
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
                : t('schedulePreviewFallbackError'),
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
        <LoadingState label={t('loadingDecisions')} />
      </section>
    )
  }

  if (error && !snapshot) {
    return (
      <section
        className="page-shell onboarding-page"
        data-testid="onboarding-page"
      >
        <ErrorState title={t('errorTitle')} description={error} />
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
          description={t('emptyDescription')}
          eyebrow={t('emptyEyebrow')}
          title={t('emptyTitle')}
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
  const storageEstimate = estimateOnboardingStorage(
    snapshot.browserProfiles,
    snapshot.config.selectedProfileIds,
  )

  function handleSecurityCardClick(
    mode: 'Encrypted' | 'Plaintext',
    target: EventTarget | null,
  ) {
    const element = target instanceof HTMLElement ? target : null
    if (element?.closest('button, input, select, textarea, a, label')) {
      return
    }
    void updateConfig((config) => ({ ...config, archiveMode: mode }))
  }

  async function updateConfig(
    updater: (c: typeof currentConfig) => typeof currentConfig,
  ) {
    setLocalError(null)
    await saveConfig(updater(currentConfig))
  }

  async function handleFinish() {
    setLocalError(null)
    if (selectedCount === 0) {
      setLocalError(t('errorSelectProfile'))
      return
    }
    const encrypted = currentConfig.archiveMode === 'Encrypted'
    if (encrypted) {
      if (!masterPassword.trim()) {
        setLocalError(t('errorNeedPassword'))
        return
      }
      if (masterPassword !== confirmPassword) {
        setLocalError(t('errorPasswordMismatch'))
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
      setLocalError(e instanceof Error ? e.message : t('errorFinishFailed'))
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
            {stepKeys.map((key, i) => (
              <div key={key} style={{ display: 'contents' }}>
                <button
                  type="button"
                  className={`stepper-step ${i < step ? 'completed' : ''} ${i === step ? 'active' : ''} ${i < step ? 'clickable' : ''}`}
                  aria-current={i === step ? 'step' : undefined}
                  aria-label={`${t(key)}${i < step ? ' ✓' : ''}`}
                  disabled={i > step}
                  onClick={() => {
                    if (i < step) setStep(i)
                  }}
                >
                  <div className="stepper-dot">
                    <span className="stepper-check">✓</span>
                    <span className="stepper-num">{i + 1}</span>
                  </div>
                  <span className="stepper-label">{t(key)}</span>
                </button>
                {i < stepKeys.length - 1 && (
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
            {t('versionLine', {
              version: buildInfo?.version ?? 'preview',
            })}
          </p>
          <p className="welcome-tagline">
            {t('welcomeTagline1')}
            <br />
            {t('welcomeTagline2')}
          </p>

          <div className="welcome-features">
            <div className="welcome-feature">
              <div className="feature-icon">↓</div>
              <div className="feature-text">
                <div className="feature-title">{t('featureBackupTitle')}</div>
                <div className="feature-desc">{t('featureBackupDesc')}</div>
              </div>
            </div>
            <div className="welcome-feature">
              <div className="feature-icon">◎</div>
              <div className="feature-text">
                <div className="feature-title">{t('featureSearchTitle')}</div>
                <div className="feature-desc">{t('featureSearchDesc')}</div>
              </div>
            </div>
            <div className="welcome-feature">
              <div className="feature-icon">◈</div>
              <div className="feature-text">
                <div className="feature-title">{t('featureInsightsTitle')}</div>
                <div className="feature-desc">{t('featureInsightsDesc')}</div>
              </div>
            </div>
          </div>

          <div className="welcome-trust">
            <div className="trust-item">
              <span className="trust-icon">⊘</span>
              <span>{t('trustLocalFirst')}</span>
            </div>
            <div className="trust-item">
              <span className="trust-icon">⊞</span>
              <span>{t('trustOpenSource')}</span>
            </div>
            <div className="trust-item">
              <span className="trust-icon">⚙</span>
              <span>{t('trustBuiltWith')}</span>
            </div>
          </div>

          <button
            className="btn-primary btn-lg"
            type="button"
            onClick={() => setStep(1)}
          >
            {t('beginSetup')}
          </button>
        </div>
      )}

      {/* STEP 1: BROWSER DETECTION */}
      {step === 1 && (
        <div className="ob-panel-container">
          <div className="ob-header">
            <div className="crosshair-mark">+</div>
            <h2 className="ob-title">{t('browserDetectionTitle')}</h2>
            <p className="ob-desc">{t('browserDetectionDesc')}</p>
          </div>

          <div className="ob-scan-status">
            <div className="status-dot status-ok" />
            <span className="mono">
              {t('scanStatus')
                .replace('{count}', String(snapshot.browserProfiles.length))
                .replace('{selected}', String(selectedCount))}
            </span>
          </div>

          <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
            <div className="panel-header">
              <span className="panel-title">{t('detectedProfiles')}</span>
              <span className="panel-action">
                {t('found').replace(
                  '{count}',
                  String(snapshot.browserProfiles.length),
                )}
              </span>
            </div>
            <div className="panel-body" style={{ padding: 0 }}>
              <div className="profile-list">
                {[...readableProfiles, ...attentionProfiles].map((profile) => {
                  const selected = snapshot.config.selectedProfileIds.includes(
                    profile.profileId,
                  )
                  const retention = browserRetentionMeta(profile, commonT)
                  const toggleProfile = () => {
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
                  }
                  return (
                    <label
                      key={profile.profileId}
                      className="profile-item"
                      style={{ cursor: 'pointer' }}
                    >
                      <input
                        type="checkbox"
                        className="sr-only"
                        checked={selected}
                        onChange={toggleProfile}
                        aria-label={`${profile.browserName} / ${profile.profileName}`}
                      />
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
                            ? t('historyFound')
                            : t('actionRequired')}
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
                            ? t('browserEngineLabel', {
                                version:
                                  profile.browserVersion ?? t('versionUnknown'),
                                engine: profile.browserFamily,
                              })
                            : profile.browserFamily === 'safari'
                              ? t('safariAccessHint')
                              : t('cannotReadHint').replace(
                                  '{fileName}',
                                  profile.historyFileName,
                                )}
                        </span>
                        {profile.historyExists ? (
                          <>
                            <span
                              className="mono dim"
                              style={{
                                fontSize: '10px',
                                marginTop: '2px',
                                display: 'block',
                              }}
                            >
                              {retention.label}
                            </span>
                            <span
                              className="mono dim"
                              style={{
                                fontSize: '10px',
                                marginTop: '2px',
                                display: 'block',
                              }}
                            >
                              {commonT('browserRetentionArchiveBoundary')}
                            </span>
                          </>
                        ) : null}
                      </div>
                    </label>
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
              <span className="info-text">{t('firefoxSafariInfo')}</span>
            </div>
          )}

          <div className="ob-actions">
            <button
              className="btn-secondary"
              type="button"
              onClick={() => setStep(0)}
            >
              {t('backButton')}
            </button>
            <button
              className="btn-primary"
              type="button"
              onClick={() => setStep(2)}
            >
              {t('continueButton')}
            </button>
          </div>
        </div>
      )}

      {/* STEP 2: STORAGE */}
      {step === 2 && (
        <div className="ob-panel-container">
          <div className="ob-header">
            <div className="crosshair-mark">+</div>
            <h2 className="ob-title">{t('storageTitle')}</h2>
            <p className="ob-desc">{t('storageDesc')}</p>
          </div>

          <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
            <div className="panel-header">
              <span className="panel-title">{t('archiveRoot')}</span>
              <span className="panel-action">{t('localFirst')}</span>
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
              <span className="panel-title">{t('sizeEstimates')}</span>
              <span className="panel-action">{t('projected')}</span>
            </div>
            <div className="panel-body">
              <div className="estimate-grid">
                <div className="estimate-item">
                  <span className="estimate-label">
                    {t('estimateArchiveDb')}
                  </span>
                  <span className="estimate-value mono">
                    {formatBytes(storageEstimate.archiveDbBytes, language)}
                  </span>
                </div>
                <div className="estimate-item">
                  <span className="estimate-label">
                    {t('estimateManifest')}
                  </span>
                  <span className="estimate-value mono">
                    {formatBytes(storageEstimate.manifestBytes, language)}
                  </span>
                </div>
                <div className="estimate-item">
                  <span className="estimate-label">
                    {t('estimateSnapshots')}
                  </span>
                  <span className="estimate-value mono">
                    {formatBytes(storageEstimate.snapshotsBytes, language)}
                  </span>
                </div>
                <div className="estimate-item highlight">
                  <span className="estimate-label">{t('estimateTotal')}</span>
                  <span className="estimate-value mono">
                    {formatBytes(storageEstimate.totalBytes, language)}
                  </span>
                </div>
              </div>
              <p
                className="mono-support"
                style={{ marginTop: 'var(--space-3)' }}
              >
                {t('estimateExplanation', {
                  count: storageEstimate.profileCount,
                  source: formatBytes(storageEstimate.sourceBytes, language),
                })}
              </p>
            </div>
          </div>

          <div className="ob-actions">
            <button
              className="btn-secondary"
              type="button"
              onClick={() => setStep(1)}
            >
              {t('backButton')}
            </button>
            <button
              className="btn-primary"
              type="button"
              onClick={() => setStep(3)}
            >
              {t('continueButton')}
            </button>
          </div>
        </div>
      )}

      {/* STEP 3: SECURITY */}
      {step === 3 && (
        <div className="ob-panel-container">
          <div className="ob-header">
            <div className="crosshair-mark">+</div>
            <h2 className="ob-title">{t('securityTitle')}</h2>
            <p className="ob-desc">{t('securityDesc')}</p>
          </div>

          <div
            aria-label={t('encryptionModeLabel')}
            className="security-options"
            role="radiogroup"
          >
            <div
              className={`security-option ${currentConfig.archiveMode === 'Encrypted' ? 'selected' : ''}`}
              onClick={(event) =>
                handleSecurityCardClick('Encrypted', event.target)
              }
            >
              <button
                aria-checked={currentConfig.archiveMode === 'Encrypted'}
                aria-label={t('encryptedSelectLabel')}
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
                    <span className="option-title">
                      🔒 {t('encryptedOption')}
                    </span>
                    <span className="tag tag-sm tag-backup">
                      {t('recommended')}
                    </span>
                  </div>
                </div>
              </button>
              <div className="option-body">
                <p className="option-desc">{t('encryptedDesc')}</p>
                {currentConfig.archiveMode === 'Encrypted' && (
                  <div className="security-form">
                    <div className="form-field">
                      <label className="field-label">
                        {t('masterPasswordLabel')}
                      </label>
                      <input
                        className="form-input"
                        type="password"
                        autoComplete="new-password"
                        value={masterPassword}
                        onChange={(e) => setMasterPassword(e.target.value)}
                        placeholder={t('masterPasswordPlaceholder')}
                      />
                    </div>
                    <div className="form-field">
                      <label className="field-label">
                        {t('confirmPasswordLabel')}
                      </label>
                      <input
                        className="form-input"
                        type="password"
                        autoComplete="new-password"
                        value={confirmPassword}
                        onChange={(e) => setConfirmPassword(e.target.value)}
                        placeholder={t('confirmPasswordPlaceholder')}
                      />
                    </div>
                    <label className="form-checkbox-row">
                      <input
                        type="checkbox"
                        checked={rememberKey}
                        onChange={(e) => setRememberKey(e.target.checked)}
                      />
                      <span>{t('storeInKeyring')}</span>
                    </label>
                  </div>
                )}
              </div>
            </div>

            <div
              className={`security-option ${currentConfig.archiveMode === 'Plaintext' ? 'selected' : ''}`}
              onClick={(event) =>
                handleSecurityCardClick('Plaintext', event.target)
              }
            >
              <button
                aria-checked={currentConfig.archiveMode === 'Plaintext'}
                aria-label={t('plaintextSelectLabel')}
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
                    <span className="option-title">
                      📄 {t('plaintextOption')}
                    </span>
                  </div>
                </div>
              </button>
              <div className="option-body">
                <p className="option-desc">{t('plaintextDesc')}</p>
                {currentConfig.archiveMode === 'Plaintext' && (
                  <div className="plaintext-tradeoffs">
                    <div className="tradeoff-row tradeoff-pro">
                      ✓ {t('tradeoffNoPassword')}
                    </div>
                    <div className="tradeoff-row tradeoff-pro">
                      ✓ {t('tradeoffEasyInspect')}
                    </div>
                    <div className="tradeoff-row tradeoff-con">
                      ✗ {t('tradeoffVisible')}
                    </div>
                    <div className="tradeoff-row tradeoff-con">
                      ✗ {t('tradeoffNoUpgrade')}
                    </div>
                  </div>
                )}
              </div>
            </div>
          </div>

          <div className="warning-box">
            <span className="warning-icon">⚠</span>
            <span className="warning-text">
              <strong>{t('passwordWarningTitle')}</strong>{' '}
              {t('passwordWarningBody')}
            </span>
          </div>

          <div className="ob-actions">
            <button
              className="btn-secondary"
              type="button"
              onClick={() => setStep(2)}
            >
              {t('backButton')}
            </button>
            <button
              className="btn-primary"
              type="button"
              onClick={() => setStep(4)}
            >
              {t('continueButton')}
            </button>
          </div>
        </div>
      )}

      {/* STEP 4: SCHEDULE */}
      {step === 4 && (
        <div className="ob-panel-container">
          <div className="ob-header">
            <div className="crosshair-mark">+</div>
            <h2 className="ob-title">{t('scheduleTitle')}</h2>
            <p className="ob-desc">{t('scheduleDesc')}</p>
          </div>

          <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
            <div className="panel-header">
              <span className="panel-title">{t('backupInterval')}</span>
              <span className="panel-action">{t('selectHours')}</span>
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
                    {t('intervalChipLabel').replace('{hours}', String(hours))}
                  </button>
                ))}
              </div>
            </div>
          </div>

          {schedulePreviewLoading ? (
            <LoadingState label={t('previewingSchedule')} />
          ) : null}

          {schedulePreviewError ? (
            <p className="inline-error" role="alert">
              {schedulePreviewError}
            </p>
          ) : null}

          {schedulePlan && (
            <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
              <div className="panel-header">
                <span className="panel-title">{t('schedulePreview')}</span>
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
              {t('backButton')}
            </button>
            <button
              className="btn-primary"
              type="button"
              onClick={() => setStep(5)}
            >
              {t('continueButton')}
            </button>
          </div>
        </div>
      )}

      {/* STEP 5: READY */}
      {step === 5 && (
        <div className="ob-panel-container">
          <div className="ob-header">
            <div className="crosshair-mark">+</div>
            <h2 className="ob-title">{t('readyTitle')}</h2>
            <p className="ob-desc">{t('readyDesc')}</p>
          </div>

          <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
            <div className="panel-header">
              <span className="panel-title">{t('configSummary')}</span>
              <span className="panel-action">{t('reviewBeforeInit')}</span>
            </div>
            <div className="panel-body">
              <div className="summary-config">
                <div className="config-row">
                  <span className="config-label">{t('configProfiles')}</span>
                  <span className="config-value">
                    {t('configProfilesValue').replace(
                      '{count}',
                      String(selectedCount),
                    )}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">{t('configStorage')}</span>
                  <span className="config-value">
                    {snapshot.directories.appRoot}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">{t('configEncryption')}</span>
                  <span className="config-value">
                    {currentConfig.archiveMode === 'Encrypted'
                      ? commonT('modeEncrypted')
                      : commonT('modePlaintext')}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">{t('configSchedule')}</span>
                  <span className="config-value">
                    {t('configScheduleValue').replace(
                      '{hours}',
                      String(currentConfig.dueAfterHours),
                    )}
                  </span>
                </div>
              </div>
            </div>
          </div>

          <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
            <div className="panel-header">
              <span className="panel-title">{t('initSteps')}</span>
              <span className="panel-action">{t('whatHappensNext')}</span>
            </div>
            <div className="panel-body">
              <div className="init-steps">
                <div className="init-step">
                  <span className="init-num">1.</span>
                  <div className="init-info">
                    <span className="init-action">{t('initStep1Action')}</span>
                    <span className="init-detail">
                      {currentConfig.archiveMode === 'Encrypted'
                        ? t('initStep1DetailEncrypted')
                        : t('initStep1DetailPlaintext')}
                    </span>
                  </div>
                </div>
                <div className="init-step">
                  <span className="init-num">2.</span>
                  <div className="init-info">
                    <span className="init-action">{t('initStep2Action')}</span>
                    <span className="init-detail">{t('initStep2Detail')}</span>
                  </div>
                </div>
                <div className="init-step">
                  <span className="init-num">3.</span>
                  <div className="init-info">
                    <span className="init-action">{t('initStep3Action')}</span>
                    <span className="init-detail">
                      {t('initStep3Detail')
                        .replace('{count}', String(selectedCount))
                        .replace('{plural}', selectedCount !== 1 ? 's' : '')}
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
              {t('backButton')}
            </button>
            <button
              className="btn-primary btn-lg"
              type="button"
              disabled={busyAction !== null}
              onClick={() => {
                void handleFinish()
              }}
            >
              {busyAction ?? t('initButton')}
            </button>
          </div>
        </div>
      )}
    </section>
  )
}
