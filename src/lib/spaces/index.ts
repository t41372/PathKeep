/**
 * Spaces, one per kind of data PathKeep keeps: the registry, the paths into
 * them, the last space used and the shortcut that switches between them.
 */
export {
  spaceById,
  spaces,
  spacesOff,
  spacesOn,
  type PageOf,
  type PaletteResultsProps,
  type Space,
  type SpaceId,
  type SpacePage,
} from './registry'
export {
  spaceBackupPath,
  spaceOfPath,
  spacePagePath,
  spacePath,
  spaceSearchPath,
  spaceStartPath,
} from './paths'
export { readLastSpace, rememberSpace } from './last-space'
export { spaceKeyIndex, spaceKeyLabel } from './space-key'
