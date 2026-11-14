/**
 * Settings → 安全. Mirrors `src/features/settings/security-section.tsx`:
 * archive encryption (on/off through the re-encrypt dialog, the keychain and
 * the password while encrypted) and App Lock (switch, passcode with change and
 * remove, Touch ID on macOS, auto-lock and lock now while it is on). The two
 * are kept apart, as in the app.
 *
 * Design changes: a first line saying every data type shares the one
 * archive, password and unlock; the encryption row's description adds
 * 「套用到所有資料類型。」.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow, SettingsGroup, SettingsSection } from '@/components/app/setting-row'
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Kbd } from '@/components/ui/kbd'
import { Switch } from '@/components/ui/switch'
import { Note } from './parts'
import { usePlatform } from './pathkeep-kit/platform'
import { PlatformSelect, ProtoBar, ProtoHint, ProtoSelect, ProtoSwitch } from './pathkeep-kit/proto-bar'
import { RowSelect } from './pathkeep-kit/row-select'
import { KeychainDialog } from './security/keychain-dialog'
import { PasscodeDialog } from './security/passcode-dialog'
import { RekeyDialog, type RekeyMode } from './security/rekey-dialog'

const idleMinutes = [1, 5, 15, 30, 60]

type LockState = 'none' | 'off' | 'on'

export function SecuritySection() {
  const platform = usePlatform()
  const [encrypted, setEncrypted] = useState(true)
  const [keychainAvailable, setKeychainAvailable] = useState(true)
  const [keychainStored, setKeychainStored] = useState(true)
  const [lock, setLock] = useState<LockState>('on')
  const [hint, setHint] = useState('書房抽屜裡的筆記本')
  const [touchIdAvailable, setTouchIdAvailable] = useState(true)

  return (
    <SettingsSection title="安全">
      <ProtoBar>
        <PlatformSelect />
        <ProtoSwitch label="存檔已加密" checked={encrypted} onChange={setEncrypted} />
        <ProtoSelect<LockState>
          label="App 鎖定"
          value={lock}
          onChange={setLock}
          options={[
            { value: 'none', label: '沒有 App 密碼' },
            { value: 'off', label: '有密碼，鎖定關閉' },
            { value: 'on', label: '有密碼，鎖定開啟' },
          ]}
        />
        <ProtoSwitch label="有系統鑰匙圈" checked={keychainAvailable} onChange={setKeychainAvailable} />
        {platform === 'macos' && <ProtoSwitch label="Touch ID 能用" checked={touchIdAvailable} onChange={setTouchIdAvailable} />}
        <ProtoHint>「目前的密碼」打 wrong 就算打錯</ProtoHint>
      </ProtoBar>
      <Note className="border-solid bg-card/50">所有資料類型都在同一個存檔裡，用同一把密碼、同一次解鎖。</Note>
      <SettingsGroup title="存檔" />
      <ArchiveRows encrypted={encrypted} onEncrypted={setEncrypted} keyring={{ available: keychainAvailable, stored: keychainStored }} onStored={setKeychainStored} />
      <SettingsGroup title="App 鎖定" note="擋在視窗前的密碼。它不加密任何東西，和存檔密碼是兩回事。" />
      <AppLockRows lock={lock} onLock={setLock} hint={hint} onHint={setHint} touchIdAvailable={touchIdAvailable} />
    </SettingsSection>
  )
}

function ArchiveRows({ encrypted, onEncrypted, keyring, onStored }: { encrypted: boolean; onEncrypted: (on: boolean) => void; keyring: { available: boolean; stored: boolean }; onStored: (stored: boolean) => void }) {
  const [rekey, setRekey] = useState<RekeyMode | null>(null)
  const [keychainOpen, setKeychainOpen] = useState(false)
  const [forgetting, setForgetting] = useState(false)

  const forget = () => {
    setForgetting(true)
    setTimeout(() => {
      setForgetting(false)
      onStored(false)
      toast.success('已從鑰匙圈刪除密碼。')
    }, 400)
  }

  return (
    <>
      <SettingRow
        title="加密存檔"
        description={`${encrypted ? 'AES-256，只有你知道密碼。' : '你的歷史以未加密的形式儲存，能讀取你檔案的人都能看到。'}套用到所有資料類型。`}
        htmlFor="settings-encrypt"
        control={<Switch id="settings-encrypt" checked={encrypted} onCheckedChange={(next) => setRekey(next ? 'encrypt' : 'decrypt')} />}
      />
      {encrypted && (
        <>
          <SettingRow
            title="把密碼存到鑰匙圈"
            description={keyring.available ? '開啟 PathKeep 時不再詢問密碼。排程備份需要它。' : '這台電腦上沒有可用的系統鑰匙圈。'}
            htmlFor="settings-keychain"
            control={<Switch id="settings-keychain" checked={keyring.available && keyring.stored} disabled={!keyring.available || forgetting} onCheckedChange={(next) => (next ? setKeychainOpen(true) : forget())} />}
          />
          <SettingRow
            title="更改密碼"
            description="密碼一旦遺失，你的歷史就無法復原。"
            control={
              <Button size="sm" variant="outline" onClick={() => setRekey('change')}>
                更改…
              </Button>
            }
          />
        </>
      )}
      <RekeyDialog
        mode={rekey}
        keyring={keyring}
        onClose={() => setRekey(null)}
        onDone={(mode, keep) => {
          onEncrypted(mode !== 'decrypt')
          if (mode !== 'decrypt' && keyring.available) onStored(keep)
        }}
      />
      <KeychainDialog open={keychainOpen} onClose={() => setKeychainOpen(false)} onSaved={() => onStored(true)} />
    </>
  )
}

function AppLockRows({ lock, onLock, hint, onHint, touchIdAvailable }: { lock: LockState; onLock: (lock: LockState) => void; hint: string; onHint: (hint: string) => void; touchIdAvailable: boolean }) {
  const platform = usePlatform()
  const [saving, setSaving] = useState(false)
  const [passcode, setPasscode] = useState<'set' | 'change' | null>(null)
  const [confirmRemove, setConfirmRemove] = useState(false)
  const [removing, setRemoving] = useState(false)
  const [touchId, setTouchId] = useState(true)
  const [idle, setIdle] = useState('15')
  const hasPasscode = lock !== 'none'
  const enabled = lock === 'on'

  /** A config save: the controls wait for it. */
  const save = (apply: () => void) => {
    setSaving(true)
    setTimeout(() => {
      setSaving(false)
      apply()
    }, 250)
  }

  const toggle = (next: boolean) => {
    if (next && !hasPasscode) return setPasscode('set')
    save(() => onLock(next ? 'on' : 'off'))
  }

  const remove = () => {
    setRemoving(true)
    setTimeout(() => {
      setRemoving(false)
      onLock('none')
      onHint('')
      toast.success('App 密碼已移除，App 鎖定已關閉。')
    }, 400)
  }

  return (
    <>
      <SettingRow
        title="App 鎖定"
        description="開啟 PathKeep 或閒置一段時間後要求輸入密碼"
        htmlFor="settings-app-lock"
        control={<Switch id="settings-app-lock" checked={enabled} disabled={saving} onCheckedChange={toggle} />}
      />
      {hasPasscode && (
        <SettingRow
          title="App 密碼"
          description={hint ? `用來解鎖 PathKeep。提示：${hint}` : '用來解鎖 PathKeep'}
          control={
            <>
              <Button size="sm" variant="ghost" disabled={removing} onClick={() => setConfirmRemove(true)}>
                移除
              </Button>
              <Button size="sm" variant="outline" onClick={() => setPasscode('change')}>
                更改…
              </Button>
            </>
          }
        />
      )}
      {enabled && (
        <>
          {platform === 'macos' && (
            <SettingRow
              title="Touch ID"
              description={touchIdAvailable ? '用指紋解鎖 PathKeep' : '這台 Mac 現在無法使用 Touch ID'}
              htmlFor="settings-touch-id"
              control={<Switch id="settings-touch-id" checked={touchId} disabled={saving || (!touchIdAvailable && !touchId)} onCheckedChange={(next) => save(() => setTouchId(next))} />}
            />
          )}
          <SettingRow
            title="自動鎖定"
            description="閒置一段時間後鎖定 PathKeep"
            htmlFor="settings-auto-lock"
            control={
              <RowSelect
                id="settings-auto-lock"
                value={idle}
                disabled={saving}
                onChange={(value) => save(() => setIdle(value))}
                options={idleMinutes.map((minutes) => ({ value: String(minutes), label: minutes === 60 ? '1 小時' : `${minutes} 分鐘` }))}
              />
            }
          />
          <SettingRow
            title="立即鎖定"
            description={
              <>
                也可以按 <Kbd>{platform === 'macos' ? '⌘L' : 'Ctrl+L'}</Kbd>
              </>
            }
            control={
              <Button size="sm" variant="outline" onClick={() => toast('原型：這裡視窗會換成鎖定畫面。')}>
                鎖定
              </Button>
            }
          />
        </>
      )}
      <PasscodeDialog
        mode={passcode}
        savedHint={hint}
        onClose={() => setPasscode(null)}
        onSaved={(next) => {
          onHint(next)
          onLock('on')
        }}
      />
      <AlertDialog open={confirmRemove} onOpenChange={setConfirmRemove}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>要移除 App 密碼嗎？</AlertDialogTitle>
            <AlertDialogDescription>App 鎖定會關閉。你的存檔和存檔密碼不受影響。</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction onClick={remove}>移除</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}
