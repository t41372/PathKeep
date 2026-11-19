/** The tool names PathKeep's agent-history library uses (`--source <tool>=<root>`). */
export type AgentTool = 'claude-code' | 'codex' | 'pi' | 'deepseek'

export const AGENT_TOOLS: readonly AgentTool[]

/**
 * The folders the library reads under a home: one per tool, plus
 * `claude-desktop`, the Claude desktop app's data folder (`Claude` beneath the
 * home under `CHB_AGENT_TOOLS_HOME`), which holds Claude Code conversations too.
 */
export type AgentRoot = AgentTool | 'claude-desktop'

/** A file the generator wrote under a root folder. */
export interface AgentFile {
  path: string
  /**
   * `conversation`: a transcript, rollout or session generation that holds a
   * conversation (Codex continuation rollouts and DeepSeek's older generation
   * included); `subagent`: a Claude Code subagent transcript;
   * `subagent-meta`: its `.meta.json`; `tool-output`: a tool output Claude
   * Code kept in `tool-results/`; `empty`: the empty transcript;
   * `session-card`: a Claude desktop app session card; `audit`: a Cowork
   * task's audit log; `output`: a file a Cowork task produced.
   */
  kind:
    | 'conversation'
    | 'subagent'
    | 'subagent-meta'
    | 'tool-output'
    | 'empty'
    | 'session-card'
    | 'audit'
    | 'output'
  bytes: number
  /** Milliseconds; the generator sets it to the file's last record. */
  modifiedAt: number
}

export interface AgentToolFacts {
  root: string
  /** Paths of the files of kind `conversation`. */
  conversationFiles: string[]
  /** Paths of the files of kind `subagent`. */
  subagentFiles: string[]
  /** Every file under the root. */
  files: AgentFile[]
  totalBytes: number
  /** The oldest `modifiedAt` among `files`. */
  earliestFileDate: number
}

/** Tokens as the tool recorded them, summed over its responses. */
export interface AgentUsage {
  input: number
  output: number
  cacheRead: number
  cacheWrite: number
  /** pi only: the dollars it recorded on each reply, summed. */
  cost?: number
}

/** A Claude Code subagent: `<session>/subagents/agent-<agentId>.jsonl`. */
export interface AgentSubagent {
  agentId: string
  file: string
  metaFile: string
  /** The parent's `Task` tool call that started it. */
  toolUseId: string
  /** The task prompt; it is also the subagent's title in a list. */
  firstUserMessage: string
  userMessages: number
  assistantMessages: number
  models: string[]
  words: string[]
}

export interface AgentConversation {
  /** The fixture's name for it, e.g. `claude-subagents` or `codex-03`. */
  key: string
  tool: AgentTool
  /** The root its files are under: its tool's, or `claude-desktop` for a Cowork task. */
  root: AgentRoot
  /**
   * The id the tool gave it: the Claude Code session id (the file name), the
   * Codex thread id, the pi or DeepSeek session id.
   */
  nativeId: string
  /** The working folder the tool recorded (absolute, symlinks resolved). */
  projectFolder: string
  /** Its files, oldest first; the last is the one its tool writes to now. */
  files: string[]
  /** What the person typed first. */
  firstUserMessage: string
  /**
   * The name the tool recorded for it (Claude Code `custom-title`, DeepSeek
   * `session/title`), else `firstUserMessage`. All are under 160 characters,
   * so nothing is cut.
   */
  title: string
  /**
   * Messages the person typed. Tool results are not counted, even where the
   * tool stores them as user records (Claude Code); neither is Codex's second
   * copy of each prompt (it records a prompt both as a model input item and
   * as a completed `UserMessage`; count one of the two).
   */
  userMessages: number
  /**
   * Replies with text for the person. A message that only calls a tool,
   * tool results and thinking are not counted. Claude Code writes each
   * content block as its own record, so its text records are what counts;
   * Codex writes replies as completed `AgentMessage` items.
   */
  assistantMessages: number
  models: string[]
  subagents: AgentSubagent[]
  /**
   * Made-up words that occur in this conversation and nowhere else in the
   * fixture, in text a reader of the conversation sees.
   */
  words: string[]
  /**
   * Made-up words that are in its files but not in that text: a turn Codex
   * reverted, the end of an output Claude Code spilled to `tool-results/`,
   * a file in a format version newer than the library reads.
   */
  unreadWords: string[]
  /** Milliseconds: its first record (Claude Code) or its header's time. */
  startedAt: number
  /** Milliseconds: its last record. */
  updatedAt: number
  usage: AgentUsage
  /** How many turns `appendAgentTurns` has added. */
  appendedTurns: number
}

export interface AgentProject {
  key: string
  folder: string
  /** False for the folder a conversation names that no longer exists. */
  exists: boolean
  git: {
    kind: 'repository' | 'worktree'
    branch: string
    /** `.git` of the repository, or the worktree's folder under `.git/worktrees`. */
    gitDir: string
    commonDir: string
    /** As written in `config`, one of them scp-style (`git@host:path`). */
    remotes: { name: string; url: string }[]
  } | null
}

/**
 * Named situations, each pointing at its conversation by `key`. A scene
 * describes its conversation as first written: the mutators update
 * `conversations` and `tools`, not the numbers here.
 */
