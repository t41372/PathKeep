/**
 * Settings › Schedule and background work (`#/settings/background`):
 * settings only, no state (`data-spaces.md` §8). The automatic backup card
 * (one schedule for every kind of data that is on), then the choices about
 * background work itself (run it or pause it, how many at a time), then
 * the way to the background tasks page, which lists what runs, waits,
 * needs a look or finished, with the queues and the data they build.
 *
 * Each row owns its own data and copy; this file only lays them out.
 */
import { ListChecks } from 'lucide-react'
import { Link } from 'react-router-dom'
import {
  SettingRow,
  SettingsGroup,
  SettingsSection,
} from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { useI18n } from '@/lib/i18n'
import { ScheduleCard } from '@/features/backup/schedule-card'
import { QueueControls } from './queue-rows'

export function BackgroundSection() {
  const { t } = useI18n()
  return (
    <SettingsSection title={t('settings.nav.background')}>
      <p className="-mt-2 mb-1 text-[13px] text-muted-foreground">
        {t('tasks.settings.oneSchedule')}
      </p>
      <ScheduleCard />
      <SettingsGroup
        title={t('tasks.settings.workGroup')}
        note={t('settingsBackground.intro')}
      />
      <QueueControls />
      <SettingRow
        title={t('tasks.settings.title')}
        description={t('tasks.settings.body')}
        control={
          <Button variant="outline" size="sm" asChild>
            <Link to="/tasks">
              <ListChecks aria-hidden />
              {t('tasks.settings.open')}
            </Link>
          </Button>
        }
      />
    </SettingsSection>
  )
}
