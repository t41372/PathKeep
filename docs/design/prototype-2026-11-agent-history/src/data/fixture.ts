/**
 * The invented archive: about 300 projects with a long tail, about 1,800
 * top-level conversations, thousands of subagents, partial usage and
 * duration coverage. Shapes and proportions follow what a real machine
 * showed (counts only); every name and line of text is made up.
 */
import { clamp, hex, lognormal, mulberry32, pick, weighted, type Rng } from './rng'
import { openings, projectWords } from './text-bank'
import type {
  Chat,
  Entrypoint,
  HarnessId,
  Location,
  MergeSuggestion,
  Project,
  ProjectKind,
  TitleSource,
  Tokens,
} from './types'

/** "Now" for the whole prototype: 2026-11-13 16:20 in Taipei. */
export const NOW = Date.UTC(2026, 10, 13, 8, 20)
export const DAY = 86_400_000
const WEEK = 7 * DAY

export const harnessStart: Record<HarnessId, number> = {
  claude: Date.UTC(2025, 3, 20),
  codex: Date.UTC(2025, 7, 10),
  pi: Date.UTC(2026, 2, 1),
  deepseek: Date.UTC(2026, 9, 5),
}

export const zeroTokens = (): Tokens => ({
  input: 0,
  cacheWrite: 0,
  cacheRead: 0,
  output: 0,
  reasoning: 0,
})

export function addTokens(into: Tokens, add: Tokens) {
  into.input += add.input
  into.cacheWrite += add.cacheWrite
  into.cacheRead += add.cacheRead
  into.output += add.output
  into.reasoning += add.reasoning
}

export const totalTokens = (t: Tokens) =>
  t.input + t.cacheWrite + t.cacheRead + t.output

// ---------------------------------------------------------------- projects

interface Draft {
  project: Project
  start: number
  end: number
  weight: number
  mix: readonly (readonly [HarnessId, number])[]
  movedAt?: number
}

const hourWeights = [
  6, 4, 2.2, 1.2, 0.5, 0.3, 0.3, 0.6, 1.5, 3, 5, 6, 4, 4.5, 6, 7, 7, 6, 4, 4,
  6, 8, 9, 8,
]

function slugs(rng: Rng) {
  const used = new Set<string>([
    'orbit-notes',
    'lumen-api',
    'tide-charts',
    'kiln',
    'dotfiles',
    'thesis-figures',
  ])
  return () => {
    for (;;) {
      const name = `${pick(rng, projectWords.prefix)}-${pick(rng, projectWords.suffix)}`
      if (!used.has(name)) {
        used.add(name)
        return name
      }
    }
  }
}

function makeProject(
  id: string,
  name: string,
  kind: ProjectKind,
  parent: string,
  path: string,
  remote?: string,
): Project {
  return {
    id,
    name,
    kind,
    remote,
    parent,
    locations: [{ path, role: 'main', exists: true, chatCount: 0 }],
    pinned: false,
    gone: false,
    chatIds: [],
    harnesses: [],
    firstActive: 0,
    lastActive: 0,
    agentMs: 0,
    tokens: zeroTokens(),
    linesAdded: 0,
    linesRemoved: 0,
    weekly: [],
  }
}

const repoMix = (rng: Rng): Draft['mix'] => {
  const style = rng()
  if (style < 0.25) return [['claude', 1]]
  if (style < 0.4) return [['codex', 1]]
  return [
    ['claude', 0.35 + rng() * 0.2],
    ['codex', 0.45 + rng() * 0.2],
    ['pi', rng() < 0.2 ? 0.08 : 0],
  ]
}

