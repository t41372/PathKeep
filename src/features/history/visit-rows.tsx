/** The timeline and search-result list: `VirtualRows` plus the row renderers. */
import { useCallback } from 'react'
import type { VisitItem } from './history-types'
import {
  DayHeader,
  LoadingRow,
  ResultRow,
  SessionHeader,
  VisitRow,
} from './timeline-rows'
import { ROW_HEIGHT, type TimelineRow } from './timeline-model'
import { VirtualRows } from './virtual-rows'

interface Props {
  rows: TimelineRow[]
  variant: 'timeline' | 'results'
  label: string
  selectedId: number | null
  isStarred: (url: string) => boolean
  browserName: (profileId: string) => string
  onSelect: (item: VisitItem) => void
  onOpen: (item: VisitItem) => void
  onEndReached: () => void
  scrollToIndex: number | null
  resetKey: string
  dimmed: boolean
}

const rowKey = (row: TimelineRow) => row.key

export function VisitRows({
  rows,
  variant,
  label,
  selectedId,
  isStarred,
  browserName,
  onSelect,
  onOpen,
  onEndReached,
  scrollToIndex,
  resetKey,
  dimmed,
}: Props) {
  const rowHeight = useCallback(
    (row: TimelineRow) =>
      row.kind === 'visit' && variant === 'results'
        ? ROW_HEIGHT.result
        : ROW_HEIGHT[row.kind],
    [variant],
  )

  const renderRow = useCallback(
    (row: TimelineRow) => {
      switch (row.kind) {
        case 'day':
          return <DayHeader row={row} />
        case 'session':
          return <SessionHeader row={row} />
        case 'loading':
          return <LoadingRow />
        case 'visit': {
          const Row = variant === 'results' ? ResultRow : VisitRow
          return (
            <Row
              item={row.item}
              selected={row.item.id === selectedId}
              starred={isStarred(row.item.url)}
              browserName={browserName(row.item.profileId)}
              onSelect={onSelect}
              onOpen={onOpen}
            />
          )
        }
      }
    },
    [variant, selectedId, isStarred, browserName, onSelect, onOpen],
  )

  return (
    <VirtualRows
      rows={rows}
      rowKey={rowKey}
      rowHeight={rowHeight}
      renderRow={renderRow}
      label={label}
      onEndReached={onEndReached}
      scrollToIndex={scrollToIndex}
      resetKey={resetKey}
      className={
        dimmed ? 'opacity-60 transition-opacity' : 'transition-opacity'
      }
    />
  )
}
