import { useApp } from '../../lib/app-context'
import { Glyph, Surface, ToggleRow } from '../../components/ui'
import { BrowserIcon, supportedBrowsers } from '../../lib/browser-icons'

export function SourcesSettings() {
  const { t, snapshot, draftConfig, updateConfig, persistConfig, runTask } =
    useApp()

  const profiles = snapshot?.browserProfiles ?? []

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

  async function handleSave() {
    await runTask(t('saveSettings'), async () => {
      await persistConfig(draftConfig)
    })
  }

  return (
    <div className="settingsTabContent">
      <section className="pageIntro">
        <h2>{t('settingsSources')}</h2>
        <p className="muted">{t('sourcesDescription')}</p>
      </section>

      {/* Supported browsers */}
      <Surface
        eyebrow={t('supportedBrowsersTitle')}
        title={t('supportedBrowsersTitle')}
        icon="web"
      >
        <div className="supportedBrowserStrip">
          {supportedBrowsers.map((browser) => (
            <div key={browser.name} className="supportedBrowserChip">
              <BrowserIcon browserName={browser.name} decorative />
              <span>{browser.name}</span>
            </div>
          ))}
        </div>
      </Surface>

      {/* Profile selector */}
      <Surface
        eyebrow={t('sourcesStep')}
        title={t('sourcesStep')}
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
                    <BrowserIcon browserName={profile.browserName} decorative />
                  </span>
                  <div className="profileIdentity">
                    <div className="profileNameStack">
                      <span className="profileName">{profile.profileName}</span>
                      <span className="browserPill">{profile.browserName}</span>
                    </div>
                    <span className="profileId">{profile.profileId}</span>
                  </div>
                </div>
                <div className="profileMetaGrid">
                  <div className="profileMetaItem">
                    <span className="profileMetaLabel">
                      {t('accountLabel')}
                    </span>
                    <span className="profileMetaValue">
                      {profile.userName ?? t('noSignedInUser')}
                    </span>
                  </div>
                  <div className="profileMetaItem">
                    <span className="profileMetaLabel">{t('statusLabel')}</span>
                    <span className="profileMetaValue">
                      {profile.historyExists
                        ? t('historyDetected')
                        : t('historyMissing')}
                    </span>
                  </div>
                </div>
              </div>
            </label>
          ))}
        </div>
      </Surface>

      {/* Archive settings */}
      <Surface
        eyebrow={t('archiveStep')}
        title={t('archiveStep')}
        icon="archive"
      >
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

      <div className="settingsActions">
        <button className="primaryButton" type="button" onClick={handleSave}>
          {t('saveSettings')}
        </button>
      </div>
    </div>
  )
}