function buildProjects(rng: Rng): Draft[] {
  const drafts: Draft[] = []
  const nextSlug = slugs(rng)
  const window = (from: number, to: number, medianDays: number) => {
    const start = from + rng() * (to - from)
    const len = clamp(lognormal(rng, medianDays, 1.1), 1, 500) * DAY
    return { start, end: Math.min(NOW, start + len) }
  }

  // Flagship projects, most active first.
  const orbit = makeProject(
    'p-orbit',
    'orbit-notes',
    'repo',
    '~/code',
    '~/code/orbit-notes',
    'github.com/kestrel-labs/orbit-notes',
  )
  orbit.pinned = true
  orbit.locations.push(
    { path: '~/projects/2025/orbit-notes', role: 'former', exists: false, chatCount: 0 },
    { path: '~/code/orbit-notes/src-tauri', role: 'subfolder', exists: true, chatCount: 0 },
    { path: '~/code/orbit-notes/.claude/worktrees/search-stream', role: 'worktree', exists: true, branch: 'feat/search-stream', chatCount: 0 },
    { path: '~/code/orbit-notes/.claude/worktrees/rekey-progress', role: 'worktree', exists: false, branch: 'feat/rekey-progress', chatCount: 0 },
    { path: '~/code/orbit-notes/.claude/worktrees/settings-tabs', role: 'worktree', exists: false, branch: 'refactor/settings-tabs', chatCount: 0 },
    { path: '~/code/orbit-notes/.claude/worktrees/cjk-phrase', role: 'worktree', exists: true, branch: 'feat/cjk-phrase', chatCount: 0 },
    { path: '~/.codex/worktrees/7f3a/orbit-notes', role: 'worktree', exists: false, branch: 'fix/safari-timezone', chatCount: 0 },
    { path: '~/.codex/worktrees/b21c/orbit-notes', role: 'worktree', exists: true, branch: 'feat/cursor-paging', chatCount: 0 },
  )
  drafts.push({
    project: orbit,
    start: Date.UTC(2025, 4, 2),
    end: NOW,
    weight: 150,
    mix: [['claude', 0.5], ['codex', 0.46], ['pi', 0.04]],
    movedAt: Date.UTC(2025, 11, 1),
  })

  const lumen = makeProject('p-lumen', 'lumen-api', 'repo', '~/work/acme', '~/work/acme/lumen-api', 'github.com/acme-co/lumen-api')
  lumen.pinned = true
  lumen.locations.push(
    { path: '~/.codex/worktrees/0c9e/lumen-api', role: 'worktree', exists: true, branch: 'feat/cursor-api', chatCount: 0 },
    { path: '~/work/acme/lumen-api/services/billing', role: 'subfolder', exists: true, chatCount: 0 },
  )
  drafts.push({ project: lumen, start: Date.UTC(2025, 8, 1), end: NOW - 2 * DAY, weight: 95, mix: [['codex', 0.7], ['claude', 0.3]] })

  const tide = makeProject('p-tide', 'tide-charts', 'repo', '~/code', '~/code/tide-charts', 'github.com/kestrel-labs/tide-charts')
  tide.pinned = true
  drafts.push({ project: tide, start: Date.UTC(2025, 6, 10), end: NOW - 5 * DAY, weight: 70, mix: [['claude', 0.7], ['codex', 0.3]] })

  const kiln = makeProject('p-kiln', 'kiln', 'repo', '~/projects/2026', '~/projects/2026/kiln', 'github.com/kestrel-labs/kiln')
  drafts.push({ project: kiln, start: Date.UTC(2026, 1, 3), end: NOW - 1 * DAY, weight: 58, mix: [['claude', 0.5], ['codex', 0.4], ['pi', 0.1]] })

  const dot = makeProject('p-dotfiles', 'dotfiles', 'repo', '~', '~/dotfiles', 'github.com/kestrel-labs/dotfiles')
  drafts.push({ project: dot, start: Date.UTC(2025, 4, 10), end: NOW - 9 * DAY, weight: 24, mix: [['claude', 0.6], ['codex', 0.4]] })

  const thesis = makeProject('p-thesis', 'thesis-figures', 'repo', '~/projects/2026', '~/projects/2026/thesis-figures', 'github.com/kestrel-labs/thesis-figures')
  drafts.push({ project: thesis, start: Date.UTC(2026, 3, 1), end: NOW - 20 * DAY, weight: 30, mix: [['claude', 0.8], ['pi', 0.2]] })

  const reading = makeProject('p-reading', '讀書筆記', 'folder', '~/Documents', '~/Documents/讀書筆記')
  drafts.push({ project: reading, start: Date.UTC(2025, 9, 1), end: NOW - 3 * DAY, weight: 22, mix: [['claude', 0.9], ['pi', 0.1]] })

  // The long tail.
  const parents: {
    parent: string
    count: number
    kind: 'repo' | 'folder' | 'mixed'
    owner?: string
    from: number
    to: number
    median: number
    goneRate: number
  }[] = [
    { parent: '~/code', count: 86, kind: 'repo', owner: 'kestrel-labs', from: Date.UTC(2025, 4, 1), to: NOW, median: 18, goneRate: 0.04 },
    { parent: '~/projects/2026', count: 60, kind: 'mixed', owner: 'kestrel-labs', from: Date.UTC(2026, 0, 1), to: NOW, median: 12, goneRate: 0.06 },
    { parent: '~/projects/2025', count: 42, kind: 'mixed', owner: 'kestrel-labs', from: Date.UTC(2025, 3, 25), to: Date.UTC(2025, 11, 20), median: 14, goneRate: 0.55 },
    { parent: '~/work/acme', count: 28, kind: 'repo', owner: 'acme-co', from: Date.UTC(2025, 7, 15), to: NOW, median: 30, goneRate: 0.08 },
    { parent: '~/work/clients', count: 12, kind: 'repo', owner: 'acme-co', from: Date.UTC(2025, 9, 1), to: NOW, median: 20, goneRate: 0.25 },
    { parent: '~/Documents', count: 20, kind: 'folder', from: Date.UTC(2025, 4, 1), to: NOW, median: 10, goneRate: 0.05 },
    { parent: '~/Desktop', count: 16, kind: 'folder', from: Date.UTC(2025, 4, 1), to: NOW, median: 3, goneRate: 0.45 },
    { parent: '~/Downloads', count: 6, kind: 'folder', from: Date.UTC(2025, 6, 1), to: NOW, median: 1, goneRate: 0.6 },
    { parent: '~/.config', count: 4, kind: 'folder', from: Date.UTC(2025, 4, 1), to: NOW, median: 6, goneRate: 0 },
  ]
  const folderNames = [...projectWords.folders]
  let n = 0
  for (const spec of parents) {
    for (let i = 0; i < spec.count; i++) {
      const asRepo = spec.kind === 'repo' || (spec.kind === 'mixed' && rng() < 0.75)
      let name: string
      if (spec.parent === '~/.config') name = ['nvim', 'zsh', 'karabiner', 'ghostty'][i]
      else if (asRepo) name = nextSlug()
      else name = folderNames.length ? folderNames.splice(Math.floor(rng() * folderNames.length), 1)[0] : nextSlug()
      const path = `${spec.parent}/${name}`
      const remote = asRepo && spec.owner && rng() < 0.85 ? `github.com/${spec.owner}/${name}` : undefined
      const project = makeProject(`p-${++n}`, name, asRepo ? 'repo' : 'folder', spec.parent, path, remote)
      if (rng() < spec.goneRate) project.locations[0].exists = false
      if (asRepo && rng() < 0.18) {
        const branch = `${pick(rng, ['feat', 'fix', 'refactor'])}/${pick(rng, projectWords.prefix)}`
        project.locations.push({
          path: `${path}/.claude/worktrees/${branch.split('/')[1]}`,
          role: 'worktree',
          exists: rng() < 0.4,
          branch,
          chatCount: 0,
        })
      }
      if (asRepo && rng() < 0.12) {
        project.locations.push({ path: `${path}/${pick(rng, ['docs', 'web', 'scripts', 'crates/core'])}`, role: 'subfolder', exists: true, chatCount: 0 })
      }
      const w = window(spec.from, spec.to, spec.median)
      drafts.push({
        project,
        start: w.start,
        end: w.end,
        weight: 0,
        mix: asRepo ? repoMix(rng) : [['claude', 0.75], ['codex', 0.2], ['pi', 0.05]],
      })
    }
  }

  // Two projects only DeepSeek touched once.
  const ds = makeProject('p-ds', 'deepseek-trial', 'folder', '~/projects/2026', '~/projects/2026/deepseek-trial')
  drafts.push({ project: ds, start: Date.UTC(2026, 9, 6), end: Date.UTC(2026, 9, 20), weight: 2, mix: [['deepseek', 1]] })

  // Scratch: home folder and temporary folders, grouped as one bucket.
  const scratch = makeProject('p-scratch', '零散資料夾', 'scratch', '', '~')
  scratch.locations[0].role = 'temp'
  for (let i = 0; i < 14; i++) {
    scratch.locations.push({
      path: rng() < 0.7 ? `/tmp/${pick(rng, ['try', 'repro', 'scratch', 'bench', 'test'])}-${hex(rng, 4)}` : `/private/var/folders/x1/${hex(rng, 6)}/T/${hex(rng, 5)}`,
      role: 'temp',
      exists: false,
      chatCount: 0,
    })
  }
  drafts.push({ project: scratch, start: Date.UTC(2025, 4, 1), end: NOW, weight: 46, mix: [['claude', 0.6], ['codex', 0.35], ['pi', 0.05]] })

  // Folder names repeat in real life (two `api` folders in different places).
  const rename = (d: Draft, name: string) => {
    const old = d.project.name
    d.project.name = name
    for (const l of d.project.locations) l.path = l.path.replace(`/${old}`, `/${name}`)
    if (d.project.remote) d.project.remote = d.project.remote.replace(`/${old}`, `/${name}`)
  }
  const byParent = (parent: string) => drafts.filter((d) => d.project.parent === parent && d.weight === 0 && d.project.kind === 'repo')
  const pairs: [string, string, string][] = [
    ['api', '~/code', '~/work/acme'],
    ['web', '~/code', '~/projects/2026'],
    ['scripts', '~/work/acme', '~/projects/2025'],
    ['docs', '~/projects/2026', '~/work/clients'],
  ]
  pairs.forEach(([name, a, b], i) => {
    rename(byParent(a)[i + 1], name)
    rename(byParent(b)[i + 1], name)
  })

  // Weights for the tail: Zipf by a shuffled rank, so the long tail has many single chats.
  const tail = drafts.filter((d) => d.weight === 0)
  tail.sort(() => rng() - 0.5)
  tail.forEach((d, rank) => {
    d.weight = Math.max(1, Math.round(330 / Math.pow(rank + 4, 1.02) + (rng() < 0.15 ? rng() * 3 : 0)))
  })
  return drafts
}

