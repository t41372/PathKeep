/**
 * Settings → Storage: where the archive lives (with a warning when its disk
 * is FAT32 or exFAT) and how much space it takes, freeing space, moving to
 * another computer and taking back what an import added to Agent
 * conversations, making and restoring a safety copy, preparing the archive
 * for PathKeep 0.5.0, and deleting everything. Exporting the history and the link preview
 * images are browsing history's, in its own settings (`data-spaces.md` §8). Each action that writes opens a
 * dialog that shows what will happen before it runs.
 */
import { useState } from 'react'
import {
  SettingRow,
  SettingsGroup,
  SettingsSection,
} from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import { ConversationImportsRow } from './conversation-imports'
import { FatVolumeNotice } from './fat-volume-notice'
import { FreeSpaceDialog } from './free-space-dialog'
import { MoveDialog } from './move-dialogs'
import { RestoreDialog } from './restore-dialog'
import { reveal, revealLabelKey } from './reveal'
import { RollbackDialog } from './rollback-dialog'
import { SafetyCopyDialog } from './safety-copy-dialog'
import { UsageCard } from './usage-card'
import { WipeDialog } from './wipe-dialog'

type Open =
  'free' | 'export' | 'import' | 'copy' | 'restore' | 'rollback' | 'wipe' | null

export function StorageSection() {
  const { t } = useI18n()
  const [open, setOpen] = useState<Open>(null)
  const close = () => setOpen(null)

  return (
    <SettingsSection title={t('settings.nav.storage')}>
      <LocationRow />
      <UsageCard />
      <SettingRow
        title={t('settingsStorage.freeSpace.rowTitle')}
        description={t('settingsStorage.freeSpace.rowDescription')}
        control={
          <Button size="sm" variant="outline" onClick={() => setOpen('free')}>
            {t('settingsStorage.freeSpace.action')}
          </Button>
        }
      />

      <SettingsGroup title={t('settingsStorage.moveGroup')} />
      <SettingRow
        title={t('settingsStorage.move.title')}
        description={t('settingsStorage.move.description')}
        control={
          <>
            <Button
              size="sm"
              variant="outline"
              onClick={() => setOpen('import')}
            >
              {t('settingsStorage.move.importAction')}
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={() => setOpen('export')}
            >
              {t('settingsStorage.move.exportAction')}
            </Button>
          </>
        }
      />
      <ConversationImportsRow />
      <SettingRow
        title={t('settingsStorage.safetyCopy.rowTitle')}
        description={t('settingsStorage.safetyCopy.rowDescription')}
        control={
          <Button size="sm" variant="outline" onClick={() => setOpen('copy')}>
            {t('settingsStorage.safetyCopy.action')}
          </Button>
        }
      />
      <SettingRow
        title={t('settingsStorage.restore.rowTitle')}
        description={t('settingsStorage.restore.rowDescription')}
        control={
          <Button
            size="sm"
            variant="outline"
            onClick={() => setOpen('restore')}
          >
            {t('settingsStorage.restore.action')}
          </Button>
        }
      />
      <SettingRow
        title={t('archiveAccess.rollback.rowTitle')}
        description={t('archiveAccess.rollback.rowDescription')}
        control={
          <Button
            size="sm"
            variant="outline"
            onClick={() => setOpen('rollback')}
          >
            {t('archiveAccess.rollback.action')}
          </Button>
        }
      />
      <SettingRow
        danger
        title={t('settingsStorage.wipe.rowTitle')}
        description={t('settingsStorage.wipe.rowDescription')}
        control={
          <Button
            size="sm"
            variant="destructive"
            onClick={() => setOpen('wipe')}
          >
            {t('settingsStorage.wipe.action')}
          </Button>
        }
      />

      <FreeSpaceDialog open={open === 'free'} onClose={close} />
      <MoveDialog
        mode={open === 'export' || open === 'import' ? open : null}
        onClose={close}
      />
      <SafetyCopyDialog open={open === 'copy'} onClose={close} />
      <RestoreDialog open={open === 'restore'} onClose={close} />
      <RollbackDialog open={open === 'rollback'} onClose={close} />
      <WipeDialog open={open === 'wipe'} onClose={close} />
    </SettingsSection>
  )
}

function LocationRow() {
  const { t } = useI18n()
  const root = useSnapshot().directories.appRoot
  return (
    <SettingRow
      title={t('settingsStorage.location.title')}
      description={<span className="font-mono text-xs break-all">{root}</span>}
      control={
        <Button
          size="sm"
          variant="outline"
          onClick={() =>
            void reveal(root, t('settingsStorage.location.failed'))
          }
        >
          {t(revealLabelKey())}
        </Button>
      }
    >
      <FatVolumeNotice />
    </SettingRow>
  )
}
