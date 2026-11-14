/**
 * 更新: check, download and install, then restart. Mirrors
 * `src/features/settings/update-row.tsx` and every state it switches on.
 * The section's prototype switch can set the state directly; the buttons
 * also walk through it (立即檢查 finds a new version).
 */
import { useEffect } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Spinner } from '@/components/ui/spinner'
import { size } from '../pathkeep-kit/format'

export type UpdateState = 'idle' | 'checking' | 'upToDate' | 'available' | 'unsupported' | 'failed' | 'downloading' | 'installing' | 'installed'

export const updateStateLabels: Record<UpdateState, string> = {
  idle: '還沒檢查',
  checking: '正在檢查',
  upToDate: '已是最新版本',
  available: '有新版本',
  unsupported: '無法自行更新',
  failed: '檢查失敗',
  downloading: '正在下載',
  installing: '正在安裝',
  installed: '已安裝，待重新啟動',
}

const NEXT_VERSION = '0.6.1'
const DOWNLOAD_BYTES = 40_480_000
const notes = `PathKeep ${NEXT_VERSION}

- Fixes a crash when a browser profile folder is renamed during a backup.
- Search opens faster on large archives.`

export function UpdateRow({ state, onState, downloaded, onDownloaded }: { state: UpdateState; onState: (state: UpdateState) => void; downloaded: number; onDownloaded: (bytes: number) => void }) {
  // Checking, downloading and installing move on by themselves.
  useEffect(() => {
    if (state === 'checking') {
      const timer = setTimeout(() => onState('available'), 900)
      return () => clearTimeout(timer)
    }
    if (state === 'downloading') {
      if (downloaded >= DOWNLOAD_BYTES) {
        onState('installing')
        return
      }
      const timer = setTimeout(() => onDownloaded(Math.min(DOWNLOAD_BYTES, downloaded + DOWNLOAD_BYTES / 14)), 220)
      return () => clearTimeout(timer)
    }
    if (state === 'installing') {
      const timer = setTimeout(() => onState('installed'), 1200)
      return () => clearTimeout(timer)
    }
  }, [state, downloaded, onState, onDownloaded])

  let description: React.ReactNode = '只有在你要求時，PathKeep 才會檢查更新。'
  let control: React.ReactNode = (
    <Button size="sm" variant="outline" onClick={() => onState('checking')}>
      立即檢查
    </Button>
  )
  let below: React.ReactNode = null

  switch (state) {
    case 'checking':
      control = (
        <Button size="sm" variant="outline" disabled>
          <Spinner />
          正在檢查…
        </Button>
      )
      break
    case 'unsupported':
      description = '這個版本的 PathKeep 無法自行更新，請從發布頁面取得新版本。'
      control = (
        <Button size="sm" variant="outline" onClick={() => toast('原型：這裡會在瀏覽器打開 PathKeep 的發布頁面。')}>
          開啟發布頁面
        </Button>
      )
      break
    case 'available':
      description = `有新版本 ${NEXT_VERSION}。`
      control = (
        <Button
          size="sm"
          onClick={() => {
            onDownloaded(0)
            onState('downloading')
          }}
        >
          下載並安裝
        </Button>
      )
      below = <pre className="max-h-40 overflow-y-auto rounded-lg bg-muted p-3 font-sans text-xs whitespace-pre-wrap text-muted-foreground">{notes}</pre>
      break
    case 'upToDate':
      description = '已是最新版本。'
      break
    case 'downloading':
    case 'installing':
      description = state === 'installing' ? '正在安裝更新…' : '正在下載更新…'
      control = null
      below = (
        <div role="status" className="flex flex-col gap-1.5">
          <Progress aria-label="正在下載更新…" value={Math.round((Math.min(downloaded, DOWNLOAD_BYTES) / DOWNLOAD_BYTES) * 100)} />
          <span className="font-mono text-xs text-muted-foreground">
            {size(downloaded)} / {size(DOWNLOAD_BYTES)}
          </span>
        </div>
      )
      break
    case 'installed':
      description = `版本 ${NEXT_VERSION} 已安裝。重新啟動 PathKeep 後生效。`
      control = (
        <Button size="sm" onClick={() => toast('原型：PathKeep 在這裡重新啟動，換上新版本。')}>
          重新啟動
        </Button>
      )
      break
    case 'failed':
      description = <span className="text-destructive">無法檢查更新。error sending request for url (https://github.com/…/latest.json): connection timed out</span>
      break
  }

  return (
    <SettingRow title="更新" description={description} control={control}>
      {below}
    </SettingRow>
  )
}
