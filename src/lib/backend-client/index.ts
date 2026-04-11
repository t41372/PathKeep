export * from './app'
export * from './archive'
export * from './audit'
export * from './dashboard'
export * from './explorer'
export * from './import'
export * from './intelligence'
export * from './remote'
export * from './schedule'
export * from './security'
export * from './shared'
export * from './support'
export * from './update'

import { appClient } from './app'
import { archiveClient } from './archive'
import { auditClient } from './audit'
import { dashboardClient } from './dashboard'
import { explorerClient } from './explorer'
import { importClient } from './import'
import { intelligenceClient } from './intelligence'
import { remoteClient } from './remote'
import { scheduleClient } from './schedule'
import { securityClient } from './security'
import { supportClient } from './support'
import { updateClient } from './update'

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
  setAppLockPasscode: appClient.setAppLockPasscode,
  clearAppLockPasscode: appClient.clearAppLockPasscode,
  lockAppSession: appClient.lockAppSession,
  unlockAppSession: appClient.unlockAppSession,
  runBackupNow: archiveClient.runBackupNow,
  queryHistory: explorerClient.queryHistory,
  loadDashboardSnapshot: dashboardClient.getSnapshot,
  loadAuditRunDetail: auditClient.getRunDetail,
  exportHistory: archiveClient.exportHistory,
  previewRemoteBackup: remoteClient.previewBackup,
  runRemoteBackup: remoteClient.runBackup,
  verifyRemoteBackup: remoteClient.verifyBackup,
  inspectTakeout: importClient.inspectTakeout,
  importTakeout: importClient.importTakeout,
  previewImportBatch: importClient.previewBatch,
  revertImportBatch: importClient.revertBatch,
  restoreImportBatch: importClient.restoreBatch,
  previewSchedule: scheduleClient.previewInstall,
  scheduleStatus: scheduleClient.getStatus,
  applySchedule: scheduleClient.applyInstall,
  removeSchedule: scheduleClient.removeInstall,
  doctor: auditClient.getHealthReport,
  repairHealth: auditClient.repairHealth,
  keyringStatus: securityClient.getKeyringStatus,
  securityStatus: securityClient.getStatus,
  keyringGetDatabaseKey: securityClient.getDatabaseKey,
  keyringStoreDatabaseKey: securityClient.storeDatabaseKey,
  keyringClearDatabaseKey: securityClient.clearDatabaseKey,
  storeS3Credentials: remoteClient.storeCredentials,
  clearS3Credentials: remoteClient.clearCredentials,
  storeAiProviderApiKey: intelligenceClient.storeProviderApiKey,
  clearAiProviderApiKey: intelligenceClient.clearProviderApiKey,
  testAiProviderConnection: intelligenceClient.testProviderConnection,
  loadAiQueueStatus: intelligenceClient.getQueueStatus,
  runAiQueueJobs: intelligenceClient.runQueueJobs,
  replayAiJob: intelligenceClient.replayJob,
  cancelAiJob: intelligenceClient.cancelJob,
  buildAiIndex: intelligenceClient.buildIndex,
  searchAiHistory: intelligenceClient.searchHistory,
  askAiAssistant: intelligenceClient.askAssistant,
  loadAiAssistantJob: intelligenceClient.getAssistantJob,
  runInsightsNow: intelligenceClient.runInsights,
  clearDerivedIntelligence: intelligenceClient.clearDerivedState,
  loadInsights: intelligenceClient.getInsightsSnapshot,
  loadThreadDetail: intelligenceClient.getThreadDetail,
  explainInsight: intelligenceClient.explainInsight,
  loadIntelligenceRuntime: intelligenceClient.getRuntime,
  retryIntelligenceJob: intelligenceClient.retryRuntimeJob,
  cancelIntelligenceJob: intelligenceClient.cancelRuntimeJob,
  previewAiIntegrations: intelligenceClient.previewIntegrations,
  resetLocalSecretVault: securityClient.resetLocalSecretVault,
  openPathInFileManager: supportClient.openPathInFileManager,
  openExternalUrl: supportClient.openExternalUrl,
  checkForAppUpdate: updateClient.checkForAppUpdate,
  downloadAndInstallAppUpdate: updateClient.downloadAndInstallAppUpdate,
  relaunchAfterUpdate: updateClient.relaunchAfterUpdate,
}
