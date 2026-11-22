import { defineMessages } from '../define'

/**
 * When PathKeep won't open the archive in its data folder (startup's
 * recovery screen), going back to PathKeep 0.5.0 and choosing on the way
 * back, and the one-time notices about the data folder.
 */
export const archiveAccess = defineMessages({
  en: {
    folder: 'Data folder',
    copyFolder: 'Copy the folder’s path',
    details: 'Details',
    copyDetails: 'Copy the details',
    checkAgain: 'Check again',
    checking: 'Checking…',
    restart: 'Restart PathKeep',
    restarting: 'Restarting…',
    restartFailed:
      'PathKeep couldn’t restart itself. Quit it and open it again. {message}',
    versions:
      'This PathKeep reads and writes archive format {build}. This archive needs format {needed}.',
    nothingChanged: 'Nothing in the data folder was changed.',
    otherWays: 'Other ways',
    currentPassword: 'The archive’s current password',
    currentPasswordHint:
      'The password this archive has now. A safety copy’s password goes in its own field.',
    wrongCurrentPassword: 'That isn’t the archive’s current password.',
    title: {
      newer: 'This archive was updated by a newer PathKeep',
      writer: 'This archive needs a newer PathKeep to change it',
      unreadable: 'PathKeep can’t read the archive’s settings file',
      controlMissing: 'The archive’s settings file is missing',
      controlInvalid: 'The archive’s settings file is damaged',
      installRecord:
        'A record of an unfinished change doesn’t fit this archive',
      storeMissing: 'A file of the archive is missing',
      handoff: 'Your Agent conversations were set aside for PathKeep 0.5.0',
      prepared: 'Ready for PathKeep 0.5.0',
      handoffInvalid: 'The folder set aside for PathKeep 0.5.0 doesn’t match',
      legacyWipe: 'A “Delete all data” from PathKeep 0.5 didn’t finish',
      interrupted: 'An earlier change to the archive didn’t finish',
      recoveryMode: 'PathKeep started without opening the archive',
      unknown: 'PathKeep can’t open this archive',
    },
    body: {
      newer:
        'A newer version of PathKeep changed this archive in a way this version can’t read. Nothing in the data folder was changed. Update PathKeep to open it.',
      writer:
        'A newer version of PathKeep changed this archive. This version could read it but can’t safely change it, and it can’t open an archive read-only. Nothing in the data folder was changed. Update PathKeep to open it.',
      unreadable:
        'The data folder is there, but PathKeep isn’t allowed to read its settings file, or the disk didn’t let it. Nothing was changed. Check that your user account can read the folder below, then check again.',
      controlMissing:
        'The data folder still holds the archive, but its settings file (config.json) is gone. PathKeep won’t open the archive without it, so nothing was changed. Rebuild the settings file, or restore from a safety copy.',
      controlInvalid:
        'PathKeep can’t make sense of the archive’s settings file (config.json), so it didn’t open the archive or change anything. Your history’s files are still there. Rebuild the settings file, or restore from a safety copy.',
      installRecord:
        'PathKeep found a record of an import, restore or update that doesn’t fit the files in the data folder. It may have come from another folder, or files changed after that change stopped. Your data is still there and nothing was changed. Restore from a safety copy, or move the record named in the details out of the folder and check again.',
      handoff:
        'Browsing history is as PathKeep 0.5.0 left it, including anything it backed up. Your Agent conversations are in a separate folder next to the data folder. Choose what happens to them before the archive opens.',
      prepared:
        'Quit PathKeep now, then install and open PathKeep 0.5.0. This version won’t open the archive again until you choose what happens to the Agent conversations.',
      handoffInvalid:
        'This archive was prepared for PathKeep 0.5.0, and the folder holding its Agent conversations is incomplete or was changed. Nothing was changed. If you moved or copied that folder, put the original back next to the data folder and check again.',
      legacyWipe:
        'PathKeep 0.5 started deleting everything in this folder and stopped partway. PathKeep can’t tell whether that record belongs to this folder, so it deleted nothing more and won’t open the archive while the record is there. Copy the data folder somewhere safe before you do anything else.',
      interrupted:
        'An import, restore or update stopped partway, and PathKeep couldn’t finish it or put things back on its own. Nothing was deleted. Restart PathKeep to try once more. If this keeps coming back, restore from a safety copy.',
      recoveryMode:
        'Only recovery works right now: restoring from a safety copy, or preparing the archive for PathKeep 0.5.0. Restart PathKeep to open the archive as usual.',
      unknown:
        'Nothing in the data folder was changed. The details below say why.',
    },
    store: {
      ask: 'The file that holds your Ask chats is missing, so this version won’t open the archive. Your browsing history is still there. Start with no Ask chats, or restore from a safety copy that has them.',
      intelligence:
        'The file that holds your site categories, search engine rules and saved page text records is missing, so this version won’t open the archive. Your browsing history is still there. Start that file empty, or restore from a safety copy that has it.',
      evidence:
        'The file that records where each visit came from is missing. PathKeep can’t make it again. Restore from a safety copy that has it.',
      other:
        'A file the archive needs is missing. Restore from a safety copy that has it.',
    },
    addStore: {
      askTitle: 'Start with no Ask chats',
      askBody:
        'PathKeep makes an empty Ask chats file. Chats that were in the missing file are gone unless a safety copy has them.',
      intelligenceTitle: 'Start that file empty',
      intelligenceBody:
        'PathKeep makes an empty file. Site categories and search engine rules you changed go back to the defaults, and saved page text has to be fetched again.',
      confirm: 'Make it empty',
      running: 'Making the file…',
      done: 'Done. Restart PathKeep to open the archive.',
      unavailable:
        'That file can’t be made here any more: it’s back, or the archive changed. Check again.',
      failed: 'Nothing was changed. {message}',
    },
    rebuild: {
      title: 'Rebuild the settings file',
      body: 'PathKeep writes a new settings file for the archive in this folder. Your history, stars, notes and conversations stay. Your settings go back to their defaults (language, schedule, AI services, which kinds of data PathKeep keeps), so set them again afterwards. The app passcode stays.',
      confirm: 'Rebuild the settings file',
      running: 'Checking the archive’s files and writing the settings file…',
      done: 'The settings file is rebuilt. Restart PathKeep to open the archive.',
      unavailable:
        'This settings file can’t be rebuilt here. Restore from a safety copy instead.',
      failed: 'The settings file wasn’t rebuilt. {message}',
    },
    restore: {
      title: 'Restore from a safety copy',
      intro:
        'Browsing history goes back to how it was when the copy was made. The archive keeps its own password and encryption, and saved Agent conversations aren’t touched. The files the restore replaces are kept as a safety copy.',
      loading: 'Looking for safety copies',
      none: 'There are no safety copies in this data folder to restore from.',
      loadFailed: 'Couldn’t list the safety copies. {message}',
      running:
        'Restoring browsing history. This can take a few minutes for a large archive.',
      done: 'Browsing history restored. Restart PathKeep to open the archive.',
      wrongPassword:
        'One of the passwords doesn’t open it. Check the archive’s current password and this copy’s password.',
      wrongCopyPassword: 'That password doesn’t open this copy.',
      needsCurrent:
        'The archive is encrypted, so PathKeep needs its current password to keep its encryption.',
      failed:
        'The restore didn’t finish. Your archive wasn’t changed. {message}',
      keyLost: 'The archive’s current password doesn’t open it at all?',
      copyEncryption: {
        title: 'Restore with this copy’s encryption instead',
        body: 'Use this only if the archive’s own key can’t be opened any more. Without that key, the archive’s saved Agent conversations can’t be read either. PathKeep keeps the current files aside as a safety copy, restores this copy’s browsing history with this copy’s password, and starts Agent conversations empty.',
        agree: 'Start Agent conversations empty',
        confirm: 'Restore with this copy’s encryption',
        unavailable:
          'This copy can’t be used that way. It has to be a safety copy this version made, from an archive that isn’t open.',
      },
    },
    handoff: {
      folder: 'Set-aside folder',
      size: 'Agent conversations in it: {size}',
      copies_one:
        'It also holds {count} safety copy or move file this version made. Safety copies there stay listed for restoring.',
      copies_other:
        'It also holds {count} safety copies and move files this version made. Safety copies there stay listed for restoring.',
      choose: 'What should happen to your Agent conversations?',
      rejoinTitle: 'Take them back',
      rejoinBody:
        'The conversations go back into the archive under its current password. Copies in the folder stay where they are.',
      freshTitle: 'Start without them',
      freshBody:
        'The archive opens with no Agent conversations. The folder stays where it is, untouched, and PathKeep doesn’t open it again. Delete it yourself once you no longer need it.',
      later:
        'Not now? Close PathKeep. Nothing changes until you choose, and PathKeep 0.5.0 can keep using the archive.',
      changedMind:
        'Changed your mind? You can take the conversations back now instead.',
      password: 'Archive password',
      passwordHint:
        'The password the archive has now. If you changed it in PathKeep 0.5.0, use the new one.',
      otherPassword: 'The archive’s password changed in PathKeep 0.5.0',
      packagePassword: 'Password when they were set aside',
      packagePasswordHint:
        'The archive’s password when you prepared it for PathKeep 0.5.0.',
      confirmRejoin: 'Take them back',
      confirmFresh: 'Start without them',
      runningRejoin:
        'Checking the set-aside conversations and bringing them back…',
      runningFresh: 'Setting up empty Agent conversations…',
      doneRejoin:
        'Your Agent conversations are back. Restart PathKeep to open the archive.',
      doneFresh:
        'The archive is ready with no Agent conversations. The set-aside folder is still where it was. Restart PathKeep to open the archive.',
      wrongPassword:
        'That password doesn’t open the archive or the set-aside conversations.',
      incomplete:
        'Preparing for PathKeep 0.5.0 stopped partway. Finish preparing first; then you can choose.',
      finish: 'Finish preparing',
      occupied:
        'The archive has Agent conversations again, so PathKeep didn’t bring the set-aside ones back over them. Both are kept.',
      failed: 'Nothing was changed. {message}',
      loadFailed: 'Couldn’t read the set-aside folder. {message}',
    },
    rollback: {
      rowTitle: 'Go back to PathKeep 0.5.0',
      rowDescription:
        'Prepare this archive so PathKeep 0.5.0 can open it again',
      action: 'Prepare…',
      title: 'Prepare for PathKeep 0.5.0',
      intro:
        'PathKeep 0.5.0 can’t open an archive this version has changed. Preparing changes it back to a form 0.5.0 opens, without copying the whole archive.',
      checking: 'Checking what preparing would move',
      sees: 'PathKeep 0.5.0 sees all your browsing history, including what you back up with this version, and can keep backing up.',
      notSees:
        'It doesn’t see your Agent conversations. They move into this folder next to the data folder:',
      conversations: 'Agent conversations: {size}',
      copies:
        'These safety copies and move files were made by this version, which 0.5.0 can’t use. They move into that folder too:',
      wipe: 'PathKeep 0.5.0’s “Delete all data” doesn’t reach that folder.',
      password:
        'The conversations keep the archive’s password as it is now, even if you change the password in PathKeep 0.5.0.',
      back: 'When you open this version again, it asks whether to take the conversations back or start without them.',
      after:
        'Once it’s prepared, this version stops using the archive: quit it, then install and open PathKeep 0.5.0.',
      confirm: 'Prepare for PathKeep 0.5.0',
      running:
        'Setting the Agent conversations aside and writing the settings file in PathKeep 0.5.0’s form…',
      loadFailed: 'Couldn’t check what preparing would move. {message}',
      unsupported:
        'This archive can’t be prepared for PathKeep 0.5.0. {message}',
      crossDevice:
        'The folder next to the data folder is on another disk, so the conversations can’t be moved there. Nothing was changed.',
      failed: 'Nothing was changed. {message}',
    },
    imports: {
      rowTitle: 'Conversations added by imports',
      rowDescription:
        'Move what a wrong import added to the trash. The conversations already here stay.',
      action: 'Review…',
      title: 'Conversations added by imports',
      intro:
        'Each import below added Agent conversations this computer didn’t have. Undo moves what one import added to the trash. Conversations that were already here, and ones backed up here since the import, stay.',
      loadFailed: 'Couldn’t list the imports. {message}',
      imported: 'Imported {date}',
      added_one: 'Added {count} conversation',
      added_other: 'Added {count} conversations',
      eligible_one: '{count} can still be moved to the trash',
      eligible_other: '{count} can still be moved to the trash',
      takenBack: 'Already taken back',
      undo: 'Undo…',
      confirmTitle_one: 'Move {count} conversation to the trash?',
      confirmTitle_other: 'Move {count} conversations to the trash?',
      confirmBody:
        'PathKeep first keeps a copy of them in a folder of their own, then moves them to the trash, where you can still read them or put them back. Conversations that were here before, and ones backed up here since the import, aren’t touched. Messages the import added to a conversation that was already here stay.',
      confirm: 'Move to the trash',
      running: 'Keeping a copy, then moving the conversations to the trash…',
      done_one: 'Moved {count} conversation to the trash.',
      done_other: 'Moved {count} conversations to the trash.',
      doneNothing:
        'Nothing was left to move: those conversations are already in the trash or were backed up here since.',
      copyAt: 'A copy of them is kept in {path}.',
      notFound: 'That import isn’t in the archive any more.',
      failed: 'Nothing was moved. {message}',
    },
    fat: 'This folder is on a disk formatted as FAT32 or exFAT, like many USB drives and memory cards. If the power goes out or the disk is unplugged while PathKeep writes, a file can be lost, and PathKeep can’t prevent that on these disks. A disk inside the computer is safer. If you keep the archive here, keep the computer plugged in during backups.',
    notices: {
      wipeRolledBack: 'The last “Delete all data” didn’t finish',
      wipeRolledBackBody:
        'PathKeep put everything back, so your archive is as it was. To delete everything, run Delete all data again.',
      wipeRecordDiscarded: 'An old “Delete all data” record was set aside',
      wipeRecordDiscardedBody:
        'It didn’t match this archive, so PathKeep left the archive alone. Anything that deletion had already moved out is listed in Free up space.',
      wipeFilesDisplaced: 'Files were moved aside',
      wipeFilesDisplacedBody:
        'While undoing an unfinished “Delete all data”, PathKeep found files where the archive’s files go back, and moved them into {path}. Free up space lists that folder.',
      checkDisk: 'Check this disk',
      checkDiskBody:
        'After an interrupted change, PathKeep found one file under two names, which can happen on FAT32 or exFAT disks when the power goes out. It kept the second name in {path}. Check the disk with your system’s disk tool before you delete that folder in Free up space.',
      other: 'PathKeep has a notice about the data folder',
    },
  },
  'zh-CN': {
    folder: '数据文件夹',
    copyFolder: '复制文件夹路径',
    details: '详细信息',
    copyDetails: '复制详细信息',
    checkAgain: '重新检查',
    checking: '正在检查…',
    restart: '重新启动 PathKeep',
    restarting: '正在重新启动…',
    restartFailed: 'PathKeep 没能自己重新启动。请退出后再打开。{message}',
    versions:
      '这个 PathKeep 读写的存档格式是 {build}，这份存档需要格式 {needed}。',
    nothingChanged: '数据文件夹里的东西都没有改动。',
    otherWays: '其他办法',
    currentPassword: '存档现在的密码',
    currentPasswordHint:
      '这份存档现在的密码。安全副本的密码填在它自己的栏位里。',
    wrongCurrentPassword: '这不是存档现在的密码。',
    title: {
      newer: '这份存档被较新版的 PathKeep 更新过',
      writer: '要修改这份存档，需要较新版的 PathKeep',
      unreadable: 'PathKeep 读不了存档的设置文件',
      controlMissing: '存档的设置文件不见了',
      controlInvalid: '存档的设置文件坏了',
      installRecord: '一份未完成操作的记录和这份存档对不上',
      storeMissing: '存档少了一个文件',
      handoff: '你的 Agent 对话已为 PathKeep 0.5.0 移到一旁',
      prepared: '已经可以用 PathKeep 0.5.0 打开',
      handoffInvalid: '为 PathKeep 0.5.0 移到一旁的文件夹对不上',
      legacyWipe: 'PathKeep 0.5 的“删除所有数据”没有做完',
      interrupted: '之前对存档的一次改动没有做完',
      recoveryMode: 'PathKeep 启动了，但没有打开存档',
      unknown: 'PathKeep 打不开这份存档',
    },
    body: {
      newer:
        '较新版的 PathKeep 改动过这份存档，这个版本读不懂改动后的格式。数据文件夹里的东西都没有改动。更新 PathKeep 后就能打开。',
      writer:
        '较新版的 PathKeep 改动过这份存档。这个版本读得懂，但无法安全地修改它，也不能以只读方式打开存档。数据文件夹里的东西都没有改动。更新 PathKeep 后就能打开。',
      unreadable:
        '数据文件夹还在，但 PathKeep 没有权限读它的设置文件，或者磁盘不让读。什么都没有改动。请确认你的用户账户能读下面这个文件夹，然后重新检查。',
      controlMissing:
        '数据文件夹里存档还在，但它的设置文件（config.json）不见了。没有它 PathKeep 不会打开存档，所以什么都没有改动。可以重新建立设置文件，或从安全副本恢复。',
      controlInvalid:
        'PathKeep 看不懂存档的设置文件（config.json），所以没有打开存档，也没有改动任何东西。你的历史文件都还在。可以重新建立设置文件，或从安全副本恢复。',
      installRecord:
        'PathKeep 发现一份导入、恢复或更新的记录，和数据文件夹里的文件对不上。它可能来自别的文件夹，或者那次操作停下后文件又被改过。你的数据都还在，什么都没有改动。可以从安全副本恢复，或把详细信息里写的那个记录文件移出文件夹，再重新检查。',
      handoff:
        '浏览记录维持 PathKeep 0.5.0 留下的样子，包括它备份进来的。你的 Agent 对话在数据文件夹旁边的另一个文件夹里。打开存档之前，请先决定怎么处理它们。',
      prepared:
        '现在请退出 PathKeep，然后安装并打开 PathKeep 0.5.0。在你决定怎么处理 Agent 对话之前，这个版本不会再打开这份存档。',
      handoffInvalid:
        '这份存档已为 PathKeep 0.5.0 准备好，但放着 Agent 对话的那个文件夹不完整，或被改过。什么都没有改动。如果你移动或复制过那个文件夹，请把原本那个放回数据文件夹旁边，再重新检查。',
      legacyWipe:
        'PathKeep 0.5 开始删除这个文件夹里的所有东西，做到一半停了。PathKeep 分不出那份记录是不是属于这个文件夹，所以没有再删除任何东西，记录还在的时候也不会打开存档。在做任何事之前，请先把数据文件夹复制到安全的地方。',
      interrupted:
        '一次导入、恢复或更新做到一半停了，PathKeep 没办法自己做完或复原。什么都没有删除。重新启动 PathKeep 会再试一次。如果一直这样，请从安全副本恢复。',
      recoveryMode:
        '现在只能做恢复：从安全副本恢复，或为 PathKeep 0.5.0 准备存档。重新启动 PathKeep 就会照常打开存档。',
      unknown: '数据文件夹里的东西都没有改动。原因写在下面的详细信息里。',
    },
    store: {
      ask: '存着 Ask 对话的文件不见了，所以这个版本不会打开存档。你的浏览记录都还在。可以从没有 Ask 对话开始，或从有它们的安全副本恢复。',
      intelligence:
        '存着网站分类、搜索引擎规则和已存网页文字记录的文件不见了，所以这个版本不会打开存档。你的浏览记录都还在。可以让这个文件从空的开始，或从有它的安全副本恢复。',
      evidence:
        '记录每条浏览从哪里来的文件不见了，PathKeep 没办法重新做出来。请从有它的安全副本恢复。',
      other: '存档需要的一个文件不见了。请从有它的安全副本恢复。',
    },
    addStore: {
      askTitle: '从没有 Ask 对话开始',
      askBody:
        'PathKeep 会建一个空的 Ask 对话文件。不见的那个文件里的对话就没有了，除非安全副本里有。',
      intelligenceTitle: '让这个文件从空的开始',
      intelligenceBody:
        'PathKeep 会建一个空的文件。你改过的网站分类和搜索引擎规则会回到默认值，已存的网页文字要重新抓取。',
      confirm: '建立空的文件',
      running: '正在建立文件…',
      done: '完成。重新启动 PathKeep 就能打开存档。',
      unavailable:
        '现在已经不能在这里建立这个文件：它回来了，或者存档变了。请重新检查。',
      failed: '什么都没有改动。{message}',
    },
    rebuild: {
      title: '重新建立设置文件',
      body: 'PathKeep 会为这个文件夹里的存档写一份新的设置文件。你的历史、星号、笔记和对话都会留着。设置会回到默认值（语言、排程、AI 服务、要保存哪几种数据），之后请再设置一次。应用密码会保留。',
      confirm: '重新建立设置文件',
      running: '正在检查存档的文件并写入设置文件…',
      done: '设置文件已重新建立。重新启动 PathKeep 就能打开存档。',
      unavailable: '这份设置文件不能在这里重新建立。请改从安全副本恢复。',
      failed: '设置文件没有重新建立。{message}',
    },
    restore: {
      title: '从安全副本恢复',
      intro:
        '浏览记录会回到副本保存时的状态。存档保留自己的密码和加密方式，已存的 Agent 对话不受影响。被恢复替换掉的文件会保留为一份安全副本。',
      loading: '正在查找安全副本',
      none: '这个数据文件夹里没有可以用来恢复的安全副本。',
      loadFailed: '无法列出安全副本。{message}',
      running: '正在恢复浏览记录。存档很大时可能需要几分钟。',
      done: '浏览记录已恢复。重新启动 PathKeep 就能打开存档。',
      wrongPassword: '有一个密码打不开。请检查存档现在的密码和这份副本的密码。',
      wrongCopyPassword: '这个密码打不开这份副本。',
      needsCurrent: '存档是加密的，PathKeep 需要它现在的密码才能保留它的加密。',
      failed: '恢复没有完成，你的存档没有改动。{message}',
      keyLost: '存档现在的密码完全打不开它？',
      copyEncryption: {
        title: '改用这份副本的加密方式恢复',
        body: '只在存档自己的密钥再也打不开时使用。没有那把密钥，存档里已存的 Agent 对话也读不了。PathKeep 会把现在的文件留成一份安全副本，用这份副本的密码恢复它的浏览记录，Agent 对话从空的开始。',
        agree: 'Agent 对话从空的开始',
        confirm: '用这份副本的加密方式恢复',
        unavailable:
          '这份副本不能这样用。它必须是这个版本做的安全副本，而且存档没有打开。',
      },
    },
    handoff: {
      folder: '移到一旁的文件夹',
      size: '里面的 Agent 对话：{size}',
      copies_one:
        '里面还有这个版本做的 {count} 份安全副本或搬家文件。那里的安全副本仍然列在恢复清单上。',
      copies_other:
        '里面还有这个版本做的 {count} 份安全副本和搬家文件。那里的安全副本仍然列在恢复清单上。',
      choose: '你的 Agent 对话要怎么处理？',
      rejoinTitle: '把它们拿回来',
      rejoinBody:
        '对话会回到存档里，使用存档现在的密码。文件夹里的副本留在原地。',
      freshTitle: '不要它们，从空的开始',
      freshBody:
        '存档打开时没有 Agent 对话。那个文件夹原样留着，PathKeep 不会再打开它。不需要时请自己删除。',
      later:
        '现在还不决定？关掉 PathKeep 就好。在你决定之前什么都不会变，PathKeep 0.5.0 也能继续使用这份存档。',
      changedMind: '改变主意了？现在也可以把对话拿回来。',
      password: '存档密码',
      passwordHint:
        '存档现在的密码。如果你在 PathKeep 0.5.0 里改过密码，请用新的。',
      otherPassword: '存档的密码在 PathKeep 0.5.0 里改过',
      packagePassword: '移到一旁时的密码',
      packagePasswordHint: '你为 PathKeep 0.5.0 准备存档时，存档用的密码。',
      confirmRejoin: '把它们拿回来',
      confirmFresh: '从空的开始',
      runningRejoin: '正在检查移到一旁的对话并把它们拿回来…',
      runningFresh: '正在建立空的 Agent 对话…',
      doneRejoin: '你的 Agent 对话回来了。重新启动 PathKeep 就能打开存档。',
      doneFresh:
        '存档已准备好，没有 Agent 对话。移到一旁的文件夹还在原处。重新启动 PathKeep 就能打开存档。',
      wrongPassword: '这个密码打不开存档或移到一旁的对话。',
      incomplete:
        '为 PathKeep 0.5.0 做的准备停在一半。请先做完准备，再来选择。',
      finish: '做完准备',
      occupied:
        '存档里又有了 Agent 对话，所以 PathKeep 没有把移到一旁的对话盖上去。两份都保留着。',
      failed: '什么都没有改动。{message}',
      loadFailed: '读不了移到一旁的文件夹。{message}',
    },
    rollback: {
      rowTitle: '退回 PathKeep 0.5.0',
      rowDescription: '把这份存档准备好，让 PathKeep 0.5.0 能再打开它',
      action: '准备…',
      title: '为 PathKeep 0.5.0 准备存档',
      intro:
        'PathKeep 0.5.0 打不开这个版本改动过的存档。准备会把它改回 0.5.0 打得开的样子，不需要复制整份存档。',
      checking: '正在检查准备时会移动什么',
      sees: 'PathKeep 0.5.0 看得到你所有的浏览记录，包括用这个版本备份的，也能继续备份。',
      notSees:
        '它看不到你的 Agent 对话。对话会移到数据文件夹旁边的这个文件夹里：',
      conversations: 'Agent 对话：{size}',
      copies:
        '下面这些安全副本和搬家文件是这个版本做的，0.5.0 用不了，也会一起移到那个文件夹：',
      wipe: 'PathKeep 0.5.0 的“删除所有数据”删不到那个文件夹。',
      password:
        '对话会保留存档现在的密码，就算之后在 PathKeep 0.5.0 里改了密码也一样。',
      back: '之后再打开这个版本时，它会问你要把对话拿回来，还是从空的开始。',
      after:
        '准备好之后，这个版本就不再使用这份存档：请退出它，然后安装并打开 PathKeep 0.5.0。',
      confirm: '为 PathKeep 0.5.0 准备',
      running:
        '正在把 Agent 对话移到一旁，并用 PathKeep 0.5.0 的格式写入设置文件…',
      loadFailed: '无法检查准备时会移动什么。{message}',
      unsupported: '这份存档不能为 PathKeep 0.5.0 准备。{message}',
      crossDevice:
        '数据文件夹旁边的位置在另一个磁盘上，对话没办法移过去。什么都没有改动。',
      failed: '什么都没有改动。{message}',
    },
    imports: {
      rowTitle: '导入时加进来的对话',
      rowDescription:
        '把导入错文件时加进来的对话移到垃圾桶。原本在这里的对话会留着。',
      action: '查看…',
      title: '导入时加进来的对话',
      intro:
        '下面每一次导入都加进了这台电脑原本没有的 Agent 对话。撤销会把那一次加进来的对话移到垃圾桶。原本就在这里的对话，以及导入之后在这里备份过的对话，都会留着。',
      loadFailed: '无法列出导入记录。{message}',
      imported: '{date} 导入',
      added_one: '加进 {count} 段对话',
      added_other: '加进 {count} 段对话',
      eligible_one: '还有 {count} 段可以移到垃圾桶',
      eligible_other: '还有 {count} 段可以移到垃圾桶',
      takenBack: '已经撤回',
      undo: '撤销…',
      confirmTitle_one: '把 {count} 段对话移到垃圾桶？',
      confirmTitle_other: '把 {count} 段对话移到垃圾桶？',
      confirmBody:
        'PathKeep 会先把它们复制一份放进单独的文件夹，再移到垃圾桶；在垃圾桶里仍然能看，也能放回来。之前就在这里的对话，以及导入之后在这里备份过的对话，都不会动。导入时补进原有对话的消息会留着。',
      confirm: '移到垃圾桶',
      running: '正在复制一份，然后把对话移到垃圾桶…',
      done_one: '已把 {count} 段对话移到垃圾桶。',
      done_other: '已把 {count} 段对话移到垃圾桶。',
      doneNothing:
        '没有可以移的对话了：它们已经在垃圾桶里，或之后在这里备份过。',
      copyAt: '它们的副本放在 {path}。',
      notFound: '存档里已经没有那次导入了。',
      failed: '什么都没有移动。{message}',
    },
    fat: '这个文件夹所在的磁盘是 FAT32 或 exFAT 格式，很多 U 盘和存储卡都是。PathKeep 写入时如果停电或磁盘被拔掉，可能会丢失文件，而在这种磁盘上 PathKeep 没办法防止。放在电脑内置的磁盘上比较安全。如果要放在这里，备份时请接上电源。',
    notices: {
      wipeRolledBack: '上次的“删除所有数据”没有做完',
      wipeRolledBackBody:
        'PathKeep 已把所有东西放回去，存档维持原样。如果要删除，请再做一次“删除所有数据”。',
      wipeRecordDiscarded: '一份旧的“删除所有数据”记录已移到一旁',
      wipeRecordDiscardedBody:
        '它和这份存档对不上，所以 PathKeep 没有动存档。那次删除已经移出去的东西列在“释放空间”里。',
      wipeFilesDisplaced: '有些文件被移到一旁',
      wipeFilesDisplacedBody:
        '复原没做完的“删除所有数据”时，PathKeep 发现存档文件要放回去的位置已经有文件，就把它们移到 {path}。“释放空间”里列着这个文件夹。',
      checkDisk: '请检查这个磁盘',
      checkDiskBody:
        '一次改动被中断后，PathKeep 发现同一个文件挂在两个名字下；FAT32 或 exFAT 磁盘停电时可能会这样。第二个名字放在 {path}。在“释放空间”删除那个文件夹之前，请先用系统的磁盘工具检查磁盘。',
      other: 'PathKeep 有一则关于数据文件夹的通知',
    },
  },
  'zh-TW': {
    folder: '資料夾',
    copyFolder: '複製資料夾路徑',
    details: '詳細資訊',
    copyDetails: '複製詳細資訊',
    checkAgain: '重新檢查',
    checking: '正在檢查…',
    restart: '重新啟動 PathKeep',
    restarting: '正在重新啟動…',
    restartFailed: 'PathKeep 沒辦法自己重新啟動。請結束後再打開。{message}',
    versions:
      '這個 PathKeep 讀寫的存檔格式是 {build}，這份存檔需要格式 {needed}。',
    nothingChanged: '資料夾裡的東西都沒有更動。',
    otherWays: '其他辦法',
    currentPassword: '存檔現在的密碼',
    currentPasswordHint: '這份存檔現在的密碼。安全副本的密碼填在它自己的欄位。',
    wrongCurrentPassword: '這不是存檔現在的密碼。',
    title: {
      newer: '這份存檔被較新版的 PathKeep 更新過',
      writer: '要修改這份存檔，需要較新版的 PathKeep',
      unreadable: 'PathKeep 讀不了存檔的設定檔',
      controlMissing: '存檔的設定檔不見了',
      controlInvalid: '存檔的設定檔壞了',
      installRecord: '一份未完成操作的紀錄和這份存檔對不上',
      storeMissing: '存檔少了一個檔案',
      handoff: '你的 Agent 對話已為 PathKeep 0.5.0 移到一旁',
      prepared: '已經可以用 PathKeep 0.5.0 打開',
      handoffInvalid: '為 PathKeep 0.5.0 移到一旁的資料夾對不上',
      legacyWipe: 'PathKeep 0.5 的「刪除所有資料」沒有做完',
      interrupted: '之前對存檔的一次改動沒有做完',
      recoveryMode: 'PathKeep 啟動了，但沒有打開存檔',
      unknown: 'PathKeep 打不開這份存檔',
    },
    body: {
      newer:
        '較新版的 PathKeep 改動過這份存檔，這個版本讀不懂改動後的格式。資料夾裡的東西都沒有更動。更新 PathKeep 後就能打開。',
      writer:
        '較新版的 PathKeep 改動過這份存檔。這個版本讀得懂，但無法安全地修改它，也不能以唯讀方式打開存檔。資料夾裡的東西都沒有更動。更新 PathKeep 後就能打開。',
      unreadable:
        '資料夾還在，但 PathKeep 沒有權限讀它的設定檔，或是磁碟不讓讀。什麼都沒有更動。請確認你的使用者帳號能讀下面這個資料夾，然後重新檢查。',
      controlMissing:
        '資料夾裡存檔還在，但它的設定檔（config.json）不見了。沒有它 PathKeep 不會打開存檔，所以什麼都沒有更動。可以重新建立設定檔，或從安全副本還原。',
      controlInvalid:
        'PathKeep 看不懂存檔的設定檔（config.json），所以沒有打開存檔，也沒有更動任何東西。你的歷史檔案都還在。可以重新建立設定檔，或從安全副本還原。',
      installRecord:
        'PathKeep 發現一份匯入、還原或更新的紀錄，和資料夾裡的檔案對不上。它可能來自別的資料夾，或是那次操作停下後檔案又被改過。你的資料都還在，什麼都沒有更動。可以從安全副本還原，或把詳細資訊裡寫的那個紀錄檔移出資料夾，再重新檢查。',
      handoff:
        '瀏覽紀錄維持 PathKeep 0.5.0 留下的樣子，包括它備份進來的。你的 Agent 對話在資料夾旁邊的另一個資料夾裡。打開存檔之前，請先決定怎麼處理它們。',
      prepared:
        '現在請結束 PathKeep，然後安裝並打開 PathKeep 0.5.0。在你決定怎麼處理 Agent 對話之前，這個版本不會再打開這份存檔。',
      handoffInvalid:
        '這份存檔已為 PathKeep 0.5.0 準備好，但放著 Agent 對話的那個資料夾不完整，或被改過。什麼都沒有更動。如果你移動或複製過那個資料夾，請把原本那個放回資料夾旁邊，再重新檢查。',
      legacyWipe:
        'PathKeep 0.5 開始刪除這個資料夾裡的所有東西，做到一半停了。PathKeep 分不出那份紀錄是不是屬於這個資料夾，所以沒有再刪除任何東西，紀錄還在的時候也不會打開存檔。在做任何事之前，請先把資料夾複製到安全的地方。',
      interrupted:
        '一次匯入、還原或更新做到一半停了，PathKeep 沒辦法自己做完或復原。什麼都沒有刪除。重新啟動 PathKeep 會再試一次。如果一直這樣，請從安全副本還原。',
      recoveryMode:
        '現在只能做復原：從安全副本還原，或為 PathKeep 0.5.0 準備存檔。重新啟動 PathKeep 就會照常打開存檔。',
      unknown: '資料夾裡的東西都沒有更動。原因寫在下面的詳細資訊裡。',
    },
    store: {
      ask: '存著 Ask 對話的檔案不見了，所以這個版本不會打開存檔。你的瀏覽紀錄都還在。可以從沒有 Ask 對話開始，或從有它們的安全副本還原。',
      intelligence:
        '存著網站分類、搜尋引擎規則和已存網頁文字紀錄的檔案不見了，所以這個版本不會打開存檔。你的瀏覽紀錄都還在。可以讓這個檔案從空的開始，或從有它的安全副本還原。',
      evidence:
        '記錄每筆瀏覽從哪裡來的檔案不見了，PathKeep 沒辦法重新做出來。請從有它的安全副本還原。',
      other: '存檔需要的一個檔案不見了。請從有它的安全副本還原。',
    },
    addStore: {
      askTitle: '從沒有 Ask 對話開始',
      askBody:
        'PathKeep 會建一個空的 Ask 對話檔案。不見的那個檔案裡的對話就沒有了，除非安全副本裡有。',
      intelligenceTitle: '讓這個檔案從空的開始',
      intelligenceBody:
        'PathKeep 會建一個空的檔案。你改過的網站分類和搜尋引擎規則會回到預設值，已存的網頁文字要重新抓取。',
      confirm: '建立空的檔案',
      running: '正在建立檔案…',
      done: '完成。重新啟動 PathKeep 就能打開存檔。',
      unavailable:
        '現在已經不能在這裡建立這個檔案：它回來了，或是存檔變了。請重新檢查。',
      failed: '什麼都沒有更動。{message}',
    },
    rebuild: {
      title: '重新建立設定檔',
      body: 'PathKeep 會為這個資料夾裡的存檔寫一份新的設定檔。你的歷史、星號、筆記和對話都會留著。設定會回到預設值（語言、排程、AI 服務、要保存哪幾種資料），之後請再設定一次。App 密碼會保留。',
      confirm: '重新建立設定檔',
      running: '正在檢查存檔的檔案並寫入設定檔…',
      done: '設定檔已重新建立。重新啟動 PathKeep 就能打開存檔。',
      unavailable: '這份設定檔不能在這裡重新建立。請改從安全副本還原。',
      failed: '設定檔沒有重新建立。{message}',
    },
    restore: {
      title: '從安全副本還原',
      intro:
        '瀏覽紀錄會回到副本建立時的狀態。存檔保留自己的密碼和加密方式，已存的 Agent 對話不受影響。被還原取代的檔案會保留為一份安全副本。',
      loading: '正在尋找安全副本',
      none: '這個資料夾裡沒有可以用來還原的安全副本。',
      loadFailed: '無法列出安全副本。{message}',
      running: '正在還原瀏覽紀錄。存檔很大時可能需要幾分鐘。',
      done: '瀏覽紀錄已還原。重新啟動 PathKeep 就能打開存檔。',
      wrongPassword: '有一組密碼打不開。請檢查存檔現在的密碼和這份副本的密碼。',
      wrongCopyPassword: '這組密碼打不開這份副本。',
      needsCurrent: '存檔是加密的，PathKeep 需要它現在的密碼才能保留它的加密。',
      failed: '還原沒有完成，你的存檔沒有更動。{message}',
      keyLost: '存檔現在的密碼完全打不開它？',
      copyEncryption: {
        title: '改用這份副本的加密方式還原',
        body: '只在存檔自己的金鑰再也打不開時使用。沒有那把金鑰，存檔裡已存的 Agent 對話也讀不了。PathKeep 會把現在的檔案留成一份安全副本，用這份副本的密碼還原它的瀏覽紀錄，Agent 對話從空的開始。',
        agree: 'Agent 對話從空的開始',
        confirm: '用這份副本的加密方式還原',
        unavailable:
          '這份副本不能這樣用。它必須是這個版本做的安全副本，而且存檔沒有打開。',
      },
    },
    handoff: {
      folder: '移到一旁的資料夾',
      size: '裡面的 Agent 對話：{size}',
      copies_one:
        '裡面還有這個版本做的 {count} 份安全副本或搬家檔。那裡的安全副本仍然列在還原清單上。',
      copies_other:
        '裡面還有這個版本做的 {count} 份安全副本和搬家檔。那裡的安全副本仍然列在還原清單上。',
      choose: '你的 Agent 對話要怎麼處理？',
      rejoinTitle: '把它們拿回來',
      rejoinBody:
        '對話會回到存檔裡，使用存檔現在的密碼。資料夾裡的副本留在原地。',
      freshTitle: '不要它們，從空的開始',
      freshBody:
        '存檔打開時沒有 Agent 對話。那個資料夾原樣留著，PathKeep 不會再打開它。不需要時請自己刪除。',
      later:
        '現在還不決定？關掉 PathKeep 就好。在你決定之前什麼都不會變，PathKeep 0.5.0 也能繼續使用這份存檔。',
      changedMind: '改變主意了？現在也可以把對話拿回來。',
      password: '存檔密碼',
      passwordHint:
        '存檔現在的密碼。如果你在 PathKeep 0.5.0 裡改過密碼，請用新的。',
      otherPassword: '存檔的密碼在 PathKeep 0.5.0 裡改過',
      packagePassword: '移到一旁時的密碼',
      packagePasswordHint: '你為 PathKeep 0.5.0 準備存檔時，存檔用的密碼。',
      confirmRejoin: '把它們拿回來',
      confirmFresh: '從空的開始',
      runningRejoin: '正在檢查移到一旁的對話並把它們拿回來…',
      runningFresh: '正在建立空的 Agent 對話…',
      doneRejoin: '你的 Agent 對話回來了。重新啟動 PathKeep 就能打開存檔。',
      doneFresh:
        '存檔已準備好，沒有 Agent 對話。移到一旁的資料夾還在原處。重新啟動 PathKeep 就能打開存檔。',
      wrongPassword: '這組密碼打不開存檔或移到一旁的對話。',
      incomplete:
        '為 PathKeep 0.5.0 做的準備停在一半。請先做完準備，再來選擇。',
      finish: '做完準備',
      occupied:
        '存檔裡又有了 Agent 對話，所以 PathKeep 沒有把移到一旁的對話蓋上去。兩份都保留著。',
      failed: '什麼都沒有更動。{message}',
      loadFailed: '讀不了移到一旁的資料夾。{message}',
    },
    rollback: {
      rowTitle: '退回 PathKeep 0.5.0',
      rowDescription: '把這份存檔準備好，讓 PathKeep 0.5.0 能再打開它',
      action: '準備…',
      title: '為 PathKeep 0.5.0 準備存檔',
      intro:
        'PathKeep 0.5.0 打不開這個版本改動過的存檔。準備會把它改回 0.5.0 打得開的樣子，不需要複製整份存檔。',
      checking: '正在檢查準備時會移動什麼',
      sees: 'PathKeep 0.5.0 看得到你所有的瀏覽紀錄，包括用這個版本備份的，也能繼續備份。',
      notSees: '它看不到你的 Agent 對話。對話會移到資料夾旁邊的這個資料夾裡：',
      conversations: 'Agent 對話：{size}',
      copies:
        '下面這些安全副本和搬家檔是這個版本做的，0.5.0 用不了，也會一起移到那個資料夾：',
      wipe: 'PathKeep 0.5.0 的「刪除所有資料」刪不到那個資料夾。',
      password:
        '對話會保留存檔現在的密碼，就算之後在 PathKeep 0.5.0 裡改了密碼也一樣。',
      back: '之後再打開這個版本時，它會問你要把對話拿回來，還是從空的開始。',
      after:
        '準備好之後，這個版本就不再使用這份存檔：請結束它，然後安裝並打開 PathKeep 0.5.0。',
      confirm: '為 PathKeep 0.5.0 準備',
      running:
        '正在把 Agent 對話移到一旁，並用 PathKeep 0.5.0 的格式寫入設定檔…',
      loadFailed: '無法檢查準備時會移動什麼。{message}',
      unsupported: '這份存檔不能為 PathKeep 0.5.0 準備。{message}',
      crossDevice:
        '資料夾旁邊的位置在另一顆磁碟上，對話沒辦法移過去。什麼都沒有更動。',
      failed: '什麼都沒有更動。{message}',
    },
    imports: {
      rowTitle: '匯入時加進來的對話',
      rowDescription:
        '把匯入錯檔案時加進來的對話移到垃圾桶。原本在這裡的對話會留著。',
      action: '檢視…',
      title: '匯入時加進來的對話',
      intro:
        '下面每一次匯入都加進了這台電腦原本沒有的 Agent 對話。撤銷會把那一次加進來的對話移到垃圾桶。原本就在這裡的對話，以及匯入之後在這裡備份過的對話，都會留著。',
      loadFailed: '無法列出匯入紀錄。{message}',
      imported: '{date} 匯入',
      added_one: '加進 {count} 段對話',
      added_other: '加進 {count} 段對話',
      eligible_one: '還有 {count} 段可以移到垃圾桶',
      eligible_other: '還有 {count} 段可以移到垃圾桶',
      takenBack: '已經撤回',
      undo: '撤銷…',
      confirmTitle_one: '把 {count} 段對話移到垃圾桶？',
      confirmTitle_other: '把 {count} 段對話移到垃圾桶？',
      confirmBody:
        'PathKeep 會先把它們複製一份放進單獨的資料夾，再移到垃圾桶；在垃圾桶裡仍然能看，也能放回來。之前就在這裡的對話，以及匯入之後在這裡備份過的對話，都不會動。匯入時補進原有對話的訊息會留著。',
      confirm: '移到垃圾桶',
      running: '正在複製一份，然後把對話移到垃圾桶…',
      done_one: '已把 {count} 段對話移到垃圾桶。',
      done_other: '已把 {count} 段對話移到垃圾桶。',
      doneNothing:
        '沒有可以移的對話了：它們已經在垃圾桶裡，或之後在這裡備份過。',
      copyAt: '它們的副本放在 {path}。',
      notFound: '存檔裡已經沒有那次匯入了。',
      failed: '什麼都沒有移動。{message}',
    },
    fat: '這個資料夾所在的磁碟是 FAT32 或 exFAT 格式，很多隨身碟和記憶卡都是。PathKeep 寫入時如果停電或磁碟被拔掉，可能會遺失檔案，而在這種磁碟上 PathKeep 沒辦法防止。放在電腦內建的磁碟上比較安全。如果要放在這裡，備份時請接上電源。',
    notices: {
      wipeRolledBack: '上次的「刪除所有資料」沒有做完',
      wipeRolledBackBody:
        'PathKeep 已把所有東西放回去，存檔維持原樣。如果要刪除，請再做一次「刪除所有資料」。',
      wipeRecordDiscarded: '一份舊的「刪除所有資料」紀錄已移到一旁',
      wipeRecordDiscardedBody:
        '它和這份存檔對不上，所以 PathKeep 沒有動存檔。那次刪除已經移出去的東西列在「釋放空間」裡。',
      wipeFilesDisplaced: '有些檔案被移到一旁',
      wipeFilesDisplacedBody:
        '復原沒做完的「刪除所有資料」時，PathKeep 發現存檔檔案要放回去的位置已經有檔案，就把它們移到 {path}。「釋放空間」裡列著這個資料夾。',
      checkDisk: '請檢查這顆磁碟',
      checkDiskBody:
        '一次改動被中斷後，PathKeep 發現同一個檔案掛在兩個名字下；FAT32 或 exFAT 磁碟停電時可能會這樣。第二個名字放在 {path}。在「釋放空間」刪除那個資料夾之前，請先用系統的磁碟工具檢查磁碟。',
      other: 'PathKeep 有一則關於資料夾的通知',
    },
  },
})
