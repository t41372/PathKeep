/**
 * Writes synthetic but realistic coding-agent histories into a fake home
 * folder: Claude Code (`.claude`), Codex (`.codex`), pi (`.pi/agent`),
 * DeepSeek Harness (`.dsh`) and the Claude desktop app (`Claude`), each in the
 * folder layout and record format that PathKeep's agent-history library
 * reads, and returns the facts specs assert on. With `CHB_AGENT_TOOLS_HOME`
 * set to that home, the app finds exactly these five folders. Shapes follow
 * the library's own test fixtures
 * (src-tauri/crates/agent-history/tests/support/{fixtures,native}.rs,
 * tests/fixtures/codex/paginated.jsonl), its adapters, and
 * docs/architecture/research/agent-history/.
 *
 * Responsibilities: the conversation files and their dates (record times and
 * file modification times), the project folders they name (one a git
 * repository with a linked worktree, one that no longer exists), named scenes,
 * and mutators that change the tools' folders between two backups.
 * Not responsible for: pointing the app at the fake home, running any agent
 * tool, or predicting how PathKeep reads the files. The facts describe what
 * was written.
 *
 * This file is the entry point; the modules in synthetic-agents/ do the work:
 * basics (random streams, dates, encodings), bank (the conversations),
 * projects (the home's layout and git), context (writing and facts), one
 * format writer per tool, one scene module per tool and one for the Claude
 * desktop app, generate and mutators.
 *
 * Output is deterministic for a given `now` and `seed` on one Node version
 * (zlib and zstd output can change between releases). Every date is a day
 * 1 to 120 days before the local day of `now`, at a local time, so the bytes
 * stay the same all day and nothing is dated today or later. Each
 * conversation draws its ids, words and times from a seed of its own, so
 * adding one leaves the others unchanged.
 *
 * Put the fake home outside any git checkout: the library looks for `.git` in
 * a project folder and in every folder above it, so inside a checkout each
 * project that is not itself a repository reports that checkout.
 */
export { AGENT_TOOLS } from './synthetic-agents/projects.mjs'
export {
  agentConversation,
  writeSyntheticAgents,
} from './synthetic-agents/generate.mjs'
export {
  addAgentConversation,
  appendAgentTurns,
  deleteAgentConversation,
  setAgentToolReadable,
} from './synthetic-agents/mutators.mjs'
