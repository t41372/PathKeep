/**
 * Step 4: encrypted or plaintext archive, the password, and whether the
 * system keychain keeps it for background backups.
 *
 * Only collects the choice; the archive is created on the last step.
 */
import { Lock, LockOpen } from 'lucide-react'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { cn } from '@/lib/cn'
import { useI18n, type MessageKey } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import { isMacOsHost } from '@/lib/runtime'
import { ChoiceCard, ChoiceGroup } from './choice-card'
import { passwordScore, type Draft } from './draft'

const strengthLabel: MessageKey[] = [
  'onboarding.encryption.weak',
  'onboarding.encryption.weak',
  'onboarding.encryption.ok',
  'onboarding.encryption.good',
  'onboarding.encryption.strong',
]

const strengthColor = [
  'bg-muted',
  'bg-red',
  'bg-[oklch(0.75_0.15_80)]',
  'bg-green',
  'bg-green',
]

export function EncryptionStep({
  draft,
  onChange,
}: {
  draft: Draft
  onChange: (patch: Partial<Draft>) => void
}) {
  const { t } = useI18n()
  const keyring = useSnapshot().keyringStatus
  const score = passwordScore(draft.password)
  const mismatch = draft.confirm.length > 0 && draft.confirm !== draft.password

  return (
    <>
      <ChoiceGroup
        value={draft.encrypt ? 'on' : 'off'}
        onValueChange={(value) => onChange({ encrypt: value === 'on' })}
        label={t('onboarding.steps.encryption.label')}
        className="grid grid-cols-2 gap-2.5"
      >
        <ChoiceCard
          value="on"
          stacked
          leading={<Lock className="size-[18px]" strokeWidth={1.75} />}
          title={t('onboarding.encryption.on.title')}
          body={t('onboarding.encryption.on.body')}
        />
        <ChoiceCard
          value="off"
          stacked
          leading={<LockOpen className="size-[18px]" strokeWidth={1.75} />}
          title={t('onboarding.encryption.off.title')}
          body={t('onboarding.encryption.off.body')}
        />
      </ChoiceGroup>

      {draft.encrypt && (
        <div className="flex animate-rise flex-col gap-3">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="onboarding-password">
              {t('onboarding.encryption.password')}
            </Label>
            <Input
              id="onboarding-password"
              type="password"
              autoComplete="new-password"
              value={draft.password}
              placeholder={t('onboarding.encryption.passwordHint')}
              onChange={(event) => onChange({ password: event.target.value })}
              className="h-[38px] bg-background dark:bg-popover"
            />
          </div>
          <div
            className="flex gap-1"
            role="meter"
            aria-label={t('onboarding.encryption.strength', {
              level: t(strengthLabel[score]),
            })}
            aria-valuemin={0}
            aria-valuemax={4}
            aria-valuenow={score}
          >
            {[1, 2, 3, 4].map((bar) => (
              <span
                key={bar}
                className={cn(
                  'h-1 flex-1 rounded-sm transition-colors duration-200',
                  bar <= score ? strengthColor[score] : 'bg-muted',
                )}
              />
            ))}
          </div>
          <span className="-mt-1.5 h-4 text-xs text-muted-foreground">
            {score > 0 && t(strengthLabel[score])}
          </span>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="onboarding-confirm">
              {t('onboarding.encryption.confirm')}
            </Label>
            <Input
              id="onboarding-confirm"
              type="password"
              autoComplete="new-password"
              value={draft.confirm}
              aria-invalid={mismatch}
              aria-describedby={mismatch ? 'onboarding-mismatch' : undefined}
              onChange={(event) => onChange({ confirm: event.target.value })}
              className="h-[38px] bg-background dark:bg-popover"
            />
            {mismatch && (
              <span
                id="onboarding-mismatch"
                className="text-xs text-destructive"
              >
                {t('onboarding.encryption.mismatch')}
              </span>
            )}
          </div>
          <label
            htmlFor="onboarding-keychain"
            className={cn(
              'flex items-center justify-between gap-4 rounded-xl border bg-card px-4 py-3.5 shadow-card',
              keyring.available ? 'cursor-pointer' : 'opacity-80',
            )}
          >
            <span className="flex flex-col gap-0.5">
              <span className="font-medium">
                {t(
                  isMacOsHost()
                    ? 'onboarding.encryption.keychainMac'
                    : 'onboarding.encryption.keychainOther',
                )}
              </span>
              <span className="text-xs text-muted-foreground">
                {t(
                  keyring.available
                    ? 'onboarding.encryption.keychainBody'
                    : 'onboarding.encryption.keychainUnavailable',
                )}
              </span>
            </span>
            <Switch
              id="onboarding-keychain"
              checked={draft.keychain && keyring.available}
              disabled={!keyring.available}
              onCheckedChange={(keychain) => onChange({ keychain })}
            />
          </label>
        </div>
      )}
    </>
  )
}
