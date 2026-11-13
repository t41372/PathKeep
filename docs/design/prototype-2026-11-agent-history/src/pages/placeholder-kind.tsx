/**
 * A planned data type. It exists so the six-type setup can be judged: the
 * rail, settings and backup all have to fit it. No real content.
 */
import { Link, useParams } from 'react-router-dom'
import { PageHeader, PageScroll, SectionCard } from '@/components/app/section-card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty'
import { kindById, type KindId } from '~/kinds'

export default function PlaceholderKindPage() {
  const { kind: id } = useParams()
  const kind = id && id in kindById ? kindById[id as KindId] : undefined
  if (!kind) {
    return <div className="flex flex-1 items-center justify-center text-[13px] text-muted-foreground">沒有這個資料類型</div>
  }
  const Icon = kind.icon
  return (
    <PageScroll>
      <PageHeader
        title={
          <span className="flex items-center gap-3">
            {kind.label}
            <Badge variant="outline" className="text-xs font-normal text-muted-foreground">
              規劃中
            </Badge>
          </span>
        }
        subtitle={kind.why}
      />
      <SectionCard title="會讀的來源">
        <div className="flex flex-wrap gap-1.5">
          {kind.sources.map((s) => (
            <span key={s} className="rounded-md border bg-background/60 px-2 py-0.5 text-[13px]">
              {s}
            </span>
          ))}
        </div>
      </SectionCard>
      <Empty className="border">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <Icon />
          </EmptyMedia>
          <EmptyTitle>這個類型還在規劃</EmptyTitle>
          <EmptyDescription>
            原型裡放它，只是為了看導航列、設定和備份頁在類型變多時放不放得下。真的做的時候，它會有自己的瀏覽、搜尋與洞察，也一起跟著排程備份。
          </EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Button variant="outline" asChild>
            <Link to="/settings/kinds">管理資料類型</Link>
          </Button>
        </EmptyContent>
      </Empty>
    </PageScroll>
  )
}
