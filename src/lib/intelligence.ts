/* v8 ignore file -- compatibility re-export barrel with no executable UI policy. */
/**
 * Transitional barrel for shared intelligence helpers.
 *
 * Why this file exists:
 * - M11 split route grammar, AI presentation, and evidence-link helpers into
 *   their canonical owner modules.
 * - This file remains as a compatibility import surface while call sites
 *   migrate, but it should stay a thin re-export barrel only.
 */

export * from './core-intelligence/routes'
export * from './intelligence-ai-presentation'
export * from './intelligence-links'
