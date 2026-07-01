/**
 * @file backend-preview-workflows.test.ts
 * @description Unit coverage for browser-preview workflow payload helpers.
 * @module lib/backend-preview-workflows
 *
 * ## Responsibilities
 * - Verify Browser Direct preview/import payloads for Chromium and Safari sources.
 * - Protect import-batch state mutation when preview mode simulates a real browser-history import.
 *
 * ## Not responsible for
 * - Re-testing command dispatch in the browser-preview facade.
 * - Re-testing full Takeout inspection parsing.
 *
 * ## Dependencies
 * - Uses the canonical preview state factory so source and batch fixtures match browser-preview runtime.
 *
 * ## Performance notes
 * - Tests operate on bounded in-memory preview fixtures only.
 */

import { describe, expect, test } from 'vitest'
import { handlePreviewWorkflowCommand } from './backend-preview-workflow-commands'
import { createMockState } from './backend-preview-state'
import { buildMockBrowserHistoryInspection } from './backend-preview-workflows'

describe('backend preview workflow helpers', () => {
  test('builds a Chromium Browser Direct dry-run payload with default source fallbacks', () => {
    const state = createMockState()

    const inspection = buildMockBrowserHistoryInspection(
      state,
      {
        browserFamily: 'chromium',
        dryRun: true,
        profileId: null,
        sourcePath: '',
      },
      true,
    )

    expect(inspection.sourcePath).toBe('/tmp/History')
    expect(inspection.importBatch).toBeNull()
    expect(inspection.recognizedFiles).toEqual([
      expect.objectContaining({
        kind: 'chromium-history-db',
        path: '/tmp/History',
        reasonCode: 'chromium-history-sqlite',
        status: 'previewed',
      }),
    ])
    expect(inspection.previewEntries[0]?.sourcePath).toBe('/tmp/History')
    expect(inspection.notes).toContain(
      'Chromium History visits are ready for Browser Direct review.',
    )
  })

  test('builds a Safari Browser Direct import and updates import-batch detail state', () => {
    const state = createMockState()
    const previousInspection = buildMockBrowserHistoryInspection(
      state,
      {
        browserFamily: 'chromium',
        dryRun: false,
        profileId: 'chrome:Archive',
        sourcePath: '/Users/test/Chrome/Archive/History',
      },
      false,
    )

    const inspection = buildMockBrowserHistoryInspection(
      state,
      {
        browserFamily: 'safari',
        dryRun: false,
        sourcePath: '/Users/test/Library/Safari/History.db',
      },
      false,
    )

    expect(inspection.importBatch).toMatchObject({
      profileId: 'safari:default',
      sourceKind: 'browser-history',
      sourcePath: '/Users/test/Library/Safari/History.db',
    })
    expect(inspection.recognizedFiles).toEqual([
      expect.objectContaining({
        kind: 'safari-history-db',
        reasonCode: 'safari-history-sqlite',
        status: 'imported',
      }),
    ])
    expect(inspection.notes).toContain(
      'Safari History.db visits are ready for Browser Direct review.',
    )
    expect(state.importBatchDetails[inspection.importBatch!.id]).toMatchObject({
      batch: {
        profileId: 'safari:default',
        sourceKind: 'browser-history',
      },
      notes: ['Preview mode simulated a Safari History.db import.'],
      recognizedFiles: inspection.recognizedFiles,
    })
    expect(
      state.snapshot.recentImportBatches.some(
        (batch) => batch.id === previousInspection.importBatch!.id,
      ),
    ).toBe(true)
  })

  test('preserves explicit Browser Direct profile ids during import', () => {
    const state = createMockState()

    const inspection = buildMockBrowserHistoryInspection(
      state,
      {
        browserFamily: 'chromium',
        dryRun: false,
        profileId: 'chrome:Work',
        sourcePath: '/Users/test/Chrome/Profile 1/History',
      },
      false,
    )

    expect(inspection.importBatch).toMatchObject({
      profileId: 'chrome:Work',
      sourceKind: 'browser-history',
      sourcePath: '/Users/test/Chrome/Profile 1/History',
    })
    expect(state.importBatchDetails[inspection.importBatch!.id]).toMatchObject({
      batch: {
        profileId: 'chrome:Work',
      },
      notes: ['Preview mode simulated a Chromium History import.'],
    })
  })

  test('routes Browser Direct commands through default preview and import requests', () => {
    const state = createMockState()

    expect(
      handlePreviewWorkflowCommand('inspect_browser_history', undefined, state),
    ).toMatchObject({
      sourcePath: '/tmp/History',
      importBatch: null,
      recognizedFiles: [
        expect.objectContaining({
          status: 'previewed',
        }),
      ],
    })
    expect(
      handlePreviewWorkflowCommand('import_browser_history', undefined, state),
    ).toMatchObject({
      sourcePath: '/tmp/History',
      importBatch: expect.objectContaining({
        sourceKind: 'browser-history',
      }),
      recognizedFiles: [
        expect.objectContaining({
          status: 'imported',
        }),
      ],
    })
  })

  test('returns an explicit browser-preview response for schedule repair', () => {
    const state = createMockState()

    expect(
      handlePreviewWorkflowCommand('repair_schedule', undefined, state),
    ).toMatchObject({
      applied: false,
      files: [],
      message: 'Repair is not available in browser preview mode.',
      platform: 'macos',
    })
  })

  test('throws for run_full_archive_restore in browser preview mode', () => {
    const state = createMockState()
    expect(() =>
      handlePreviewWorkflowCommand(
        'run_full_archive_restore',
        undefined,
        state,
      ),
    ).toThrow('Snapshot restore is not available in browser preview.')
  })
})