export interface AgentScenes {
  /** Claude Code: three subagents started by three parallel `Task` calls. */
  subagents: { conversation: string; words: string[] }
  /**
   * pi: the person went back and asked the second question again, so one
   * reply (`parentId`) has two user messages under it, each with its word.
   */
  branch: {
    conversation: string
    parentId: string
    childIds: [string, string]
    words: [string, string]
  }
  /**
   * Codex: a thread whose last turn was reverted twice. Codex wrote the rest
   * to new rollouts named `<thread>_<rollout id>` that inherit the kept part
   * of the previous one through `history_base`, so one conversation has three
   * files. The reverted turns stay in the files (`revertedWords`); they are
   * not part of the conversation's line.
   */
  codexResumed: {
    conversation: string
    files: [string, string, string]
    rolloutIds: [string, string, string]
    revertedWords: [string, string]
  }
  /** Codex: an archived thread, `archived_sessions/rollout-….jsonl.zst`. */
  codexArchived: { conversation: string; file: string }
  /** pi: usage and cost on every reply. */
  piUsage: {
    conversation: string
    usage: AgentUsage
    assistantMessages: number
  }
  /**
   * DeepSeek Harness: a session started under format v3 and continued under
   * v4. The v4 file is zstd frames (a header frame, then one per turn); the
   * v3 file stays beside it as the older generation.
   */
  deepseekCurrent: {
    conversation: string
    file: string
    olderGeneration: string
    frames: number
  }
  /** DeepSeek Harness: an older format (v3), written without compression. */
  deepseekPlain: { conversation: string; file: string; version: 3 }
  /** Claude Code: about 2 MB of benchmark runs; words at the first, middle and last turn. */
  large: {
    conversation: string
    bytes: number
    turns: number
    firstWord: string
    middleWord: string
    lastWord: string
  }
  /**
   * Claude Code: a pasted screenshot (inline PNG), a tool output of about
   * 15 KB in the transcript, and one Claude Code spilled to `tool-results/`
   * leaving a 2 KB preview; its last word is only in the spilled file.
   */
  attachments: {
    conversation: string
    image: { mediaType: string; bytes: number; width: number; height: number }
    longOutput: { toolUseId: string; bytes: number }
    spilledOutput: {
      toolUseId: string
      file: string
      bytes: number
      word: string
    }
  }
  /** Claude Code: record types no reader knows, at these lines (0-based). */
  unknownEvents: {
    conversation: string
    records: { line: number; type: string; record: Record<string, unknown> }[]
  }
  /** Codex: a line cut off in the middle of the rollout (0-based). */
  malformed: { conversation: string; file: string; line: number }
  /** Claude Code: a transcript of 0 bytes. It is no conversation. */
  emptyFile: { file: string }
  /** DeepSeek Harness: a file in format v5, newer than the library reads. */
  newerVersion: { conversation: string; file: string; version: 5 }
  /** Project keys: a git repository and a linked worktree of it. */
  git: { repository: string; worktree: string }
  /** Claude Code: a conversation whose folder was deleted. */
  missingFolder: { conversation: string; folder: string }
  /** Claude Code: a conversation renamed with `/rename`. */
  customTitle: { conversation: string; title: string }
  /**
   * The Claude desktop app's folder: a Desktop Code session card for a
   * session it adopted from the command line (`desktopCode.conversation`,
   * whose transcript stays in `.claude`), and a local Cowork task: its card,
   * its folder with the transcript under `.claude/projects` (entrypoint
   * `local-agent`), an audit log and an output file. The cards and the audit
   * log carry only keys the research names; the cards have no title.
   */
  claudeDesktop: {
    desktopCode: { card: string; conversation: string }
    cowork: {
      conversation: string
      card: string
      body: string
      transcript: string
      audit: string
      output: string
    }
  }
}

export interface SyntheticAgents {
  /** The home folder with symlinks resolved (on macOS `/var` is `/private/var`). */
  home: string
  now: number
  seed: number
  roots: Record<AgentRoot, string>
  tools: Record<AgentRoot, AgentToolFacts>
  projects: AgentProject[]
  /** Oldest first. */
  conversations: AgentConversation[]
  /** Conversations `deleteAgentConversation` removed from the tools' folders. */
  removed: AgentConversation[]
  /** Roots `setAgentToolReadable` made unreadable. */
  unreadable: AgentRoot[]
  scenes: AgentScenes
}

/**
 * Writes the four tools' folders (`.claude`, `.codex`, `.pi/agent`, `.dsh`),
 * the Claude desktop app's folder (`Claude`) and the project folders under
 * `home`: the layout the app resolves when `CHB_AGENT_TOOLS_HOME` is `home`.
 * Keep `home` outside any git checkout. Dates fall 1 to 120 days before the
 * local day of `now`; seed 11 by default.
 */
export function writeSyntheticAgents(
  home: string,
  options?: { now?: number; seed?: number },
): SyntheticAgents

/** The conversation with this key; throws if there is none. */
export function agentConversation(
  agents: SyntheticAgents,
  key: string,
): AgentConversation

/**
 * Adds follow-up turns to a conversation's current file, each with a new
 * word in `words`, timed late on the day before `now`. A compressed Codex
 * rollout cannot be appended to (Codex resumes into a plain one).
 */
export function appendAgentTurns(
  agents: SyntheticAgents,
  key: string,
  options?: { turns?: number },
): SyntheticAgents

/**
 * Writes a new conversation for `tool`, newer than every other one, in
 * `project` (a key of a project with conversations in the bank) or a project
 * that tool already uses.
 */
export function addAgentConversation(
  agents: SyntheticAgents,
  tool: AgentTool,
  options?: { project?: string },
): SyntheticAgents

/** Deletes a conversation's files; its facts move to `removed`. */
export function deleteAgentConversation(
  agents: SyntheticAgents,
  key: string,
): SyntheticAgents

/**
 * Removes (or restores) every permission on one root folder. Throws on
 * Windows, where chmod does not stop reads, and when running as root.
 */
export function setAgentToolReadable(
  agents: SyntheticAgents,
  root: AgentRoot,
  readable: boolean,
): SyntheticAgents
