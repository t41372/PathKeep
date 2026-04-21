/**
 * @file types.ts
 * @description Barrel export for split Core Intelligence front-end type owners.
 * @module core-intelligence/types
 *
 * ## Responsibilities
 * - Preserve the existing public import path for deterministic Core Intelligence front-end contracts.
 * - Re-export the focused type-owner modules without changing payload names.
 *
 * ## Not responsible for
 * - Defining payload shapes inline.
 * - Owning AI/LLM contracts from src/lib/types/intelligence.ts.
 *
 * ## Dependencies
 * - Depends on the focused modules under src/lib/core-intelligence/types-*.ts.
 * - Consumed by dashboard, explorer, intelligence, settings, and backend-client surfaces.
 *
 * ## Performance notes
 * - Type-only barrel; keeping the public surface stable avoids churn across many route modules while shrinking the maintenance hotspot.
 */

export * from './types-primitives'
export * from './types-overview'
export * from './types-navigation'
export * from './types-analysis'
export * from './types-outputs'