// ------------------------------------------------------------------- chats

function modelFor(harness: HarnessId, t: number, rng: Rng): string {
  const d = (y: number, m: number) => Date.UTC(y, m, 1)
  if (harness === 'claude') {
    if (t < d(2025, 7)) return 'claude-opus-4-6'
    if (t < d(2025, 10)) return 'claude-opus-4-7'
    if (t < d(2026, 1)) return 'claude-opus-4-8'
    if (t < d(2026, 4)) return rng() < 0.7 ? 'claude-fable-5' : 'claude-opus-5'
    if (t < d(2026, 7)) return rng() < 0.6 ? 'claude-fable-5-1' : 'claude-opus-5'
    if (t < d(2026, 10)) return 'claude-opus-5-5'
    return rng() < 0.8 ? 'claude-opus-5-5' : 'claude-fable-5-2'
  }
  if (harness === 'codex') {
    if (t < d(2026, 2)) return 'gpt-6-sol'
    if (t < d(2026, 7)) return 'gpt-6-astra'
    return 'gpt-6.1-sol'
  }
  if (harness === 'pi') return rng() < 0.85 ? 'kimi-k3' : 'qwen3.8-27b'
  return 'deepseek-v4'
}

function claudeVersion(t: number) {
  const months = Math.floor((t - Date.UTC(2025, 3, 1)) / (30 * DAY))
  return `2.${Math.floor(months / 3)}.${(months * 7) % 40}`
}

