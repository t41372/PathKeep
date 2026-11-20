/**
 * The tooltip of the Agent Insights bar charts. The stock one prints a
 * number or nothing; these say "No record" where nothing was recorded,
 * which is not the same as 0 (`agent-history.md` §1.11), and add how many
 * of the bar's conversations have the figure.
 *
 * Responsibilities: drawing what `describe` says about the hovered bar.
 * Not responsible for: what that is (each card's own `describe`).
 */
import { ChartTooltip } from '@/components/evilcharts/ui/recharts-tooltip'

export interface TooltipLine {
  key: string
  label: string
  color: string
  value: string
}

export interface BucketWords {
  title: string
  lines: TooltipLine[]
  note?: string | null
}

interface ContentProps<Row> {
  active?: boolean
  payload?: readonly { payload?: unknown }[]
  describe: (row: Row) => BucketWords
}

function Content<Row>({ active, payload, describe }: ContentProps<Row>) {
  const row = payload?.[0]?.payload as Row | undefined
  if (!active || !row) return <span className="p-4" />
  const words = describe(row)
  return (
    <div className="grid min-w-36 gap-1.5 rounded-lg border border-border/50 bg-background px-2.5 py-1.5 text-xs shadow-xl">
      <div className="font-medium">{words.title}</div>
      <div className="grid gap-1.5">
        {words.lines.map((line) => (
          <div key={line.key} className="flex items-center gap-2">
            <span
              aria-hidden
              className="size-2.5 shrink-0 rounded-[2px]"
              style={{ background: line.color }}
            />
            <span className="flex-1 text-muted-foreground">{line.label}</span>
            <span className="font-mono font-medium text-foreground tabular-nums">
              {line.value}
            </span>
          </div>
        ))}
      </div>
      {words.note && (
        <div className="max-w-60 text-muted-foreground">{words.note}</div>
      )}
    </div>
  )
}

/**
 * A chart child: Recharts' tooltip with this content. Null values are
 * kept, so a bar with nothing on record still has a tooltip that says so.
 */
export function BucketTooltip<Row>({
  describe,
}: {
  describe: (row: Row) => BucketWords
}) {
  return (
    <ChartTooltip
      cursor={false}
      filterNull={false}
      content={<Content<Row> describe={describe} />}
    />
  )
}
