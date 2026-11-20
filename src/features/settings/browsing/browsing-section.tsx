/**
 * Settings › Browsing history (`#/settings/browsing`, `data-spaces.md` §8):
 * the settings that belong to browsing history alone. Which profiles to back
 * up and importing are on the space's backup page, and the page opens by
 * saying so.
 *
 * Responsibilities: the Online group (link previews with its two options,
 * the preview images kept, page summaries) and the Export group (the whole
 * history as one file). A kind that is off has no page here; an old link
 * says so. Not responsible for: the rows themselves, each in its own file.
 */
import { Link } from 'react-router-dom'
import { SettingsGroup, SettingsSection } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { useI18n } from '@/lib/i18n'
import { useKindState } from '@/lib/queries/data-kinds'
import { spaceBackupPath } from '@/lib/spaces'
import { ExportRow } from './export-row'
import { LinkPreviewsSwitchRow, PageSummariesSwitchRow } from './online-rows'
import { PreviewImagesRow } from './preview-images-row'

export function BrowsingSection() {
  const { t } = useI18n()
  const state = useKindState('browsing')
  if (state === 'off') return <OffNote />
  return (
    <SettingsSection title={t('settings.nav.browsing')}>
      <div className="-mt-2 mb-1 flex flex-wrap items-baseline gap-x-2 gap-y-1 text-[13px] leading-relaxed text-muted-foreground">
        <p>{t('settingsBrowsing.backupNote')}</p>
        <Link
          to={spaceBackupPath('browsing')}
          className="font-medium text-brand hover:underline"
        >
          {t('settingsBrowsing.openBackup')}
        </Link>
      </div>
      <SettingsGroup
        title={t('settingsBrowsing.online.title')}
        note={t('settingsBrowsing.online.note')}
      />
      <LinkPreviewsSwitchRow />
      <PreviewImagesRow />
      <PageSummariesSwitchRow />
      <SettingsGroup title={t('settingsBrowsing.exportGroup')} />
      <ExportRow />
    </SettingsSection>
  )
}

function OffNote() {
  const { t } = useI18n()
  return (
    <SettingsSection title={t('settings.nav.browsing')}>
      <p className="text-[13px] leading-relaxed text-muted-foreground">
        {t('settingsBrowsing.off')}
      </p>
      <Button variant="outline" size="sm" className="self-start" asChild>
        <Link to="/settings/kinds">{t('settings.nav.kinds')}</Link>
      </Button>
    </SettingsSection>
  )
}
