/**
 * Settings → 一般. Mirrors `src/features/settings/general-section.tsx`:
 * language, clock, Insights' time zone, appearance, window material (macOS
 * and Windows only), open at login and the menu bar / tray icon.
 *
 * Design change: the 連網 group (連結預覽, 頁面摘要) moved to Settings ›
 * 瀏覽紀錄; in its place one row points there, shown only while browsing
 * history is on. Language stays here.
 */
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { SettingRow, SettingsSection } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { useProto } from '~/proto-state'
import { DesktopRows } from './general/desktop-rows'
import { INSIGHTS_ZONE, InsightsZoneRow, TRAVEL_ZONE } from './general/insights-zone-row'
import { usePlatform } from './pathkeep-kit/platform'
import { PlatformSelect, ProtoBar, ProtoSwitch } from './pathkeep-kit/proto-bar'
import { RowSelect } from './pathkeep-kit/row-select'

export function GeneralSection() {
  const { enabledKinds } = useProto()
  const platform = usePlatform()
  const [travelling, setTravelling] = useState(false)
  const [desktopFailed, setDesktopFailed] = useState(false)
  return (
    <SettingsSection title="一般">
      <ProtoBar>
        <PlatformSelect />
        <ProtoSwitch label="電腦在另一個時區" checked={travelling} onChange={setTravelling} />
        <ProtoSwitch label="讀不到登入啟動和選單列" checked={desktopFailed} onChange={setDesktopFailed} />
      </ProtoBar>
      <LanguageRow />
      <ClockRow />
      <InsightsZoneRow computer={travelling ? TRAVEL_ZONE : INSIGHTS_ZONE} />
      <AppearanceRow />
      {platform !== 'linux' && <WindowMaterialRow />}
      <DesktopRows failed={desktopFailed} onRetried={() => setDesktopFailed(false)} />
      {enabledKinds.includes('browsing') && (
        <SettingRow
          title="連網"
          description="會連線到網站的只有瀏覽紀錄的連結預覽和頁面摘要，開關搬到「瀏覽紀錄」的設定裡了。"
          control={
            <Button variant="outline" size="sm" asChild>
              <Link to="/settings/browsing">瀏覽紀錄的設定</Link>
            </Button>
          }
        />
      )}
    </SettingsSection>
  )
}

function LanguageRow() {
  const [value, setValue] = useState('zh-TW')
  return (
    <SettingRow
      title="語言"
      description="介面語言"
      htmlFor="settings-language"
      control={
        <RowSelect
          id="settings-language"
          value={value}
          onChange={setValue}
          options={[
            { value: 'system', label: '跟隨系統' },
            { value: 'en', label: 'English' },
            { value: 'zh-CN', label: '简体中文' },
            { value: 'zh-TW', label: '繁體中文' },
          ]}
        />
      }
    />
  )
}

function ClockRow() {
  const [value, setValue] = useState('system')
  return (
    <SettingRow
      title="時間格式"
      description="時間怎麼顯示"
      htmlFor="settings-clock"
      control={
        <RowSelect
          id="settings-clock"
          value={value}
          onChange={setValue}
          options={[
            { value: 'system', label: '跟隨系統' },
            { value: 'h12', label: '12 小時制（下午 1:30）' },
            { value: 'h23', label: '24 小時制（13:30）' },
          ]}
        />
      }
    />
  )
}

/** Drives the prototype's own theme; 跟隨系統 follows the system's light or dark. */
function AppearanceRow() {
  const { theme, set } = useProto()
  const [preference, setPreference] = useState<string>(theme)
  const choose = (value: string) => {
    setPreference(value)
    const dark = value === 'dark' || (value === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches)
    set({ theme: dark ? 'dark' : 'light' })
  }
  return (
    <SettingRow
      title="外觀"
      description="淺色、深色或跟隨系統"
      htmlFor="settings-appearance"
      control={
        <RowSelect
          id="settings-appearance"
          value={preference}
          onChange={choose}
          options={[
            { value: 'light', label: '淺色' },
            { value: 'dark', label: '深色' },
            { value: 'system', label: '跟隨系統' },
          ]}
        />
      }
    />
  )
}

/** Drives the prototype's simulated glass window. */
function WindowMaterialRow() {
  const { glass, set } = useProto()
  return (
    <SettingRow
      title="視窗"
      description="不透明，或用系統的毛玻璃，透出模糊的桌面"
      htmlFor="settings-window-material"
      control={
        <RowSelect
          id="settings-window-material"
          value={glass ? 'glass' : 'solid'}
          onChange={(value) => set({ glass: value === 'glass' })}
          options={[
            { value: 'solid', label: '不透明' },
            { value: 'glass', label: '毛玻璃' },
          ]}
        />
      }
    />
  )
}
