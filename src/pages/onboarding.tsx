import { useState } from 'react'
import { useApp } from '../lib/app-context'
import { FieldBlock, Glyph, Surface, ToggleRow } from '../components/ui'
import { BrowserIcon, supportedBrowsers } from '../lib/browser-icons'
import type { ArchiveMode } from '../lib/types'

type OnboardingStep = 'welcome' | 'sources' | 'security' | 'schedule' | 'done'

const STEPS: OnboardingStep[] = [
  'welcome',
  'sources',
  'security',
  'schedule',
  'done',
]

export function OnboardingPage() {
  const { t, snapshot, draftConfig, updateConfig, handleInitialize } = useApp()

  const [step, setStep] = useState<OnboardingStep>('welcome')
  const [masterPassword, setMasterPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [rememberKey, setRememberKey] = useState(false)

  const profiles = snapshot?.browserProfiles ?? []
  const stepIndex = STEPS.indexOf(step)

  function isProfileSelected(profileId: string) {
    return draftConfig.selectedProfileIds.includes(profileId)
  }

  function toggleProfile(profileId: string) {
    const current = draftConfig.selectedProfileIds
    const next = current.includes(profileId)
      ? current.filter((id) => id !== profileId)
      : [...current, profileId]
    updateConfig({ selectedProfileIds: next })
  }

  function goNext() {
    if (stepIndex < STEPS.length - 1) {
      setStep(STEPS[stepIndex + 1])
    }
  }

  function goBack() {
    if (stepIndex > 0) {
      setStep(STEPS[stepIndex - 1])
    }
  }

  async function handleFinish() {
    await handleInitialize(masterPassword, confirmPassword, rememberKey)
  }

  const stepLabels: Record<OnboardingStep, string> = {
    welcome: t('onboardingWelcome'),
    sources: t('onboardingSources'),
    security: t('onboardingSecurity'),
    schedule: t('onboardingSchedule'),
    done: t('onboardingDone'),
  }

  return (
    <div className="onboardingShell">
      {/* Step indicator */}
      <div className="stepIndicator">
        {STEPS.map((s, i) => (
          <div
            key={s}
            className={`stepDot ${i < stepIndex ? 'completed' : ''} ${s === step ? 'current' : ''}`}
          >
            <div className="stepCircle">
              {i < stepIndex ? (
                <Glyph icon="check" filled />
              ) : (
                <span>{i + 1}</span>
              )}
            </div>
            <span className="stepLabel">{stepLabels[s]}</span>
          </div>
        ))}
      </div>

      {/* Step content */}
      <div className="onboardingContent">
        {step === 'welcome' && (
          <Surface
            eyebrow={t('onboardingWelcome')}
            title={t('productName')}
            icon="history"
          >
            <p>{t('setupDescription')}</p>
            <div className="supportedBrowserStrip">
              {supportedBrowsers.map((browser) => (
                <div key={browser.name} className="supportedBrowserChip">
                  <BrowserIcon browserName={browser.name} decorative />
                  <span>{browser.name}</span>
                </div>
              ))}
            </div>
          </Surface>
        )}

        {step === 'sources' && (
          <Surface
            eyebrow={t('onboardingSources')}
            title={t('sourcesDescription')}
            icon="person"
          >
            <div className="profileList">
              {profiles.map((profile) => (
                <label
                  key={profile.profileId}
                  className={`profileRow ${isProfileSelected(profile.profileId) ? 'selected' : ''} ${!profile.historyExists ? 'disabled' : ''}`}
                >
                  <input
                    checked={isProfileSelected(profile.profileId)}
                    className="profileCheckbox"
                    disabled={!profile.historyExists}
                    type="checkbox"
                    onChange={() => toggleProfile(profile.profileId)}
                  />
                  <span className="profileCheckboxVisual" aria-hidden="true">
                    <Glyph filled icon="check" />
                  </span>
                  <div className="profileCardBody">
                    <div className="profileHeaderLine">
                      <span className="profileBrowserMark" aria-hidden="true">
                        <BrowserIcon
                          browserName={profile.browserName}
                          decorative
                        />
                      </span>
                      <div className="profileIdentity">
                        <div className="profileNameStack">
                          <span className="profileName">
                            {profile.profileName}
                          </span>
                          <span className="browserPill">
                            {profile.browserName}
                          </span>
                        </div>
                        <span className="profileId">
                          {profile.userName ?? t('noSignedInUser')}
                        </span>
                      </div>
                    </div>
                  </div>
                </label>
              ))}
            </div>
          </Surface>
        )}

        {step === 'security' && (
          <Surface
            eyebrow={t('onboardingSecurity')}
            title={t('archiveStep')}
            icon="shield"
          >
            <FieldBlock label={t('archiveMode')}>
              <select
                className="selectInput"
                value={draftConfig.archiveMode}
                onChange={(e) =>
                  updateConfig({
                    archiveMode: e.target.value as ArchiveMode,
                  })
                }
              >
                <option value="Encrypted">{t('encrypted')}</option>
                <option value="Plaintext">{t('plaintext')}</option>
              </select>
            </FieldBlock>

            {draftConfig.archiveMode === 'Encrypted' && (
              <>
                <FieldBlock label={t('masterPassword')}>
                  <input
                    className="textInput"
                    type="password"
                    placeholder={t('passwordPlaceholder')}
                    value={masterPassword}
                    onChange={(e) => setMasterPassword(e.target.value)}
                  />
                </FieldBlock>
                <FieldBlock label={t('confirmPassword')}>
                  <input
                    className="textInput"
                    type="password"
                    placeholder={t('passwordPlaceholder')}
                    value={confirmPassword}
                    onChange={(e) => setConfirmPassword(e.target.value)}
                  />
                </FieldBlock>
                <ToggleRow
                  label={t('rememberKey')}
                  checked={rememberKey}
                  onChange={setRememberKey}
                />
                <div className="warningBanner">
                  <Glyph icon="warning" />
                  <div>
                    <strong>{t('encryptionWarningTitle')}</strong>
                    <p>{t('encryptionWarningBody')}</p>
                  </div>
                </div>
              </>
            )}
          </Surface>
        )}

        {step === 'schedule' && (
          <Surface
            eyebrow={t('onboardingSchedule')}
            title={t('scheduleDescription')}
            icon="schedule"
          >
            <FieldBlock label={t('dueAfterHours')}>
              <input
                className="textInput"
                type="number"
                min={1}
                value={draftConfig.dueAfterHours}
                onChange={(e) =>
                  updateConfig({ dueAfterHours: Number(e.target.value) })
                }
              />
            </FieldBlock>
            <ToggleRow
              label={t('captureFavicons')}
              checked={draftConfig.captureFavicons}
              onChange={(checked) => updateConfig({ captureFavicons: checked })}
            />
            <ToggleRow
              label={t('gitAudit')}
              checked={draftConfig.gitEnabled}
              onChange={(checked) => updateConfig({ gitEnabled: checked })}
            />
          </Surface>
        )}

        {step === 'done' && (
          <Surface
            eyebrow={t('onboardingDone')}
            title={t('reviewStep')}
            icon="check_circle"
          >
            <p>{t('reviewDescription')}</p>
            <div className="reviewSummary">
              <div className="reviewItem">
                <span className="fieldLabel">{t('archiveMode')}</span>
                <span>{draftConfig.archiveMode}</span>
              </div>
              <div className="reviewItem">
                <span className="fieldLabel">{t('profiles')}</span>
                <span>{draftConfig.selectedProfileIds.length}</span>
              </div>
              <div className="reviewItem">
                <span className="fieldLabel">{t('dueAfterHours')}</span>
                <span>{draftConfig.dueAfterHours}h</span>
              </div>
            </div>
          </Surface>
        )}
      </div>

      {/* Navigation buttons */}
      <div className="onboardingNav">
        {stepIndex > 0 && (
          <button className="secondaryButton" type="button" onClick={goBack}>
            <Glyph icon="arrow_back" />
            {t('backButton') ?? 'Back'}
          </button>
        )}
        <div className="onboardingNavSpacer" />
        {step === 'done' ? (
          <button
            className="primaryButton"
            type="button"
            onClick={handleFinish}
          >
            <Glyph icon="rocket_launch" />
            {t('createArchive')}
          </button>
        ) : (
          <button className="primaryButton" type="button" onClick={goNext}>
            {stepLabels[STEPS[stepIndex + 1]] ?? t('onboardingDone')}
            <Glyph icon="arrow_forward" />
          </button>
        )}
      </div>
    </div>
  )
}
