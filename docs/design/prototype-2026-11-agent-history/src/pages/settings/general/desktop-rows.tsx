/**
 * Open at login and the menu bar (tray) icon. Mirrors
 * `src/features/settings/desktop-rows.tsx`: skeletons while the OS is asked,
 * a switch that waits for the OS to confirm, and the error row with
 * 再試一次 when the state cannot be read (`failed`, set by the section's
 * prototype switch). On Windows and Linux the icon is called 系統匣圖示.
 */
import { useEffect, useState } from 'react'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import { usePlatform } from '../pathkeep-kit/platform'

type Setting = 'launchAtLogin' | 'menuBarIcon'

export function DesktopRows({ failed, onRetried }: { failed: boolean; onRetried: () => void }) {
  const platform = usePlatform()
  const mac = platform === 'macos'
  const [loading, setLoading] = useState(true)
  const [retrying, setRetrying] = useState(false)
  const [state, setState] = useState<Record<Setting, boolean>>({ launchAtLogin: true, menuBarIcon: true })

  useEffect(() => {
    const timer = setTimeout(() => setLoading(false), 450)
    return () => clearTimeout(timer)
  }, [])

  if (loading) {
    return (
      <>
        <Skeleton className="h-[76px] rounded-xl" />
        <Skeleton className="h-[76px] rounded-xl" />
      </>
    )
  }

  if (failed) {
    return (
      <SettingRow
        title={mac ? '登入時啟動和選單列圖示' : '登入時啟動和系統匣圖示'}
        description={<span className="text-destructive">PathKeep 無法讀取這些設定。The operation couldn’t be completed. (SMAppServiceErrorDomain error 1.)</span>}
        control={
          <Button
            size="sm"
            variant="outline"
            disabled={retrying}
            onClick={() => {
              setRetrying(true)
              setTimeout(() => {
                setRetrying(false)
                onRetried()
              }, 500)
            }}
          >
            再試一次
          </Button>
        }
      />
    )
  }

  return (
    <>
      <DesktopSwitch id="settings-launchAtLogin" title="登入時啟動" description="登入電腦時在背景啟動 PathKeep" checked={state.launchAtLogin} onChange={(next) => setState((s) => ({ ...s, launchAtLogin: next }))} />
      <DesktopSwitch id="settings-menuBarIcon" title={mac ? '選單列圖示' : '系統匣圖示'} description="顯示備份狀態和快速搜尋" checked={state.menuBarIcon} onChange={(next) => setState((s) => ({ ...s, menuBarIcon: next }))} />
    </>
  )
}

/** Moves only once the OS confirms, so it is disabled for a moment after each change. */
function DesktopSwitch({ id, title, description, checked, onChange }: { id: string; title: string; description: string; checked: boolean; onChange: (next: boolean) => void }) {
  const [pending, setPending] = useState(false)
  return (
    <SettingRow
      title={title}
      description={description}
      htmlFor={id}
      control={
        <Switch
          id={id}
          checked={checked}
          disabled={pending}
          onCheckedChange={(next) => {
            setPending(true)
            setTimeout(() => {
              setPending(false)
              onChange(next)
            }, 300)
          }}
        />
      }
    />
  )
}
