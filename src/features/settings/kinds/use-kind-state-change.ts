/**
 * Changing a kind's state from Manage data types: on, paused or off.
 *
 * Responsibilities: showing the new state at once and going back to the
 * stored one if the backend refuses; sending Agent conversations that are
 * off through the preview instead (the backend refuses to turn them on any
 * other way); the toast after turning a kind off; and saying why a change
 * was refused (the last kind on, a change made elsewhere meanwhile).
 * Not responsible for: disabling what cannot be changed (the card does
 * that up front; the backend's `last-active-kind` stays the authority).
 */
import { useState } from 'react'
import { toast } from 'sonner'
import {
  dataKindsClient,
  dataKindsErrorCode,
  isStoreDamage,
  type DataKindId,
  type DataKindList,
  type KindState,
} from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { queryClient, queryKeys } from '@/lib/query'
import type { DataSpace } from '@/lib/spaces'
import { useOpenEnableAgent } from '@/features/enable-agent/links'

export function useKindStateChange() {
  const { t } = useI18n()
  const openEnable = useOpenEnableAgent()
  const [pending, setPending] = useState<
    Partial<Record<DataKindId, KindState>>
  >({})

  const change = async (
    space: DataSpace,
    list: DataKindList,
    next: KindState,
  ) => {
    const current = list.kinds.find((row) => row.kind === space.kind)?.state
    if (!current || next === current || pending[space.kind]) return
    // Agent conversations turn on only through the preview (`data-spaces.md` §2).
    if (space.kind === 'agent-history' && current === 'off') {
      openEnable()
      return
    }
    const name = t(space.name)
    setPending((now) => ({ ...now, [space.kind]: next }))
    try {
      const result = await dataKindsClient.setState(
        space.kind,
        next,
        list.policyToken,
      )
      queryClient.setQueryData<DataKindList>(queryKeys.dataKinds, {
        kinds: result.kinds,
        policyToken: result.policyToken,
      })
      void queryClient.invalidateQueries({ queryKey: queryKeys.agentSettings })
      if (next === 'off') toast(t('dataKinds.manage.turnedOff', { name }))
    } catch (error) {
      if (isStoreDamage(error)) {
        // The card says which file and when, once the list is read again.
        toast.error(t('dataKinds.manage.changeFailed', { name }), {
          description: `${t('dataKinds.damage.generic')} ${t('dataKinds.damage.notAffected')}`,
        })
        void queryClient.invalidateQueries({ queryKey: queryKeys.dataKinds })
        return
      }
      switch (dataKindsErrorCode(error)) {
        case 'last-active-kind':
          toast.error(t('dataKinds.manage.lastOnRefused'))
          break
        case 'policy-changed':
          toast(t('dataKinds.manage.changedElsewhere'))
          void queryClient.invalidateQueries({ queryKey: queryKeys.dataKinds })
          break
        case 'enable-preview-required':
          openEnable()
          break
        default:
          toast.error(t('dataKinds.manage.changeFailed', { name }), {
            description: describeError(error, 'set_data_kind_state'),
          })
      }
    } finally {
      setPending((now) => {
        const rest = { ...now }
        delete rest[space.kind]
        return rest
      })
    }
  }

  return { pending, change }
}
