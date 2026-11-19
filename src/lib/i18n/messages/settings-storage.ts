import { defineMessages } from '../define'

/** Settings → Storage: space, export, moving computers, safety copies, restore and delete. */
export const settingsStorage = defineMessages({
  en: {
    moveGroup: 'Move and restore',
    copyUnchecked:
      'Your previous archive is kept as a safety copy, but PathKeep couldn’t check it: {reason}. It will be checked before you restore from it.',
    location: {
      title: 'Archive location',
      showFinder: 'Show in Finder',
      show: 'Show in folder',
      failed: 'Could not open the folder',
    },
    usage: {
      title: 'Disk usage',
      archive: 'Archive',
      indexes: 'Search indexes',
      snapshots: 'Snapshots',
      other: 'Other',
      failed: 'Could not measure disk usage right now.',
    },
    freeSpace: {
      rowTitle: 'Free up space',
      rowDescription:
        'Delete old snapshots, exports and leftovers. Your history stays.',
      action: 'Review…',
      title: 'Free up space',
      intro: 'Pick what to delete. Your history is not on this list.',
      loadFailed: 'Could not check what can be deleted. {message}',
      nothing: 'Nothing to clean up right now.',
      bucket: {
        snapshots: {
          title: 'Browser history copies',
          description:
            'Copies of your browsers’ history files saved during backups, and copies that can’t be restored from.',
        },
        exports: {
          title: 'Exports',
          description: 'Files you exported earlier.',
        },
        staging: {
          title: 'Temporary files',
          description: 'Left over from backups and imports.',
        },
        quarantine: {
          title: 'Set-aside files',
          description: 'Files PathKeep moved aside during a restore or repair.',
        },
      },
      copies: 'Safety copies',
      copyMade: 'Made {date}',
      notRestorable: 'Can’t be restored from',
      copyWarning: 'You can’t restore from a safety copy once it’s deleted.',
      noCopyLeft:
        'No safety copy will be left to restore from. Your history itself stays.',
      delete: 'Delete {size}',
      done: 'Freed {size}.',
      failed: 'Nothing was deleted. {message}',
    },
    previews: {
      title: 'Link preview images',
      description_one: '{count} image, {size}',
      description_other: '{count} images, {size}',
      statsFailed: 'Could not measure the preview images.',
      action: 'Clear…',
      confirmTitle: 'Delete all link preview images?',
      confirmBody_one:
        '{count} image ({size}) is deleted. PathKeep downloads previews again only if link previews are on.',
      confirmBody_other:
        '{count} images ({size}) are deleted. PathKeep downloads previews again only if link previews are on.',
      cleared: 'Freed {size}.',
      clearFailed: 'Could not delete the preview images',
    },
    export: {
      title: 'Export history',
      description: 'Save your whole history as one file',
      format: 'Export format',
      formats: {
        html: 'HTML',
        markdown: 'Markdown',
        text: 'Plain text',
        jsonl: 'JSON Lines',
      },
      action: 'Export…',
      stop: 'Stop',
      stopping: 'Stopping…',
      cancelled: 'Export stopped. No file was saved.',
      progressLabel: 'Export progress',
      progress: '{written} of about {total} visits · {size}',
      progressNoTotal: '{written} visits · {size}',
      finishing: 'Saving the file…',
      leftUnderMinute: 'Less than a minute left',
      left_one: 'About {count} minute left',
      left_other: 'About {count} minutes left',
      done_one: 'Exported {count} visit.',
      done_other: 'Exported {count} visits.',
      failed: 'Export failed',
    },
    move: {
      title: 'Move to another computer',
      description:
        'Pack everything into one file, or open a file made on another computer.',
      exportAction: 'Export…',
      importAction: 'Import…',
      choose: 'Choose…',
      exportTitle: 'Pack PathKeep into one file',
      exportIntro:
        'The file holds your history, where each visit came from, your Ask chats and your settings, encrypted with a password you choose here. Search and Insights are rebuilt on the other computer.',
      saveTo: 'Save to',
      exportPassword: 'Password for this file',
      exportPasswordConfirm: 'Type it again',
      exportPasswordHint:
        'You’ll need it to open the file on the other computer, where it becomes the archive’s password. It can’t be recovered.',
      passwordsDiffer: 'The two passwords don’t match.',
      stays:
        'Your app passcode, the backup schedule and the safety copies stay on this computer.',
      export: 'Export',
      exported: 'Saved {size}.',
      exportFailed: 'Could not write the file. {message}',
      importTitle: 'Open a PathKeep file',
      importIntro:
        'PathKeep checks the file first and shows what is in it. Nothing changes until you confirm.',
      file: 'File',
      check: 'Check file',
      checkFailed: 'This file can’t be used. {message}',
      sourcePassword: 'Password of this file',
      oldPasswordHint:
        'This file was made by PathKeep 0.3: its password is the archive password you used on that computer.',
      wrongPassword: 'That password doesn’t open this file.',
      replaces:
        'This replaces the archive on this computer. The current one is kept as a safety copy you can go back to. It takes about as much disk space as the archive; Free up space can delete it. The safety copies already on this computer stay as they are.',
      replace: 'Replace my archive',
      import: 'Import',
      imported: 'Import finished.',
      importFailed: 'Import failed. Your archive was not changed. {message}',
      newerFile:
        'This file was made by a newer version of PathKeep. Update PathKeep in Settings → About, then open the file again. Your archive was not changed.',
      summary: {
        made: 'Made',
        version: 'PathKeep version',
        encrypted: 'Encrypted',
        yes: 'Yes',
        no: 'No',
        size: 'Size',
        upgrade:
          'It was made by an older PathKeep and will be updated as it opens.',
        noChats:
          'Files from PathKeep 0.3 don’t include Ask chats; they stay on the other computer.',
        copies:
          'Files from PathKeep 0.5 and earlier also hold the safety copies of the computer they were made on. Any in this file are added to the ones here.',
      },
    },
    restore: {
      rowTitle: 'Restore from a safety copy',
      rowDescription:
        'PathKeep keeps a copy before risky changes like a new password',
      action: 'Choose…',
      title: 'Restore from a safety copy',
      intro:
        'Your archive goes back to how it was when the copy was made. Its password and encryption stay as they are now.',
      loadFailed: 'Could not list the safety copies. {message}',
      empty: 'There are no safety copies yet.',
      op: {
        rekey: 'Before a password change',
        reconcile: 'Before an encryption repair',
        import: 'Before an import',
        restore: 'Before a restore',
        periodic: 'Regular copy',
        manual: 'Made by you',
        agentRepair: 'Before a conversation repair',
        unknown: 'Safety copy',
      },
      password: 'Password of this copy',
      passwordHint:
        'The password the archive had when this copy was made. Your current password stays.',
      encryptsCopy:
        'This copy was made before encryption was turned on. It is encrypted with your current password as it is restored, which takes longer.',
      staysUnencrypted:
        'Your archive isn’t encrypted, so this copy is restored unencrypted, like the rest of your archive.',
      warning:
        'Anything added since then is not in the copy. Your current archive is kept as a safety copy you can go back to. It takes about as much disk space as the archive; Free up space can delete it.',
      confirm: 'Restore',
      done: 'Archive restored.',
      failed: 'The restore did not finish. {message}',
      unchecked: 'Not checked yet',
      conversationsUnreadable: 'Conversations couldn’t be read',
      conversationsUnreadableNote:
        'Saved Agent conversations couldn’t be read when this copy was made. It restores browsing history like any other copy, but it can’t be used to repair conversations.',
      checkedFirst:
        'This copy hasn’t been checked yet. It’s checked before the restore; if it can’t be read, your archive stays as it is.',
    },
    safetyCopy: {
      rowTitle: 'Make a safety copy',
      rowDescription:
        'A copy of the archive as it is now, to restore from later',
      action: 'Make a copy…',
      title: 'Make a safety copy',
      intro:
        'PathKeep copies the archive as it is right now. You can restore from the copy later in Restore from a safety copy.',
      folder: 'It goes into this folder:',
      holds:
        'It holds everything in the archive as it is now: browsing history and any saved Agent conversations.',
      unreadable:
        'Saved Agent conversations can’t be read right now. The copy keeps their files exactly as they are and can restore browsing history, but it can’t be used to repair conversations.',
      space:
        'It takes about as much disk space as the archive. Free up space can delete it.',
      checking: 'Checking what the copy would hold',
      checkFailed: 'Couldn’t check what the copy would hold. {message}',
      confirm: 'Make the copy',
      copying:
        'Copying the archive. This can take a few minutes for a large archive.',
      done: 'Safety copy made.',
      failed: 'No copy was made. {message}',
      stale:
        'The archive changed after the check, so no copy was made. Check again.',
      checkAgain: 'Check again',
      preservationFailed:
        'PathKeep couldn’t read a file it has to copy, so it made no copy. {message}',
    },
    wipe: {
      rowTitle: 'Delete all data',
      rowDescription:
        'Archive, indexes, snapshots and exports are removed. This cannot be undone.',
      action: 'Delete…',
      title: 'Delete all data?',
      intro: 'PathKeep removes everything it stored and starts over.',
      previewFailed: 'Could not check what would be deleted. {message}',
      summary: '{visits} and {size} of files will be deleted:',
      keychain: 'The archive password saved in the keychain is removed too.',
      schedule: 'Automatic backup is turned off and removed from the system:',
      final:
        'This cannot be undone. Export or move your data first if you might want it.',
      scheduleFailed:
        'Your data is deleted, but automatic backup could not be removed. {message} Turn it off in Backup after setup.',
      unfinished:
        'Deleting did not finish. {message} PathKeep finishes it before you set up again.',
      stopped:
        'Deleting stopped partway. {message} When PathKeep starts again, it either finishes deleting or puts everything back.',
      typeWord: 'Type {word} to confirm',
      confirm: 'Delete forever',
      failed: 'Nothing was deleted. {message}',
    },
  },
  'zh-CN': {
    moveGroup: '迁移与恢复',
    copyUnchecked:
      '之前的存档已保留为安全副本，但 PathKeep 没能检查它：{reason}。从它恢复之前会先检查。',
    location: {
      title: '存档位置',
      showFinder: '在访达中显示',
      show: '在文件夹中显示',
      failed: '无法打开文件夹',
    },
    usage: {
      title: '磁盘用量',
      archive: '存档',
      indexes: '搜索索引',
      snapshots: '快照',
      other: '其他',
      failed: '暂时无法计算磁盘用量。',
    },
    freeSpace: {
      rowTitle: '释放空间',
      rowDescription: '删除旧快照、导出文件和残留文件。你的历史不受影响。',
      action: '查看…',
      title: '释放空间',
      intro: '选择要删除的内容。你的历史不在这个列表里。',
      loadFailed: '无法检查可以删除的内容。{message}',
      nothing: '现在没有需要清理的内容。',
      bucket: {
        snapshots: {
          title: '浏览器历史文件的副本',
          description: '备份时保存的浏览器历史文件，以及无法用来恢复的副本。',
        },
        exports: {
          title: '导出文件',
          description: '你之前导出的文件。',
        },
        staging: {
          title: '临时文件',
          description: '备份和导入时留下的文件。',
        },
        quarantine: {
          title: '移到一边的文件',
          description: 'PathKeep 在恢复或修复时移到一边的文件。',
        },
      },
      copies: '安全副本',
      copyMade: '{date} 创建',
      notRestorable: '无法用来恢复',
      copyWarning: '安全副本删除后就不能再用来恢复。',
      noCopyLeft: '删完就没有可以用来恢复的安全副本了。你的历史本身不受影响。',
      delete: '删除 {size}',
      done: '已释放 {size}。',
      failed: '没有删除任何内容。{message}',
    },
    previews: {
      title: '链接预览图',
      description_one: '{count} 张图片，{size}',
      description_other: '{count} 张图片，{size}',
      statsFailed: '无法计算预览图占用的空间。',
      action: '清除…',
      confirmTitle: '要删除所有链接预览图吗？',
      confirmBody_one:
        '将删除 {count} 张图片（{size}）。只有开启链接预览时，PathKeep 才会重新下载。',
      confirmBody_other:
        '将删除 {count} 张图片（{size}）。只有开启链接预览时，PathKeep 才会重新下载。',
      cleared: '已释放 {size}。',
      clearFailed: '无法删除预览图',
    },
    export: {
      title: '导出历史',
      description: '把你的全部历史保存为一个文件',
      format: '导出格式',
      formats: {
        html: 'HTML',
        markdown: 'Markdown',
        text: '纯文本',
        jsonl: 'JSON Lines',
      },
      action: '导出…',
      stop: '停止',
      stopping: '正在停止…',
      cancelled: '导出已停止，没有保存文件。',
      progressLabel: '导出进度',
      progress: '已写入 {written} / 约 {total} 条记录 · {size}',
      progressNoTotal: '已写入 {written} 条记录 · {size}',
      finishing: '正在保存文件…',
      leftUnderMinute: '剩余不到一分钟',
      left_one: '大约还要 {count} 分钟',
      left_other: '大约还要 {count} 分钟',
      done_one: '已导出 {count} 条记录。',
      done_other: '已导出 {count} 条记录。',
      failed: '导出失败',
    },
    move: {
      title: '迁移到另一台电脑',
      description: '把所有内容打包成一个文件，或打开在另一台电脑上制作的文件。',
      exportAction: '导出…',
      importAction: '导入…',
      choose: '选择…',
      exportTitle: '把 PathKeep 打包成一个文件',
      exportIntro:
        '文件包含你的历史、每条记录的来源、Ask 对话和设置，并用你在这里设置的密码加密。搜索和智能分析会在另一台电脑上重建。',
      saveTo: '保存到',
      exportPassword: '文件密码',
      exportPasswordConfirm: '再输入一次',
      exportPasswordHint:
        '在另一台电脑上打开文件时需要它，之后它就是那边存档的密码。忘记了就无法找回。',
      passwordsDiffer: '两次输入的密码不一致。',
      stays: '应用密码、备份计划和安全副本会留在这台电脑上。',
      export: '导出',
      exported: '已保存 {size}。',
      exportFailed: '无法写入文件。{message}',
      importTitle: '打开 PathKeep 文件',
      importIntro:
        'PathKeep 会先检查文件并显示其中的内容。在你确认之前不会有任何改动。',
      file: '文件',
      check: '检查文件',
      checkFailed: '这个文件无法使用。{message}',
      sourcePassword: '文件密码',
      oldPasswordHint:
        '这个文件由 PathKeep 0.3 导出：它的密码就是你在那台电脑上用的存档密码。',
      wrongPassword: '这个密码打不开这个文件。',
      replaces:
        '这会替换这台电脑上的存档。当前的存档会保留为一份安全副本，之后还能恢复回来；它占用的空间和存档差不多，可以在“释放空间”删除。这台电脑原有的安全副本保持不变。',
      replace: '替换我的存档',
      import: '导入',
      imported: '导入完成。',
      importFailed: '导入失败，你的存档没有改动。{message}',
      newerFile:
        '这个文件是较新版本的 PathKeep 做的。请先在「设置 → 关于」更新 PathKeep，再重新打开这个文件。你的存档没有改动。',
      summary: {
        made: '制作时间',
        version: 'PathKeep 版本',
        encrypted: '已加密',
        yes: '是',
        no: '否',
        size: '大小',
        upgrade: '它由较旧版本的 PathKeep 制作，打开时会自动更新。',
        noChats:
          'PathKeep 0.3 导出的文件不含助理对话；它们仍留在原来的电脑上。',
        copies:
          'PathKeep 0.5 及更早版本导出的文件还带着原来那台电脑的安全副本；这个文件里的会加到这台电脑的安全副本中。',
      },
    },
    restore: {
      rowTitle: '从安全副本恢复',
      rowDescription: '在更改密码等有风险的操作之前，PathKeep 会先保存一份副本',
      action: '选择…',
      title: '从安全副本恢复',
      intro: '你的存档会回到副本保存时的状态；密码和加密方式保持现在的样子。',
      loadFailed: '无法列出安全副本。{message}',
      empty: '还没有任何安全副本。',
      op: {
        rekey: '更改密码之前',
        reconcile: '修复加密之前',
        import: '导入之前',
        restore: '恢复之前',
        periodic: '定期副本',
        manual: '你创建的副本',
        agentRepair: '修复对话之前',
        unknown: '安全副本',
      },
      password: '这份副本的密码',
      passwordHint: '保存这份副本时存档使用的密码。恢复后仍然使用当前的密码。',
      encryptsCopy:
        '这份副本是开启加密之前保存的，恢复时会用当前的密码加密，需要多花一些时间。',
      staysUnencrypted:
        '当前的存档没有加密，这份副本恢复后也不加密，和存档的其他部分一样。',
      warning:
        '之后新增的内容不在副本里。当前的存档会保留为一份安全副本，之后还能恢复回来；它占用的空间和存档差不多，可以在“释放空间”删除。',
      confirm: '恢复',
      done: '存档已恢复。',
      failed: '恢复没有完成。{message}',
      unchecked: '尚未检查',
      conversationsUnreadable: '对话读不了',
      conversationsUnreadableNote:
        '创建这份副本时，已存的 Agent 对话读不了。它和其他副本一样能恢复浏览记录，但不能用来修复对话。',
      checkedFirst:
        '这份副本还没检查过。恢复之前会先检查；读不出来的话，你的存档保持原样。',
    },
    safetyCopy: {
      rowTitle: '创建安全副本',
      rowDescription: '把存档现在的样子复制一份，以后可以从它恢复',
      action: '创建…',
      title: '创建安全副本',
      intro:
        'PathKeep 会把存档现在的样子复制一份。以后可以在「从安全副本恢复」里用这份副本恢复。',
      folder: '副本会放在这个文件夹：',
      holds:
        '副本里是存档现在的全部内容：浏览记录，以及已存的 Agent 对话（如果有的话）。',
      unreadable:
        '已存的 Agent 对话现在读不了。副本会把它们的文件原样带走，能用来恢复浏览记录，但不能用来修复对话。',
      space: '它占用的空间和存档差不多，可以在「释放空间」里删除。',
      checking: '正在检查副本会包含什么',
      checkFailed: '无法检查副本会包含什么。{message}',
      confirm: '创建副本',
      copying: '正在复制存档。存档很大时可能要几分钟。',
      done: '安全副本已创建。',
      failed: '没有创建副本。{message}',
      stale: '检查之后存档有了变化，所以没有创建副本。请重新检查。',
      checkAgain: '重新检查',
      preservationFailed:
        'PathKeep 读不了一个必须复制的文件，所以没有创建副本。{message}',
    },
    wipe: {
      rowTitle: '删除所有数据',
      rowDescription: '存档、索引、快照和导出文件都会被删除，无法恢复。',
      action: '删除…',
      title: '要删除所有数据吗？',
      intro: 'PathKeep 会删除它保存的一切，然后从头开始。',
      previewFailed: '无法检查将被删除的内容。{message}',
      summary: '将删除 {visits}，共 {size} 的文件：',
      keychain: '保存在钥匙串里的存档密码也会被删除。',
      schedule: '自动备份也会关闭，并从系统中移除：',
      final: '此操作无法撤销。如果以后可能还需要，请先导出或迁移你的数据。',
      scheduleFailed:
        '数据已删除，但自动备份没能移除。{message} 设置完成后，请到「备份」关闭它。',
      unfinished:
        '删除没有完成。{message} 重新设置之前，PathKeep 会先把它完成。',
      stopped:
        '删除中途停止了。{message} PathKeep 下次启动时，会把删除做完，或把所有内容放回原处。',
      typeWord: '输入 {word} 以确认',
      confirm: '永久删除',
      failed: '没有删除任何内容。{message}',
    },
  },
  'zh-TW': {
    moveGroup: '搬移與還原',
    copyUnchecked:
      '先前的存檔已保留為安全副本，但 PathKeep 沒能檢查它：{reason}。從它還原之前會先檢查。',
    location: {
      title: '存檔位置',
      showFinder: '在 Finder 中顯示',
      show: '在資料夾中顯示',
      failed: '無法開啟資料夾',
    },
    usage: {
      title: '磁碟用量',
      archive: '存檔',
      indexes: '搜尋索引',
      snapshots: '快照',
      other: '其他',
      failed: '暫時無法計算磁碟用量。',
    },
    freeSpace: {
      rowTitle: '釋放空間',
      rowDescription: '刪除舊快照、匯出檔案和殘留檔案。你的歷史不受影響。',
      action: '檢視…',
      title: '釋放空間',
      intro: '選擇要刪除的內容。你的歷史不在這個清單裡。',
      loadFailed: '無法檢查可以刪除的內容。{message}',
      nothing: '目前沒有需要清理的內容。',
      bucket: {
        snapshots: {
          title: '瀏覽器歷史檔的複本',
          description: '備份時存下的瀏覽器歷史檔，以及無法用來還原的副本。',
        },
        exports: {
          title: '匯出檔案',
          description: '你之前匯出的檔案。',
        },
        staging: {
          title: '暫存檔案',
          description: '備份和匯入時留下的檔案。',
        },
        quarantine: {
          title: '移到一旁的檔案',
          description: 'PathKeep 在還原或修復時移到一旁的檔案。',
        },
      },
      copies: '安全副本',
      copyMade: '{date} 建立',
      notRestorable: '無法用來還原',
      copyWarning: '安全副本刪除後就不能再拿來還原。',
      noCopyLeft: '刪完就沒有可以用來還原的安全副本了。你的歷史本身不受影響。',
      delete: '刪除 {size}',
      done: '已釋放 {size}。',
      failed: '沒有刪除任何內容。{message}',
    },
    previews: {
      title: '連結預覽圖',
      description_one: '{count} 張圖片，{size}',
      description_other: '{count} 張圖片，{size}',
      statsFailed: '無法計算預覽圖佔用的空間。',
      action: '清除…',
      confirmTitle: '要刪除所有連結預覽圖嗎？',
      confirmBody_one:
        '將刪除 {count} 張圖片（{size}）。只有開啟連結預覽時，PathKeep 才會重新下載。',
      confirmBody_other:
        '將刪除 {count} 張圖片（{size}）。只有開啟連結預覽時，PathKeep 才會重新下載。',
      cleared: '已釋放 {size}。',
      clearFailed: '無法刪除預覽圖',
    },
    export: {
      title: '匯出歷史',
      description: '把你的全部歷史存成一個檔案',
      format: '匯出格式',
      formats: {
        html: 'HTML',
        markdown: 'Markdown',
        text: '純文字',
        jsonl: 'JSON Lines',
      },
      action: '匯出…',
      stop: '停止',
      stopping: '正在停止…',
      cancelled: '匯出已停止，沒有儲存檔案。',
      progressLabel: '匯出進度',
      progress: '已寫入 {written} / 約 {total} 筆記錄 · {size}',
      progressNoTotal: '已寫入 {written} 筆記錄 · {size}',
      finishing: '正在儲存檔案…',
      leftUnderMinute: '剩不到一分鐘',
      left_one: '大約還要 {count} 分鐘',
      left_other: '大約還要 {count} 分鐘',
      done_one: '已匯出 {count} 筆記錄。',
      done_other: '已匯出 {count} 筆記錄。',
      failed: '匯出失敗',
    },
    move: {
      title: '搬到另一台電腦',
      description: '把所有內容打包成一個檔案，或開啟在另一台電腦上製作的檔案。',
      exportAction: '匯出…',
      importAction: '匯入…',
      choose: '選擇…',
      exportTitle: '把 PathKeep 打包成一個檔案',
      exportIntro:
        '檔案包含你的歷史、每筆記錄的來源、Ask 對話和設定，並用你在這裡設定的密碼加密。搜尋和智慧分析會在另一台電腦上重建。',
      saveTo: '儲存到',
      exportPassword: '檔案密碼',
      exportPasswordConfirm: '再輸入一次',
      exportPasswordHint:
        '在另一台電腦上開啟檔案時需要它，之後它就是那邊存檔的密碼。忘記了就無法找回。',
      passwordsDiffer: '兩次輸入的密碼不一致。',
      stays: 'App 密碼、備份排程和安全副本會留在這台電腦上。',
      export: '匯出',
      exported: '已儲存 {size}。',
      exportFailed: '無法寫入檔案。{message}',
      importTitle: '開啟 PathKeep 檔案',
      importIntro:
        'PathKeep 會先檢查檔案並顯示其中的內容。在你確認之前不會有任何變動。',
      file: '檔案',
      check: '檢查檔案',
      checkFailed: '這個檔案無法使用。{message}',
      sourcePassword: '檔案密碼',
      oldPasswordHint:
        '這個檔案由 PathKeep 0.3 匯出：它的密碼就是你在那台電腦上用的存檔密碼。',
      wrongPassword: '這組密碼打不開這個檔案。',
      replaces:
        '這會取代這台電腦上的存檔。目前的存檔會保留為一份安全副本，之後還能還原回來；它佔用的空間和存檔差不多，可以在「釋放空間」刪除。這台電腦原有的安全副本維持原樣。',
      replace: '取代我的存檔',
      import: '匯入',
      imported: '匯入完成。',
      importFailed: '匯入失敗，你的存檔沒有變動。{message}',
      newerFile:
        '這個檔案是較新版本的 PathKeep 做的。請先在「設定 → 關於」更新 PathKeep，再重新開啟這個檔案。你的存檔沒有變動。',
      summary: {
        made: '製作時間',
        version: 'PathKeep 版本',
        encrypted: '已加密',
        yes: '是',
        no: '否',
        size: '大小',
        upgrade: '它由較舊版本的 PathKeep 製作，開啟時會自動更新。',
        noChats:
          'PathKeep 0.3 匯出的檔案不含助理對話；它們仍留在原來的電腦上。',
        copies:
          'PathKeep 0.5 及更早版本匯出的檔案還帶著原來那台電腦的安全副本；這個檔案裡的會加到這台電腦的安全副本裡。',
      },
    },
    restore: {
      rowTitle: '從安全副本還原',
      rowDescription: '在更改密碼等有風險的操作之前，PathKeep 會先保存一份副本',
      action: '選擇…',
      title: '從安全副本還原',
      intro: '你的存檔會回到副本保存時的狀態；密碼和加密方式維持現在的樣子。',
      loadFailed: '無法列出安全副本。{message}',
      empty: '還沒有任何安全副本。',
      op: {
        rekey: '更改密碼之前',
        reconcile: '修復加密之前',
        import: '匯入之前',
        restore: '還原之前',
        periodic: '定期副本',
        manual: '你建立的副本',
        agentRepair: '修復對話之前',
        unknown: '安全副本',
      },
      password: '這份副本的密碼',
      passwordHint: '保存這份副本時存檔使用的密碼。還原後仍然使用目前的密碼。',
      encryptsCopy:
        '這份副本是開啟加密之前保存的，還原時會用目前的密碼加密，需要多花一些時間。',
      staysUnencrypted:
        '目前的存檔沒有加密，這份副本還原後也不加密，和存檔的其他部分一樣。',
      warning:
        '之後新增的內容不在副本裡。目前的存檔會保留為一份安全副本，之後還能還原回來；它佔用的空間和存檔差不多，可以在「釋放空間」刪除。',
      confirm: '還原',
      done: '存檔已還原。',
      failed: '還原沒有完成。{message}',
      unchecked: '尚未檢查',
      conversationsUnreadable: '對話讀不了',
      conversationsUnreadableNote:
        '建立這份副本時，已存的 Agent 對話讀不了。它跟其他副本一樣能還原瀏覽紀錄，但不能用來修復對話。',
      checkedFirst:
        '這份副本還沒檢查過。還原之前會先檢查；讀不出來的話，你的存檔保持原樣。',
    },
    safetyCopy: {
      rowTitle: '建立安全副本',
      rowDescription: '把存檔現在的樣子複製一份，之後可以從它還原',
      action: '建立…',
      title: '建立安全副本',
      intro:
        'PathKeep 會把存檔現在的樣子複製一份。之後可以在「從安全副本還原」用這份副本還原。',
      folder: '副本會放在這個資料夾：',
      holds:
        '副本裡是存檔現在的全部內容：瀏覽紀錄，以及已存的 Agent 對話（如果有的話）。',
      unreadable:
        '已存的 Agent 對話現在讀不了。副本會把它們的檔案原封不動地帶走，能用來還原瀏覽紀錄，但不能用來修復對話。',
      space: '它佔用的空間和存檔差不多，可以在「釋放空間」刪除。',
      checking: '正在檢查副本會包含什麼',
      checkFailed: '無法檢查副本會包含什麼。{message}',
      confirm: '建立副本',
      copying: '正在複製存檔。存檔很大時可能要幾分鐘。',
      done: '安全副本已建立。',
      failed: '沒有建立副本。{message}',
      stale: '檢查之後存檔有了變化，所以沒有建立副本。請重新檢查。',
      checkAgain: '重新檢查',
      preservationFailed:
        'PathKeep 讀不了一個必須複製的檔案，所以沒有建立副本。{message}',
    },
    wipe: {
      rowTitle: '刪除所有資料',
      rowDescription: '存檔、索引、快照和匯出檔案都會被刪除，無法復原。',
      action: '刪除…',
      title: '要刪除所有資料嗎？',
      intro: 'PathKeep 會刪除它儲存的一切，然後從頭開始。',
      previewFailed: '無法檢查將被刪除的內容。{message}',
      summary: '將刪除 {visits}，共 {size} 的檔案：',
      keychain: '存在鑰匙圈裡的存檔密碼也會被刪除。',
      schedule: '自動備份也會關閉，並從系統中移除：',
      final: '此操作無法復原。如果之後可能還需要，請先匯出或搬移你的資料。',
      scheduleFailed:
        '資料已刪除，但自動備份沒能移除。{message} 設定完成後，請到「備份」關閉它。',
      unfinished:
        '刪除沒有做完。{message} 重新設定之前，PathKeep 會先把它做完。',
      stopped:
        '刪除中途停止了。{message} PathKeep 下次啟動時，會把刪除做完，或把所有內容放回原處。',
      typeWord: '輸入 {word} 以確認',
      confirm: '永久刪除',
      failed: '沒有刪除任何內容。{message}',
    },
  },
})
