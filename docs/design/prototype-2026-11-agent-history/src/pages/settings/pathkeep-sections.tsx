/**
 * The sections every data type shares. Short on purpose: they exist so the
 * two-group nav reads true, and to say where per-type choices live now
 * (AI access, storage by type).
 */
import { FolderOpen } from 'lucide-react'
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { SettingRow, SettingsSection } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { bytes } from '~/data/format'
import { kinds, type Kind } from '~/kinds'
import { useProto } from '~/proto-state'
import { archiveBytes } from '../backup/sources-data'
import { Group, Note } from './parts'

function Toggle({ initial, label }: { initial: boolean; label: string }) {
  const [on, setOn] = useState(initial)
  return <Switch checked={on} onCheckedChange={setOn} aria-label={label} />
}

export function GeneralSection() {
  const { theme, set, enabledKinds } = useProto()
  return (
    <SettingsSection title="一般">
      <SettingRow
        title="語言"
        control={
          <Select defaultValue="zh-TW">
            <SelectTrigger size="sm" className="w-[140px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="system">跟系統</SelectItem>
              <SelectItem value="en">English</SelectItem>
              <SelectItem value="zh-CN">简体中文</SelectItem>
              <SelectItem value="zh-TW">繁體中文</SelectItem>
            </SelectContent>
          </Select>
        }
      />
      <SettingRow
        title="外觀"
        control={
          <ToggleGroup type="single" variant="outline" size="sm" value={theme} onValueChange={(v) => v && set({ theme: v as 'light' | 'dark' })}>
            <ToggleGroupItem value="light" className="px-3">
              淺色
            </ToggleGroupItem>
            <ToggleGroupItem value="dark" className="px-3">
              深色
            </ToggleGroupItem>
          </ToggleGroup>
        }
      />
      <SettingRow title="登入時開啟 PathKeep" description="在背景待命，照排程備份。" control={<Toggle initial label="登入時開啟" />} />
      <SettingRow title="選單列圖示" description="從選單列看備份狀態、立即備份、搜尋。" control={<Toggle initial label="選單列圖示" />} />
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

export function SecuritySection() {
  return (
    <SettingsSection title="安全">
      <Note className="border-solid bg-card/50">所有資料類型都在同一個存檔裡，用同一把密碼、同一次解鎖。</Note>
      <SettingRow title="存檔加密" description="已加密（SQLCipher）。關掉加密的存檔，Agent 對話也跟著不加密。" control={<Button variant="outline" size="sm">更改…</Button>} />
      <SettingRow title="密碼" control={<Button variant="outline" size="sm">更改密碼…</Button>} />
      <SettingRow title="用 Touch ID 解鎖" control={<Toggle initial label="Touch ID" />} />
      <SettingRow
        title="閒置多久自動鎖定"
        control={
          <Select defaultValue="15">
            <SelectTrigger size="sm" className="w-[120px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="5">5 分鐘</SelectItem>
              <SelectItem value="15">15 分鐘</SelectItem>
              <SelectItem value="60">1 小時</SelectItem>
              <SelectItem value="never">不鎖定</SelectItem>
            </SelectContent>
          </Select>
        }
      />
    </SettingsSection>
  )
}

export function AiSection() {
  const { enabledKinds } = useProto()
  return (
    <SettingsSection title="AI">
      <SettingRow title="對話模型" description="LM Studio · qwen3.8-27b（本機）" control={<Button variant="outline" size="sm">更改…</Button>} />
      <SettingRow title="Embedding 模型" description="內建的靜態模型（本機）" control={<Button variant="outline" size="sm">更改…</Button>} />
      <Group title="各類型的資料能不能給 AI 讀" intro="在各資料類型自己的設定裡分開開。">
        <div className="flex flex-col gap-1.5 text-[13px]">
          {kinds
            .filter((k) => k.real && enabledKinds.includes(k.id))
            .map((k) => (
              <Link key={k.id} to={`/settings/${k.id}`} className="flex w-fit items-center gap-1.5 text-brand hover:underline">
                <k.icon className="size-3.5" />
                {k.label}
                <span className="text-muted-foreground">{k.id === 'agent' ? '：預設都關' : '：助理與語意搜尋已開'}</span>
              </Link>
            ))}
        </div>
      </Group>
    </SettingsSection>
  )
}

export function StorageSection() {
  const { enabledKinds } = useProto()
  const shown = kinds.filter((k) => archiveBytes[k.id])
  const total = shown.reduce((s, k) => s + archiveBytes[k.id]!, 0)
  return (
    <SettingsSection title="儲存空間">
      <SettingRow
        title="存檔位置"
        description={<span className="font-mono text-[12px]">~/Library/Application Support/PathKeep/archive</span>}
        control={
          <Button variant="outline" size="sm">
            <FolderOpen />
            在 Finder 顯示
          </Button>
        }
      />
      <SettingRow title="存檔大小" description={`共 ${bytes(total)}`}>
        <div className="flex flex-col gap-2.5">
          {shown.map((k) => (
            <div key={k.id} className="flex flex-col gap-1">
              <div className="flex justify-between text-[13px]">
                <span className="flex items-center gap-1.5">
                  <k.icon className="size-3.5 text-muted-foreground" />
                  {k.label}
                  {!enabledKinds.includes(k.id) && <span className="text-muted-foreground">（沒有開啟，資料保留）</span>}
                </span>
                <span className="font-mono tabular">{bytes(archiveBytes[k.id]!)}</span>
              </div>
              <div className="h-1.5 overflow-hidden rounded-full bg-muted">
                <span className="block h-full rounded-full bg-foreground/60" style={{ width: `${(archiveBytes[k.id]! / total) * 100}%` }} />
              </div>
            </div>
          ))}
        </div>
      </SettingRow>
      <SettingRow title="清出空間" description="刪掉舊的安全備份、重建搜尋索引。不會刪任何資料。" control={<Button variant="outline" size="sm">看看能清多少…</Button>} />
    </SettingsSection>
  )
}

export function AboutSection() {
  return (
    <SettingsSection title="關於">
      <SettingRow title="PathKeep 0.6.0-dev" description="這是設計原型，資料全是假的。" />
      <SettingRow title="檢查更新" control={<Button variant="outline" size="sm">檢查</Button>} />
    </SettingsSection>
  )
}

/** A planned type that the six-type preset puts in the nav. */
export function PlannedKindSection({ kind }: { kind: Kind }) {
  return (
    <SettingsSection title={kind.label}>
      <Note>
        這個類型還在規劃，沒有設定。之後會讀：{kind.sources.join('、')}。原型裡放它，只是為了看設定的左欄在類型變多時排不排得下。
      </Note>
    </SettingsSection>
  )
}
