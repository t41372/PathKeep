/**
 * The changes a visitor makes in the demo. Each is answered the way the
 * backend answers a change that went through, built from the request over
 * what the recording says now, and kept nowhere: when the app reads the
 * same thing again it gets the recording, and a reload starts over. So a
 * change takes on screen without an error, and nothing is saved. Also the
 * update check, which this build cannot do.
 *
 * Responsible for: those answers, each typed with the reply the app's own
 * client declares, so a version that changes one fails `tsc -p demo` before
 * it is recorded again. Not responsible for: reads (`backend.ts`). A
 * command not listed here gets the demo's refusal (`backend.ts`): the
 * demo does not answer what it cannot build from the recording.
 */
import type {
  annotationsClient,
  UrlAnnotation,
} from '@/lib/backend-client/annotations'
import type { appClient } from '@/lib/backend-client/app'
import type { archiveClient } from '@/lib/backend-client/archive'
import type { contentEnrichmentClient } from '@/lib/backend-client/content-enrichment'
import type { desktopClient } from '@/lib/backend-client/desktop'
import type { explorerClient } from '@/lib/backend-client/explorer'
import type { insightsClient } from '@/lib/backend-client/insights'
import type { intelligenceClient } from '@/lib/backend-client/intelligence'
import type { scheduleClient } from '@/lib/backend-client/schedule'
import type { starsClient } from '@/lib/backend-client/stars'
import type { supportClient } from '@/lib/backend-client/support'
import type { updateClient } from '@/lib/backend-client/update'
import type { AppConfig } from '@/lib/types'
import { RELEASES_PAGE_URL } from '@/lib/update'

/** What a client call resolves to: the answer the app expects back. */
type Reply<F> = F extends (...args: never[]) => Promise<infer R> ? R : never

/** What the recording says now, as the backend would answer it. */
export interface Recorded {
  /** The latest recorded answer to a read; the demo's refusal if it has none. */
  latest<T>(command: string): T
  /** The recorded answer to exactly this read, if there is one. */
  exactly<T>(command: string, args: unknown): T | undefined
}

type Write = (args: never, recorded: Recorded) => unknown

/** A note or tags saved for a page, over what the page had. */
function annotation(
  recorded: Recorded,
  url: string,
  change: Partial<UrlAnnotation>,
): UrlAnnotation {
  const before = recorded.exactly<UrlAnnotation | null>('get_url_annotation', {
    url,
  })
  const at = new Date().toISOString()
  return {
    url,
    notes: '',
    tags: [],
    createdAt: at,
    ...before,
    ...change,
    updatedAt: at,
  }
}

/** The installed backup schedule, as the backend reports a change to it. */
const scheduleApplied = (
  recorded: Recorded,
): Reply<typeof scheduleClient.applyInstall> => {
  const status =
    recorded.latest<Reply<typeof scheduleClient.getStatus>>('schedule_status')
  return {
    applied: true,
    platform: status.platform,
    files: status.detectedFiles,
    message: '',
    stepResults: [],
  }
}

