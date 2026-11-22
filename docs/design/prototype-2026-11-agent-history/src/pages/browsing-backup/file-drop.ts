/**
 * Which import a file dropped on the window belongs to. A copy of
 * `dropTarget` in `src/features/backup/file-drop.ts`: browser files have fixed
 * names or end in `.sqlite` / `.db`; a `.zip`, a `.json` or a folder with
 * "takeout" in its name is a Takeout export; any other folder is taken to be
 * a browser profile.
 */
const browserHistoryFiles = new Set(['history', 'history.db', 'places.sqlite'])

export function dropTarget(path: string): 'takeout' | 'browser' {
  const name = path.split(/[\\/]/).filter(Boolean).pop()?.toLowerCase() ?? ''
  if (browserHistoryFiles.has(name) || /\.(sqlite|db)$/.test(name)) return 'browser'
  if (/\.(zip|json)$/.test(name) || name.includes('takeout')) return 'takeout'
  return /\.[a-z0-9]+$/.test(name) ? 'takeout' : 'browser'
}
