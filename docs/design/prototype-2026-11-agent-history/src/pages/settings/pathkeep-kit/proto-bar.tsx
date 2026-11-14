/**
 * Prototype-only switches drawn inside a settings section or dialog, so a
 * reviewer can see each state the real component switches on. Dashed, and
 * every label starts with 「原型：」. Not part of the design.
 */
import type { ReactNode } from 'react'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { platformNames, setPlatform, usePlatform, type Platform } from './platform'

export function ProtoBar({ children }: { children: ReactNode }) {
  return <div className="flex flex-wrap items-center gap-x-5 gap-y-2 rounded-xl border border-dashed px-3.5 py-2.5 text-xs text-muted-foreground">{children}</div>
}

export function ProtoSwitch({ label, checked, onChange }: { label: string; checked: boolean; onChange: (next: boolean) => void }) {
  return (
    <label className="flex items-center gap-2">
      原型：{label}
      <Switch checked={checked} onCheckedChange={onChange} aria-label={`原型：${label}`} />
    </label>
  )
}

export function ProtoSelect<T extends string>({ label, value, options, onChange }: { label: string; value: T; options: { value: T; label: string }[]; onChange: (next: T) => void }) {
  return (
    <label className="flex items-center gap-2">
      原型：{label}
      <Select value={value} onValueChange={(next) => onChange(next as T)}>
        <SelectTrigger size="sm" className="h-7 min-w-[110px] bg-card text-xs" aria-label={`原型：${label}`}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option.value} value={option.value} className="text-xs">
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </label>
  )
}

/** The system the shared sections act as if they run on. */
export function PlatformSelect() {
  const platform = usePlatform()
  return (
    <ProtoSelect<Platform>
      label="系統"
      value={platform}
      onChange={setPlatform}
      options={(Object.keys(platformNames) as Platform[]).map((id) => ({ value: id, label: platformNames[id] }))}
    />
  )
}

/** A line of explanation inside a ProtoBar. */
export function ProtoHint({ children }: { children: ReactNode }) {
  return <span>原型：{children}</span>
}
