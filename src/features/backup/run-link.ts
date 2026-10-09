/**
 * Opens the Backup screen on one run's sheet (`?run=`, read by the runs card),
 * for a finished backup's toast, which is shown outside the router.
 */

/** The hash router follows a change to the hash like any navigation. */
export function openRun(runId: number) {
  window.location.hash = `/backup?run=${runId}`
}
