/**
 * Switching the interface language. The UI changes at once; the choice is
 * also saved to config, because boot applies the configured language and the
 * scheduler and worker read it for their notifications.
 */
import { useCallback } from 'react'
import { toast } from 'sonner'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import type { LanguagePreference } from '@/lib/types'
import { useSaveConfig } from './app'

export function useChooseLanguage() {
  const { t, setPreference } = useI18n()
  const { mutate } = useSaveConfig()

  return useCallback(
    (next: LanguagePreference) => {
      setPreference(next)
      mutate((config) => ({ ...config, preferredLanguage: next }), {
        onError: (error) =>
          toast.error(t('settings.saveFailed'), {
            description: describeError(error, 'save_config'),
          }),
      })
    },
    [setPreference, mutate, t],
  )
}
