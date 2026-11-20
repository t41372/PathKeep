/**
 * "Scratch folders go to Loose folders" (`agent-history.md` §7, §5.9): the
 * places whose conversations don't each count as a project, as removable
 * chips, and adding one.
 *
 * Responsibilities: showing the saved places (the home folder itself as
 * `~`, others under it with `~/`), removing one, and adding one: picked in
 * the system's folder picker in the app, typed where there is none (a
 * plain browser on the dev bridge). A typed place must be a full path and
 * not already listed. While a change is on its way the chips can't be
 * changed again; a refused change puts the list back and says why.
 * Not responsible for: regrouping the projects, which the backend does
 * when the list changes.
 */
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { Plus, X } from 'lucide-react'
import { useId, useState, type FormEvent } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { Skeleton } from '@/components/ui/skeleton'
import { tildePath } from '@/features/agent-projects/project-words'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import {
  useSaveViewerPreferences,
  useViewerPreferences,
} from '@/lib/queries/agent-viewer'
import { hasTauriGuestApi } from '@/lib/runtime'

/** From the top of a disk: `/…`, `C:\…` or a `\\server\share` path. */
function isFullPath(path: string) {
  return /^(\/|[A-Za-z]:[\\/]|\\\\)/.test(path)
}

/** Without a trailing separator, so `/tmp/` and `/tmp` are one place. */
function tidy(path: string) {
  const trimmed = path.trim()
  return trimmed.length > 1 ? trimmed.replace(/[\\/]+$/, '') : trimmed
}

async function pickFolder(title: string) {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const picked = await open({ title, multiple: false, directory: true })
  return typeof picked === 'string' ? picked : null
}

function AddPlace({
  places,
  busy,
  onAdd,
}: {
  places: readonly string[]
  busy: boolean
  /** Resolves once saved; rejects (and the popover stays) when refused. */
  onAdd: (place: string) => Promise<boolean>
}) {
  const { t } = useI18n()
  const fieldId = useId()
  const [open, setOpen] = useState(false)
  const [path, setPath] = useState('')
  const [problem, setProblem] = useState<string | null>(null)
  const native = hasTauriGuestApi()

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    const place = tidy(path)
    if (!isFullPath(place))
      return setProblem(t('settingsAgent.scratch.notAbsolute'))
    if (places.includes(place))
      return setProblem(t('settingsAgent.scratch.already'))
    if (await onAdd(place)) {
      setOpen(false)
      setPath('')
    }
  }

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        setProblem(null)
      }}
    >
      <PopoverTrigger asChild>
        <Button variant="outline" size="sm" disabled={busy}>
          <Plus aria-hidden />
          {t('settingsAgent.scratch.add')}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-[360px] p-3">
        <form
          aria-label={t('settingsAgent.scratch.addTitle')}
          className="flex flex-col gap-2.5"
          onSubmit={(event) => void submit(event)}
        >
          <h3 className="text-[13px] font-medium">
            {t('settingsAgent.scratch.addTitle')}
          </h3>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor={fieldId}>{t('settingsAgent.scratch.folder')}</Label>
            <div className="flex gap-2">
              <Input
                id={fieldId}
                value={path}
                spellCheck={false}
                autoFocus
                className="font-mono text-xs"
                aria-invalid={problem !== null}
                aria-describedby={`${fieldId}-hint`}
                onChange={(event) => {
                  setPath(event.target.value)
                  setProblem(null)
                }}
              />
              {native && (
                <Button
                  type="button"
                  variant="outline"
                  onClick={() =>
                    void pickFolder(t('settingsAgent.scratch.addTitle')).then(
                      (picked) => picked && setPath(picked),
                    )
                  }
                >
                  {t('settingsAgent.scratch.choose')}
                </Button>
              )}
            </div>
            <span
              id={`${fieldId}-hint`}
              role={problem ? 'alert' : undefined}
              className={
                problem
                  ? 'text-xs text-destructive'
                  : 'text-xs text-muted-foreground'
              }
            >
              {problem ?? t('settingsAgent.scratch.typePath')}
            </span>
          </div>
          <div className="flex justify-end gap-2">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => setOpen(false)}
            >
              {t('settingsAgent.scratch.cancel')}
            </Button>
            <Button type="submit" size="sm" disabled={busy || !path.trim()}>
              {busy
                ? t('settingsAgent.scratch.saving')
                : t('settingsAgent.scratch.confirm')}
            </Button>
          </div>
        </form>
      </PopoverContent>
    </Popover>
  )
}

export function ScratchRow({ home }: { home: string | null }) {
  const { t } = useI18n()
  const prefs = useViewerPreferences()
  const save = useSaveViewerPreferences()
  const [busy, setBusy] = useState(false)
  const still = !!useReducedMotion()
  const places = prefs.data?.scratchLocations

  const change = async (next: string[]) => {
    setBusy(true)
    try {
      await save({ scratchLocations: next })
      return true
    } catch (error) {
      toast.error(t('settingsAgent.saveFailed'), {
        description: describeError(error),
      })
      return false
    } finally {
      setBusy(false)
    }
  }

  const label = (place: string) =>
    home !== null && tidy(home) === place
      ? t('settingsAgent.scratch.home')
      : tildePath(place, home)

  return (
    <SettingRow
      title={t('settingsAgent.scratch.title')}
      description={t('settingsAgent.scratch.description')}
      control={
        places && (
          <AddPlace
            places={places}
            busy={busy}
            onAdd={(place) => change([...places, place])}
          />
        )
      }
    >
      {!places ? (
        <div aria-hidden className="flex gap-1.5">
          <Skeleton className="h-6 w-16 rounded-md" />
          <Skeleton className="h-6 w-36 rounded-md" />
          <Skeleton className="h-6 w-28 rounded-md" />
        </div>
      ) : places.length === 0 ? (
        <p className="text-xs text-muted-foreground">
          {t('settingsAgent.scratch.none')}
        </p>
      ) : (
        <ul
          aria-label={t('settingsAgent.scratch.title')}
          aria-busy={busy}
          className="flex flex-wrap gap-1.5"
        >
          <AnimatePresence initial={false}>
            {places.map((place) => (
              <motion.li
                key={place}
                layout={!still}
                initial={still ? false : { opacity: 0, scale: 0.92 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={still ? { opacity: 0 } : { opacity: 0, scale: 0.92 }}
                transition={{ duration: 0.16, ease: 'easeOut' }}
                title={place}
                className="flex items-center gap-1 rounded-md border bg-background/60 py-0.5 pr-1 pl-2 font-mono text-[12px]"
              >
                {label(place)}
                <button
                  type="button"
                  disabled={busy}
                  aria-label={t('settingsAgent.scratch.remove', {
                    place: label(place),
                  })}
                  onClick={() =>
                    void change(places.filter((item) => item !== place))
                  }
                  className="rounded p-0.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:opacity-50"
                >
                  <X aria-hidden className="size-3" />
                </button>
              </motion.li>
            ))}
          </AnimatePresence>
        </ul>
      )}
    </SettingRow>
  )
}
