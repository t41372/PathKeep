/**
 * @file security-section.tsx
 * @description Renders the Archive Key Settings section — the auto-save toggle
 * that controls whether the archive password is remembered in the system keychain.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Display an auto-save toggle for `config.rememberDatabaseKeyInKeyring`.
 * - ON: persist the flag. The actual keychain write happens inside
 *   `ArchiveUnlockGate` on the next successful unlock (or the gate's
 *   "remember" checkbox handles it immediately if the user unlocks then).
 * - OFF: persist the flag AND clear the stored keychain entry so auto-unlock
 *   stops working immediately.
 * - Show current keychain status (stored / not stored / unavailable) honestly.
 *
 * ## Not responsible for
 * - Prompting for the archive password (that is the `ArchiveUnlockGate`).
 * - Deciding whether the archive is encrypted (callers gate on that).
 * - Rendering the app-lock passcode flow.
 *
 * ## Dependencies
 * - `useShellData()` for snapshot and `saveConfig`.
 * - `backend` facade for `keyringClearDatabaseKey`.
 * - `useI18n()` (global) for all copy — keys are in `security.*` and `settings.*`.
 * - `Field`, `Toggle` from `paper-form-primitives`.
 * - `SettingsSavedChip` / `useSavedFeedback` for the auto-save confirmation.
 * - `PaperCard`, `PaperCardBody`, `PaperCardHeader` from the design system.
 *
 * ## Performance notes
 * - Pure section render; no background polling. The keychain flag is tiny and
 *   saves immediately via the existing `saveConfig` path.
 */

import { useCallback } from 'react'
import { PaperCard, PaperCardBody, PaperCardHeader } from '@/components/cards'
import { backend } from '@/lib/backend-client'
import { useI18n } from '@/lib/i18n'
import { useShellData } from '../../app/shell-data-context'
import { Field, Toggle } from './paper-form-primitives'
import type { SettingsSectionNavItem } from './section-nav-items'
import { SettingsSavedChip } from './settings-saved-feedback'
import { useSavedFeedback } from './use-saved-feedback'

interface SecuritySectionProps {
  navItem: SettingsSectionNavItem
}

/**
 * Renders the Archive Key section in Privacy & Access, exposing the keychain
 * remember-password toggle as a single auto-save control.
 */
export function SecuritySection({ navItem }: SecuritySectionProps) {
  const { snapshot, saveConfig } = useShellData()
  const { t } = useI18n()
  const { visible: savedVisible, flash } = useSavedFeedback()

  const keyringAvailable = snapshot?.keyringStatus?.available ?? false
  const keyringStored = snapshot?.keyringStatus?.storedSecret ?? false
  const keyringBackend = snapshot?.keyringStatus?.backend ?? ''
  const rememberEnabled =
    snapshot?.config?.rememberDatabaseKeyInKeyring ?? false

  const handleToggle = useCallback(
    async (next: boolean) => {
      // No snapshot (still loading) or no keychain on this machine → the toggle
      // is inert. Bail before any save so we never flash a dishonest "Saved" for
      // a switch that cannot actually hold state.
      if (!snapshot || !keyringAvailable) return
      try {
        if (!next) {
          // Clear the stored key so auto-unlock stops working immediately.
          await backend.keyringClearDatabaseKey().catch(() => undefined)
        }
        await saveConfig(
          { ...snapshot.config, rememberDatabaseKeyInKeyring: next },
          { quiet: true },
        )
        flash()
      } catch {
        // saveConfig already surfaces errors through the shell error channel;
        // no additional handling needed here.
      }
    },
    [flash, keyringAvailable, saveConfig, snapshot],
  )

  // Build the status line describing the current keychain state.
  let statusText: string
  if (!keyringAvailable) {
    statusText = t('security.keychainStatusUnavailable')
  } else if (keyringStored) {
    statusText = t('security.keychainStatusStored', { backend: keyringBackend })
  } else {
    statusText = t('security.keychainStatusNotStored')
  }

  return (
    <PaperCard id={navItem.id} testId="settings-security-section">
      <PaperCardHeader
        title={t('security.keychainSectionTitle')}
        right={<SettingsSavedChip visible={savedVisible} />}
      />
      <PaperCardBody>
        <Field
          label={t('security.keychainToggleLabel')}
          help={keyringAvailable ? t('security.keychainToggleHelp') : undefined}
        >
          <Toggle
            value={rememberEnabled && keyringAvailable}
            onChange={(next) => void handleToggle(next)}
            onLabel={t('settings.enabled')}
            offLabel={
              keyringAvailable
                ? t('settings.disabled')
                : t('security.keychainStatusUnavailable')
            }
            disabled={!keyringAvailable}
            testId="keychain-remember-toggle"
          />
          <p className="text-ink-muted mt-2 font-mono text-[10.5px]">
            {statusText}
          </p>
        </Field>
      </PaperCardBody>
    </PaperCard>
  )
}
