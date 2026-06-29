/**
 * This module wraps a focused slice of desktop commands behind a typed front-end client.
 *
 * Why this file exists:
 * - The `backend-client` layer keeps page components from having to know raw command names or transport details.
 * - If a route needs desktop data, start here before reaching for legacy preview helpers.
 *
 * Main declarations:
 * - `backend`
 *
 * Source-of-truth notes:
 * - Transport boundaries are defined by `docs/architecture/desktop-command-surface.md`.
 * - This layer should stay typed, boring, and free of user-facing copy so routes can keep ownership of UX decisions.
 */

export * from './annotations'
export * from './app'
export * from './archive'
export * from './audit'
export * from './content-enrichment'
export * from './dashboard'
export * from './explorer'
export * from './import'
export * from './intelligence'
export * from './migration'
export * from './schedule'
export * from './security'
export * from './shared'
export * from './stars'
export * from './support'
export * from './update'

import { annotationsClient } from './annotations'
import { appClient } from './app'
import { archiveClient } from './archive'
import { auditClient } from './audit'
import { contentEnrichmentClient } from './content-enrichment'
import { dashboardClient } from './dashboard'
import { explorerClient } from './explorer'
import { importClient } from './import'
import { intelligenceClient } from './intelligence'
import { migrationClient } from './migration'
import { scheduleClient } from './schedule'
import { securityClient } from './security'
import { starsClient } from './stars'
import { supportClient } from './support'
import { updateClient } from './update'

/**
 * Exposes the compatibility backend facade for callers that still want one typed object
 * instead of importing per-domain clients directly.
 *
 * This stays current for real frontend consumers; browser-preview fallback behavior is
 * owned by `backend-client/shared.ts` plus `src/lib/backend.ts`, not by stale route-local mocks.
 */
