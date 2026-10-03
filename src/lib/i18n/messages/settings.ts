import { defineMessages } from '../define'

/** Settings: the page, General and Security. AI, Storage and About have their own files. */
export const settings = defineMessages({
  en: {
    title: 'Settings',
    saveFailed: 'Could not save this setting',
    nav: {
      label: 'Settings sections',
      general: 'General',
      security: 'Security',
      ai: 'AI',
      storage: 'Storage',
      about: 'About',
    },
    secret: {
      again: 'Type it again',
      minLength: 'At least {count} characters.',
      tooShort: 'Use at least {count} characters.',
      mismatch: 'The two entries don’t match.',
    },
    general: {
      language: {
        title: 'Language',
        description: 'Interface language',
        system: 'Match system',
      },
      appearance: {
        title: 'Appearance',
        description: 'Light, dark or match system',
      },
      desktop: {
        title: 'Open at login and menu bar icon',
        loadFailed: 'PathKeep could not read these settings. {message}',
        changeFailed: 'Could not change this. {message}',
        login: {
          title: 'Open at login',
          description: 'Start PathKeep in the background when you log in',
        },
        menuBar: {
          title: 'Menu bar icon',
          trayTitle: 'Tray icon',
          description: 'Shows backup status and quick search',
        },
      },
      online: {
        title: 'Online',
        note: 'These two contact websites. Everything else stays on this computer.',
      },
      previews: {
        title: 'Link previews',
        description:
          'Download the preview image each page offers. The site sees a request from your computer.',
      },
      summaries: {
        title: 'Page summaries',
        description:
          'Fetch a short summary of pages you visited, so search can find them by what they say. Sites never get your cookies or accounts.',
        loadFailed: 'Could not load this setting.',
      },
    },
    security: {
      archiveGroup: 'Archive',
      lockGroup: 'App lock',
      lockNote:
        'A passcode in front of the window. It does not encrypt anything and is separate from the archive password.',
      encrypt: {
        title: 'Encrypt archive',
        on: 'AES-256. Only you have the password.',
        off: 'Your history is stored unencrypted. Anyone who can read your files can read it.',
      },
      keychain: {
        title: 'Save password in the keychain',
        description:
          'Unlocks PathKeep without asking. Scheduled backups need it.',
        unavailable: 'The system keychain is not available on this computer.',
        dialogTitle: 'Save password in the keychain',
        dialogBody:
          'Enter the password you use to unlock the archive. It goes into your system keychain.',
        password: 'Archive password',
        saved: 'Password saved in the keychain.',
        saveFailed: 'Could not save the password. {message}',
        removed: 'Password removed from the keychain.',
        removeFailed: 'The password was not removed from the keychain',
      },
      password: {
        title: 'Change password',
        description:
          'If you lose the password, your history cannot be recovered.',
        action: 'Change…',
      },
      rekey: {
        title: {
          encrypt: 'Encrypt the archive',
          change: 'Change the archive password',
          decrypt: 'Turn off encryption?',
        },
        intro: {
          encrypt: 'Choose a password. You’ll need it to open your history.',
          change: 'Choose a new password. The old one stops working.',
          decrypt: 'PathKeep will store your history without encryption.',
        },
        newPassword: 'New password',
        keepInKeychain: 'Save it in the system keychain',
        noReset:
          'Nobody can reset this password, including PathKeep. Keep it somewhere safe.',
        stepsTitle: 'What happens',
        step: {
          copy: 'PathKeep saves a copy of the archive as it is now:',
          writeEncrypted: 'It writes an encrypted copy with the new password.',
          writePlain: 'It writes an unencrypted copy.',
          swap: 'The new file replaces the old one only if every step worked. The saved copy stays, so you can go back.',
        },
        plainWarning:
          'Anyone who can read your files will be able to read your history.',
        warning: {
          'archive-locked': 'The archive is locked. Unlock it first.',
        },
        run: {
          encrypt: 'Encrypt',
          change: 'Change password',
          decrypt: 'Turn off encryption',
        },
        running:
          'Rewriting the archive. A large archive takes a few minutes; keep PathKeep open.',
        done: {
          encrypt: 'The archive is encrypted.',
          change: 'Password changed.',
          decrypt: 'Encryption is off.',
        },
        keychainFailed:
          'The archive changed, but the keychain could not be updated. Turn “Save password in the keychain” off and on again.',
        previewFailed: 'Could not check this change. {message}',
        failed: 'The archive was not changed. {message}',
      },
      appLock: {
        title: 'App lock',
        description: 'Ask for a passcode when PathKeep opens or sits idle',
      },
      passcode: {
        title: 'Passcode',
        description: 'Used to unlock PathKeep',
        label: 'Passcode',
        set: 'Set…',
        change: 'Change…',
        remove: 'Remove',
        setTitle: 'Set a passcode',
        changeTitle: 'Change the passcode',
        dialogBody:
          'You’ll type it to unlock PathKeep. It is not your archive password.',
        save: 'Save passcode',
        savedOn: 'Passcode saved. App lock is on.',
        changed: 'Passcode changed.',
        saveFailed: 'Could not save the passcode. {message}',
        removeTitle: 'Remove the passcode?',
        removeBody:
          'App lock turns off. Your archive and its password are not affected.',
        removed: 'Passcode removed. App lock is off.',
        removeFailed: 'Could not remove the passcode',
      },
      autoLock: {
        title: 'Auto-lock',
        description: 'Lock PathKeep after it sits idle',
        minutes_one: '{count} minute',
        minutes_other: '{count} minutes',
        hour: '1 hour',
      },
      lockNow: {
        title: 'Lock now',
        description: 'Or press',
        action: 'Lock',
      },
    },
  },
  'zh-CN': {
    title: '设置',
    saveFailed: '无法保存这项设置',
    nav: {
      label: '设置分区',
      general: '通用',
      security: '安全',
      ai: 'AI',
      storage: '存储',
      about: '关于',
    },
    secret: {
      again: '再输入一次',
      minLength: '至少 {count} 个字符。',
      tooShort: '请至少输入 {count} 个字符。',
      mismatch: '两次输入的不一样。',
    },
    general: {
      language: {
        title: '语言',
        description: '界面语言',
        system: '跟随系统',
      },
      appearance: {
        title: '外观',
        description: '浅色、深色或跟随系统',
      },
      desktop: {
        title: '登录时启动和菜单栏图标',
        loadFailed: 'PathKeep 无法读取这些设置。{message}',
        changeFailed: '无法更改。{message}',
        login: {
          title: '登录时启动',
          description: '登录电脑时在后台启动 PathKeep',
        },
        menuBar: {
          title: '菜单栏图标',
          trayTitle: '托盘图标',
          description: '显示备份状态和快速搜索',
        },
      },
      online: {
        title: '联网',
        note: '只有这两项会访问网站，其他一切都留在这台电脑上。',
      },
      previews: {
        title: '链接预览',
        description: '下载每个页面提供的预览图。网站会看到来自你电脑的请求。',
      },
      summaries: {
        title: '页面摘要',
        description:
          '抓取你看过的页面的简短摘要，让搜索能按内容找到它们。网站不会拿到你的 Cookie 或账号。',
        loadFailed: '无法加载这项设置。',
      },
    },
    security: {
      archiveGroup: '存档',
      lockGroup: '应用锁',
      lockNote: '挡在窗口前的密码。它不加密任何东西，和存档密码是两回事。',
      encrypt: {
        title: '加密存档',
        on: 'AES-256，只有你知道密码。',
        off: '你的历史以未加密的形式保存，能读取你文件的人都能看到。',
      },
      keychain: {
        title: '把密码存到钥匙串',
        description: '打开 PathKeep 时不再询问密码。定时备份需要它。',
        unavailable: '这台电脑上没有可用的系统钥匙串。',
        dialogTitle: '把密码存到钥匙串',
        dialogBody: '输入你用来解锁存档的密码，它会保存到系统钥匙串里。',
        password: '存档密码',
        saved: '密码已存到钥匙串。',
        saveFailed: '无法保存密码。{message}',
        removed: '已从钥匙串删除密码。',
        removeFailed: '没能从钥匙串删除密码',
      },
      password: {
        title: '更改密码',
        description: '密码一旦丢失，你的历史就无法恢复。',
        action: '更改…',
      },
      rekey: {
        title: {
          encrypt: '加密存档',
          change: '更改存档密码',
          decrypt: '要关闭加密吗？',
        },
        intro: {
          encrypt: '设置一个密码，以后打开你的历史需要用到它。',
          change: '设置新密码，旧密码会失效。',
          decrypt: 'PathKeep 会以不加密的方式保存你的历史。',
        },
        newPassword: '新密码',
        keepInKeychain: '存到系统钥匙串',
        noReset:
          '没有人能重置这个密码，PathKeep 也不能。请把它保存在安全的地方。',
        stepsTitle: '会发生什么',
        step: {
          copy: 'PathKeep 先把当前的存档保存一份副本：',
          writeEncrypted: '再用新密码写出一份加密的存档。',
          writePlain: '再写出一份不加密的存档。',
          swap: '只有每一步都成功，新文件才会替换旧文件。副本会保留，需要时可以退回。',
        },
        plainWarning: '能读取你文件的人都能看到你的历史。',
        warning: {
          'archive-locked': '存档已锁定，请先解锁。',
        },
        run: {
          encrypt: '加密',
          change: '更改密码',
          decrypt: '关闭加密',
        },
        running: '正在重写存档。存档较大时需要几分钟，请保持 PathKeep 打开。',
        done: {
          encrypt: '存档已加密。',
          change: '密码已更改。',
          decrypt: '加密已关闭。',
        },
        keychainFailed:
          '存档已更改，但钥匙串没能更新。请把「把密码存到钥匙串」关掉再打开。',
        previewFailed: '无法检查这项更改。{message}',
        failed: '存档没有改动。{message}',
      },
      appLock: {
        title: '应用锁',
        description: '打开 PathKeep 或闲置一段时间后要求输入密码',
      },
      passcode: {
        title: '应用密码',
        description: '用来解锁 PathKeep',
        label: '应用密码',
        set: '设置…',
        change: '更改…',
        remove: '移除',
        setTitle: '设置应用密码',
        changeTitle: '更改应用密码',
        dialogBody: '解锁 PathKeep 时输入。它不是你的存档密码。',
        save: '保存密码',
        savedOn: '应用密码已保存，应用锁已开启。',
        changed: '应用密码已更改。',
        saveFailed: '无法保存应用密码。{message}',
        removeTitle: '要移除应用密码吗？',
        removeBody: '应用锁会关闭。你的存档和存档密码不受影响。',
        removed: '应用密码已移除，应用锁已关闭。',
        removeFailed: '无法移除应用密码',
      },
      autoLock: {
        title: '自动锁定',
        description: '闲置一段时间后锁定 PathKeep',
        minutes_one: '{count} 分钟',
        minutes_other: '{count} 分钟',
        hour: '1 小时',
      },
      lockNow: {
        title: '立即锁定',
        description: '也可以按',
        action: '锁定',
      },
    },
  },
  'zh-TW': {
    title: '設定',
    saveFailed: '無法儲存這項設定',
    nav: {
      label: '設定分區',
      general: '一般',
      security: '安全',
      ai: 'AI',
      storage: '儲存',
      about: '關於',
    },
    secret: {
      again: '再輸入一次',
      minLength: '至少 {count} 個字元。',
      tooShort: '請至少輸入 {count} 個字元。',
      mismatch: '兩次輸入的不一樣。',
    },
    general: {
      language: {
        title: '語言',
        description: '介面語言',
        system: '跟隨系統',
      },
      appearance: {
        title: '外觀',
        description: '淺色、深色或跟隨系統',
      },
      desktop: {
        title: '登入時啟動和選單列圖示',
        loadFailed: 'PathKeep 無法讀取這些設定。{message}',
        changeFailed: '無法變更。{message}',
        login: {
          title: '登入時啟動',
          description: '登入電腦時在背景啟動 PathKeep',
        },
        menuBar: {
          title: '選單列圖示',
          trayTitle: '系統匣圖示',
          description: '顯示備份狀態和快速搜尋',
        },
      },
      online: {
        title: '連網',
        note: '只有這兩項會連線到網站，其他一切都留在這台電腦上。',
      },
      previews: {
        title: '連結預覽',
        description: '下載每個頁面提供的預覽圖。網站會看到來自你電腦的請求。',
      },
      summaries: {
        title: '頁面摘要',
        description:
          '抓取你看過的頁面的簡短摘要，讓搜尋能依內容找到它們。網站不會拿到你的 Cookie 或帳號。',
        loadFailed: '無法載入這項設定。',
      },
    },
    security: {
      archiveGroup: '存檔',
      lockGroup: 'App 鎖定',
      lockNote: '擋在視窗前的密碼。它不加密任何東西，和存檔密碼是兩回事。',
      encrypt: {
        title: '加密存檔',
        on: 'AES-256，只有你知道密碼。',
        off: '你的歷史以未加密的形式儲存，能讀取你檔案的人都能看到。',
      },
      keychain: {
        title: '把密碼存到鑰匙圈',
        description: '開啟 PathKeep 時不再詢問密碼。排程備份需要它。',
        unavailable: '這台電腦上沒有可用的系統鑰匙圈。',
        dialogTitle: '把密碼存到鑰匙圈',
        dialogBody: '輸入你用來解鎖存檔的密碼，它會儲存到系統鑰匙圈。',
        password: '存檔密碼',
        saved: '密碼已存到鑰匙圈。',
        saveFailed: '無法儲存密碼。{message}',
        removed: '已從鑰匙圈刪除密碼。',
        removeFailed: '沒能從鑰匙圈刪除密碼',
      },
      password: {
        title: '更改密碼',
        description: '密碼一旦遺失，你的歷史就無法復原。',
        action: '更改…',
      },
      rekey: {
        title: {
          encrypt: '加密存檔',
          change: '更改存檔密碼',
          decrypt: '要關閉加密嗎？',
        },
        intro: {
          encrypt: '設定一組密碼，之後開啟你的歷史需要用到它。',
          change: '設定新密碼，舊密碼會失效。',
          decrypt: 'PathKeep 會以不加密的方式儲存你的歷史。',
        },
        newPassword: '新密碼',
        keepInKeychain: '存到系統鑰匙圈',
        noReset:
          '沒有人能重設這組密碼，PathKeep 也不能。請把它存放在安全的地方。',
        stepsTitle: '會發生什麼',
        step: {
          copy: 'PathKeep 先把目前的存檔保存一份副本：',
          writeEncrypted: '再用新密碼寫出一份加密的存檔。',
          writePlain: '再寫出一份不加密的存檔。',
          swap: '只有每一步都成功，新檔案才會取代舊檔案。副本會保留，需要時可以退回。',
        },
        plainWarning: '能讀取你檔案的人都能看到你的歷史。',
        warning: {
          'archive-locked': '存檔已鎖定，請先解鎖。',
        },
        run: {
          encrypt: '加密',
          change: '更改密碼',
          decrypt: '關閉加密',
        },
        running: '正在重寫存檔。存檔較大時需要幾分鐘，請保持 PathKeep 開啟。',
        done: {
          encrypt: '存檔已加密。',
          change: '密碼已更改。',
          decrypt: '加密已關閉。',
        },
        keychainFailed:
          '存檔已變更，但鑰匙圈沒能更新。請把「把密碼存到鑰匙圈」關掉再打開。',
        previewFailed: '無法檢查這項變更。{message}',
        failed: '存檔沒有變動。{message}',
      },
      appLock: {
        title: 'App 鎖定',
        description: '開啟 PathKeep 或閒置一段時間後要求輸入密碼',
      },
      passcode: {
        title: 'App 密碼',
        description: '用來解鎖 PathKeep',
        label: 'App 密碼',
        set: '設定…',
        change: '更改…',
        remove: '移除',
        setTitle: '設定 App 密碼',
        changeTitle: '更改 App 密碼',
        dialogBody: '解鎖 PathKeep 時輸入。它不是你的存檔密碼。',
        save: '儲存密碼',
        savedOn: 'App 密碼已儲存，App 鎖定已開啟。',
        changed: 'App 密碼已更改。',
        saveFailed: '無法儲存 App 密碼。{message}',
        removeTitle: '要移除 App 密碼嗎？',
        removeBody: 'App 鎖定會關閉。你的存檔和存檔密碼不受影響。',
        removed: 'App 密碼已移除，App 鎖定已關閉。',
        removeFailed: '無法移除 App 密碼',
      },
      autoLock: {
        title: '自動鎖定',
        description: '閒置一段時間後鎖定 PathKeep',
        minutes_one: '{count} 分鐘',
        minutes_other: '{count} 分鐘',
        hour: '1 小時',
      },
      lockNow: {
        title: '立即鎖定',
        description: '也可以按',
        action: '鎖定',
      },
    },
  },
})
