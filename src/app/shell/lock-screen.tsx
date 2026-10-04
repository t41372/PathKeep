/**
 * Full-screen unlock prompt. Two flavors share it: the app passcode
 * (app lock) and the archive password (encrypted archive without a key).
 */
import { ArrowRight } from 'lucide-react'
import { useEffect, useRef, useState, type FormEvent } from 'react'
import { BrandMark } from '@/components/app/brand-mark'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { useSession } from '../session'

export function LockScreen({ kind }: { kind: 'app' | 'archive' }) {
  const { t } = useI18n()
  const session = useSession()
  const [secret, setSecret] = useState('')
  const [remember, setRemember] = useState(true)
  const [busy, setBusy] = useState(false)
  const [failed, setFailed] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => inputRef.current?.focus(), [])

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    if (!secret || busy) return
    setBusy(true)
    setFailed(false)
    try {
      if (kind === 'app') await session.unlockApp(secret)
      else await session.unlockArchive(secret, remember)
    } catch {
      setFailed(true)
      setSecret('')
      inputRef.current?.focus()
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/40 backdrop-blur-2xl">
      <form
        onSubmit={submit}
        className="flex w-[320px] animate-rise flex-col items-center gap-3 text-center"
      >
        <BrandMark className="mb-2 size-12" />
        <h1 className="text-lg font-semibold tracking-tight">
          {t(
            kind === 'app' ? 'shell.lock.appTitle' : 'shell.lock.archiveTitle',
          )}
        </h1>
        <p className="text-sm text-muted-foreground">
          {t(kind === 'app' ? 'shell.lock.appBody' : 'shell.lock.archiveBody')}
        </p>
        <div
          className={cn('mt-2 flex w-full gap-2', failed && 'animate-shake')}
        >
          <Input
            ref={inputRef}
            type="password"
            value={secret}
            autoComplete="current-password"
            aria-label={t(
              kind === 'app' ? 'shell.lock.passcode' : 'shell.lock.password',
            )}
            placeholder={t(
              kind === 'app' ? 'shell.lock.passcode' : 'shell.lock.password',
            )}
            onChange={(event) => setSecret(event.target.value)}
            aria-invalid={failed}
            className="h-10 bg-card"
          />
          <Button
            type="submit"
            size="icon"
            className="size-10 shrink-0"
            disabled={!secret || busy}
            aria-label={t('shell.lock.unlock')}
          >
            {busy ? <Spinner /> : <ArrowRight />}
          </Button>
        </div>
        {kind === 'archive' && (
          <Label className="gap-2 self-start text-sm font-normal text-muted-foreground">
            <Checkbox
              checked={remember}
              onCheckedChange={(value) => setRemember(value === true)}
            />
            {t('shell.lock.remember')}
          </Label>
        )}
        <p role="alert" className="h-5 text-sm text-destructive">
          {failed ? t('shell.lock.wrong') : ''}
        </p>
      </form>
    </div>
  )
}
