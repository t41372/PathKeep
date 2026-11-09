/**
 * The "Pattern" tab of Advanced search: a regex put together from pieces
 * (this text, one of, a number, anything up to the next slash…), so nobody
 * has to remember regex syntax. Controlled; `composePattern` writes the regex.
 */
import { Plus, X } from 'lucide-react'
import { useId } from 'react'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { useI18n } from '@/lib/i18n'
import {
  editable,
  patternExamples,
  takesText,
  type EditablePattern,
  type EditablePiece,
  type PatternExample,
  type PieceKind,
} from './regex-pieces'

const kinds: readonly PieceKind[] = [
  'text',
  'oneOf',
  'number',
  'word',
  'segment',
  'any',
]

function PieceRow({
  piece,
  index,
  onChange,
  onRemove,
}: {
  piece: EditablePiece
  index: number
  onChange: (piece: EditablePiece) => void
  onRemove: () => void
}) {
  const { t } = useI18n()
  const name = t('historySearch.builder.pattern.piece', { n: index + 1 })
  return (
    <li className="flex items-center gap-1.5">
      <span
        aria-hidden
        className="w-4 shrink-0 text-right font-mono text-[11px] text-muted-foreground"
      >
        {index + 1}
      </span>
      <Select
        value={piece.kind}
        onValueChange={(kind) =>
          onChange({ ...piece, kind: kind as PieceKind })
        }
      >
        <SelectTrigger
          size="sm"
          aria-label={name}
          className="w-[190px] shrink-0 text-[13px]"
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {kinds.map((kind) => (
            <SelectItem key={kind} value={kind} className="text-[13px]">
              {t(`historySearch.builder.pattern.kind.${kind}`)}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {takesText(piece.kind) ? (
        <Input
          value={piece.value}
          onChange={(event) =>
            onChange({ ...piece, value: event.target.value })
          }
          aria-label={`${name}: ${t(`historySearch.builder.pattern.kind.${piece.kind}`)}`}
          placeholder={t(
            piece.kind === 'oneOf'
              ? 'historySearch.builder.pattern.oneOfPlaceholder'
              : 'historySearch.builder.pattern.textPlaceholder',
          )}
          autoComplete="off"
          spellCheck={false}
          className="h-8 min-w-0 flex-1 font-mono text-[12px]"
        />
      ) : (
        <span className="flex-1" />
      )}
      <Button
        type="button"
        variant="ghost"
        size="icon"
        aria-label={t('historySearch.builder.pattern.remove', { n: index + 1 })}
        onClick={onRemove}
        className="size-7 shrink-0 text-muted-foreground"
      >
        <X className="size-3.5" />
      </Button>
    </li>
  )
}

function Option({
  checked,
  onChange,
  label,
}: {
  checked: boolean
  onChange: (checked: boolean) => void
  label: string
}) {
  const id = useId()
  return (
    <div className="flex items-center gap-2">
      <Checkbox
        id={id}
        checked={checked}
        onCheckedChange={(next) => onChange(next === true)}
      />
      <Label htmlFor={id} className="text-[13px] font-normal">
        {label}
      </Label>
    </div>
  )
}

export function PatternFields({
  form,
  onChange,
}: {
  form: EditablePattern
  onChange: (form: EditablePattern) => void
}) {
  const { t } = useI18n()
  const setPieces = (pieces: EditablePiece[]) => onChange({ ...form, pieces })
  const hasOneOf = form.pieces.some((piece) => piece.kind === 'oneOf')
  return (
    <div className="flex flex-col gap-3">
      <p className="text-xs text-muted-foreground">
        {t('historySearch.builder.pattern.intro')}
      </p>
      <div className="flex flex-col gap-1.5">
        <span className="text-xs font-medium text-muted-foreground">
          {t('historySearch.builder.pattern.examples')}
        </span>
        <div className="flex flex-wrap gap-1.5">
          {(Object.keys(patternExamples) as PatternExample[]).map((name) => (
            <Button
              key={name}
              type="button"
              variant="outline"
              size="sm"
              onClick={() =>
                onChange({
                  pieces: editable(patternExamples[name]),
                  atStart: false,
                  atEnd: false,
                  matchCase: false,
                })
              }
              className="h-7 px-2.5 text-xs font-normal"
            >
              {t(`historySearch.builder.pattern.example.${name}`)}
            </Button>
          ))}
        </div>
      </div>
      <ol className="flex flex-col gap-1.5">
        {form.pieces.map((piece, index) => (
          <PieceRow
            key={piece.id}
            piece={piece}
            index={index}
            onChange={(next) =>
              setPieces(
                form.pieces.map((other) =>
                  other.id === next.id ? next : other,
                ),
              )
            }
            onRemove={() =>
              setPieces(form.pieces.filter((other) => other.id !== piece.id))
            }
          />
        ))}
      </ol>
      {hasOneOf && (
        <p className="-mt-1.5 pl-[22px] text-[11px] text-muted-foreground">
          {t('historySearch.builder.pattern.oneOfHint')}
        </p>
      )}
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-7 w-fit gap-1.5 px-2 text-xs"
          >
            <Plus className="size-3.5" aria-hidden />
            {t('historySearch.builder.pattern.add')}
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start">
          {kinds.map((kind) => (
            <DropdownMenuItem
              key={kind}
              onSelect={() =>
                setPieces([...form.pieces, ...editable([{ kind, value: '' }])])
              }
              className="text-[13px]"
            >
              {t(`historySearch.builder.pattern.kind.${kind}`)}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
      <div className="flex flex-col gap-2 border-t pt-3">
        <Option
          checked={form.atStart}
          onChange={(atStart) => onChange({ ...form, atStart })}
          label={t('historySearch.builder.pattern.atStart')}
        />
        <Option
          checked={form.atEnd}
          onChange={(atEnd) => onChange({ ...form, atEnd })}
          label={t('historySearch.builder.pattern.atEnd')}
        />
        <Option
          checked={form.matchCase}
          onChange={(matchCase) => onChange({ ...form, matchCase })}
          label={t('historySearch.builder.pattern.matchCase')}
        />
        <p className="text-[11px] text-muted-foreground">
          {t('historySearch.builder.pattern.matches')}
        </p>
      </div>
    </div>
  )
}
