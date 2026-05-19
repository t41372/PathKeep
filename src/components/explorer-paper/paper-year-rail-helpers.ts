/**
 * Helpers extracted from PaperYearRail.
 *
 * Lives next to the component but in its own file so the rail can export only
 * its component (keeping react-refresh happy) while the helper stays unit-
 * testable on its own.
 */

/**
 * Pick a sensible ISO target when the user clicks a year cell. The newest
 * year jumps straight to the archive's last loaded day so users land on
 * familiar territory; older years jump to mid-June so empty Jan/Feb don't
 * leave the contact sheet looking dead.
 */
export function pickYearJumpIso(
  year: number,
  bounds: { lastYear: number; lastIso: string },
): string {
  if (year === bounds.lastYear) return bounds.lastIso
  return `${year}-06-15`
}
