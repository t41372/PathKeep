/**
 * Spaces, one per kind of data PathKeep keeps: the registry, which of them
 * are on, the paths into them, the last space used and the shortcut that
 * switches between them.
 */
export {
  hasPages,
  spaceById,
  spaceOfKind,
  spaces,
  spacesOff,
  spacesOn,
  useSpacesOff,
  useSpacesOn,
  type DataSpace,
  type PageOf,
  type PaletteResultsProps,
  type Space,
  type SpaceId,
  type SpacePage,
  type SpaceWithPagesId,
} from './registry'
export { kindStateOf, useKindList, type KindListAnswer } from './kind-states'
export {
  agentChatPath,
  agentProjectPath,
  agentProjectsPath,
  pageElsewhere,
  spaceBackupPath,
  spaceOfPath,
  spacePagePath,
  spacePath,
  spaceSearchPath,
  spaceStartPath,
} from './paths'
export { readLastSpace, rememberSpace } from './last-space'
export { spaceKeyIndex, spaceKeyLabel } from './space-key'
