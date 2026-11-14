/**
 * Encrypt, change the password of, or decrypt the archive. Mirrors
 * `src/features/settings/rekey-dialog.tsx` (and the rules in `use-rekey.ts`):
 * the new password (or, for change and decrypt, the current one first), then
 * what will happen, then the run with its progress and Stop.
 *
 * Prototype: the current password is wrong when it is the word `wrong`;
 * anything else is right.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { MaintenanceProgress, operationSteps, StoppedLine } from '../pathkeep-kit/maintenance-progress'
import { ErrorLine, Notice } from '../pathkeep-kit/notice'
import { appPaths, usePlatform } from '../pathkeep-kit/platform'
import { emptySecret, NewSecretFields, secretProblem, type NewSecret } from './new-secret-fields'

export type RekeyMode = 'encrypt' | 'change' | 'decrypt'

/** Same rule onboarding uses for a new archive password. */
const ARCHIVE_PASSWORD_MIN_LENGTH = 10

const copy = {
  encrypt: { title: '加密存檔', intro: '設定一組密碼，之後開啟你的歷史需要用到它。', run: '加密', done: '存檔已加密。' },
  change: { title: '更改存檔密碼', intro: '先輸入目前的密碼，再設定新密碼。舊密碼會失效。', run: '更改密碼', done: '密碼已更改。' },
  decrypt: { title: '要關閉加密嗎？', intro: '先輸入目前的密碼，PathKeep 會改以不加密的方式儲存你的歷史。', run: '關閉加密', done: '加密已關閉。' },
} satisfies Record<RekeyMode, Record<string, string>>

export interface Keyring {
  available: boolean
  stored: boolean
}

export function RekeyDialog({ mode, keyring, onClose, onDone }: { mode: RekeyMode | null; keyring: Keyring; onClose: () => void; onDone: (mode: RekeyMode, keepInKeychain: boolean) => void }) {
  return (
    <Dialog open={mode !== null} onOpenChange={(open) => !open && onClose()}>
      {mode && <RekeyFlow key={mode} mode={mode} keyring={keyring} onClose={onClose} onDone={onDone} />}
    </Dialog>
  )
}

function RekeyFlow({ mode, keyring, onClose, onDone }: { mode: RekeyMode; keyring: Keyring; onClose: () => void; onDone: (mode: RekeyMode, keepInKeychain: boolean) => void }) {
  const [current, setCurrent] = useState('')
  const [secret, setSecret] = useState<NewSecret>(emptySecret)
  const [keep, setKeep] = useState(keyring.available && (mode === 'encrypt' || keyring.stored))
  const [loading, setLoading] = useState(false)
  const [preview, setPreview] = useState(false)
  const [wrongCurrent, setWrongCurrent] = useState(false)
  const [running, setRunning] = useState(false)
  const [stopped, setStopped] = useState(false)

  const needsCurrent = mode !== 'encrypt'
  const needsPassword = mode !== 'decrypt'
  const ready = (!needsCurrent || current.length > 0) && (!needsPassword || secretProblem(secret, ARCHIVE_PASSWORD_MIN_LENGTH) === null)
  const busy = loading || running

  const load = () => {
    setLoading(true)
    setTimeout(() => {
      setLoading(false)
      if (needsCurrent && current === 'wrong') setWrongCurrent(true)
      else setPreview(true)
    }, 500)
  }

  return (
    <DialogContent className="sm:max-w-md" showCloseButton={!running} onInteractOutside={(event) => running && event.preventDefault()} onEscapeKeyDown={(event) => running && event.preventDefault()}>
      <form
        className="flex flex-col gap-5"
        onSubmit={(event) => {
          event.preventDefault()
          if (busy) return
          if (!preview) {
            if (ready) load()
          } else {
            setStopped(false)
            setRunning(true)
          }
        }}
      >
        <DialogHeader>
          <DialogTitle>{copy[mode].title}</DialogTitle>
          <DialogDescription>{copy[mode].intro}</DialogDescription>
        </DialogHeader>

        {!preview && needsCurrent && (
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="rekey-current">目前的密碼</Label>
            <Input
              id="rekey-current"
              type="password"
              autoComplete="current-password"
              autoFocus
              disabled={busy}
              value={current}
              aria-invalid={wrongCurrent}
              aria-describedby={wrongCurrent ? 'rekey-error' : undefined}
              onChange={(event) => {
                setCurrent(event.target.value)
                setWrongCurrent(false)
              }}
            />
          </div>
        )}

        {!preview && needsPassword && (
          <>
            <NewSecretFields idPrefix="rekey" label="新密碼" secret={secret} minLength={ARCHIVE_PASSWORD_MIN_LENGTH} onChange={setSecret} disabled={busy} autoFocus={!needsCurrent} />
            {keyring.available && (
              <div className="flex items-center gap-2">
                <Checkbox id="rekey-keychain" checked={keep} onCheckedChange={(value) => setKeep(value === true)} />
                <Label htmlFor="rekey-keychain" className="font-normal">
                  存到系統鑰匙圈
                </Label>
              </div>
            )}
            <Notice tone="warning">沒有人能重設這組密碼，PathKeep 也不能。請把它存放在安全的地方。</Notice>
          </>
        )}

        {preview && <PreviewSteps mode={mode} />}

        {running && (
          <MaintenanceProgress
            steps={mode === 'change' ? operationSteps.passwordChange : operationSteps.encryptionChange}
            onDone={() => {
              setRunning(false)
              toast.success(copy[mode].done)
              onDone(mode, keep)
              onClose()
            }}
            onStopped={() => {
              setRunning(false)
              setStopped(true)
            }}
          />
        )}
        {stopped && <StoppedLine />}

        {wrongCurrent && <ErrorLine id="rekey-error">這不是目前的密碼，存檔沒有變動。</ErrorLine>}

        <DialogFooter>
          {preview ? (
            <Button type="button" variant="ghost" disabled={busy} onClick={() => setPreview(false)}>
              上一步
            </Button>
          ) : (
            <Button type="button" variant="ghost" disabled={busy} onClick={onClose}>
              取消
            </Button>
          )}
          <Button type="submit" variant={preview && mode === 'decrypt' ? 'destructive' : 'default'} disabled={busy || !ready}>
            {busy && <Spinner />}
            {preview ? copy[mode].run : '繼續'}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  )
}

function PreviewSteps({ mode }: { mode: RekeyMode }) {
  const platform = usePlatform()
  return (
    <div className="flex flex-col gap-3">
      <h3 className="text-sm font-medium">會發生什麼</h3>
      <ol className="flex list-decimal flex-col gap-2 pl-5 text-[13px] text-muted-foreground">
        <li>
          PathKeep 先把目前的存檔保存一份副本：
          <span className="mt-0.5 block font-mono text-xs break-all text-foreground">{mode === 'change' ? '' : appPaths(platform).rekeySnapshot}</span>
        </li>
        <li>{mode === 'decrypt' ? '再寫出一份不加密的存檔。' : '再用新密碼寫出一份加密的存檔。'}</li>
        <li>只有每一步都成功，新檔案才會取代舊檔案。副本會保留，需要時可以退回。</li>
      </ol>
      {mode === 'decrypt' && <Notice tone="warning">能讀取你檔案的人都能看到你的歷史。</Notice>}
    </div>
  )
}
