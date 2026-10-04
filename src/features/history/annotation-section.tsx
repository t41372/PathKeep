/**
 * The user's own words about a page, at the bottom of the detail panel: tags,
 * then the note. Both come from one `get_url_annotation` read, so loading and
 * failure are shown once for the pair.
 */
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useI18n } from '@/lib/i18n'
import { NoteField } from './note-editor'
import { useUrlAnnotation } from './queries'
import { TagField } from './tag-editor'

export function AnnotationSection({
  url,
  profileId,
  onFilterTag,
}: {
  url: string
  profileId?: string
  onFilterTag: (tag: string) => void
}) {
  const { t } = useI18n()
  const annotation = useUrlAnnotation(url)

  if (annotation.isPending) {
    return (
      <div className="flex flex-col gap-[18px]" aria-hidden>
        <div className="flex flex-col gap-1.5">
          <span className="text-xs text-muted-foreground">
            {t('historyPage.tags.label')}
          </span>
          <Skeleton className="h-9 w-full rounded-md" />
        </div>
        <div className="flex flex-col gap-1.5">
          <span className="text-xs text-muted-foreground">
            {t('history.detail.note')}
          </span>
          <Skeleton className="h-[72px] w-full rounded-md" />
        </div>
      </div>
    )
  }
  if (annotation.isError) {
    return (
      <div className="flex flex-col items-start gap-1.5">
        <span className="text-xs text-muted-foreground">
          {t('historyPage.tags.label')} · {t('history.detail.note')}
        </span>
        <p className="text-[13px] text-muted-foreground">
          {t('historyPage.tags.loadFailed')}
        </p>
        <Button
          variant="outline"
          size="xs"
          onClick={() => void annotation.refetch()}
        >
          {t('common.retry')}
        </Button>
      </div>
    )
  }
  return (
    <>
      <TagField
        url={url}
        profileId={profileId}
        initial={annotation.data?.tags ?? []}
        onFilter={onFilterTag}
      />
      <NoteField
        url={url}
        profileId={profileId}
        initial={annotation.data?.notes ?? ''}
      />
    </>
  )
}
