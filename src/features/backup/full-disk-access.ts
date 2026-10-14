/**
 * Opens macOS System Settings at Full Disk Access, the one permission Safari
 * backups need. Shared by the Sources card and the run sheet so both offer the
 * same fix and report a failure the same way.
 */
import { toast } from 'sonner'
import { supportClient } from '@/lib/backend-client/support'
import { describeError } from '@/lib/errors'
import { macosFullDiskAccessSettingsUrl } from '@/lib/platform-guidance'

export function openFullDiskAccess(failed: string) {
  return supportClient
    .openExternalUrl(macosFullDiskAccessSettingsUrl)
    .catch((error) => {
      toast.error(failed, {
        description: describeError(error, 'open_external_url'),
      })
    })
}
