/**
 * The browsing space's Home: close to today's app Home. Its backup button
 * backs up browsing only. Existing users meet Agent history here: until they
 * turn it on, an intro card offers the preview (nothing is read before that).
 */
import { HardDriveDownload } from 'lucide-react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import { PageHeader, PageScroll } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { useProto } from '~/proto-state'
import { OnThisDayCard, TrendCard, YearCard } from './browsing/home-cards'
import { AgentIntroCard } from './home/cards'

export default function BrowsingHomePage() {
  const { agentEnabled } = useProto()
  return (
    <PageScroll>
      <PageHeader
        eyebrow="2026 年 11 月 13 日 星期五"
        title="午安"
        subtitle="已保存 182 萬筆瀏覽紀錄，從 2019 年 1 月開始；這週新增 8,412 筆。"
        actions={
          <>
            <Link to="/browsing/backup" className="flex h-9 items-center gap-2 rounded-lg border bg-card px-3 text-[13px] shadow-card hover:bg-card/90">
              <span className="size-2 rounded-full bg-green" />
              上次備份 今天 15:02
            </Link>
            <Button variant="outline" onClick={() => toast.success('開始備份「瀏覽紀錄」', { description: '只備份這個類型。要全部一起備份，用左下角背景任務選單裡的「全部立即備份」。' })}>
              <HardDriveDownload />
              立即備份
            </Button>
          </>
        }
      />
      {!agentEnabled && <AgentIntroCard />}
      <div className="grid grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)] gap-4 max-[900px]:grid-cols-1">
        <TrendCard />
        <OnThisDayCard />
      </div>
      <YearCard />
    </PageScroll>
  )
}
