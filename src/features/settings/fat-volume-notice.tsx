/**
 * The warning for a data folder on a FAT32 or exFAT disk
 * (`get_storage_volume`): those file systems can lose a file when the power
 * goes out while PathKeep writes, which PathKeep can't prevent there. It
 * advises and doesn't block: many people keep data on such drives on purpose.
 * Shown where the folder is shown: Settings › Storage and onboarding's
 * storage step.
 *
 * Nothing shows while the check runs or when it fails: the warning is
 * advice, and the rest of either screen doesn't depend on it.
 */
import { useQuery } from '@tanstack/react-query'
import { archiveAccessClient } from '@/lib/backend-client/archive-access'
import { useI18n } from '@/lib/i18n'
import { Notice } from './notice'

export function FatVolumeNotice() {
  const { t } = useI18n()
  const volume = useQuery({
    queryKey: ['storage-volume'],
    queryFn: () => archiveAccessClient.storageVolume(),
    staleTime: 60_000,
    retry: false,
  })
  if (!volume.data?.fat) return null
  return (
    <div data-fat-notice>
      <Notice tone="warning">{t('archiveAccess.fat')}</Notice>
    </div>
  )
}
