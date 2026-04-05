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
import type { ArchiveMode } from '../../lib/types'

export function SecuritySettings() {
  const {
    t,
    archiveStatus,
    keyringStatus,
    sessionDatabaseKey,
    draftConfig,
    updateConfig,
    persistConfig,
    handleUnlockWithPassword,
    handleRotateEncryption,
    runTask,
    setNotice,
    setError,
  } = useApp()

  const [unlockPassword, setUnlockPassword] = useState('')
  const [rekeyPassword, setRekeyPassword] = useState('')
  const [rememberKey, setRememberKey] = useState(
    draftConfig.rememberDatabaseKeyInKeyring,
  )

  async function handleUnlock() {
    await handleUnlockWithPassword(unlockPassword)
    setUnlockPassword('')
  }

  async function handleRotate() {
    await handleRotateEncryption(rekeyPassword, 'Encrypted')
    setRekeyPassword('')
  }

  async function handleSwitchToPlaintext() {
    await runTask(t('convertToPlaintext'), async () => {
      await backend.rekeyArchive({ newMode: 'Plaintext', newKey: null })
      await backend.keyringClearDatabaseKey()
      await backend.resetLocalSecretVault()
      await backend.clearSessionDatabaseKey()
      setNotice(t('plaintextSuccess'))
    })
  }

  async function handleRememberKey() {
    if (!sessionDatabaseKey) {
      setError(t('unlockBeforeRemember'))
      return
    }
    await runTask(t('storeRememberedKey'), async () => {
      await backend.keyringStoreDatabaseKey(sessionDatabaseKey)
      setNotice(t('rememberStored'))
    })
  }

  async function handleClearRememberedKey() {
    await runTask(t('clearRememberedKey'), async () => {
      await backend.keyringClearDatabaseKey()
      setNotice(t('rememberCleared'))
    })
  }

  async function handleSave() {
    await runTask(t('saveSettings'), async () => {
      await persistConfig({
        ...draftConfig,
        rememberDatabaseKeyInKeyring: rememberKey,
      })
    })
  }

  const isEncrypted = draftConfig.archiveMode === 'Encrypted'

  return (
    <div className="settingsTabContent">
      <section className="pageIntro">
        <h2>{t('settingsSecurity')}</h2>
        <p className="muted">{t('securityDescription')}</p>
      </section>

      {/* Archive status */}
      <Surface
        eyebrow={t('archiveMode')}
        title={t('archiveMode')}
        icon="shield"
      >
        <DataRow
          label={t('archiveMode')}
          value={
            <StatusTag tone={isEncrypted ? 'success' : 'neutral'}>
              {isEncrypted ? t('encrypted') : t('plaintext')}
            </StatusTag>
          }
        />
        <DataRow
          label={t('statusLabel')}
          value={
            <StatusTag tone={archiveStatus.unlocked ? 'success' : 'danger'}>
              {archiveStatus.unlocked ? t('unlocked') : t('locked')}
            </StatusTag>
          }
        />

        {!archiveStatus.initialized && (
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
        )}
      </Surface>

      {/* Unlock form */}
      {isEncrypted && !archiveStatus.unlocked && archiveStatus.initialized && (
        <Surface
          eyebrow={t('unlockArchive')}
          title={t('unlockArchive')}
          icon="lock_open"
        >
          <FieldBlock label={t('masterPassword')}>
            <input
              className="textInput"
              type="password"
              placeholder={t('passwordPlaceholder')}
              value={unlockPassword}
              onChange={(e) => setUnlockPassword(e.target.value)}
            />
          </FieldBlock>
          <button
            className="primaryButton"
            type="button"
            onClick={handleUnlock}
          >
            <Glyph icon="lock_open" />
            {t('unlockArchive')}
          </button>
        </Surface>
      )}

      {/* Key rotation */}
      {archiveStatus.initialized && archiveStatus.unlocked && isEncrypted && (
        <Surface
          eyebrow={t('rotateKey')}
          title={t('rotateKey')}
          icon="autorenew"
        >
          <FieldBlock label={t('masterPassword')}>
            <input
              className="textInput"
              type="password"
              placeholder={t('passwordPlaceholder')}
              value={rekeyPassword}
              onChange={(e) => setRekeyPassword(e.target.value)}
            />
          </FieldBlock>
          <div className="pathActions">
            <button
              className="primaryButton"
              type="button"
              onClick={handleRotate}
            >
              <Glyph icon="autorenew" />
              {t('rotateKey')}
            </button>
            <button
              className="dangerButton"
              type="button"
              onClick={handleSwitchToPlaintext}
            >
              <Glyph icon="lock_open" />
              {t('convertToPlaintext')}
            </button>
          </div>
        </Surface>
      )}

      {/* Keyring */}
      <Surface
        eyebrow={t('keyringSection')}
        title={t('keyringSection')}
        icon="vpn_key"
      >
        <DataRow
          label={t('keyringBackend')}
          value={keyringStatus.backend || t('notAvailable')}
        />
        <DataRow
          label={t('keyringAvailable')}
          value={
            <StatusTag tone={keyringStatus.available ? 'success' : 'neutral'}>
              {keyringStatus.available ? t('yes') : t('no')}
            </StatusTag>
          }
        />
        <DataRow
          label={t('storedInKeyring')}
          value={
            <StatusTag
              tone={keyringStatus.storedSecret ? 'success' : 'neutral'}
            >
              {keyringStatus.storedSecret ? t('yes') : t('no')}
            </StatusTag>
          }
        />

        <ToggleRow
          label={t('rememberKey')}
          checked={rememberKey}
          onChange={setRememberKey}
        />

        <div className="pathActions">
          <button
            className="secondaryButton"
            type="button"
            onClick={handleRememberKey}
          >
            <Glyph icon="vpn_key" />
            {t('storeRememberedKey')}
          </button>
          <button
            className="secondaryButton"
            type="button"
            onClick={handleClearRememberedKey}
          >
            <Glyph icon="delete" />
            {t('clearRememberedKey')}
          </button>
        </div>
      </Surface>

      <div className="settingsActions">
        <button className="primaryButton" type="button" onClick={handleSave}>
          {t('saveSettings')}
        </button>
      </div>
    </div>
  )
}
