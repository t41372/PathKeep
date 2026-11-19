import { defineMessages } from '../define'

/**
 * Repairing saved Agent conversations whose files are damaged
 * (`?repair=agent`): what is wrong, which repair applies and what it does,
 * where conversations come back from, the result, and the refusal the move
 * and encryption dialogs show while conversations can't be read.
 */
export const agentRepair = defineMessages({
  en: {
    open: 'Repair conversations…',
    title: 'Repair Agent conversations',
    intro:
      'Browsing history isn’t affected. Nothing changes until you choose to repair.',
    whatsWrong: 'What’s wrong',
    checking: 'Checking what can be brought back…',
    checkingHint:
      'Checking only reads. Browsing history can be viewed and backed up meanwhile.',
    held: 'This check is kept for 15 minutes. After that, PathKeep checks again.',
    actions: {
      rebuild: {
        title: 'Rebuild the lists from the saved conversations',
        body: 'The saved conversations themselves are intact; their lists, search and insights are not. PathKeep builds those again from the conversations, even ones the tools have already deleted. Your marks on conversations, such as stars, changed titles and the trash, stay, and so does the backup history.',
        count_one: '{count} conversation can be read again.',
        count_other: '{count} conversations can be read again.',
        run: 'Rebuild the lists',
      },
      fromCopy: {
        title: 'Bring conversations back from a safety copy',
        body: 'Saved conversations go back to how they were in this copy. Anything saved after it was made isn’t in it.',
        made: '{label} · {date}',
        count_one: 'The copy holds {count} conversation.',
        count_other: 'The copy holds {count} conversations.',
        run: 'Repair from this copy',
      },
      startFresh: {
        title: 'Start over with what still checks out',
        body: 'The file that holds the saved conversations is damaged. PathKeep starts new conversation files and puts back every saved version that still checks out, with the stars and titles that can still be read.',
        count_one: '{count} conversation comes back.',
        count_other: '{count} conversations come back.',
        allBack: 'Every conversation comes back.',
        leftBehind_one:
          '{count} conversation can’t be brought back. It stays only in the copy of the damaged files.',
        leftBehind_other:
          '{count} conversations can’t be brought back. They stay only in the copy of the damaged files.',
        uncounted:
          'How many can’t be brought back can’t be counted: the damaged files can’t be listed.',
        toolsDeleted:
          'Conversations the tools have already deleted come back only if they checked out above. The rest stay in the copy of the damaged files, where PathKeep can’t read them.',
        recapture:
          'The next backup saves again every conversation still in the tools’ folders.',
        run: 'Start over',
      },
    },
    kept: 'What’s there now is kept as a safety copy, so nothing is deleted:',
    keptHint:
      'It’s listed in Restore from a safety copy, and Free up space can delete it.',
    reviewSources:
      'Conversations aren’t backed up again until you choose the folders to back up on the Agent backup page.',
    sources: {
      title: 'Bring conversations back from',
      local: 'The damaged files',
      localHint: 'Every saved version that still checks out',
      unreadable:
        'Conversations couldn’t be read when this copy was made, so it can’t repair them.',
      unchecked:
        'This copy hasn’t been checked, so it can’t be used for a repair.',
      none: 'There’s no safety copy to repair from.',
      loading: 'Listing the safety copies',
      loadFailed: 'Couldn’t list the safety copies. {message}',
      check: 'Check this copy',
      checkLocal: 'Check the damaged files',
      passwordAsked:
        'This copy was made under another password. Enter it to check the copy.',
    },
    errors: {
      stale:
        'The check has expired, or something changed after it. Check again to repair.',
      preservation:
        'PathKeep couldn’t read a file it has to keep, so nothing was changed. {message}',
      copyUnreadable:
        'Conversations couldn’t be read when this copy was made, so it can’t repair them. Choose another copy.',
      copyNotListed:
        'This copy isn’t in the list any more. Choose another one.',
      copyIncomplete:
        'This copy doesn’t have all the conversation files, so it can’t repair them. Choose another copy.',
      copyInvalid:
        'This copy doesn’t check out, so it can’t be used for a repair. {message}',
      schemaTooNew:
        'These conversations were saved by a newer version of PathKeep. Update PathKeep to read them; a repair can’t help.',
      healthy:
        'Saved conversations open normally now. There’s nothing to repair.',
      evidenceReadable:
        'The saved conversations themselves are intact, so they’re rebuilt from themselves, not from a copy.',
      pairUnverified:
        'The two files that hold the conversations don’t belong together, so PathKeep can’t rebuild from them. Choose a safety copy.',
      wrongPassword: 'That isn’t this copy’s password.',
      busy: 'Another change to the archive is running. Try again when it’s done.',
      checkFailed: 'Couldn’t check what can be brought back. {message}',
      repairFailed: 'The repair didn’t finish. {message}',
      checkAgain: 'Check again',
    },
    done: {
      title: 'Saved conversations open again',
      count_one: '{count} conversation can be read.',
      count_other: '{count} conversations can be read.',
      kept: 'What was there before is kept as a safety copy:',
      reviewSources:
        'Before the next backup, choose the folders to back up again.',
      openBackup: 'Open Agent backup',
      close: 'Done',
    },
    required: {
      export:
        'Saved Agent conversations can’t be read right now, so they can’t be packed into the file. Repair them first, then export again. Nothing was changed.',
      import:
        'This computer’s saved Agent conversations can’t be read right now, so PathKeep can’t replace the archive yet. Repair them first, then open the file again. Nothing was changed.',
      encryption:
        'Turning encryption on or off rewrites every file of the archive, and the saved Agent conversations can’t be read right now. Repair them first, then try again. Nothing was changed.',
    },
  },
  'zh-CN': {
    open: '修复对话…',
    title: '修复 Agent 对话',
    intro: '浏览记录不受影响。在你点修复之前，什么都不会改动。',
    whatsWrong: '出了什么问题',
    checking: '正在检查能找回哪些对话…',
    checkingHint: '检查只读取数据。检查期间照样可以查看和备份浏览记录。',
    held: '这次检查的结果保留 15 分钟，过了就要重新检查。',
    actions: {
      rebuild: {
        title: '用已存的对话重建列表',
        body: '已存的对话本身完好，坏掉的是它们的列表、搜索和洞察。PathKeep 会根据对话重新建好这些，工具已经删掉的对话也一样。你在对话上做的标记（加星、改过的标题、回收站等）和备份记录都会保留。',
        count_one: '重建后能读到 {count} 段对话。',
        count_other: '重建后能读到 {count} 段对话。',
        run: '重建列表',
      },
      fromCopy: {
        title: '从安全副本找回对话',
        body: '已存的对话会回到这份副本里的样子。副本创建之后才存下的内容不在里面。',
        made: '{label} · {date}',
        count_one: '这份副本里有 {count} 段对话。',
        count_other: '这份副本里有 {count} 段对话。',
        run: '用这份副本修复',
      },
      startFresh: {
        title: '重新开始，找回还能核对的部分',
        body: '存放已存对话的文件坏了。PathKeep 会建立新的对话文件，把每个仍能核对无误的已存版本放回去，还读得到的加星和标题也一起带回。',
        count_one: '会找回 {count} 段对话。',
        count_other: '会找回 {count} 段对话。',
        allBack: '每段对话都会回来。',
        leftBehind_one:
          '有 {count} 段对话找不回来，只会留在坏文件的那份副本里。',
        leftBehind_other:
          '有 {count} 段对话找不回来，只会留在坏文件的那份副本里。',
        uncounted: '找不回来的有几段，没办法算出来：坏掉的文件列不出对话。',
        toolsDeleted:
          '工具已经删掉的对话，只有上面核对无误的才会回来；其余的只留在坏文件的那份副本里，PathKeep 读不到。',
        recapture: '下次备份会把工具文件夹里还在的每段对话重新存一次。',
        run: '重新开始',
      },
    },
    kept: '现在的文件会保留成一份安全副本，不会删掉：',
    keptHint: '它会列在「从安全副本恢复」里，也可以在「释放空间」里删除。',
    reviewSources:
      '要先在 Agent 对话的备份页重新选择要备份的文件夹，才会再备份对话。',
    sources: {
      title: '从哪里找回对话',
      local: '坏掉的文件',
      localHint: '找回每个仍能核对无误的已存版本',
      unreadable: '创建这份副本时对话就已经读不了，所以不能用来修复对话。',
      unchecked: '这份副本还没检查过，不能用来修复。',
      none: '没有能用来修复的安全副本。',
      loading: '正在列出安全副本',
      loadFailed: '无法列出安全副本。{message}',
      check: '检查这份副本',
      checkLocal: '检查坏掉的文件',
      passwordAsked: '这份副本是用另一个密码创建的。输入那个密码才能检查。',
    },
    errors: {
      stale: '检查的结果已经过期，或者之后有东西变了。请重新检查再修复。',
      preservation:
        'PathKeep 读不了一个必须保留的文件，所以什么都没有改动。{message}',
      copyUnreadable:
        '创建这份副本时对话就已经读不了，不能用来修复。请选另一份。',
      copyNotListed: '这份副本已经不在列表里了。请选另一份。',
      copyIncomplete: '这份副本缺了部分对话文件，不能用来修复。请选另一份。',
      copyInvalid: '这份副本核对不通过，不能用来修复。{message}',
      schemaTooNew:
        '这些对话是较新版本的 PathKeep 存的。更新 PathKeep 就能读，修复帮不上忙。',
      healthy: '已存的对话现在能正常打开，不需要修复。',
      evidenceReadable: '已存的对话本身完好，会直接用它们重建，不用副本。',
      pairUnverified:
        '存放对话的两个文件对不上，PathKeep 没法用它们重建。请选一份安全副本。',
      wrongPassword: '这不是这份副本的密码。',
      busy: '存档正在进行另一项改动。等它完成后再试一次。',
      checkFailed: '无法检查能找回哪些对话。{message}',
      repairFailed: '修复没有完成。{message}',
      checkAgain: '重新检查',
    },
    done: {
      title: '已存的对话又能打开了',
      count_one: '现在能读到 {count} 段对话。',
      count_other: '现在能读到 {count} 段对话。',
      kept: '原来的文件保留成了一份安全副本：',
      reviewSources: '下次备份之前，请重新选择要备份的文件夹。',
      openBackup: '打开 Agent 对话的备份页',
      close: '完成',
    },
    required: {
      export:
        '已存的 Agent 对话现在读不了，没法打包进文件。请先修复，再导出一次。什么都没有改动。',
      import:
        '这台电脑已存的 Agent 对话现在读不了，所以还不能替换存档。请先修复，再打开这个文件一次。什么都没有改动。',
      encryption:
        '开关加密会重写存档的每个文件，而已存的 Agent 对话现在读不了。请先修复，再试一次。什么都没有改动。',
    },
  },
  'zh-TW': {
    open: '修復對話…',
    title: '修復 Agent 對話',
    intro: '瀏覽紀錄不受影響。在你按下修復之前，什麼都不會變動。',
    whatsWrong: '出了什麼問題',
    checking: '正在檢查能救回哪些對話…',
    checkingHint: '檢查只會讀取。檢查時照樣可以查看和備份瀏覽紀錄。',
    held: '這次檢查的結果保留 15 分鐘，超過就要重新檢查。',
    actions: {
      rebuild: {
        title: '用已存的對話重建清單',
        body: '已存的對話本身完好，壞掉的是它們的清單、搜尋和洞察。PathKeep 會從對話重新建好這些，工具已經刪掉的對話也一樣。你在對話上做的記號（星號、改過的標題、垃圾桶等）和備份紀錄都會留著。',
        count_one: '重建後可以讀到 {count} 段對話。',
        count_other: '重建後可以讀到 {count} 段對話。',
        run: '重建清單',
      },
      fromCopy: {
        title: '從安全副本取回對話',
        body: '已存的對話會回到這份副本裡的樣子。副本建立之後才存下的內容不在裡面。',
        made: '{label} · {date}',
        count_one: '這份副本裡有 {count} 段對話。',
        count_other: '這份副本裡有 {count} 段對話。',
        run: '用這份副本修復',
      },
      startFresh: {
        title: '重新開始，救回還能核對的部分',
        body: '存放已存對話的檔案壞了。PathKeep 會建立新的對話檔案，把每個仍能核對無誤的已存版本放回去，還讀得到的星號和標題也一起帶回。',
        count_one: '會救回 {count} 段對話。',
        count_other: '會救回 {count} 段對話。',
        allBack: '每段對話都會回來。',
        leftBehind_one:
          '有 {count} 段對話救不回來，只會留在壞檔案的那份副本裡。',
        leftBehind_other:
          '有 {count} 段對話救不回來，只會留在壞檔案的那份副本裡。',
        uncounted: '救不回來的有幾段，沒辦法算出來：壞掉的檔案列不出對話。',
        toolsDeleted:
          '工具已經刪掉的對話，只有上面核對無誤的才會回來；其餘的只留在壞檔案的那份副本裡，PathKeep 讀不到。',
        recapture: '下次備份會把工具資料夾裡還在的每段對話重新存一次。',
        run: '重新開始',
      },
    },
    kept: '現在的檔案會留成一份安全副本，不會刪掉：',
    keptHint: '它會列在「從安全副本還原」裡，也可以在「釋放空間」刪除。',
    reviewSources:
      '要先在 Agent 對話的備份頁重新選擇要備份的資料夾，才會再備份對話。',
    sources: {
      title: '從哪裡取回對話',
      local: '壞掉的檔案',
      localHint: '救回每個仍能核對無誤的已存版本',
      unreadable: '建立這份副本時對話就已經讀不了，所以不能用來修復對話。',
      unchecked: '這份副本還沒檢查過，不能用來修復。',
      none: '沒有能用來修復的安全副本。',
      loading: '正在列出安全副本',
      loadFailed: '無法列出安全副本。{message}',
      check: '檢查這份副本',
      checkLocal: '檢查壞掉的檔案',
      passwordAsked: '這份副本是用另一個密碼建立的。輸入那個密碼才能檢查。',
    },
    errors: {
      stale: '檢查的結果已經過期，或之後有東西變了。請重新檢查再修復。',
      preservation:
        'PathKeep 讀不了一個必須保留的檔案，所以什麼都沒有變動。{message}',
      copyUnreadable:
        '建立這份副本時對話就已經讀不了，不能用來修復。請選另一份。',
      copyNotListed: '這份副本已經不在清單上了。請選另一份。',
      copyIncomplete: '這份副本缺了部分對話檔案，不能用來修復。請選另一份。',
      copyInvalid: '這份副本核對不過，不能用來修復。{message}',
      schemaTooNew:
        '這些對話是較新版本的 PathKeep 存的。更新 PathKeep 就能讀，修復幫不上忙。',
      healthy: '已存的對話現在可以正常打開，不需要修復。',
      evidenceReadable: '已存的對話本身完好，會直接從它們重建，不用副本。',
      pairUnverified:
        '存放對話的兩個檔案對不起來，PathKeep 無法從它們重建。請選一份安全副本。',
      wrongPassword: '這不是這份副本的密碼。',
      busy: '存檔正在進行另一項變更。等它完成再試一次。',
      checkFailed: '無法檢查能救回哪些對話。{message}',
      repairFailed: '修復沒有完成。{message}',
      checkAgain: '重新檢查',
    },
    done: {
      title: '已存的對話又能打開了',
      count_one: '現在可以讀到 {count} 段對話。',
      count_other: '現在可以讀到 {count} 段對話。',
      kept: '原本的檔案留成了一份安全副本：',
      reviewSources: '下次備份之前，請重新選擇要備份的資料夾。',
      openBackup: '打開 Agent 對話的備份頁',
      close: '完成',
    },
    required: {
      export:
        '已存的 Agent 對話現在讀不了，沒辦法打包進檔案。請先修復，再匯出一次。什麼都沒有變動。',
      import:
        '這台電腦已存的 Agent 對話現在讀不了，所以還不能取代存檔。請先修復，再打開這個檔案一次。什麼都沒有變動。',
      encryption:
        '開關加密會重寫存檔的每個檔案，而已存的 Agent 對話現在讀不了。請先修復，再試一次。什麼都沒有變動。',
    },
  },
})
