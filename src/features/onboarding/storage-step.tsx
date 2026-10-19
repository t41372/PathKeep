/**
 * Step 3: where the archive will be created and roughly how big it gets.
 *
 * The location comes from the app's data folder and can't be changed from
 * the UI, so the step offers to show the folder instead of a dead "Change"
 * button. The estimate uses file sizes only; visit counts aren't known until
 * the first backup reads the history.
 */
import { FolderOpen } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { supportClient } from '@/lib/backend-client/support'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { estimateOnboardingStorage } from '@/lib/onboarding-estimates'
import { useSnapshot } from '@/lib/queries/app'
import { isMacOsHost } from '@/lib/runtime'
import { InlineError } from './inline-error'

export function StorageStep({ selected }: { selected: string[] }) {
  const { t } = useI18n()
  const format = useFormat()
  const snapshot = useSnapshot()
  const [revealError, setRevealError] = useState<string | null>(null)
  const root = snapshot.directories.appRoot
  const estimate = estimateOnboardingStorage(snapshot.browserProfiles, selected)

  async function reveal() {
    setRevealError(null)
    try {
      await supportClient.openPathInFileManager(root)
    } catch (error) {
      setRevealError(describeError(error, 'open_path_in_file_manager'))
    }
  }

  const tiles = [
    {
      label: t('onboarding.storage.history'),
      value: format.bytes(estimate.sourceBytes),
    },
    {
      label: t('onboarding.storage.estimate'),
      value: `≈ ${format.bytes(estimate.totalBytes)}`,
    },
    {
      label: t('onboarding.storage.browsers'),
      value: format.number(estimate.profileCount),
    },
  ]

  return (
    <>
      <section className="flex flex-col gap-3.5 rounded-xl border bg-card p-[18px] shadow-card">
        <h2 className="text-[13px] font-medium">
          {t('onboarding.storage.location')}
        </h2>
        <div className="flex gap-2">
          <div
            className="flex h-9 min-w-0 flex-1 items-center truncate rounded-lg border bg-popover px-3 font-mono text-[13px]"
            title={root}
          >
            <span className="truncate">{root}</span>
          </div>
          <Button
            variant="outline"
            className="h-9"
            onClick={() => void reveal()}
          >
            <FolderOpen />
            {t(
              isMacOsHost()
                ? 'onboarding.storage.revealMac'
                : 'onboarding.storage.revealOther',
            )}
          </Button>
        </div>
        {revealError && (
          <InlineError
            title={t('onboarding.storage.revealFailed')}
            detail={revealError}
          />
        )}
      </section>
      <dl className="grid grid-cols-3 gap-2.5">
        {tiles.map((tile) => (
          <div
            key={tile.label}
            className="flex flex-col gap-1 rounded-[10px] border bg-card p-3.5 shadow-card"
          >
            <dt className="text-xs text-muted-foreground">{tile.label}</dt>
            <dd className="text-xl font-semibold tabular">{tile.value}</dd>
          </div>
        ))}
      </dl>
      <p className="text-[13px] leading-normal text-muted-foreground">
        {t('onboarding.storage.note')}
      </p>
    </>
  )
}
