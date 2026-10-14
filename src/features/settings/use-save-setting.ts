/**
 * Saves one config change from a settings row and says so when it fails.
 * Rows show the saved value from the snapshot, so a failed save simply leaves
 * the control where it was; the toast says why.
 */
import { useCallback } from 'react'
import { toast } from 'sonner'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { useSaveConfig } from '@/lib/queries/app'
import type { AppConfig } from '@/lib/types'

export function useSaveSetting() {
  const { t } = useI18n()
  const save = useSaveConfig()
  const { mutateAsync } = save

  const run = useCallback(
    async (update: (config: AppConfig) => AppConfig) => {
      try {
        await mutateAsync(update)
        return true
      } catch (error) {
        toast.error(t('settings.saveFailed'), {
          description: describeError(error, 'save_config'),
        })
        return false
      }
    },
    [mutateAsync, t],
  )

  return { save: run, saving: save.isPending }
}
