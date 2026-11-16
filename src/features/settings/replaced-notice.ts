/**
 * The toast a restore or an import ends with. The archive it replaced is kept
 * as a safety copy, and PathKeep checks that copy as it keeps it. When the
 * check could not be made, the toast says so and why: a copy that can't be
 * read is then known about now, not first when it is needed.
 */
import { toast } from 'sonner'
import type { useI18n } from '@/lib/i18n'

export function announceReplaced(
  t: ReturnType<typeof useI18n>['t'],
  title: string,
  unchecked: string | null,
) {
  if (!unchecked) {
    toast.success(title)
    return
  }
  toast.warning(title, {
    description: t('settingsStorage.copyUnchecked', { reason: unchecked }),
    // It stays until it is closed: a restore or an import runs for minutes,
    // often while the user is away, and this is the one time it is said.
    duration: Infinity,
    closeButton: true,
  })
}