export const writes: Record<string, Write> = {
  // Settings: the snapshot the backend sends back after saving, with the
  // visitor's settings in it.
  save_config: (
    args: { config: AppConfig },
    recorded,
  ): Reply<typeof appClient.saveConfig> => ({
    ...recorded.latest<Reply<typeof appClient.getSnapshot>>('app_snapshot'),
    config: args.config,
  }),
  // These change what other reads say (page summaries, the stored key), so
  // the snapshot stays as recorded and the app's next read shows the same.
  set_content_fetch_settings: (
    _: unknown,
    recorded,
  ): Reply<typeof contentEnrichmentClient.setContentFetchSettings> =>
    recorded.latest<Reply<typeof appClient.getSnapshot>>('app_snapshot'),
  store_ai_provider_api_key: (
    _: unknown,
    recorded,
  ): Reply<typeof intelligenceClient.storeProviderApiKey> =>
    recorded.latest<Reply<typeof appClient.getSnapshot>>('app_snapshot'),
  clear_ai_provider_api_key: (
    _: unknown,
    recorded,
  ): Reply<typeof intelligenceClient.clearProviderApiKey> =>
    recorded.latest<Reply<typeof appClient.getSnapshot>>('app_snapshot'),

  // Opening at login and the menu bar icon: the new state, as the OS would
  // report it.
  set_launch_at_login: (
    args: { enabled: boolean },
    recorded,
  ): Reply<typeof desktopClient.setLaunchAtLogin> => ({
    ...recorded.latest<Reply<typeof desktopClient.get>>(
      'get_desktop_integration',
    ),
    launchAtLogin: args.enabled,
  }),
  set_menu_bar_icon: (
    args: { enabled: boolean },
    recorded,
  ): Reply<typeof desktopClient.setMenuBarIcon> => ({
    ...recorded.latest<Reply<typeof desktopClient.get>>(
      'get_desktop_integration',
    ),
    menuBarIcon: args.enabled,
  }),

  // The lock as it stands. Without a passcode the backend leaves the
  // session unlocked when asked to lock (`app_lock.rs`), and the app says a
  // passcode comes first; a passcode set here is not kept.
  lock_app_session: (
    _: unknown,
    recorded,
  ): Reply<typeof appClient.lockAppSession> =>
    recorded.latest<Reply<typeof appClient.getLockStatus>>('app_lock_status'),
  set_app_lock_passcode: (
    _: unknown,
    recorded,
  ): Reply<typeof appClient.setAppLockPasscode> =>
    recorded.latest<Reply<typeof appClient.getLockStatus>>('app_lock_status'),
  clear_app_lock_passcode: (
    _: unknown,
    recorded,
  ): Reply<typeof appClient.clearAppLockPasscode> =>
    recorded.latest<Reply<typeof appClient.getLockStatus>>('app_lock_status'),

  // A page's note and tags, saved over what it had.
  set_url_notes: (
    args: {
      request: { url: string; notes: string; sourceProfile?: string | null }
    },
    recorded,
  ): Reply<typeof annotationsClient.setUrlNotes> =>
    annotation(recorded, args.request.url, {
      notes: args.request.notes,
      sourceProfile: args.request.sourceProfile ?? null,
    }),
  replace_url_tags: (
    args: {
      request: { url: string; tags: string[]; sourceProfile?: string | null }
    },
    recorded,
  ): Reply<typeof annotationsClient.replaceUrlTags> =>
    annotation(recorded, args.request.url, {
      tags: args.request.tags,
      sourceProfile: args.request.sourceProfile ?? null,
    }),

  // The backend answers these with nothing.
  set_star: (): Reply<typeof starsClient.setStar> => undefined,
  unset_star: (): Reply<typeof starsClient.unsetStar> => undefined,
  mark_og_images_shown: (): Reply<typeof explorerClient.markOgImagesShown> =>
    undefined,
  set_site_category: (): Reply<typeof insightsClient.setSiteCategory> =>
    undefined,
  // The app's own error log; the demo keeps none.
  record_frontend_error: () => undefined,

  // Back up now, a moment after the last backup (the demo's clock stands
  // there): a run that found nothing new. The app reads only the run and
  // its warnings (`backup-runner.tsx`).
  run_backup_now: (
    _: unknown,
    recorded,
  ): Reply<typeof archiveClient.runBackupNow> => {
    const last =
      recorded.latest<Reply<typeof appClient.getSnapshot>>('app_snapshot')
        .recentRuns[0]
    const at = new Date().toISOString()
    return {
      dueSkipped: false,
      run: last && {
        ...last,
        id: last.id + 1,
        startedAt: at,
        finishedAt: at,
        trigger: 'manual',
        newVisits: 0,
        newUrls: 0,
        newDownloads: 0,
      },
      profiles: [],
      warnings: [],
    }
  },

  // The backup schedule: set, removed or repaired as asked.
  apply_schedule: (
    _: unknown,
    recorded,
  ): Reply<typeof scheduleClient.applyInstall> => scheduleApplied(recorded),
  remove_schedule: (
    _: unknown,
    recorded,
  ): Reply<typeof scheduleClient.removeInstall> => scheduleApplied(recorded),
  repair_schedule: (
    _: unknown,
    recorded,
  ): Reply<typeof scheduleClient.repairInstall> => scheduleApplied(recorded),

  // Background jobs: as they stand.
  run_ai_queue_jobs: (
    _: unknown,
    recorded,
  ): Reply<typeof intelligenceClient.runQueueJobs> =>
    recorded.latest<Reply<typeof intelligenceClient.getQueueStatus>>(
      'load_ai_queue_status',
    ),
  retry_intelligence_job: (
    _: unknown,
    recorded,
  ): Reply<typeof intelligenceClient.retryRuntimeJob> =>
    recorded.latest<Reply<typeof intelligenceClient.getRuntime>>(
      'load_intelligence_runtime',
    ),
  cancel_intelligence_job: (
    _: unknown,
    recorded,
  ): Reply<typeof intelligenceClient.cancelRuntimeJob> =>
    recorded.latest<Reply<typeof intelligenceClient.getRuntime>>(
      'load_intelligence_runtime',
    ),

  // Checking for an update: the demo cannot update itself, and says so the
  // way the app does for any build that cannot, with the releases page.
  check_for_app_update: (
    _: unknown,
    recorded,
  ): Reply<typeof updateClient.checkForAppUpdate> => ({
    availability: {
      supported: false,
      checkedAt: new Date().toISOString(),
      available: false,
      currentVersion:
        recorded.latest<Reply<typeof appClient.getBuildInfo>>('app_build_info')
          .version,
      version: null,
      notes: null,
      publishedAt: null,
      error: null,
      downloadUrl: RELEASES_PAGE_URL,
    },
    pendingUpdate: null,
  }),

  // Exporting: as when the save dialog is closed, nothing written and
  // nothing to say (`export-row.tsx`).
  export_history: (): Reply<typeof archiveClient.exportHistory> => null,
  // Showing a file in Finder: a browser has none to show it in.
  open_path_in_file_manager: (): Reply<
    typeof supportClient.openPathInFileManager
  > => '',
  reveal_logs: (): Reply<typeof supportClient.revealLogs> => '',
}
