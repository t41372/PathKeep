import { useApp } from '../../lib/app-context'
import {
  DataRow,
  FieldBlock,
  PathRow,
  Surface,
  ToggleRow,
} from '../../components/ui'
import { languageLabel } from '../../lib/i18n'
import type { LanguagePreference } from '../../lib/types'

const LANGUAGES: LanguagePreference[] = ['system', 'en', 'zh-CN', 'zh-TW']

export function GeneralSettings() {
  const {
    t,
    resolvedLanguage,
    draftConfig,
    directories,
    buildInfo,
    updateConfig,
    persistConfig,
    handleOpenPath,
    copyText,
    runTask,
  } = useApp()

  async function handleSave() {
    await runTask(t('saveSettings'), async () => {
      await persistConfig(draftConfig)
    })
  }

  return (
    <div className="settingsTabContent">
      <section className="pageIntro">
        <h2>{t('settingsGeneral')}</h2>
      </section>

      <Surface
        eyebrow={t('languageLabel')}
        title={t('languageLabel')}
        icon="translate"
      >
        <FieldBlock label={t('languageLabel')}>
          <select
            className="selectInput"
            value={draftConfig.preferredLanguage}
            onChange={(e) =>
              updateConfig({
                preferredLanguage: e.target.value as LanguagePreference,
              })
            }
          >
            {LANGUAGES.map((lang) => (
              <option key={lang} value={lang}>
                {languageLabel(lang, resolvedLanguage)}
              </option>
            ))}
          </select>
        </FieldBlock>
      </Surface>

      <Surface
        eyebrow={t('storagePaths')}
        title={t('storagePaths')}
        icon="folder"
      >
        <PathRow
          label={t('storagePath')}
          value={directories.appRoot}
          onOpen={() => handleOpenPath(directories.appRoot)}
          onCopy={() => copyText(directories.appRoot)}
        />
        <PathRow
          label={t('archiveDatabase')}
          value={directories.archiveDatabasePath}
          onOpen={() => handleOpenPath(directories.archiveDatabasePath)}
          onCopy={() => copyText(directories.archiveDatabasePath)}
        />
        <PathRow
          label={t('auditRepository')}
          value={directories.auditRepoPath}
          onOpen={() => handleOpenPath(directories.auditRepoPath)}
          onCopy={() => copyText(directories.auditRepoPath)}
        />
      </Surface>

      <Surface
        eyebrow={t('appBehavior')}
        title={t('appBehavior')}
        icon="settings"
      >
        <ToggleRow
          label={t('appAutostart')}
          checked={draftConfig.appAutostart}
          onChange={(checked) => updateConfig({ appAutostart: checked })}
        />
      </Surface>

      {buildInfo && (
        <Surface
          eyebrow={t('buildInfoTitle')}
          title={t('buildInfoTitle')}
          icon="info"
        >
          <DataRow label={t('versionLabel')}>{buildInfo.version}</DataRow>
          <DataRow label={t('commitLabel')}>{buildInfo.gitCommitShort}</DataRow>
          <DataRow label={t('dirtyLabel')}>
            {buildInfo.gitDirty ? t('workingTreeDirty') : t('workingTreeClean')}
          </DataRow>
        </Surface>
      )}

      <div className="settingsActions">
        <button className="primaryButton" type="button" onClick={handleSave}>
          {t('saveSettings')}
        </button>
      </div>
    </div>
  )
}
