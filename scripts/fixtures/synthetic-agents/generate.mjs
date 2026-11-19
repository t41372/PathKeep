/**
 * Responsibilities: writing the whole fixture in a fixed order (projects,
 * scenes, background conversations) and returning the facts.
 * Not responsible for: changing the fixture afterwards (mutators.mjs).
 */

import { mkdirSync, realpathSync } from 'node:fs'
import { TEMPLATES, turnsFrom } from './bank.mjs'
import { DAYS, between, dayAt, pick, seeded } from './basics.mjs'
import { begin, createContext, toolFacts, word } from './context.mjs'
import { writeClaude } from './formats/claude-code.mjs'
import { writeCodex } from './formats/codex.mjs'
import { writeDeepseek } from './formats/deepseek.mjs'
import { writePi } from './formats/pi.mjs'
import {
  AGENT_TOOLS,
  PROJECTS,
  ROOT_KEYS,
  TOOL_PROJECTS,
  writeProjects,
} from './projects.mjs'
import {
  attachmentScene,
  customTitleScene,
  emptyFileScene,
  largeScene,
  missingFolderScene,
  subagentScene,
  unknownEventsScene,
} from './scenes/claude-code.mjs'
import { claudeDesktopScene } from './scenes/claude-desktop.mjs'
import {
  codexArchivedScene,
  codexResumedScene,
  malformedScene,
} from './scenes/codex.mjs'
import {
  deepseekCurrentScene,
  deepseekPlainScene,
  newerVersionScene,
} from './scenes/deepseek.mjs'
import { branchScene, piUsageScene } from './scenes/pi.mjs'

/** Background conversations per tool; with the scenes, 50 in all. */
const BACKGROUND = { 'claude-code': 11, codex: 9, pi: 9, deepseek: 7 }

export const WRITERS = {
  'claude-code': writeClaude,
  codex: writeCodex,
  pi: writePi,
  deepseek: writeDeepseek,
}

function background(ctx, tool) {
  for (let index = 1; index <= BACKGROUND[tool]; index += 1) {
    const key = `${tool}-${String(index).padStart(2, '0')}`
    const random = seeded(ctx.seed, key)
    const project = pick(random, TOOL_PROJECTS[tool])
    const { stacks } = PROJECTS[project]
    const template = pick(
      random,
      TEMPLATES.filter((t) => t.stacks.some((s) => stacks.includes(s))),
    )
    const start = dayAt(
      ctx.today,
      between(random, 1, DAYS),
      between(random, 9, 19),
      between(random, 0, 59),
    )
    const c = begin(ctx, key, tool, project, start, random)
    const turns = turnsFrom(template.turns, word(c))
    // Half of the DeepSeek sessions carry a name the person gave them.
    const title =
      tool === 'deepseek' && random() < 0.5
        ? turns[0].user.split(/[.?。，,？]/)[0].slice(0, 48)
        : undefined
    WRITERS[tool](c, turns, { title })
  }
}

/**
 * Writes the four tools' folders, the Claude desktop app's folder and the
 * project folders under `home` (created if missing; its real path is used,
 * as the tools record it).
 */
export function writeSyntheticAgents(
  home,
  { now = Date.now(), seed = 11 } = {},
) {
  mkdirSync(home, { recursive: true })
  const ctx = createContext(realpathSync(home), now, seed)
  const projects = writeProjects(ctx)
  const scenes = {
    subagents: subagentScene(ctx),
    branch: branchScene(ctx),
    codexResumed: codexResumedScene(ctx),
    codexArchived: codexArchivedScene(ctx),
    piUsage: piUsageScene(ctx),
    deepseekCurrent: deepseekCurrentScene(ctx),
    deepseekPlain: deepseekPlainScene(ctx),
    large: largeScene(ctx),
    attachments: attachmentScene(ctx),
    unknownEvents: unknownEventsScene(ctx),
    malformed: malformedScene(ctx),
    emptyFile: emptyFileScene(ctx),
    newerVersion: newerVersionScene(ctx),
    git: { repository: 'pathkeep', worktree: 'pathkeep-search' },
    missingFolder: missingFolderScene(ctx),
    customTitle: customTitleScene(ctx),
  }
  for (const tool of AGENT_TOOLS) background(ctx, tool)
  // Last, so nothing written before it depends on it.
  scenes.claudeDesktop = claudeDesktopScene(ctx)
  ctx.conversations.sort(
    (a, b) => a.startedAt - b.startedAt || a.key.localeCompare(b.key),
  )
  return {
    home: ctx.home,
    now,
    seed,
    roots: ctx.roots,
    tools: Object.fromEntries(
      ROOT_KEYS.map((root) => [
        root,
        toolFacts(ctx.roots[root], ctx.files[root]),
      ]),
    ),
    projects,
    conversations: ctx.conversations,
    removed: [],
    unreadable: [],
    scenes,
  }
}

/** The conversation with this fixture key; throws if there is none. */
export function agentConversation(agents, key) {
  const found = agents.conversations.find((c) => c.key === key)
  if (!found) throw new Error(`no synthetic agent conversation ${key}`)
  return found
}
