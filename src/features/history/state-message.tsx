/** Loading skeletons, empty states and error states for the History lists. */
import {
  Filter,
  Globe,
  History,
  SearchX,
  Star,
  TriangleAlert,
  type LucideIcon,
} from 'lucide-react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty'
import { useI18n } from '@/lib/i18n'
import { LoadingRow } from './timeline-rows'

const icons: Record<string, LucideIcon> = {
  history: History,
  filter: Filter,
  search: SearchX,
  star: Star,
  sites: Globe,
}

interface Props {
  loading?: boolean
  error?: Error | null
  icon?: keyof typeof icons
  title?: string
  body?: string
  onRetry?: () => void
  children?: ReactNode
}

export function StateMessage({
  loading,
  error,
  icon,
  title,
  body,
  onRetry,
  children,
}: Props) {
  const { t } = useI18n()

  if (loading) {
    return (
      <div
        className="flex-1 overflow-hidden px-[18px] pt-3"
        role="status"
        aria-label={t('history.state.loading')}
      >
        {Array.from({ length: 12 }, (_, index) => (
          <div
            key={index}
            className="h-[38px]"
            style={{ opacity: 1 - index * 0.06 }}
          >
            <LoadingRow />
          </div>
        ))}
      </div>
    )
  }

  const Icon = error ? TriangleAlert : icons[icon ?? 'history']
  return (
    <Empty className="animate-rise flex-1">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <Icon />
        </EmptyMedia>
        <EmptyTitle className="text-base">{title}</EmptyTitle>
        <EmptyDescription>{error ? error.message : body}</EmptyDescription>
      </EmptyHeader>
      {(onRetry || children) && (
        <EmptyContent>
          {onRetry && (
            <Button variant="outline" size="sm" onClick={onRetry}>
              {t('common.retry')}
            </Button>
          )}
          {children}
        </EmptyContent>
      )}
    </Empty>
  )
}
