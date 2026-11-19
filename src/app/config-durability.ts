/**
 * Says so when a settings save was applied but the disk did not confirm it
 * will survive a power loss (`configDurabilityWarning`). The settings are in
 * effect, so it is a warning that does not get in the way, shown once per
 * save whichever screen saved, onboarding included. Not responsible for the
 * save itself (`saveConfig` in `lib/queries/app`).
 */
import { useEffect } from 'react'
import { toast } from 'sonner'
import { useI18n } from '@/lib/i18n'
import { onConfigNotDurable } from '@/lib/queries/app'

export function useConfigDurabilityNotice() {
  const { t } = useI18n()
  useEffect(
    () =>
      onConfigNotDurable((detail) => {
        // One toast however many saves in a row: each replaces the last.
        toast.warning(t('settings.notDurable.title'), {
          id: 'config-not-durable',
          description: t('settings.notDurable.body', { detail }),
        })
      }),
    [t],
  )
}
