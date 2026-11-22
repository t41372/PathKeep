/**
 * 加密: today's step (src/features/onboarding/encryption-step.tsx): encrypted
 * or not, the password with its strength, and whether the system keychain
 * keeps it for background backups. §3.3 adds one sentence under the title and
 * one to the "不加密" description.
 */
import { Lock, LockOpen } from 'lucide-react'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { cn } from '@/lib/cn'
import { ChoiceCard, ChoiceGroup } from './choice'
import { passwordScore, type Draft } from './draft'
import { StepHeading } from './steps'

const strengthLabel = ['太短', '太短', '還可以', '不錯', '很強']
const strengthColor = ['bg-muted', 'bg-red', 'bg-[oklch(0.75_0.15_80)]', 'bg-green', 'bg-green']

export function EncryptionStep({ draft, onChange, keychainAvailable, mac }: { draft: Draft; onChange: (patch: Partial<Draft>) => void; keychainAvailable: boolean; mac: boolean }) {
  const score = passwordScore(draft.password)
  const mismatch = draft.confirm.length > 0 && draft.confirm !== draft.password

  return (
    <>
      <StepHeading
        title="要不要加密？"
        body={
          <>
            加密後，沒有密碼就打不開存檔。密碼丟了無法找回。
            <br />
            所有資料類型放在同一個存檔，這裡選的套用到全部。
          </>
        }
      />
      <ChoiceGroup value={draft.encrypt ? 'on' : 'off'} onValueChange={(value) => onChange({ encrypt: value === 'on' })} label="加密" className="grid grid-cols-2 gap-2.5">
        <ChoiceCard value="on" stacked leading={<Lock className="size-[18px]" strokeWidth={1.75} />} title="加密" body="AES-256，需要密碼解鎖" />
        <ChoiceCard value="off" stacked leading={<LockOpen className="size-[18px]" strokeWidth={1.75} />} title="不加密" body="不需要密碼。登入你這個帳號的人都能讀取。Agent 對話裡常有程式碼、金鑰和私人路徑，請想清楚。" />
      </ChoiceGroup>

      {draft.encrypt && (
        <div className="flex animate-rise flex-col gap-3">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="onboarding-password">密碼</Label>
            <Input
              id="onboarding-password"
              type="password"
              autoComplete="new-password"
              value={draft.password}
              placeholder="至少 10 個字元"
              onChange={(event) => onChange({ password: event.target.value })}
              className="h-[38px] bg-popover"
            />
          </div>
          <div className="flex gap-1" role="meter" aria-label={`密碼強度：${strengthLabel[score]}`} aria-valuemin={0} aria-valuemax={4} aria-valuenow={score}>
            {[1, 2, 3, 4].map((bar) => (
              <span key={bar} className={cn('h-1 flex-1 rounded-sm transition-colors duration-200', bar <= score ? strengthColor[score] : 'bg-muted')} />
            ))}
          </div>
          <span className="-mt-1.5 h-4 text-xs text-muted-foreground">{score > 0 && strengthLabel[score]}</span>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="onboarding-confirm">再輸入一次</Label>
            <Input
              id="onboarding-confirm"
              type="password"
              autoComplete="new-password"
              value={draft.confirm}
              aria-invalid={mismatch}
              aria-describedby={mismatch ? 'onboarding-mismatch' : undefined}
              onChange={(event) => onChange({ confirm: event.target.value })}
              className="h-[38px] bg-popover"
            />
            {mismatch && (
              <span id="onboarding-mismatch" className="text-xs text-destructive">
                兩次輸入的密碼不一樣。
              </span>
            )}
          </div>
          <label htmlFor="onboarding-keychain" className={cn('flex items-center justify-between gap-4 rounded-xl border bg-card px-4 py-3.5 shadow-card', keychainAvailable ? 'cursor-pointer' : 'opacity-80')}>
            <span className="flex flex-col gap-0.5">
              <span className="font-medium">{mac ? '存到 macOS 鑰匙圈' : '存到系統鑰匙圈'}</span>
              <span className="text-xs text-muted-foreground">
                {keychainAvailable ? '背景備份要靠它解鎖存檔，沒有它就無法運行。' : 'PathKeep 在這裡找不到可用的系統鑰匙圈，背景備份將無法解鎖加密的存檔。'}
              </span>
            </span>
            <Switch id="onboarding-keychain" checked={draft.keychain && keychainAvailable} disabled={!keychainAvailable} onCheckedChange={(keychain) => onChange({ keychain })} />
          </label>
        </div>
      )}
    </>
  )
}
