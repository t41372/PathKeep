/**
 * Responsibilities: the fake home's layout: where each tool keeps its history
 * under the home, the project folders the conversations name, and the git
 * repository with a linked worktree.
 * Not responsible for: conversation files or their facts.
 */

import { mkdirSync, utimesSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { DAYS, dayAt } from './basics.mjs'

export const AGENT_TOOLS = ['claude-code', 'codex', 'pi', 'deepseek']

/**
 * The folders the library reads under a home (capture/defaults.rs): one per
 * tool, plus the Claude desktop app's data folder. Under `CHB_AGENT_TOOLS_HOME`
 * the app looks for all five directly beneath that one folder.
 */
export const ROOTS = {
  'claude-code': '.claude',
  codex: '.codex',
  pi: path.join('.pi', 'agent'),
  deepseek: '.dsh',
  'claude-desktop': 'Claude',
}

/** The keys of `ROOTS`; `claude-desktop` holds Claude Code conversations too. */
export const ROOT_KEYS = Object.keys(ROOTS)

/**
 * Project folders, under `<home>/code` unless they name another `base`.
 * `pathkeep` is a git repository and `pathkeep-search` a linked worktree of
 * it; `old-prototype` is named by a conversation but never created.
 */
export const PROJECTS = {
  pathkeep: { folder: 'pathkeep', stacks: ['rust', 'web'] },
  'pathkeep-search': { folder: 'pathkeep-search', stacks: ['rust', 'web'] },
  tidepool: { folder: 'tidepool', stacks: ['rust'] },
  'notes-site': { folder: 'notes-site', stacks: ['web'] },
  ledger: { folder: '記帳本', stacks: ['python'] },
  'infra-scripts': { folder: 'infra-scripts', stacks: ['ops'] },
  'old-prototype': { folder: 'old-prototype', stacks: ['web'], gone: true },
  // The folder a Cowork task in the Claude desktop app worked in.
  'trip-notes': { folder: 'trip-notes', base: 'Documents', stacks: [] },
}

/** The projects each tool's background conversations are spread over. */
export const TOOL_PROJECTS = {
  'claude-code': ['pathkeep', 'pathkeep-search', 'notes-site', 'ledger'],
  codex: ['pathkeep', 'pathkeep-search', 'tidepool', 'infra-scripts'],
  pi: ['pathkeep', 'notes-site', 'infra-scripts'],
  deepseek: ['tidepool', 'ledger'],
}

export const projectFolder = (home, project) =>
  path.join(home, PROJECTS[project].base ?? 'code', PROJECTS[project].folder)

export function writeProjects(ctx) {
  for (const [key, project] of Object.entries(PROJECTS))
    if (!project.gone)
      mkdirSync(projectFolder(ctx.home, key), { recursive: true })
  const repository = projectFolder(ctx.home, 'pathkeep')
  const worktree = projectFolder(ctx.home, 'pathkeep-search')
  const gitDir = path.join(repository, '.git')
  const admin = path.join(gitDir, 'worktrees', 'pathkeep-search')
  const remotes = [
    { name: 'origin', url: 'https://git.example.com/lin/pathkeep.git' },
    { name: 'mirror', url: 'git@git.example.com:lin/pathkeep-mirror.git' },
  ]
  mkdirSync(path.join(gitDir, 'refs', 'heads'), { recursive: true })
  mkdirSync(path.join(gitDir, 'objects'), { recursive: true })
  // Dated with the oldest conversations, like everything else the fixture writes.
  const cloned = dayAt(ctx.today, DAYS, 9, 0)
  const write = (file, text) => {
    mkdirSync(path.dirname(file), { recursive: true })
    writeFileSync(file, text)
    utimesSync(file, cloned / 1000, cloned / 1000)
  }
  write(path.join(gitDir, 'HEAD'), 'ref: refs/heads/main\n')
  write(
    path.join(gitDir, 'config'),
    '[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n' +
      remotes
        .map(
          ({ name, url }) =>
            `[remote "${name}"]\n\turl = ${url}\n\tfetch = +refs/heads/*:refs/remotes/${name}/*\n`,
        )
        .join('') +
      '[branch "main"]\n\tremote = origin\n\tmerge = refs/heads/main\n',
  )
  write(path.join(admin, 'HEAD'), 'ref: refs/heads/search-ranking\n')
  write(path.join(admin, 'commondir'), '../..\n')
  write(path.join(admin, 'gitdir'), `${path.join(worktree, '.git')}\n`)
  write(path.join(worktree, '.git'), `gitdir: ${admin}\n`)
  return Object.entries(PROJECTS).map(([key, project]) => ({
    key,
    folder: projectFolder(ctx.home, key),
    exists: !project.gone,
    git:
      key === 'pathkeep'
        ? {
            kind: 'repository',
            branch: 'main',
            gitDir,
            commonDir: gitDir,
            remotes,
          }
        : key === 'pathkeep-search'
          ? {
              kind: 'worktree',
              branch: 'search-ranking',
              gitDir: admin,
              commonDir: gitDir,
              remotes,
            }
          : null,
  }))
}
