import { defineMessages } from '../define'

/**
 * Long whole-archive operations: their steps, progress and stopping, and
 * what a refusal says about a file it names (which file, and a failed write).
 */
export const maintenance = defineMessages({
  en: {
    phases: {
      exporting: 'Preparing the file',
      importing: 'Opening the file',
      converting: 'Copying the archive',
      rewrapping: 'Changing the password',
      restoring: 'Restoring',
      pruning: 'Removing old snapshots',
      copying: 'Copying the archive',
      snapshotting: 'Taking a snapshot',
      verifying: 'Checking the copy',
      indexing: 'Building the search index',
      projecting: 'Linking pages',
      publishing: 'Finishing',
      merging: 'Merging duplicates',
      preparingConversationRepair: 'Checking the saved conversations',
      repairingConversations: 'Repairing the saved conversations',
    },
    progress: '{done} of {total}',
    stop: 'Stop',
    stopping: 'Stopping…',
    stopped: 'Stopped. Nothing was changed.',
    progressLabel: 'Progress',
    file: {
      browsing: 'A file of the archive',
      evidence: 'The file that holds the saved conversations',
      readModel:
        'The file that holds the conversations’ lists, search and insights',
      heldGeneration:
        'A conversation file that disappeared while PathKeep had it open',
    },
    writeFailed:
      'PathKeep couldn’t write one of the files it was making. Check that the disk has free space and that PathKeep can write to that folder, then try again.',
  },
  'zh-CN': {
    phases: {
      exporting: '正在准备文件',
      importing: '正在打开文件',
      converting: '正在复制存档',
      rewrapping: '正在更换密码',
      restoring: '正在恢复',
      pruning: '正在删除旧快照',
      copying: '正在复制存档',
      snapshotting: '正在建立快照',
      verifying: '正在检查副本',
      indexing: '正在建立搜索索引',
      projecting: '正在关联页面',
      publishing: '即将完成',
      merging: '正在合并重复项',
      preparingConversationRepair: '正在检查已存的对话',
      repairingConversations: '正在修复已存的对话',
    },
    progress: '{done} / {total}',
    stop: '停止',
    stopping: '正在停止…',
    stopped: '已停止，没有任何改动。',
    progressLabel: '进度',
    file: {
      browsing: '存档的文件',
      evidence: '存放已存对话的文件',
      readModel: '存放对话列表、搜索和洞察的文件',
      heldGeneration: 'PathKeep 打开着时不见了的对话文件',
    },
    writeFailed:
      'PathKeep 没能写入它正在生成的一个文件。请确认磁盘还有空间、PathKeep 可以写入那个文件夹，然后再试一次。',
  },
  'zh-TW': {
    phases: {
      exporting: '正在準備檔案',
      importing: '正在開啟檔案',
      converting: '正在複製存檔',
      rewrapping: '正在更換密碼',
      restoring: '正在還原',
      pruning: '正在刪除舊快照',
      copying: '正在複製存檔',
      snapshotting: '正在建立快照',
      verifying: '正在檢查副本',
      indexing: '正在建立搜尋索引',
      projecting: '正在連結頁面',
      publishing: '即將完成',
      merging: '正在合併重複項目',
      preparingConversationRepair: '正在檢查已存的對話',
      repairingConversations: '正在修復已存的對話',
    },
    progress: '{done} / {total}',
    stop: '停止',
    stopping: '正在停止…',
    stopped: '已停止，沒有任何變動。',
    progressLabel: '進度',
    file: {
      browsing: '存檔的檔案',
      evidence: '存放已存對話的檔案',
      readModel: '存放對話清單、搜尋和洞察的檔案',
      heldGeneration: 'PathKeep 開著時不見了的對話檔案',
    },
    writeFailed:
      'PathKeep 沒能寫入它正在產生的一個檔案。請確認磁碟還有空間、PathKeep 可以寫入那個資料夾，然後再試一次。',
  },
})
