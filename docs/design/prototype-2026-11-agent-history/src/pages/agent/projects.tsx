/**
 * 專案. Laid out like the other pages (header, cards), with one row grid in
 * every view:
 * - 預設: 釘選 (your sections and pinned projects) above 其他專案 (the
 *   rest, with its own search and filters, so you find one and drag it up);
 * - 清單: every project in a sortable table with the number columns;
 * - 資料夾: by where they live on disk, collapsed until you open a folder.
 * Every project row has the same ⋯ menu (also on right-click); clicking a
 * row opens that project's conversations.
 */
import { FolderTree, LayoutList, Rows3 } from 'lucide-react'
import { useSearchParams } from 'react-router-dom'
import { PageHeader } from '@/components/app/section-card'
import { FluidPage } from '~/components/fluid-page'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { num } from '~/data/format'
import { DefaultView } from './projects/default-view'
import { FolderView } from './projects/folder-view'
import { ProjectDialogs } from './projects/project-settings'
import { ListView } from './projects/list-view'
import { SuggestionsSheet } from './projects/suggestions-sheet'
import { useProjectRows } from './projects/use-rows'

type View = 'default' | 'list' | 'tree'

export default function ProjectsPage() {
  const { list } = useProjectRows()
  const [params, setParams] = useSearchParams()
  const view: View = (params.get('view') as View | null) ?? 'default'
  const setParam = (key: string, value: string | null) =>
    setParams(
      (p) => {
        if (value) p.set(key, value)
        else p.delete(key)
        return p
      },
      { replace: true },
    )

  return (
    <FluidPage>
      <PageHeader
        title="專案"
        subtitle={`${num(list.length)} 個專案。一個專案可以包含好幾個資料夾；點一個專案看它的對話。`}
        actions={
          <ToggleGroup type="single" variant="outline" size="sm" value={view} onValueChange={(v) => v && setParam('view', v === 'default' ? null : v)} aria-label="檢視方式">
            <ToggleGroupItem value="default" className="px-2.5 text-[12.5px]">
              <Rows3 />
              預設
            </ToggleGroupItem>
            <ToggleGroupItem value="list" className="px-2.5 text-[12.5px]">
              <LayoutList />
              清單
            </ToggleGroupItem>
            <ToggleGroupItem value="tree" className="px-2.5 text-[12.5px]">
              <FolderTree />
              資料夾
            </ToggleGroupItem>
          </ToggleGroup>
        }
      />
      {view === 'default' ? <DefaultView /> : view === 'list' ? <ListView /> : <FolderView />}
      <SuggestionsSheet open={params.get('review') === '1'} onOpenChange={(o) => setParam('review', o ? '1' : null)} />
      <ProjectDialogs />
    </FluidPage>
  )
}
