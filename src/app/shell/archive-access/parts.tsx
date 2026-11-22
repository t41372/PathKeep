/**
 * Small pieces the recovery screen's panels share: a password field, a
 * folder or detail line that can be copied, a working state, an expanding
 * section and the step after a successful repair.
 *
 * The recovery screen runs while the desktop answers only the recovery
 * commands, so nothing here may call another one: no "Show in folder" (the
 * file manager command isn't among them) and no polled progress (neither is
 * the progress command). Paths are copied instead, and work in progress is an
 * honest sliding bar with what is happening.
 */
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { useState, type ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Progress } from '@/components/ui/progress'
import { Spinner } from '@/components/ui/spinner'
import { archiveAccessClient } from '@/lib/backend-client/archive-access'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { CopyButton } from '@/features/backup/path-actions'

/** Opens and closes its content with a short height and fade. */
export function Unfold({
  open,
  children,
}: {
  open: boolean
  children: ReactNode
}) {
  const still = useReducedMotion()
  return (
    <AnimatePresence initial={false}>
      {open && (
        <motion.div
          key="unfold"
          initial={still ? { opacity: 0 } : { height: 0, opacity: 0 }}
          animate={{ height: 'auto', opacity: 1 }}
          exit={still ? { opacity: 0 } : { height: 0, opacity: 0 }}
          transition={{ duration: 0.18, ease: 'easeOut' }}
          className="overflow-hidden"
        >
          {children}
        </motion.div>
      )}
    </AnimatePresence>
  )
}

export function PasswordField({
  id,
  label,
  hint,
  value,
  onChange,
  invalid = false,
  disabled = false,
}: {
  id: string
  label: string
  hint?: string
  value: string
  onChange: (value: string) => void
  invalid?: boolean
  disabled?: boolean
}) {
  return (
    <div className="flex flex-col gap-1.5 text-left">
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        type="password"
        autoComplete="current-password"
        value={value}
        disabled={disabled}
        aria-invalid={invalid}
        aria-describedby={hint ? `${id}-hint` : undefined}
        onChange={(event) => onChange(event.target.value)}
      />
      {hint && (
        <p id={`${id}-hint`} className="text-xs text-muted-foreground">
          {hint}
        </p>
      )}
    </div>
  )
}

/** A path on its own line, with a button that copies it. */
export function PathLine({ label, path }: { label: string; path: string }) {
  const { t } = useI18n()
  return (
    <div className="flex flex-col gap-1 text-left text-[13px]">
      <span className="text-muted-foreground">{label}</span>
      <span className="flex items-start gap-1.5">
        <span data-path className="min-w-0 flex-1 font-mono text-xs break-all">
          {path}
        </span>
        <CopyButton text={path} label={t('archiveAccess.copyFolder')} />
      </span>
    </div>
  )
}

/** The backend's own words, collapsed, to copy into a report. */
export function Details({ text }: { text: string }) {
  const { t } = useI18n()
  return (
    <details className="group w-full rounded-lg border px-3 py-2 text-left text-[13px]">
      <summary className="cursor-default text-muted-foreground select-none">
        {t('archiveAccess.details')}
      </summary>
      <div className="mt-2 flex items-start gap-1.5">
        <pre
          data-access-detail
          className="max-h-40 min-w-0 flex-1 overflow-auto font-mono text-xs whitespace-pre-wrap text-muted-foreground"
        >
          {text}
        </pre>
        <CopyButton text={text} label={t('archiveAccess.copyDetails')} />
      </div>
    </details>
  )
}

/** Work that runs inside one command: a sliding bar and what it is doing. */
export function Working({ label }: { label: string }) {
  return (
    <div role="status" className="flex flex-col gap-1.5 text-left">
      <Progress className="h-1.5" aria-label={label} />
      <span className="text-xs text-muted-foreground">{label}</span>
    </div>
  )
}

/**
 * What follows a repair or a restore that worked: the desktop starts again,
 * so the ordinary startup opens the repaired archive. Also when only the
 * opening had failed: the session would otherwise keep that failure.
 */
export function AfterRepair({ text }: { text?: string }) {
  const { t } = useI18n()
  const [restarting, setRestarting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const restart = async () => {
    setRestarting(true)
    setError(null)
    try {
      await archiveAccessClient.restart()
    } catch (restartError) {
      setError(describeError(restartError, 'restart_after_archive_recovery'))
      setRestarting(false)
    }
  }

  return (
    <div className="flex flex-col gap-3 text-left">
      {text && (
        <p role="status" className="text-[13px]">
          {text}
        </p>
      )}
      <Button
        className="self-start"
        disabled={restarting}
        onClick={() => void restart()}
      >
        {restarting && <Spinner />}
        {restarting
          ? t('archiveAccess.restarting')
          : t('archiveAccess.restart')}
      </Button>
      {error && (
        <p role="alert" className="text-[13px] text-destructive">
          {t('archiveAccess.restartFailed', { message: error })}
        </p>
      )}
    </div>
  )
}

/** A bordered block with a heading: one action the screen offers. */
export function Panel({
  title,
  children,
  labelledBy,
}: {
  title: string
  children: ReactNode
  labelledBy: string
}) {
  return (
    <section
      aria-labelledby={labelledBy}
      className="flex w-full flex-col gap-3 rounded-xl border bg-card p-[18px] text-left shadow-card"
    >
      <h2 id={labelledBy} className="text-sm font-semibold">
        {title}
      </h2>
      {children}
    </section>
  )
}