function sampleTime(rng: Rng, start: number, end: number): number {
  const span = Math.max(DAY, end - start)
  for (;;) {
    let t = start + span * Math.pow(rng(), 0.8)
    const day = new Date(t + 8 * 3600_000)
    const weekday = day.getUTCDay()
    if ((weekday === 0 || weekday === 6) && rng() < 0.45) continue
    const hour = weighted(rng, hourWeights.map((w, h) => [h, w] as const))
    const midnight = Math.floor((t + 8 * 3600_000) / DAY) * DAY - 8 * 3600_000
    t = midnight + hour * 3600_000 + rng() * 3600_000
    if (t <= NOW - 10 * 60_000) return t
  }
}

function harnessFor(rng: Rng, mix: Draft['mix'], t: number): HarnessId {
  const available = mix.filter(([h, w]) => w > 0 && harnessStart[h] <= t)
  if (available.length === 0) return 'claude'
  return weighted(rng, available)
}

function turnTokens(rng: Rng, harness: HarnessId): Tokens {
  if (harness === 'claude') {
    const output = lognormal(rng, 1700, 0.9)
    return {
      output,
      cacheRead: output * lognormal(rng, 270, 0.5),
      cacheWrite: output * lognormal(rng, 3.5, 0.5),
      input: output * lognormal(rng, 0.025, 0.6),
      reasoning: 0,
    }
  }
  if (harness === 'codex') {
    const prompt = lognormal(rng, 150_000, 0.6)
    const output = lognormal(rng, 2100, 0.8)
    return { input: prompt * 0.03, cacheRead: prompt * 0.97, cacheWrite: 0, output, reasoning: output * 0.42 }
  }
  const output = lognormal(rng, 900, 0.8)
  return { input: lognormal(rng, 6000, 0.7), cacheRead: lognormal(rng, 26_000, 0.7), cacheWrite: harness === 'pi' ? lognormal(rng, 3000, 0.7) : 0, output, reasoning: output * 0.3 }
}

