/**
 * "所有事件": the native records PathKeep keeps byte for byte, one row each,
 * with a link to the conversation event each one feeds. Records the reader
 * doesn't draw (snapshots, permission changes, cost state) appear here too.
 */
import { useVirtualizer } from '@tanstack/react-virtual'
import { useRef } from 'react'
import { bytes, num, time } from '~/data/format'
import type { Transcript } from '~/data/transcript'

export function RawEvents({ transcript, onJump }: { transcript: Transcript; onJump: (event: number) => void }) {
  const ref = useRef<HTMLDivElement>(null)
  const rows = transcript.raw
  const v = useVirtualizer({ count: rows.length, getScrollElement: () => ref.current, estimateSize: () => 30, overscan: 20 })
  const unseen = rows.filter((r) => r.event === null).length
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <p className="border-b px-5 py-2.5 text-[12.5px] text-muted-foreground">
        原始紀錄 {num(rows.length)} 筆，PathKeep 一字不改地保存。上一頁的對話是從這些紀錄整理出來的；其中 {num(unseen)} 筆（檔案快照、權限切換、用量狀態等）不會出現在對話裡。
      </p>
      <div className="grid grid-cols-[64px_minmax(0,1fr)_64px_72px_72px] gap-3 border-b px-5 py-1.5 text-[11.5px] font-medium text-muted-foreground">
        <span>#</span>
        <span>類型</span>
        <span>時間</span>
        <span className="text-right">大小</span>
        <span />
      </div>
      <div ref={ref} className="thin-scroll min-h-0 flex-1 overflow-y-auto">
        <div style={{ height: v.getTotalSize(), position: 'relative' }}>
          {v.getVirtualItems().map((item) => {
            const r = rows[item.index]
            return (
              <div
                key={item.key}
                style={{ position: 'absolute', top: 0, left: 0, right: 0, height: 30, transform: `translateY(${item.start}px)` }}
                className="grid grid-cols-[64px_minmax(0,1fr)_64px_72px_72px] items-center gap-3 px-5 font-mono text-[11.5px] hover:bg-muted/60"
              >
                <span className="text-muted-foreground tabular">{r.ordinal + 1}</span>
                <span className="truncate">{r.type}</span>
                <span className="text-muted-foreground tabular">{time(r.ts)}</span>
                <span className="text-right text-muted-foreground tabular">{bytes(r.bytes)}</span>
                {r.event !== null ? (
                  <button type="button" onClick={() => onJump(r.event!)} className="justify-self-end font-sans text-[12px] text-brand hover:underline">
                    在對話中看
                  </button>
                ) : (
                  <span className="justify-self-end font-sans text-[11.5px] text-muted-foreground">不顯示</span>
                )}
              </div>
            )
          })}
        </div>
      </div>
    </div>
  )
}
