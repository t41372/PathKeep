/**
 * Prototype-only switches (theme, which data types are on,
 * upgrade state). Kept in localStorage so a reload stays where you were;
 * `?kinds=agent` style URL parameters override for screenshots.
 */
import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react'
import type { KindId } from './kinds'

export type KindPreset = 'agent' | 'two' | 'six' | 'browsing'

export interface ProtoState {
  theme: 'light' | 'dark'
  preset: KindPreset
  /** Upgrade case: Agent history exists in this version but the user hasn't turned it on. */
  agentEnabled: boolean
  /** The space last visited; shared pages (Ask, Settings) keep its tint and rail. */
  space: KindId
  /** Simulate the glass window material (the desktop shows through). */
  glass: boolean
  /** Estimate cost at list prices for chats the tool didn't record a cost for (a user setting). */
  estimateCost: boolean
}

const presetKinds: Record<KindPreset, KindId[]> = {
  agent: ['agent'],
  browsing: ['browsing'],
  two: ['browsing', 'agent'],
  six: ['browsing', 'agent', 'terminal', 'messages', 'webchat', 'notifications'],
}

const defaults: ProtoState = { theme: 'light', preset: 'two', agentEnabled: true, space: 'agent', glass: false, estimateCost: true }

function load(): ProtoState {
  let saved: Partial<ProtoState> = {}
  try {
    saved = JSON.parse(localStorage.getItem('pk-proto') ?? '{}') as Partial<ProtoState>
  } catch {
    saved = {}
  }
  const url = new URLSearchParams(location.search)
  const fromUrl: Partial<ProtoState> = {}
  if (url.get('theme') === 'dark' || url.get('theme') === 'light') fromUrl.theme = url.get('theme') as 'dark'
  const k = url.get('kinds')
  if (k && k in presetKinds) fromUrl.preset = k as KindPreset
  if (url.get('agent') === 'off') fromUrl.agentEnabled = false
  if (url.get('glass')) fromUrl.glass = url.get('glass') !== '0'
  return { ...defaults, ...saved, ...fromUrl }
}

interface Ctx extends ProtoState {
  set: (patch: Partial<ProtoState>) => void
  enabledKinds: KindId[]
}

const ProtoContext = createContext<Ctx | null>(null)

export function ProtoProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState(load)
  useEffect(() => {
    try {
      localStorage.setItem('pk-proto', JSON.stringify(state))
    } catch {
      // Private window: the switches just don't persist.
    }
    const html = document.documentElement
    html.classList.toggle('dark', state.theme === 'dark')
    if (state.glass) html.dataset.windowMaterial = 'glass'
    else delete html.dataset.windowMaterial
  }, [state])
  const value = useMemo<Ctx>(() => {
    const enabledKinds = presetKinds[state.preset].filter((k) => k !== 'agent' || state.agentEnabled)
    // The current space must be one that is on.
    const space = enabledKinds.includes(state.space) ? state.space : (enabledKinds[0] ?? 'browsing')
    return {
      ...state,
      space,
      set: (patch) =>
        setState((s) => {
          const next = { ...s, ...patch }
          // Turning Agent on from the browsing-only setup adds it as a second type.
          if (patch.agentEnabled === true && next.preset === 'browsing') next.preset = 'two'
          return next
        }),
      enabledKinds,
    }
  }, [state])
  return <ProtoContext.Provider value={value}>{children}</ProtoContext.Provider>
}

export function useProto() {
  const ctx = useContext(ProtoContext)
  if (!ctx) throw new Error('useProto outside ProtoProvider')
  return ctx
}
