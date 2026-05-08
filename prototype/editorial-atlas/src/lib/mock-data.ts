export interface HistoryEntry {
  id: string
  url: string
  title: string
  domain: string
  favicon?: string
  timestamp: Date
  type: 'article' | 'repository' | 'video' | 'social' | 'docs' | 'search' | 'other'
  note?: string
  snippet?: string
  bookmarked: boolean
  visitCount: number
  timeSpent?: string
  source: string
}

export interface DayGroup {
  date: Date
  label: string
  entries: HistoryEntry[]
}

// Mock data with multilingual content (繁簡中日韓英)
export const mockEntries: HistoryEntry[] = [
  {
    id: '1',
    url: 'https://aeon.co/essays/the-case-for-slow-productivity',
    title: 'The Case for Slow Productivity',
    domain: 'aeon.co',
    timestamp: new Date('2026-05-24T09:42:00'),
    type: 'article',
    note: 'Important read for my writing on focus.',
    bookmarked: true,
    visitCount: 1,
    timeSpent: '4m 28s',
    source: 'Arc Browser',
  },
  {
    id: '2',
    url: 'https://nesslabs.com/second-brain',
    title: 'Building a Second Brain, Step by Step',
    domain: 'nesslabs.com',
    timestamp: new Date('2026-05-24T08:17:00'),
    type: 'article',
    snippet: '"Your second brain is not about storing everything, it\'s about building trusted pathways."',
    bookmarked: false,
    visitCount: 2,
    source: 'Chrome',
  },
  {
    id: '3',
    url: 'https://github.com/microsoft/markitdown',
    title: 'microsoft/markitdown',
    domain: 'github.com',
    timestamp: new Date('2026-05-24T07:34:00'),
    type: 'repository',
    note: 'Add to automation toolkit.',
    bookmarked: false,
    visitCount: 1,
    source: 'Firefox',
  },
  {
    id: '4',
    url: 'https://stratechery.com/the-platform-trap',
    title: 'The Platform Trap',
    domain: 'stratechery.com',
    timestamp: new Date('2026-05-23T22:11:00'),
    type: 'article',
    snippet: '"The moat is not technology; it\'s the network and data gravity they create."',
    bookmarked: true,
    visitCount: 1,
    source: 'Safari',
  },
  {
    id: '5',
    url: 'https://noahpinion.substack.com/goodharts-law',
    title: "Goodhart's Law in the Wild",
    domain: 'noahpinion.substack.com',
    timestamp: new Date('2026-05-23T18:48:00'),
    type: 'article',
    note: 'Connect to article I\'m writing on metrics.',
    bookmarked: false,
    visitCount: 1,
    source: 'Arc Browser',
  },
  // 中文内容
  {
    id: '6',
    url: 'https://www.zhihu.com/question/123456789',
    title: '如何评价「慢生产力」这个概念？',
    domain: 'zhihu.com',
    timestamp: new Date('2026-05-23T16:20:00'),
    type: 'social',
    bookmarked: false,
    visitCount: 3,
    source: 'Chrome',
  },
  {
    id: '7',
    url: 'https://sspai.com/post/87654321',
    title: '從零開始搭建個人知識管理系統',
    domain: 'sspai.com',
    timestamp: new Date('2026-05-23T14:05:00'),
    type: 'article',
    note: '很好的入門文章，適合推薦給朋友。',
    bookmarked: true,
    visitCount: 2,
    timeSpent: '12m 45s',
    source: 'Safari',
  },
  // 日本語コンテンツ
  {
    id: '8',
    url: 'https://note.com/example/memory-palace',
    title: '記憶の宮殿：デジタル時代の知識整理術',
    domain: 'note.com',
    timestamp: new Date('2026-05-22T21:30:00'),
    type: 'article',
    bookmarked: false,
    visitCount: 1,
    source: 'Chrome',
  },
  {
    id: '9',
    url: 'https://zenn.dev/articles/rust-tauri-intro',
    title: 'Tauri + Rustで作るモダンなデスクトップアプリ',
    domain: 'zenn.dev',
    timestamp: new Date('2026-05-22T19:15:00'),
    type: 'docs',
    note: 'PathKeep の参考になる',
    bookmarked: true,
    visitCount: 4,
    source: 'Firefox',
  },
  // 한국어 콘텐츠
  {
    id: '10',
    url: 'https://velog.io/@example/productivity-tips',
    title: '개발자를 위한 생산성 향상 팁 10가지',
    domain: 'velog.io',
    timestamp: new Date('2026-05-22T15:45:00'),
    type: 'article',
    bookmarked: false,
    visitCount: 1,
    source: 'Chrome',
  },
  {
    id: '11',
    url: 'https://www.youtube.com/watch?v=dQw4w9WgXcQ',
    title: 'Understanding Memory: A Neuroscience Perspective',
    domain: 'youtube.com',
    timestamp: new Date('2026-05-22T12:00:00'),
    type: 'video',
    timeSpent: '18m 32s',
    bookmarked: false,
    visitCount: 1,
    source: 'Arc Browser',
  },
  {
    id: '12',
    url: 'https://docs.rs/tauri/latest/tauri/',
    title: 'tauri - Rust Documentation',
    domain: 'docs.rs',
    timestamp: new Date('2026-05-22T10:30:00'),
    type: 'docs',
    bookmarked: true,
    visitCount: 12,
    source: 'Firefox',
  },
]

export function groupEntriesByDay(entries: HistoryEntry[]): DayGroup[] {
  const groups = new Map<string, HistoryEntry[]>()
  
  entries.forEach(entry => {
    const dateKey = entry.timestamp.toISOString().split('T')[0]
    if (!groups.has(dateKey)) {
      groups.set(dateKey, [])
    }
    groups.get(dateKey)!.push(entry)
  })

  const today = new Date()
  const yesterday = new Date(today)
  yesterday.setDate(yesterday.getDate() - 1)

  return Array.from(groups.entries())
    .sort(([a], [b]) => b.localeCompare(a))
    .map(([dateKey, dayEntries]) => {
      const date = new Date(dateKey)
      let label: string
      
      if (dateKey === today.toISOString().split('T')[0]) {
        label = 'Today'
      } else if (dateKey === yesterday.toISOString().split('T')[0]) {
        label = 'Yesterday'
      } else {
        label = date.toLocaleDateString('en-US', { 
          weekday: 'long',
          month: 'short', 
          day: 'numeric',
          year: 'numeric'
        })
      }

      return {
        date,
        label,
        entries: dayEntries.sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime()),
      }
    })
}

// Browsing intensity data for heatmap
export const browsingIntensityData = [
  // Week data: [Mon, Tue, Wed, Thu, Fri, Sat, Sun] x 4 time slots
  // Values 0-4 represent intensity levels
  [1, 2, 1, 3, 2, 0, 0], // 12 AM - 6 AM
  [2, 3, 4, 3, 3, 1, 1], // 6 AM - 12 PM  
  [4, 4, 3, 4, 4, 2, 1], // 12 PM - 6 PM
  [3, 2, 2, 2, 1, 2, 2], // 6 PM - 12 AM
]

export const archiveStats = {
  totalPages: 7842,
  totalSize: '1.3 GB',
  syncStatus: 'Synced never (local only)',
  lastIndexed: new Date('2026-05-24T06:03:00'),
}
