/**
 * What Home warns about (`archive.md`: a failing or stalled automatic backup,
 * a browser PathKeep can't read, and the archive password at risk must not
 * stay hidden in Backup or Security). Each alert says what is wrong and links
 * to the place that fixes it; when nothing is wrong there are none.
 *
 * Responsible for: deciding which alerts apply, from the snapshot and the
 * scheduler's status. Not responsible for: rendering (`alerts-list.tsx`) or
 * the fixes themselves, which live in Backup and Settings → Security.
 */
import { useMemo } from 'react'
import { scheduleCopy } from '@/features/backup/schedule-copy'
import { useI18n } from '@/lib/i18n'
import { browserDiscoveryState } from '@/lib/platform-guidance'
import { useSaveConfig, useSnapshot } from '@/lib/queries/app'
import { useScheduleStatus } from '@/lib/queries/schedule'

export interface HomeAlert {
  /** Stable across renders: the issue code or the alert's kind. */
  id: string
  tone: 'error' | 'warning'
  title: string
  detail: string
  /** Where it is fixed; none when nothing in PathKeep can fix it. */
  action?: { label: string; to: string }
  /** For a risk the user may accept: hides the alert for good. */
  dismiss?: { label: string; run: () => void }
}

const BACKUP = '/backup'
const SECURITY = '/settings/security'

export function useHomeAlerts(): HomeAlert[] {
  const { t } = useI18n()
  const snapshot = useSnapshot()
  const schedule = useScheduleStatus()
  const { mutate: saveConfig } = useSaveConfig()

  return useMemo(() => {
    const alerts: HomeAlert[] = []
    const openBackup = { label: t('home.alerts.openBackup'), to: BACKUP }
    const openSecurity = { label: t('home.alerts.openSecurity'), to: SECURITY }

    for (const issue of schedule.data?.issues ?? []) {
      // Dismissible issues describe what this platform can't check (Linux
      // has no install-status detection), not something gone wrong.
      if (issue.severity === 'info' || issue.dismissible) continue
      alerts.push({
        id: `schedule:${issue.code}`,
        tone: issue.severity === 'error' ? 'error' : 'warning',
        title:
          scheduleCopy(t, issue.titleKey) ?? t('home.alerts.scheduleFallback'),
        detail:
          scheduleCopy(t, issue.consequenceKey) ??
          scheduleCopy(t, issue.detailKey) ??
          '',
        action: openBackup,
      })
    }

    const discovery = browserDiscoveryState(
      snapshot.browserDiscoveryIssue,
      snapshot.browserProfiles.length,
    )
    if (discovery === 'full-disk-access' || discovery === 'discovery-error') {
      const key =
        discovery === 'full-disk-access' ? 'fullDiskAccess' : 'discoveryError'
      alerts.push({
        id: `discovery:${discovery}`,
        tone: 'warning',
        title: t(`home.alerts.${key}.title`),
        detail: t(`home.alerts.${key}.detail`),
        action: openBackup,
      })
    }

    const { config, keyringStatus: keyring } = snapshot
    if (snapshot.archiveStatus.encrypted) {
      if (config.rememberDatabaseKeyInKeyring && !keyring.available) {
        // The system's keychain is missing; nothing in Security brings it back.
        alerts.push({
          id: 'keychain:unavailable',
          tone: 'warning',
          title: t('home.alerts.keychainUnavailable.title'),
          detail: t('home.alerts.keychainUnavailable.detail'),
        })
      } else if (config.rememberDatabaseKeyInKeyring && !keyring.storedSecret) {
        alerts.push({
          id: 'keychain:missing',
          tone: 'warning',
          title: t('home.alerts.keyMissing.title'),
          detail: t('home.alerts.keyMissing.detail'),
          action: openSecurity,
        })
      } else if (!keyring.storedSecret && !config.passwordKeptElsewhere) {
        // Nowhere but in the user's memory. Without a keychain the switch in
        // Security is off for good, so only keeping it elsewhere is left.
        alerts.push({
          id: 'password:risk',
          tone: 'warning',
          title: t('home.alerts.passwordRisk.title'),
          detail: keyring.available
            ? t('home.alerts.passwordRisk.detail')
            : t('home.alerts.passwordRisk.noKeychain'),
          action: keyring.available
            ? { label: t('home.alerts.passwordRisk.keep'), to: SECURITY }
            : undefined,
          dismiss: {
            label: t('home.alerts.passwordRisk.dismiss'),
            run: () =>
              saveConfig((next) => ({ ...next, passwordKeptElsewhere: true })),
          },
        })
      }
    }
    return alerts
  }, [t, snapshot, schedule.data, saveConfig])
}
