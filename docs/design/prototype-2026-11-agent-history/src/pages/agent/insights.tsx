/**
 * Agent insights: the Agent space's first page. A report to look at, not a
 * tool to operate, so everything is open at once like a dashboard: the five
 * numbers, the year at a glance, then (for the chosen range) time and
 * projects, tokens, when, what was done, and cost, most useful first. Every
 * card says what it counts and over which window, and every
 * chart is interactive (tooltips, click through to chats and projects).
 */
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { Skeleton } from '@/components/ui/skeleton'
import { PageHeader } from '@/components/app/section-card'
import { FluidPage } from '~/components/fluid-page'
import { insightsFor, rangeWindow, type Insights, type Range, type Scope } from './insights/data'
import {
  ChangesCard,
  CostCard,
  HarnessTimeCard,
  KpiRow,
  LongestCard,
  MethodNote,
  ModelsCard,
  PresenceCard,
  RhythmCard,
  TokenMixCard,
  TokensCard,
  ToolsCard,
  TopProjectsCard,
  WorkCard,
  YearSection,
} from './insights/cards'
import { ScopeControls } from './insights/parts'

/** Shown at once while the numbers are computed (the real app gets them from the backend). */
function InsightsSkeleton() {
  return (
    <>
      <div className="grid grid-cols-[repeat(auto-fit,minmax(190px,1fr))] gap-3">
        {Array.from({ length: 5 }, (_, i) => (
          <Skeleton key={i} className="h-[104px] rounded-xl" />
        ))}
      </div>
      <div className="grid grid-cols-[repeat(auto-fill,minmax(min(100%,440px),1fr))] gap-4">
        {Array.from({ length: 4 }, (_, i) => (
          <Skeleton key={i} className="h-[340px] rounded-xl" />
        ))}
      </div>
    </>
  )
}

/**
 * The range is remembered: someone who reads the last 30 days keeps getting the last
 * 30 days. The first time it is all time. In the app this is a saved setting; here
 * localStorage stands in. Tool and project filters are not remembered: they are a
 * one-off question, and coming back to a silently filtered report would mislead.
 */
const RANGE_KEY = 'pk-insights-range'
const RANGES: Range[] = ['7', '30', '90', '365', 'all']

function savedRange(): Range {
  try {
    const v = localStorage.getItem(RANGE_KEY) as Range | null
    if (v && RANGES.includes(v)) return v
  } catch {
    // Private window or blocked storage: fall back to all time.
  }
  return 'all'
}

function saveRange(range: Range) {
  try {
    localStorage.setItem(RANGE_KEY, range)
  } catch {
    // Not remembered; the page still works.
  }
}

export default function AgentInsightsPage() {
  const [scope, setScopeState] = useState<Scope>(() => ({ range: savedRange(), harnesses: [] }))
  const setScope = (next: Scope) => {
    if (next.range !== scope.range) saveRange(next.range)
    setScopeState(next)
  }
  const [computed, setComputed] = useState<{ scope: Scope; ins: Insights } | null>(null)
  // Compute after the first paint, so the page (and the space switch) shows at once.
  useEffect(() => {
    const t = setTimeout(() => setComputed({ scope, ins: insightsFor(scope) }), 0)
    return () => clearTimeout(t)
  }, [scope])
  const ins = computed?.scope === scope ? computed.ins : null

  return (
    <FluidPage>
      <PageHeader
        title={
          <>
            洞察
            {/* Every card but the year calendar follows the range, so the title says which one. */}
            <span className="ml-1 text-[18px] font-normal tracking-normal text-muted-foreground">（{rangeWindow[scope.range]}）</span>
          </>
        }
        actions={<ScopeControls scope={scope} onChange={setScope} />} />
      {ins ? <Dashboard ins={ins} scope={scope} /> : <InsightsSkeleton />}
    </FluidPage>
  )
}

/**
 * One wall of cards, no section headings: as many columns as fit, each card
 * as tall as its content, and no row leaves a gap (a short card no longer
 * forces an empty slot next to a tall one). Each brick measures its card and
 * spans that many 8 px rows. Placement is in reading order, never dense, so a
 * card's top edge is never above an earlier card's: the order below is the
 * order of importance. Nothing inside spans several columns (that would cut
 * the columns), which is why the year calendar sits above the wall.
 *
 * 456 px is the narrowest column the weekly rhythm grid fits in, and still
 * gives two columns at a 1080 px window.
 */
const WALL = 'grid grid-cols-[repeat(auto-fill,minmax(min(100%,456px),1fr))] auto-rows-[8px] gap-x-4'
const ROW = 8
const GAP = 16

function Brick({ children }: { children: React.ReactNode }) {
  const ref = useRef<HTMLDivElement>(null)
  const [span, setSpan] = useState(40)
  useLayoutEffect(() => {
    const card = ref.current?.firstElementChild as HTMLElement | null
    if (!card) return
    const measure = () => setSpan(Math.ceil((card.getBoundingClientRect().height + GAP) / ROW))
    measure()
    const ro = new ResizeObserver(measure)
    ro.observe(card)
    return () => ro.disconnect()
  }, [])
  return (
    <div ref={ref} style={{ gridRowEnd: `span ${span}` }}>
      <div className="flex flex-col">{children}</div>
    </div>
  )
}

function Dashboard({ ins, scope }: { ins: Insights; scope: Scope }) {
  return (
    <>
      <KpiRow ins={ins} />
      <YearSection scope={scope} />
      {/* Time and where it went, then tokens, then when, then what was done, then cost. */}
      <div className={WALL}>
        <Brick><WorkCard ins={ins} scope={scope} /></Brick>
        <Brick><TopProjectsCard ins={ins} scope={scope} /></Brick>
        <Brick><TokensCard ins={ins} scope={scope} /></Brick>
        <Brick><TokenMixCard ins={ins} /></Brick>
        <Brick><RhythmCard ins={ins} scope={scope} /></Brick>
        <Brick><PresenceCard ins={ins} /></Brick>
        <Brick><HarnessTimeCard ins={ins} /></Brick>
        <Brick><LongestCard ins={ins} /></Brick>
        <Brick><ChangesCard ins={ins} /></Brick>
        <Brick><ToolsCard ins={ins} /></Brick>
        <Brick><ModelsCard ins={ins} /></Brick>
        <Brick><CostCard ins={ins} /></Brick>
        <Brick><MethodNote /></Brick>
      </div>
    </>
  )
}
