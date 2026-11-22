/**
 * 記錄檔, 當機報告 and 複製診斷資訊. Mirrors
 * `src/features/settings/diagnostics-rows.tsx`: the paths with copy
 * buttons, the latest crash (none, one, or one that closed PathKeep), and the
 * metadata-only report the user can read before copying. The report stays in
 * English on purpose, as in the app: maintainers read it.
 */
import { Copy } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { NOW } from '~/data/fixture'
import { browserSources } from '../../backup/sources-data'
import { dayAndTime } from '../pathkeep-kit/format'
import { appPaths, keyringBackends, usePlatform, type Platform } from '../pathkeep-kit/platform'

export type CrashState = 'none' | 'latest' | 'fatal'

const crash = {
  recordedAt: NOW - 86_400_000 - 3 * 3600_000,
  source: 'rust-panic',
  message: "thread 'search-index' panicked at crates/vault-search/src/segment.rs:211: segment checksum mismatch",
}

function copy(text: string, done: string, failed: string) {
  return navigator.clipboard.writeText(text).then(
    () => toast.success(done),
    (error: unknown) => toast.error(failed, { description: String(error) }),
  )
}

/** One labelled path with a copy button. */
function PathLine({ label, path }: { label: string; path: string }) {
  return (
    <li className="flex items-center gap-2">
      <span className="w-24 shrink-0 text-muted-foreground">{label}</span>
      <code className="min-w-0 flex-1 font-mono text-xs break-all">{path}</code>
      <Button size="icon-xs" variant="ghost" title="複製路徑" aria-label={`複製路徑：${label}`} onClick={() => void copy(path, '路徑已複製。', '無法複製路徑')}>
        <Copy />
      </Button>
    </li>
  )
}

export function LogsRow() {
  const paths = appPaths(usePlatform())
  return (
    <SettingRow
      title="記錄檔"
      description="回報問題時可以附上這些檔案。"
      control={
        <Button size="sm" variant="outline" onClick={() => toast('原型：這裡會打開記錄檔資料夾。')}>
          顯示記錄檔
        </Button>
      }
    >
      <ul className="flex flex-col gap-1.5 border-t pt-3 text-[13px]">
        <PathLine label="資料夾" path={paths.logs} />
        <PathLine label="應用程式記錄檔" path={paths.rustLog} />
        <PathLine label="視窗記錄檔" path={paths.frontendLog} />
      </ul>
    </SettingRow>
  )
}

export function CrashReportsRow({ state }: { state: CrashState }) {
  const paths = appPaths(usePlatform())
  return (
    <SettingRow
      title="當機報告"
      description="有東西意外停止時，PathKeep 會在這裡儲存一份報告。"
      control={
        <Button size="sm" variant="outline" onClick={() => toast('原型：這裡會打開當機報告資料夾。')}>
          顯示資料夾
        </Button>
      }
    >
      <div className="flex flex-col gap-2 border-t pt-3 text-[13px]">
        <ul>
          <PathLine label="資料夾" path={paths.crashReports} />
        </ul>
        {state === 'none' ? (
          <p className="text-muted-foreground">沒有當機紀錄。</p>
        ) : (
          <p className="break-words">
            最近一次（{dayAndTime(crash.recordedAt)}）：{crash.message}
            {state === 'fatal' && ' PathKeep 不得不關閉。'}
          </p>
        )}
      </div>
    </SettingRow>
  )
}

const userAgents: Record<Platform, string> = {
  macos: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)',
  windows: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36 Edg/142.0.0.0',
  linux: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)',
}

/** The report text, built the way the app builds it, from invented settings. */
function diagnosticsReport(platform: Platform, dirty: boolean, crashState: CrashState) {
  const paths = appPaths(platform)
  const lines = [
    `PathKeep 0.6.0-dev (a88d4fb${dirty ? ', modified' : ''})`,
    `Platform: ${userAgents[platform]}`,
    'Archive: Encrypted, initialized yes',
    'App Lock: on',
    `Keyring: ${keyringBackends[platform]}`,
    `Browsers selected: ${browserSources.length}`,
    'Background work: running, 2 at a time',
    'AI: on, semantic search on, assistant on, MCP off',
    'Online: link previews on, page summaries off',
    'Latest run: #1284 backup success at 2026-11-13T07:02:11Z',
    `Logs: ${paths.logs}`,
    `Crash reports: ${paths.crashReports}`,
    crashState === 'none' ? 'Latest crash: none' : `Latest crash: ${new Date(crash.recordedAt).toISOString()} ${crash.source}${crashState === 'fatal' ? ' (fatal)' : ''}: ${crash.message}`,
  ]
  return lines.join('\n')
}

export function DiagnosticsReportRow({ dirty, crashState }: { dirty: boolean; crashState: CrashState }) {
  const platform = usePlatform()
  const [shown, setShown] = useState(false)
  const report = diagnosticsReport(platform, dirty, crashState)
  return (
    <SettingRow
      title="複製診斷資訊"
      description="版本、系統和設定等資訊，可以貼到問題回報裡。不含歷史紀錄、密碼或金鑰。"
      control={
        <>
          <Button size="sm" variant="ghost" aria-expanded={shown} onClick={() => setShown((value) => !value)}>
            {shown ? '隱藏' : '查看會複製的內容'}
          </Button>
          <Button size="sm" variant="outline" onClick={() => void copy(report, '診斷資訊已複製。', '無法複製')}>
            複製
          </Button>
        </>
      }
    >
      {shown && <pre className="max-h-64 overflow-auto rounded-lg bg-muted p-3 font-mono text-xs break-all whitespace-pre-wrap">{report}</pre>}
    </SettingRow>
  )
}
