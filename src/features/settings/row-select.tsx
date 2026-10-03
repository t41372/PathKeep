/** The compact select the prototype puts at the right of a settings row. */
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'

export interface RowOption {
  value: string
  label: string
}

export function RowSelect({
  id,
  value,
  options,
  onChange,
  disabled,
  label,
}: {
  id?: string
  value: string
  options: RowOption[]
  onChange: (value: string) => void
  disabled?: boolean
  /** Accessible name when the row title is not wired through `htmlFor`. */
  label?: string
}) {
  return (
    <Select value={value} onValueChange={onChange} disabled={disabled}>
      <SelectTrigger
        id={id}
        size="sm"
        aria-label={label}
        className="min-w-[120px] bg-card text-[13px]"
      >
        <SelectValue />
      </SelectTrigger>
      <SelectContent align="end">
        {options.map((option) => (
          <SelectItem key={option.value} value={option.value}>
            {option.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}