const price: Record<string, [number, number, number, number]> = {
  // input, cache write, cache read, output; USD per million tokens (invented)
  claude: [5, 6.25, 0.5, 25],
  pi: [0.6, 0.6, 0.15, 2.5],
}

function buildChats(rng: Rng, drafts: Draft[]): Chat[] {
  const chats: Chat[] = []
  let id = 0
  for (const draft of drafts) {
    const { project } = draft
    const prNext = { n: Math.floor(rng() * 300) + 20 }
    // People run several agents side by side, especially with worktrees:
    // some sessions start close to the previous one in the same project.
    const worktrees = project.locations.filter((l) => l.role === 'worktree').length
    const sideBySide = project.kind !== 'repo' ? 0.1 : worktrees >= 2 ? 0.82 : worktrees === 1 ? 0.55 : 0.3
    let previous = 0
    for (let c = 0; c < draft.weight; c++) {
      const nearPrevious = previous > 0 && rng() < sideBySide
      const started = nearPrevious
        ? Math.min(NOW - 15 * 60_000, Math.max(draft.start, previous + (rng() - 0.3) * 20 * 60_000))
        : sampleTime(rng, draft.start, draft.end)
      previous = started
      const harness = harnessFor(rng, draft.mix, started)
      // Location: former path before the move, otherwise main, a worktree or a subfolder.
      const options = project.locations.filter((l) => l.role !== 'former')
      let location: Location = project.locations[0]
      if (draft.movedAt && started < draft.movedAt) {
        location = project.locations.find((l) => l.role === 'former') ?? location
      } else if (project.kind === 'scratch') {
        location = pick(rng, project.locations)
      } else if (options.length > 1 && rng() < 0.3) {
        const extra = options.filter((l) => l.role !== 'main')
        const want = harness === 'codex' ? extra.filter((l) => l.path.includes('.codex')) : extra.filter((l) => !l.path.includes('.codex'))
        location = pick(rng, want.length ? want : extra)
      }
      location.chatCount++

      const medianTurns = harness === 'codex' ? 4 : harness === 'claude' ? 6 : 5
      const turns = Math.round(clamp(lognormal(rng, medianTurns, 1.0), 1, 420))
      let agentMs = 0
      let gaps = 0
      let tokens: Tokens | null = zeroTokens()
      for (let t = 0; t < turns; t++) {
        // Side-by-side sessions are the long autonomous runs you leave working while you talk to another agent.
        const turnMs = clamp(lognormal(rng, (harness === 'codex' ? 150_000 : harness === 'claude' ? 110_000 : 45_000) * (nearPrevious ? 5 : 1), 1.0), 3000, 3 * 3600_000)
        agentMs += turnMs
        gaps += t === 0 ? 0 : Math.min(15 * 60_000, lognormal(rng, nearPrevious ? 30_000 : 80_000, 1.2))
        addTokens(tokens, turnTokens(rng, harness))
      }
      const resumedLater = rng() < 0.09 ? lognormal(rng, 2 * DAY, 1) : 0
      const ended = Math.min(NOW - 60_000, started + agentMs + gaps + resumedLater)

      const subagentCount =
        harness === 'claude'
          ? rng() < 0.3
            ? 0
            : Math.round(clamp(lognormal(rng, 6, 1.1), 1, 140))
          : harness === 'codex'
            ? rng() < 0.55
              ? 0
              : Math.round(clamp(lognormal(rng, 1.5, 0.7), 1, 8))
            : 0
      const subagentShare = subagentCount ? clamp(0.05 + subagentCount * 0.02, 0, 0.6) : 0
      if (tokens) {
        const scale = 1 / (1 - subagentShare)
        tokens.input *= scale
        tokens.cacheWrite *= scale
        tokens.cacheRead *= scale
        tokens.output *= scale
        tokens.reasoning *= scale
      }
      const early = started < Date.UTC(2025, 7, 1)
      if (harness === 'claude' && (early ? rng() < 0.8 : rng() < 0.03)) tokens = null
      if (harness === 'codex' && rng() < 0.02) tokens = null

      const [title0, prompt] = pick(rng, openings)
      const titleSource: TitleSource =
        harness === 'claude'
          ? weighted(rng, [['ai', 58], ['first-prompt', 30], ['custom', 6], ['summary', 6]] as const)
          : harness === 'codex'
            ? weighted(rng, [['ai', 35], ['first-prompt', 65]] as const)
            : harness === 'deepseek'
              ? 'ai'
              : 'first-prompt'
      const title = titleSource === 'first-prompt' ? prompt.split(/[。！？]|[.!?](?=\s|$)/)[0].slice(0, 44) : title0

      const branch =
        project.kind === 'repo'
          ? location.branch ?? (rng() < 0.55 ? 'main' : `${pick(rng, ['feat', 'fix', 'chore', 'refactor'])}/${pick(rng, projectWords.prefix)}-${pick(rng, projectWords.suffix)}`)
          : undefined
      const edits = rng() < 0.72
      const linesAdded = edits ? Math.round(lognormal(rng, 60, 1.3) * Math.sqrt(turns)) : 0
      const linesRemoved = edits ? Math.round(linesAdded * lognormal(rng, 0.45, 0.6)) : 0
      const toolCalls = Math.round(turns * clamp(lognormal(rng, 7, 0.7), 1, 60))
      const events = Math.round(turns * 6 + toolCalls * 2.2 + subagentCount * 2)
      const entrypoint: Entrypoint =
        harness === 'claude'
          ? weighted(rng, [['cli', 72], ['vscode', 12], ['desktop', 12], ['sdk', 4]] as const)
          : harness === 'codex'
            ? weighted(rng, [['cli', 60], ['vscode', 25], ['app', 15]] as const)
            : 'cli'
      const model = modelFor(harness, started, rng)
      const models = [model]
      if (harness === 'claude' && subagentCount > 3 && rng() < 0.4) models.push('claude-sonnet-5-5')
      if (harness === 'codex' && subagentCount > 0) models.push('codex-auto-review')

      const gone =
        harness === 'claude' && ended < Date.UTC(2025, 9, 15)
          ? rng() < 0.72
          : harness === 'codex' && rng() < 0.015
      const recordedCost =
        tokens && harness === 'claude' && started > Date.UTC(2026, 5, 1)
          ? (tokens.input * price.claude[0] + tokens.cacheWrite * price.claude[1] + tokens.cacheRead * price.claude[2] + tokens.output * price.claude[3]) / 1e6
          : tokens && harness === 'pi'
            ? (tokens.input * price.pi[0] + tokens.cacheWrite * price.pi[1] + tokens.cacheRead * price.pi[2] + tokens.output * price.pi[3]) / 1e6
            : undefined

      chats.push({
        id: ++id,
        harness,
        projectId: project.id,
        cwd: location.path,
        title,
        titleSource,
        firstPrompt: prompt,
        started,
        ended,
        agentMs,
        agentMsSource: harness === 'claude' && started > Date.UTC(2025, 8, 15) ? 'recorded' : 'derived',
        turns,
        toolCalls,
        subagentCount,
        tokens: tokens
          ? {
              input: Math.round(tokens.input),
              cacheWrite: Math.round(tokens.cacheWrite),
              cacheRead: Math.round(tokens.cacheRead),
              output: Math.round(tokens.output),
              reasoning: Math.round(tokens.reasoning),
            }
          : null,
        models,
        branch,
        commit: harness === 'codex' && project.kind === 'repo' ? hex(rng, 7) : undefined,
        pr:
          branch && branch !== 'main' && project.remote && rng() < 0.16
            ? { number: prNext.n++, repo: project.remote.replace('github.com/', '') }
            : undefined,
        linesAdded,
        linesRemoved,
        filesTouched: edits ? Math.round(clamp(lognormal(rng, 4, 0.9), 1, 120)) : 0,
        entrypoint,
        version: harness === 'claude' ? claudeVersion(started) : harness === 'codex' ? `0.${90 + Math.floor((started - harnessStart.codex) / (20 * DAY))}.0` : '1.4.2',
        effort: harness === 'pi' ? undefined : started > Date.UTC(2026, 1, 1) ? weighted(rng, [['high', 50], ['xhigh', 30], ['medium', 15], ['max', 5]] as const) : undefined,
        permissionMode:
          harness === 'claude'
            ? weighted(rng, [['default', 50], ['acceptEdits', 30], ['plan', 12], ['bypassPermissions', 8]] as const)
            : harness === 'codex'
              ? weighted(rng, [['on-request', 70], ['never', 30]] as const)
              : undefined,
        compactions: turns > 25 ? Math.floor(turns / 30) + (rng() < 0.5 ? 1 : 0) : 0,
        apiErrors: rng() < 0.12 ? Math.ceil(rng() * 3) : 0,
        starred: rng() < 0.025,
        source: gone ? 'gone' : 'present',
        sizeBytes: Math.round(events * lognormal(rng, 5200, 0.6)),
        events,
        recordedCostUsd: recordedCost,
      })
    }
  }
  return chats
}

