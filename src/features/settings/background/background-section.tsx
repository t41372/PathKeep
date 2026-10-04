/**
 * Settings → Background work: the one place that shows what PathKeep does
 * after a backup and lets the user steer it. Replaces the old Jobs page and
 * sidebar job strip.
 *
 * Order: the controls that govern every queue, then each queue, then the
 * online work, then the data the work produces. Each row owns its own data
 * and copy; this file only lays them out.
 */
import { SettingsGroup, SettingsSection } from '@/components/app/setting-row'
import { useI18n } from '@/lib/i18n'
import { InsightsDataRow, PageDetailsRow } from './derived-data-rows'
import { LinkPreviewsRow, PageSummariesRow } from './online-work-rows'
import { AiQueueRow, InsightsQueueRow, QueueControls } from './queue-rows'

export function BackgroundSection() {
  const { t } = useI18n()
  return (
    <SettingsSection title={t('settings.nav.background')}>
      <p className="-mt-2 mb-1 text-[13px] text-muted-foreground">
        {t('settingsBackground.intro')}
      </p>
      <QueueControls />
      <InsightsQueueRow />
      <AiQueueRow />
      <PageSummariesRow />
      <LinkPreviewsRow />
      <SettingsGroup
        title={t('settingsBackground.data.group')}
        note={t('settingsBackground.data.note')}
      />
      <InsightsDataRow />
      <PageDetailsRow />
    </SettingsSection>
  )
}