export const backend = {
  getAppBuildInfo: appClient.getBuildInfo,
  loadAppLockStatus: appClient.getLockStatus,
  getAppSnapshot: appClient.getSnapshot,
  saveConfig: appClient.saveConfig,
  initializeArchive: archiveClient.initializeArchive,
  rekeyArchive: securityClient.executeRekey,
  previewRekeyArchive: securityClient.previewRekey,
  previewSnapshotRestore: archiveClient.previewSnapshotRestore,
  runSnapshotRestore: archiveClient.runSnapshotRestore,
  previewRetentionPrune: archiveClient.previewRetentionPrune,
  runRetentionPrune: archiveClient.runRetentionPrune,
  setSessionDatabaseKey: appClient.setSessionDatabaseKey,
  clearSessionDatabaseKey: appClient.clearSessionDatabaseKey,
  reconcileArchiveEncryption: appClient.reconcileArchiveEncryption,
  setAppLockPasscode: appClient.setAppLockPasscode,
  clearAppLockPasscode: appClient.clearAppLockPasscode,
  lockAppSession: appClient.lockAppSession,
  unlockAppSession: appClient.unlockAppSession,
  runBackupNow: archiveClient.runBackupNow,
  queryHistory: explorerClient.queryHistory,
  loadHistoryFavicons: explorerClient.loadHistoryFavicons,
  loadHistoryOgImages: explorerClient.loadHistoryOgImages,
  markOgImagesShown: explorerClient.markOgImagesShown,
  triggerOgImageRefetch: explorerClient.triggerOgImageRefetch,
  prefetchOgImages: explorerClient.prefetchOgImages,
  getOgImageStorageStats: explorerClient.getOgImageStorageStats,
  getOgImageCoverageStats: explorerClient.getOgImageCoverageStats,
  clearOgImageCache: explorerClient.clearOgImageCache,
  runOgImageCleanup: explorerClient.runOgImageCleanup,
  getBrowseDayInsights: explorerClient.getBrowseDayInsights,
  getUrlAnnotation: annotationsClient.getUrlAnnotation,
  setUrlNotes: annotationsClient.setUrlNotes,
  replaceUrlTags: annotationsClient.replaceUrlTags,
  listUrlAnnotations: annotationsClient.listUrlAnnotations,
  searchUrlAnnotations: annotationsClient.searchUrlAnnotations,
  setStar: starsClient.setStar,
  unsetStar: starsClient.unsetStar,
  getStarStatus: starsClient.getStarStatus,
  listStars: starsClient.listStars,
  getStarCounts: starsClient.getStarCounts,
  getContentFetchSettings: contentEnrichmentClient.getContentFetchSettings,
  setContentFetchSettings: contentEnrichmentClient.setContentFetchSettings,
  listVisitEnrichment: contentEnrichmentClient.listVisitEnrichment,
  contentFetchNow: contentEnrichmentClient.contentFetchNow,
  enqueueContentFetchWorkingSet:
    contentEnrichmentClient.enqueueContentFetchWorkingSet,
  exportAppData: migrationClient.exportAppData,
  previewAppDataImport: migrationClient.previewAppDataImport,
  applyAppDataImport: migrationClient.applyAppDataImport,
  loadDashboardSnapshot: dashboardClient.getSnapshot,
  loadAuditRunDetail: auditClient.getRunDetail,
  exportHistory: archiveClient.exportHistory,
  inspectTakeout: importClient.inspectTakeout,
  importTakeout: importClient.importTakeout,
  inspectBrowserHistory: importClient.inspectBrowserHistory,
  importBrowserHistory: importClient.importBrowserHistory,
  previewImportBatch: importClient.previewBatch,
  revertImportBatch: importClient.revertBatch,
  restoreImportBatch: importClient.restoreBatch,
  previewSchedule: scheduleClient.previewInstall,
  scheduleStatus: scheduleClient.getStatus,
  applySchedule: scheduleClient.applyInstall,
  removeSchedule: scheduleClient.removeInstall,
  repairSchedule: scheduleClient.repairInstall,
  doctor: auditClient.getHealthReport,
  repairHealth: auditClient.repairHealth,
  keyringStatus: securityClient.getKeyringStatus,
  securityStatus: securityClient.getStatus,
  keyringGetDatabaseKey: securityClient.getDatabaseKey,
  keyringStoreDatabaseKey: securityClient.storeDatabaseKey,
  keyringClearDatabaseKey: securityClient.clearDatabaseKey,
  storeAiProviderApiKey: intelligenceClient.storeProviderApiKey,
  clearAiProviderApiKey: intelligenceClient.clearProviderApiKey,
  testAiProviderConnection: intelligenceClient.testProviderConnection,
  loadAiQueueStatus: intelligenceClient.getQueueStatus,
  runAiQueueJobs: intelligenceClient.runQueueJobs,
  replayAiJob: intelligenceClient.replayJob,
  cancelAiJob: intelligenceClient.cancelJob,
  buildAiIndex: intelligenceClient.buildIndex,
  estimateReembed: intelligenceClient.estimateReembed,
  searchAiHistory: intelligenceClient.searchHistory,
  askAiAssistant: intelligenceClient.askAssistant,
  loadAiAssistantJob: intelligenceClient.getAssistantJob,
  sendAiChat: intelligenceClient.sendChat,
  cancelAiChat: intelligenceClient.cancelChat,
  saveAiConversation: intelligenceClient.saveConversation,
  listAiConversations: intelligenceClient.listConversations,
  loadAiConversation: intelligenceClient.loadConversation,
  deleteAiConversation: intelligenceClient.deleteConversation,
  renameAiConversation: intelligenceClient.renameConversation,
  listSearchEngineRules: intelligenceClient.listSearchEngineRules,
  upsertSearchEngineRule: intelligenceClient.upsertSearchEngineRule,
  deleteSearchEngineRule: intelligenceClient.deleteSearchEngineRule,
  clearDerivedIntelligence: intelligenceClient.clearDerivedState,
  loadIntelligenceRuntime: intelligenceClient.getRuntime,
  retryIntelligenceJob: intelligenceClient.retryRuntimeJob,
  cancelIntelligenceJob: intelligenceClient.cancelRuntimeJob,
  previewAiIntegrations: intelligenceClient.previewIntegrations,
  downloadStaticEmbeddingModel: intelligenceClient.downloadStaticEmbeddingModel,
  cancelStaticEmbeddingModelDownload:
    intelligenceClient.cancelStaticEmbeddingModelDownload,
  resetAiIndexBuild: intelligenceClient.resetAiIndexBuild,
  resetLocalSecretVault: securityClient.resetLocalSecretVault,
  openPathInFileManager: supportClient.openPathInFileManager,
  openExternalUrl: supportClient.openExternalUrl,
  exportConversationFile: supportClient.exportConversationFile,
  revealLogs: supportClient.revealLogs,
  checkForAppUpdate: updateClient.checkForAppUpdate,
  downloadAndInstallAppUpdate: updateClient.downloadAndInstallAppUpdate,
  relaunchAfterUpdate: updateClient.relaunchAfterUpdate,
}
