/**
 * The preview's table: one row per tool folder found on this computer, as
 * each finishes counting. Skeleton rows show at once, before the backend
 * has answered, and one stays while folders are still being counted.
 *
 * Columns: tool, folder, files that look like conversations (and subagent
 * files where the layout shows them), size of everything counted, and the
 * earliest file date. A folder that could not be read, or was read only in
 * part, says why under its path; the other rows are unaffected.
 */
import { TriangleAlert } from 'lucide-react'
import { Skeleton } from '@/components/ui/skeleton'
import type { RootInventory } from '@/lib/backend-client/data-kinds'
import { cn } from '@/lib/cn'
import { useFormat, useI18n } from '@/lib/i18n'
import { ToolBadge } from '@/features/data-kinds/tool-badge'
import { folderLabel } from '@/features/data-kinds/tools'
import {
  conversationFiles,
  rowNote,
  subagentFiles,
  wire,
} from './preview-model'

function SkeletonRow() {
  return (
    <tr aria-hidden className="border-t [&_td]:px-3 [&_td]:py-2.5">
      <td>
        <Skeleton className="h-4 w-24" />
      </td>
      <td>
        <Skeleton className="h-3.5 w-48" />
      </td>
      <td>
        <Skeleton className="ml-auto h-4 w-12" />
      </td>
      <td>
        <Skeleton className="ml-auto h-4 w-14" />
      </td>
      <td>
        <Skeleton className="ml-auto h-4 w-20" />
      </td>
    </tr>
  )
}

function FolderRow({ row }: { row: RootInventory }) {
  const { t } = useI18n()
  const format = useFormat()
  const note = rowNote(row)
  const counted = row.readable && row.countBasis !== 'unavailable'
  const subagents = subagentFiles(row.counts)
  const label = folderLabel(row.tool, row.path)
  return (
    <tr
      data-tool={row.tool}
      className="animate-rise border-t align-top [&_td]:px-3 [&_td]:py-2"
    >
      <td>
        <ToolBadge id={row.tool} />
      </td>
      <td className="max-w-[300px]">
        {label && (
          <span className="block text-[13px] font-medium">{t(label)}</span>
        )}
        <span className="font-mono text-xs [overflow-wrap:anywhere] text-muted-foreground">
          {row.path}
        </span>
        {note && (
          <span
            className={cn(
              'mt-1 flex gap-1.5 text-xs leading-snug',
              row.readable ? 'text-muted-foreground' : 'text-foreground',
            )}
          >
            <TriangleAlert
              aria-hidden
              className="mt-px size-3.5 shrink-0 text-brand"
            />
            {t(note)}
          </span>
        )}
      </td>
      <td className="text-right whitespace-nowrap tabular">
        {counted ? (
          <>
            {format.number(conversationFiles(row.counts))}
            {subagents > 0 && (
              <span className="block text-xs text-muted-foreground">
                {t('enableAgent.preview.subagents', { count: subagents })}
              </span>
            )}
          </>
        ) : row.readable ? (
          <span className="text-xs text-muted-foreground">
            {t('enableAgent.preview.noCount')}
          </span>
        ) : (
          '—'
        )}
      </td>
      <td className="text-right whitespace-nowrap tabular">
        {row.readable ? format.bytes(wire(row.counts.sourceBytes)) : '—'}
      </td>
      <td className="text-right whitespace-nowrap tabular">
        {row.earliestAt === null ? '—' : format.date(row.earliestAt)}
      </td>
    </tr>
  )
}

export function PreviewTable({
  rows,
  counting,
}: {
  rows: readonly RootInventory[]
  /** Still walking: keep skeleton rows for the folders not counted yet. */
  counting: boolean
}) {
  const { t } = useI18n()
  const skeletons = counting ? (rows.length === 0 ? 3 : 1) : 0
  return (
    <div className="overflow-hidden rounded-lg border">
      <table className="w-full text-[13px]" aria-busy={counting}>
        <caption className="sr-only">
          {t('enableAgent.preview.tableLabel')}
        </caption>
        <thead className="bg-muted/60 text-xs text-muted-foreground">
          <tr className="[&_th]:px-3 [&_th]:py-2 [&_th]:font-medium">
            <th scope="col" className="text-left">
              {t('enableAgent.preview.columns.tool')}
            </th>
            <th scope="col" className="text-left">
              {t('enableAgent.preview.columns.folder')}
            </th>
            <th scope="col" className="text-right">
              {t('enableAgent.preview.columns.files')}
            </th>
            <th scope="col" className="text-right">
              {t('enableAgent.preview.columns.size')}
            </th>
            <th scope="col" className="text-right">
              {t('enableAgent.preview.columns.earliest')}
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <FolderRow key={row.rootId} row={row} />
          ))}
          {Array.from({ length: skeletons }, (_, index) => (
            <SkeletonRow key={`skeleton-${index}`} />
          ))}
        </tbody>
      </table>
    </div>
  )
}
