/**
 * Browsing's results in ⌘K: up to six pages whose words match, one row per
 * page, each opening History on that visit; above them, the note that the
 * search index is still catching up when it is.
 *
 * Responsibilities: the search and its rows. Not responsible for the group
 * around them or the "Search … for …" row after them, which the palette
 * draws for every space (`src/app/shell/command-palette.tsx`).
 */
import { useQuery } from '@tanstack/react-query'
import { CommandItem, CommandShortcut } from '@/components/ui/command'
import { Favicon } from '@/components/app/favicon'
import { SearchCoverageNotice } from '@/components/app/search-coverage'
import { explorerClient } from '@/lib/backend-client/explorer'
import { useFormat } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import { spacePath, type PaletteResultsProps } from '@/lib/spaces'

export default function PaletteResults({ term, open }: PaletteResultsProps) {
  const format = useFormat()
  const visits = useQuery({
    queryKey: [...queryKeys.archiveData, 'palette', term],
    // One row per page: a page visited ten times should not fill the list.
    queryFn: () =>
      explorerClient.searchPages({
        q: term,
        limit: 6,
        sort: 'relevance',
        includeTotal: false,
      }),
    staleTime: 30_000,
  })

  return (
    <>
      {/* An index still catching up: these six can't be the whole answer. */}
      <SearchCoverageNotice
        status={visits.data?.searchState ?? null}
        className="px-2 py-1.5"
      />
      {(visits.data?.items ?? []).map((visit) => (
        <CommandItem
          key={visit.id}
          value={`visit:${visit.id}`}
          onSelect={() =>
            open(
              spacePath(
                'browsing',
                'history',
                `?q=${encodeURIComponent(term)}&visit=${visit.id}`,
              ),
            )
          }
        >
          <Favicon
            domain={visit.domain}
            src={visit.favicon?.dataUrl}
            className="size-4"
          />
          <span className="truncate">{visit.title || visit.url}</span>
          <CommandShortcut className="tracking-normal">
            {format.dayAndTime(visit.visitedAt)}
          </CommandShortcut>
        </CommandItem>
      ))}
    </>
  )
}
