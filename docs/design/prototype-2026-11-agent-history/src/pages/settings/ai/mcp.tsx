/**
 * The MCP server switch with its connection details, and what other apps
 * read through it (every request, newest first, a page at a time as the list
 * scrolls). Mirrors McpRow in src/features/settings/ai-section.tsx and
 * src/features/settings/mcp-log-row.tsx; the requests are fake.
 */
import { Check, Copy } from 'lucide-react'
import { useEffect, useState } from 'react'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { Switch } from '@/components/ui/switch'
import { num } from '~/data/format'
import { useAi } from './state'

const MCP_COMMAND = '/Applications/PathKeep.app/Contents/MacOS/pathkeep --worker mcp-server'

export function McpRow() {
  const ai = useAi()
  return (
    <SettingRow
      title="MCP 伺服器"
      description="讓 Claude 等工具讀取你的存檔，唯讀。"
      htmlFor="settings-mcp"
      control={<Switch id="settings-mcp" checked={ai.mcpOn} onCheckedChange={(mcpOn) => ai.set({ mcpOn })} />}
    >
      {ai.mcpOn && <McpDetails />}
    </SettingRow>
  )
}

function McpDetails() {
  const { mcpCase } = useAi()
  const [copied, setCopied] = useState(false)
  const copy = () =>
    void navigator.clipboard.writeText(MCP_COMMAND).then(() => {
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1500)
    })
  return (
    <div className="flex flex-col gap-2 border-t pt-3 text-[13px]">
      <p className="text-muted-foreground">PathKeep 不會一直執行伺服器。需要時，你的工具會用下面的指令自行啟動。這個開關只是允許它這樣做。</p>
      <span className="font-medium">連線資訊</span>
      {mcpCase === 'loading' ? (
        <Skeleton className="h-9 w-full" />
      ) : mcpCase === 'failed' ? (
        <p className="text-destructive">無法載入連線資訊。</p>
      ) : (
        <div className="flex items-start gap-2 rounded-lg bg-muted p-2.5">
          <code className="min-w-0 flex-1 font-mono text-xs break-all">{MCP_COMMAND}</code>
          <Button size="icon-xs" variant="ghost" aria-label={copied ? '已複製' : '複製'} onClick={copy}>
            {copied ? <Check /> : <Copy />}
          </Button>
        </div>
      )}
    </div>
  )
}

interface McpRecord {
  id: number
  tool: string
  query?: string
  results?: number
  returned?: number
  domain?: string
  at: string
}

const queries = ['tauri updater 簽章', 'sqlcipher 金鑰輪替', 'react virtual list 高度', '上週看的 launchd 文件', 'rust async cancel safety', 'bun workspaces', '台北 牙醫 預約', 'oklch 對比度']
const domains = [undefined, 'docs.rs', undefined, 'developer.apple.com', undefined, undefined, undefined, 'github.com']
const days = ['今天', '今天', '昨天', '昨天', '11月11日', '11月10日', '11月9日', '11月8日']

/** Requests 1…n, newest first; one in a few is not a search. */
function fakeRecords(from: number, count: number): McpRecord[] {
  return Array.from({ length: count }, (_, i) => {
    const n = from + i
    const day = days[Math.min(days.length - 1, Math.floor(n / 16))]
    const at = `${day} ${String(23 - (n % 14)).padStart(2, '0')}:${String((n * 37) % 60).padStart(2, '0')}`
    if (n % 9 === 4) return { id: n, tool: 'archive-status', at }
    if (n % 13 === 7) return { id: n, tool: 'usage-guide', at }
    if (n % 17 === 11) return { id: n, tool: 'list-profiles', at }
    const results = 40 + ((n * 53) % 900)
    return { id: n, tool: 'search-history', query: queries[n % queries.length], results, returned: n % 5 === 3 ? undefined : Math.min(20, results), domain: domains[n % domains.length], at }
  })
}

const PAGE = 50
const TOTAL = 130

export function McpLogRow() {
  const ai = useAi()
  const [records, setRecords] = useState<McpRecord[]>(() => fakeRecords(0, PAGE))
  const [loadingMore, setLoadingMore] = useState(false)
  const empty = ai.mcpCase === 'empty'
  if (!ai.mcpOn && (empty || ai.mcpCase === 'failed')) return null

  const loadMore = () => {
    if (loadingMore || records.length >= TOTAL) return
    setLoadingMore(true)
    window.setTimeout(() => {
      setRecords((r) => [...r, ...fakeRecords(r.length, Math.min(PAGE, TOTAL - r.length))])
      setLoadingMore(false)
    }, 800)
  }

  return (
    <SettingRow title="其他 App 讀了什麼" description="App 透過 MCP 伺服器發出的每個請求：問了什麼、拿到多少。">
      {ai.mcpCase === 'loading' ? (
        <div className="flex flex-col gap-2 border-t pt-3" aria-hidden>
          <Skeleton className="h-4 w-2/3" />
          <Skeleton className="h-4 w-1/2" />
        </div>
      ) : empty || ai.mcpCase === 'failed' ? (
        <p className="border-t pt-3 text-[13px] text-muted-foreground">{ai.mcpCase === 'failed' ? '無法載入請求：database is locked' : '還沒有 App 來問過。'}</p>
      ) : (
        <Requests records={records} hasMore={records.length < TOTAL} loadingMore={loadingMore} onLoadMore={loadMore} />
      )}
    </SettingRow>
  )
}

function Requests({ records, hasMore, loadingMore, onLoadMore }: { records: McpRecord[]; hasMore: boolean; loadingMore: boolean; onLoadMore: () => void }) {
  const [nearEnd, setNearEnd] = useState(false)
  useEffect(() => {
    if (nearEnd && hasMore && !loadingMore) onLoadMore()
  }, [nearEnd, hasMore, loadingMore, onLoadMore])
  return (
    <div
      className="max-h-[364px] overflow-y-auto border-t"
      onScroll={(event) => {
        const el = event.currentTarget
        setNearEnd(el.scrollHeight - el.scrollTop - el.clientHeight < 8 * 52)
      }}
    >
      <ul aria-label="透過 MCP 伺服器的請求" className="flex w-full flex-col">
        {records.map((record) => (
          <li key={record.id} className="flex h-[52px] w-full flex-col justify-center gap-0.5 border-b">
            <Request record={record} />
          </li>
        ))}
      </ul>
      <div className="py-2 text-xs text-muted-foreground" aria-live="polite">
        {loadingMore ? (
          <span className="flex items-center gap-2">
            <Spinner />
            正在載入更早的請求…
          </span>
        ) : hasMore ? null : (
          '所有請求都在這裡了。'
        )}
      </div>
    </div>
  )
}

const toolLabels: Record<string, string> = {
  'search-history': '搜尋了你的歷史',
  'archive-status': '查看了存檔狀態',
  'usage-guide': '讀了 PathKeep 的用法',
}

function Request({ record }: { record: McpRecord }) {
  const tool = toolLabels[record.tool] ?? `使用了 ${record.tool}`
  const details = [
    record.at,
    record.results != null && (record.returned != null ? `交出 ${num(record.returned)} 頁（符合 ${num(record.results)} 頁）` : `符合 ${num(record.results)} 頁`),
    record.domain && `限 ${record.domain}`,
  ].filter(Boolean)
  return (
    <>
      <span className="truncate text-[13px]">
        <span className="font-medium">{tool}</span>
        {record.query && <span className="text-muted-foreground"> · 「{record.query}」</span>}
      </span>
      <span className="truncate text-xs text-muted-foreground tabular-nums">{details.join(' · ')}</span>
    </>
  )
}
