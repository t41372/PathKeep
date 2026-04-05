import { useState } from 'react'
import { useApp } from '../../lib/app-context'
import {
  DataRow,
  FieldBlock,
  Glyph,
  StatusTag,
  Surface,
  ToggleRow,
} from '../../components/ui'
import { backend } from '../../lib/backend'
import { formatDateTime } from '../../lib/format'

export function RemoteSettings() {
  const {
    t,
    resolvedLanguage,
    draftConfig,
    updateRemoteBackup,
    persistConfig,
    runTask,
    setNotice,
    setError,
  } = useApp()

  const remoteBackup = draftConfig.remoteBackup

  const [s3AccessKeyId, setS3AccessKeyId] = useState('')
  const [s3SecretAccessKey, setS3SecretAccessKey] = useState('')

  async function handleStoreCredentials() {
    if (!s3AccessKeyId.trim() || !s3SecretAccessKey.trim()) {
      setError(t('enterS3Credentials'))
      return
    }
    await runTask(t('saveCredentials'), async () => {
      await backend.storeS3Credentials({
        accessKeyId: s3AccessKeyId.trim(),
        secretAccessKey: s3SecretAccessKey.trim(),
      })
      setS3AccessKeyId('')
      setS3SecretAccessKey('')
      setNotice(t('s3CredentialsStored'))
    })
  }

  async function handleClearCredentials() {
    await runTask(t('clearCredentials'), async () => {
      await backend.clearS3Credentials()
      setS3AccessKeyId('')
      setS3SecretAccessKey('')
      setNotice(t('s3CredentialsCleared'))
    })
  }

  async function handleSave() {
    await runTask(t('saveSettings'), async () => {
      await persistConfig(draftConfig)
    })
  }

  const lastUploaded =
    formatDateTime(remoteBackup.lastUploadedAt, resolvedLanguage) ??
    t('noRemoteUploadYet')

  return (
    <div className="settingsTabContent">
      <section className="pageIntro">
        <h2>{t('settingsRemote')}</h2>
        <p className="muted">{t('remoteBackupDescription')}</p>
      </section>

      <Surface
        eyebrow={t('remoteBackupTitle')}
        title={t('remoteBackupTitle')}
        icon="cloud_upload"
      >
        <ToggleRow
          label={t('remoteBackupEnabled')}
          checked={remoteBackup.enabled}
          onChange={(checked) => updateRemoteBackup({ enabled: checked })}
        />

        {remoteBackup.enabled && (
          <>
            <FieldBlock label={t('s3Bucket')}>
              <input
                className="textInput"
                type="text"
                value={remoteBackup.bucket}
                onChange={(e) => updateRemoteBackup({ bucket: e.target.value })}
              />
            </FieldBlock>
            <FieldBlock label={t('s3Region')}>
              <input
                className="textInput"
                type="text"
                value={remoteBackup.region}
                onChange={(e) => updateRemoteBackup({ region: e.target.value })}
              />
            </FieldBlock>
            <FieldBlock label={t('s3Endpoint')}>
              <input
                className="textInput"
                type="text"
                placeholder={t('s3EndpointPlaceholder')}
                value={remoteBackup.endpoint ?? ''}
                onChange={(e) =>
                  updateRemoteBackup({
                    endpoint: e.target.value || null,
                  })
                }
              />
            </FieldBlock>
            <FieldBlock label={t('s3Prefix')}>
              <input
                className="textInput"
                type="text"
                value={remoteBackup.prefix}
                onChange={(e) => updateRemoteBackup({ prefix: e.target.value })}
              />
            </FieldBlock>
            <ToggleRow
              label={t('s3PathStyle')}
              checked={remoteBackup.pathStyle}
              onChange={(checked) => updateRemoteBackup({ pathStyle: checked })}
            />
            <ToggleRow
              label={t('uploadAfterBackup')}
              checked={remoteBackup.uploadAfterBackup}
              onChange={(checked) =>
                updateRemoteBackup({ uploadAfterBackup: checked })
              }
            />

            <DataRow label={t('lastRemoteUpload')} value={lastUploaded} />
            {remoteBackup.lastError && (
              <DataRow
                label={t('lastRemoteError')}
                value={
                  <StatusTag tone="danger">{remoteBackup.lastError}</StatusTag>
                }
              />
            )}

            {/* S3 Credentials */}
            <div className="remoteCredentials">
              <FieldBlock label={t('s3AccessKeyId')}>
                <input
                  className="textInput"
                  type="password"
                  value={s3AccessKeyId}
                  onChange={(e) => setS3AccessKeyId(e.target.value)}
                />
              </FieldBlock>
              <FieldBlock label={t('s3SecretAccessKey')}>
                <input
                  className="textInput"
                  type="password"
                  value={s3SecretAccessKey}
                  onChange={(e) => setS3SecretAccessKey(e.target.value)}
                />
              </FieldBlock>
              <div className="pathActions">
                <button
                  className="secondaryButton"
                  type="button"
                  onClick={handleStoreCredentials}
                >
                  <Glyph icon="vpn_key" />
                  {t('saveCredentials')}
                </button>
                <button
                  className="secondaryButton"
                  type="button"
                  onClick={handleClearCredentials}
                >
                  <Glyph icon="delete" />
                  {t('clearCredentials')}
                </button>
              </div>
              <DataRow
                label={t('credentialsSaved')}
                value={
                  <StatusTag
                    tone={remoteBackup.credentialsSaved ? 'success' : 'neutral'}
                  >
                    {remoteBackup.credentialsSaved ? t('yes') : t('no')}
                  </StatusTag>
                }
              />
            </div>
          </>
        )}
      </Surface>

      <div className="settingsActions">
        <button className="primaryButton" type="button" onClick={handleSave}>
          {t('saveSettings')}
        </button>
      </div>
    </div>
  )
}
