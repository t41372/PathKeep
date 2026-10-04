/**
 * The right-hand panel for the selected page: title and URL, open / star /
 * copy, the link preview, visit stats, the same-session list, tags and the
 * note.
 *
 * Responsible for: layout and the three actions.
 * Not responsible for: choosing the selection or loading lists.
 */
import { Copy, ExternalLink, Star, X } from 'lucide-react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { cn } from '@/lib/cn'
import { useFormat, useI18n } from '@/lib/i18n'
import { DetailStats } from './detail-stats'
import type { DetailTarget, VisitItem } from './history-types'
import { AnnotationSection } from './annotation-section'
import { LinkPreview } from './link-preview'
import { openInBrowser } from './open-link'
import { SiteIcon } from './site-icon'

interface Props {
  target: DetailTarget
  starred: boolean
  sessionMates: VisitItem[]
  onClose: () => void
  onToggleStar: () => void
  onSelectMate: (item: VisitItem) => void
  /** Filters History to pages carrying this tag. */
  onFilterTag: (tag: string) => void
}

function IconAction({
  label,
  onClick,
  pressed,
  children,
}: {
  label: string
  onClick: () => void
  pressed?: boolean
  children: React.ReactNode
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="outline"
          size="icon-sm"
          aria-label={label}
          aria-pressed={pressed}
          onClick={onClick}
          className="size-[34px] shrink-0"
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  )
}

const MAX_SESSION_ROWS = 8

export function DetailPanel({
  target,
  starred,
  sessionMates,
  onClose,
  onToggleStar,
  onSelectMate,
  onFilterTag,
}: Props) {
  const { t } = useI18n()
  const format = useFormat()

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(target.url)
      toast(t('history.detail.copied'))
    } catch {
      toast.error(t('history.detail.copyFailed'))
    }
  }

  return (
    <aside
      className="flex h-full w-80 flex-col gap-[18px] overflow-y-auto p-6"
      aria-label={t('history.detail.label')}
    >
      <div className="flex items-start gap-3">
        <SiteIcon
          domain={target.domain}
          className="size-9 rounded-[9px] text-[15px]"
        />
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <h2 className="leading-snug font-semibold break-words">
            {target.title?.trim() || target.domain}
          </h2>
          <span className="font-mono text-xs break-all text-muted-foreground">
            {target.url}
          </span>
        </div>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label={t('history.detail.close')}
          onClick={onClose}
          className="text-muted-foreground"
        >
          <X />
        </Button>
      </div>

      <div className="flex gap-2">
        <Button
          className="h-[34px] flex-1"
          onClick={() =>
            void openInBrowser(target.url, t('history.detail.openFailed'))
          }
        >
          <ExternalLink />
          {t('history.detail.open')}
        </Button>
        <IconAction
          label={t(starred ? 'history.detail.unstar' : 'history.detail.star')}
          pressed={starred}
          onClick={onToggleStar}
        >
          <Star
            className={cn(starred ? 'fill-brand text-brand' : 'text-brand')}
          />
        </IconAction>
        <IconAction
          label={t('history.detail.copy')}
          onClick={() => void copy()}
        >
          <Copy />
        </IconAction>
      </div>

      <LinkPreview
        key={`preview:${target.url}`}
        url={target.url}
        domain={target.domain}
        title={target.title?.trim() || target.domain}
      />

      <DetailStats key={`stats:${target.url}`} url={target.url} />

      {sessionMates.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <span className="text-xs text-muted-foreground">
            {t('history.detail.sameSession')}
          </span>
          {sessionMates.slice(0, MAX_SESSION_ROWS).map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => onSelectMate(item)}
              className="flex items-center gap-2 rounded py-1 text-left text-[13px] outline-none hover:text-brand focus-visible:ring-2 focus-visible:ring-ring/50"
            >
              <span className="font-mono text-[11px] text-muted-foreground">
                {format.time(item.visitTime)}
              </span>
              <span className="truncate">{item.title?.trim() || item.url}</span>
            </button>
          ))}
          {sessionMates.length > MAX_SESSION_ROWS && (
            <span className="text-xs text-muted-foreground">
              {t('history.detail.sessionMore', {
                count: sessionMates.length - MAX_SESSION_ROWS,
              })}
            </span>
          )}
        </div>
      )}

      <AnnotationSection
        key={`annotations:${target.url}`}
        url={target.url}
        profileId={target.profileId}
        onFilterTag={onFilterTag}
      />
    </aside>
  )
}