// ------------------------------------------------------------ aggregation

function aggregate(projects: Project[], chats: Chat[]) {
  const byId = new Map(projects.map((p) => [p.id, p]))
  const weekStart = NOW - 26 * WEEK
  for (const chat of chats) {
    const p = byId.get(chat.projectId)!
    p.chatIds.push(chat.id)
    if (!p.harnesses.includes(chat.harness)) p.harnesses.push(chat.harness)
    p.firstActive = p.firstActive ? Math.min(p.firstActive, chat.started) : chat.started
    p.lastActive = Math.max(p.lastActive, chat.ended)
    p.agentMs += chat.agentMs
    if (chat.tokens) addTokens(p.tokens, chat.tokens)
    p.linesAdded += chat.linesAdded
    p.linesRemoved += chat.linesRemoved
    if (p.weekly.length === 0) p.weekly = new Array(26).fill(0)
    if (chat.started >= weekStart) p.weekly[Math.min(25, Math.floor((chat.started - weekStart) / WEEK))]++
  }
  for (const p of projects) {
    // A folder is only known because a conversation ran there.
    p.locations = p.locations.filter((l) => l.chatCount > 0 || l.role === 'main')
    if (p.weekly.length === 0) p.weekly = new Array(26).fill(0)
    p.gone = p.locations.every((l) => !l.exists)
    p.chatIds.sort((a, b) => b - a)
  }
}

