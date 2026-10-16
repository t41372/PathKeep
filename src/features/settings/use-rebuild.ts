/**
 * Rebuilds the chosen model's search index from nothing and says whether it
 * started. Shared by the Rebuild row and the note on an index made from an
 * older version of the page text; the confirmation is `RebuildDialog`.
 */
import { useMutation } from '@tanstack/react-query'
import { toast } from 'sonner'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import type { SemanticIndex } from './semantic-row'

export function useRebuild(index: SemanticIndex) {
  const { t } = useI18n()
  return useMutation({
    mutationFn: () => index.rebuild('full'),
    onSuccess: () => toast.success(t('settingsAi.rebuild.started')),
    onError: (error) =>
      toast.error(
        t('settingsAi.rebuild.failed', { message: describeError(error) }),
      ),
  })
}
