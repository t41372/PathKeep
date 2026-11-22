/**
 * Words for the time numbers, in one place so every screen says the same
 * thing. The example is part of the label, not hidden in a tooltip: "有
 * Agent 在跑的時間" didn't explain itself.
 */
export const timeWords = {
  /** Clock time with at least one agent working; parallel agents count once. */
  agent: 'Agent 工作時間',
  agentHow: '按時鐘算：同時開幾個 Agent 都只算一次。兩個 Agent 一起跑 1 小時，算 1 小時。',
  /** Each conversation's agent time added up; parallel agents count once each. */
  summed: '分開加總',
  summedHow: '每段對話各算各的再加起來。兩個 Agent 一起跑 1 小時，算 2 小時。',
  concurrency: (n: number) => `平均同時開 ${n.toFixed(1)} 個`,
  present: '你在場的時間',
  presentHow: '估算：你在跟 Agent 來回的時段；中間超過 15 分鐘沒動靜就不算。',
} as const

/**
 * A project can hold several folders; joining folders is not "merging projects".
 * They are managed in 專案設定: a menu item called 「資料夾」 reads like "open the folder".
 */
export const folderWords = {
  settings: '專案設定',
  add: '加入資料夾…',
  addFrom: (name: string) => `把資料夾加到「${name}」`,
  remove: '從這個專案移出',
  suggestions: (n: number) => `${n} 個資料夾可能屬於別的專案`,
} as const
