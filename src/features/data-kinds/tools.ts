/**
 * The Agent tools PathKeep reads: each one's name and its own icon, so
 * every screen writes a tool the same way (`agent-history.md`: each tool
 * by its own icon and name, never an abbreviation).
 *
 * Responsibilities: the id → name and icon map, and the name of a folder
 * whose path does not say what it is. Not responsible for:
 * which tools exist on this computer (the backend's `detect_data_sources`),
 * their order (`AGENT_TOOLS`), or drawing them (`tool-badge.tsx`).
 * Icon provenance: `src/assets/harness-icons/README.md`.
 */
import claudeCodeIcon from '@/assets/harness-icons/claude-code.svg'
import codexDarkIcon from '@/assets/harness-icons/codex-dark.svg'
import codexIcon from '@/assets/harness-icons/codex.svg'
import deepseekIcon from '@/assets/harness-icons/deepseek.png'
import piIcon from '@/assets/harness-icons/pi.svg'
import { AGENT_TOOLS, type AgentToolId } from '@/lib/backend-client/data-kinds'
import type { MessageKey } from '@/lib/i18n'

export interface ToolLook {
  name: MessageKey
  icon: string
  /** A second icon for the dark theme, where the first would vanish. */
  iconDark?: string
}

export const toolLooks: Record<AgentToolId, ToolLook> = {
  'claude-code': { name: 'dataKinds.tools.claudeCode', icon: claudeCodeIcon },
  codex: {
    name: 'dataKinds.tools.codex',
    icon: codexIcon,
    iconDark: codexDarkIcon,
  },
  pi: { name: 'dataKinds.tools.pi', icon: piIcon },
  deepseek: { name: 'dataKinds.tools.deepseek', icon: deepseekIcon },
}

export function toolName(id: AgentToolId): MessageKey {
  return toolLooks[id].name
}

/** `ids` in the backend's order, once each. */
export function inToolOrder(ids: Iterable<AgentToolId>): AgentToolId[] {
  const set = new Set(ids)
  return AGENT_TOOLS.filter((id) => set.has(id))
}

/**
 * What a tool folder is, when its path does not say it. The Claude desktop
 * app keeps its local Cowork sessions in a folder named `Claude` in the
 * system's app-data folder (`~/Library/Application Support/Claude`,
 * `~/.config/Claude`, `%APPDATA%\Claude`), which the backend reads as
 * Claude Code; Claude Code's own folder is `.claude`.
 */
export function folderLabel(
  tool: AgentToolId,
  path: string | null | undefined,
): MessageKey | undefined {
  if (tool !== 'claude-code' || !path) return undefined
  const name = path.split(/[\\/]/).filter(Boolean).pop()
  return name === 'Claude' ? 'dataKinds.folders.claudeDesktop' : undefined
}
