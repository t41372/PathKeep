/* v8 ignore file -- barrel-only progress export with no executable UI contract. */
/**
 * @file index.ts
 * @description Public barrel for shared progress components.
 * @module components/progress
 *
 * ## Responsibilities
 * - Preserve a short import path for task progress primitives.
 * - Keep shared progress renderers discoverable for Import, Jobs, shell overlays, and future background work.
 *
 * ## Not responsible for
 * - Defining task state or backend progress event contracts.
 * - Adding route-specific progress helpers.
 *
 * ## Dependencies
 * - Re-exports `task-progress.tsx`.
 *
 * ## Performance notes
 * - Barrel-only module with no runtime work beyond re-exporting component owners.
 */

export * from './task-progress'