// ------------------------------------------------------- merge suggestions

function suggestions(rng: Rng, drafts: Draft[]): MergeSuggestion[] {
  const out: MergeSuggestion[] = []
  const repos = drafts.filter((d) => d.project.kind === 'repo' && d.project.remote && d.project.parent === '~/code')
  // Same remote: a project left behind under an old path.
  for (let i = 0; i < 6 && i < repos.length; i++) {
    const live = repos[i * 3].project
    const ghost = makeProject(`p-ghost-${i}`, live.name, 'repo', '~/projects/2025', `~/projects/2025/${live.name}`, live.remote)
    ghost.locations[0].exists = false
    drafts.push({ project: ghost, start: Date.UTC(2025, 5, 1), end: Date.UTC(2025, 10, 20), weight: 2 + Math.floor(rng() * 9), mix: [['claude', 1]] })
    out.push({ id: `s-remote-${i}`, kind: 'same-remote', intoId: live.id, fromId: ghost.id, evidence: `兩個資料夾的 git 遠端都是 ${live.remote}` })
  }
  // Claude worktrees that were deleted before PathKeep could read their .git.
  for (let i = 0; i < 5; i++) {
    const host = repos[i * 2 + 1].project
    const branch = `feat/${pick(rng, projectWords.prefix)}`
    const ghost = makeProject(`p-wt-${i}`, branch.split('/')[1], 'folder', `${host.locations[0].path}/.claude/worktrees`, `${host.locations[0].path}/.claude/worktrees/${branch.split('/')[1]}`)
    ghost.locations[0].exists = false
    ghost.locations[0].role = 'worktree'
    drafts.push({ project: ghost, start: Date.UTC(2026, 3, 1), end: Date.UTC(2026, 8, 1), weight: 1 + Math.floor(rng() * 4), mix: [['claude', 1]] })
    out.push({ id: `s-wt-${i}`, kind: 'worktree', intoId: host.id, fromId: ghost.id, evidence: `資料夾在 ${host.name}/.claude/worktrees/ 底下，分支 ${branch}；資料夾已刪，備份時沒讀到 .git` })
  }
  // Older backups that recorded a subfolder as its own project.
  for (let i = 0; i < 3; i++) {
    const host = repos[i * 4 + 2].project
    const sub = pick(rng, ['notebooks', 'packages/web', 'infra'])
    const ghost = makeProject(`p-sub-${i}`, `${host.name}/${sub}`, 'folder', host.locations[0].path, `${host.locations[0].path}/${sub}`)
    drafts.push({ project: ghost, start: Date.UTC(2025, 7, 1), end: Date.UTC(2026, 2, 1), weight: 2 + Math.floor(rng() * 6), mix: [['codex', 1]] })
    out.push({ id: `s-sub-${i}`, kind: 'subfolder', intoId: host.id, fromId: ghost.id, evidence: `資料夾在 ${host.name} 的 git 根目錄裡面` })
  }
  return out
}

// ------------------------------------------------------------------- build

export interface Archive {
  projects: Project[]
  projectById: Map<string, Project>
  chats: Chat[]
  chatById: Map<number, Chat>
  suggestions: MergeSuggestion[]
}

export function buildArchive(seed = 20261113): Archive {
  const rng = mulberry32(seed)
  const drafts = buildProjects(rng)
  const merge = suggestions(rng, drafts)
  const chatsRaw = buildChats(rng, drafts)
  const projects = drafts.map((d) => d.project)
  aggregate(projects, chatsRaw)
  // Newest first; ids follow that order so "chat 1" is the latest.
  chatsRaw.sort((a, b) => b.ended - a.ended)
  const remap = new Map<number, number>()
  chatsRaw.forEach((chat, i) => {
    remap.set(chat.id, i + 1)
    chat.id = i + 1
  })
  for (const p of projects) p.chatIds = p.chatIds.map((id) => remap.get(id)!).sort((a, b) => a - b)
  return {
    projects,
    projectById: new Map(projects.map((p) => [p.id, p])),
    chats: chatsRaw,
    chatById: new Map(chatsRaw.map((c) => [c.id, c])),
    suggestions: merge,
  }
}
