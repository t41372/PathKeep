import fs from 'node:fs'
import path from 'node:path'

const workspaceRoot = process.cwd()
const lcovPath = process.argv[2] ?? path.join('coverage', 'rust.lcov.info')

const lcov = fs.readFileSync(lcovPath, 'utf8')
const records = parseLcov(lcov).filter((record) =>
  isWorkspaceRustSource(record.file),
)

if (records.length === 0) {
  console.error(`No Rust coverage records found in ${lcovPath}.`)
  process.exit(1)
}

const uncoveredLines = records
  .map((record) => ({
    file: record.file,
    lines: [...record.lines.entries()]
      .filter(([, count]) => count === 0)
      .map(([line]) => line),
  }))
  .filter((record) => record.lines.length > 0)

const uncoveredFunctions = []
let totalFunctionCount = 0

for (const record of records) {
  const source = fs.readFileSync(record.file, 'utf8')
  const functions = parseRustFunctions(source)
  totalFunctionCount += functions.length

  for (const fn of functions) {
    const relevantLines = [...record.lines.entries()].filter(
      ([line]) => line >= fn.startLine && line <= fn.endLine,
    )
    if (relevantLines.length === 0) {
      continue
    }

    totalFunctionCount += 1
    const covered = relevantLines.some(([, count]) => count > 0)
    if (!covered) {
      uncoveredFunctions.push({ file: record.file, ...fn })
    }
  }
}

if (uncoveredLines.length > 0 || uncoveredFunctions.length > 0) {
  if (uncoveredLines.length > 0) {
    console.error('Uncovered Rust source lines:')
    for (const record of uncoveredLines) {
      console.error(`- ${relative(record.file)}:${record.lines.join(',')}`)
    }
  }

  if (uncoveredFunctions.length > 0) {
    console.error('Uncovered Rust source functions:')
    for (const fn of uncoveredFunctions) {
      console.error(`- ${relative(fn.file)}:${fn.startLine} ${fn.name}`)
    }
  }

  process.exit(1)
}

const totalLineCount = records.reduce(
  (sum, record) => sum + record.lines.size,
  0,
)
console.log(
  `Rust coverage verified at 100% for ${totalLineCount} instrumented source lines and ${totalFunctionCount} source functions.`,
)

function relative(file) {
  return path.relative(workspaceRoot, file) || file
}

function isWorkspaceRustSource(file) {
  return (
    file.startsWith(path.join(workspaceRoot, 'src-tauri')) &&
    file.endsWith('.rs') &&
    file.includes(`${path.sep}src${path.sep}`)
  )
}

function parseLcov(content) {
  const records = []
  let current = null

  for (const rawLine of content.split(/\r?\n/)) {
    if (rawLine.startsWith('SF:')) {
      current = { file: rawLine.slice(3), lines: new Map() }
      continue
    }

    if (!current) {
      continue
    }

    if (rawLine.startsWith('DA:')) {
      const [line, count] = rawLine.slice(3).split(',', 2)
      current.lines.set(Number(line), Number(count))
      continue
    }

    if (rawLine === 'end_of_record') {
      records.push(current)
      current = null
    }
  }

  return records
}

function parseRustFunctions(source) {
  const masked = maskNonCode(source)
  const lineOffsets = buildLineOffsets(masked)
  const functions = []
  const pattern =
    /^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:const\s+)?(?:unsafe\s+)?(?:extern\s+"[^"]+"\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\b/gm

  for (const match of masked.matchAll(pattern)) {
    const startOffset = match.index ?? 0
    const bodyEndOffset = findFunctionBodyEnd(masked, startOffset)
    if (bodyEndOffset == null) {
      continue
    }

    functions.push({
      name: match[1],
      startLine: lineNumberForOffset(lineOffsets, startOffset),
      endLine: lineNumberForOffset(lineOffsets, bodyEndOffset),
    })
  }

  return functions
}

function buildLineOffsets(source) {
  const offsets = [0]
  for (let index = 0; index < source.length; index += 1) {
    if (source[index] === '\n') {
      offsets.push(index + 1)
    }
  }
  return offsets
}

function lineNumberForOffset(offsets, offset) {
  let low = 0
  let high = offsets.length - 1
  while (low <= high) {
    const mid = Math.floor((low + high) / 2)
    if (offsets[mid] <= offset) {
      low = mid + 1
    } else {
      high = mid - 1
    }
  }
  return high + 1
}

function findFunctionBodyEnd(source, startOffset) {
  let index = startOffset
  let depth = 0
  let bodyStarted = false

  while (index < source.length) {
    const char = source[index]

    if (char === '{') {
      depth += 1
      bodyStarted = true
      index += 1
      continue
    }

    if (char === '}') {
      depth -= 1
      index += 1
      if (bodyStarted && depth === 0) {
        return index - 1
      }
      continue
    }

    if (!bodyStarted && char === ';') {
      return null
    }

    index += 1
  }

  return null
}

function maskNonCode(source) {
  let index = 0
  let output = ''

  while (index < source.length) {
    const char = source[index]
    const next = source[index + 1]

    if (char === '/' && next === '/') {
      output += '  '
      index += 2
      while (index < source.length && source[index] !== '\n') {
        output += ' '
        index += 1
      }
      continue
    }

    if (char === '/' && next === '*') {
      output += '  '
      index += 2
      let depth = 1
      while (index < source.length && depth > 0) {
        if (source[index] === '\n') {
          output += '\n'
          index += 1
        } else if (source[index] === '/' && source[index + 1] === '*') {
          output += '  '
          index += 2
          depth += 1
        } else if (source[index] === '*' && source[index + 1] === '/') {
          output += '  '
          index += 2
          depth -= 1
        } else {
          output += ' '
          index += 1
        }
      }
      continue
    }

    const rawStringHashes = rawStringOpeningHashes(source, index)
    if (rawStringHashes != null) {
      const openingLength = rawStringHashes + 2
      output += ' '.repeat(openingLength)
      index += openingLength
      while (index < source.length) {
        if (
          source[index] === '"' &&
          source.slice(index + 1, index + 1 + rawStringHashes) ===
            '#'.repeat(rawStringHashes)
        ) {
          output += ' '.repeat(rawStringHashes + 1)
          index += rawStringHashes + 1
          break
        }
        output += source[index] === '\n' ? '\n' : ' '
        index += 1
      }
      continue
    }

    if (char === '"') {
      output += ' '
      index += 1
      while (index < source.length) {
        if (source[index] === '\\') {
          output += '  '
          index += 2
        } else {
          const current = source[index]
          output += current === '\n' ? '\n' : ' '
          index += 1
          if (current === '"') {
            break
          }
        }
      }
      continue
    }

    if (char === "'") {
      output += ' '
      index += 1
      while (index < source.length) {
        if (source[index] === '\\') {
          output += '  '
          index += 2
        } else {
          const current = source[index]
          output += current === '\n' ? '\n' : ' '
          index += 1
          if (current === "'") {
            break
          }
        }
      }
      continue
    }

    output += char
    index += 1
  }

  return output
}

function rawStringOpeningHashes(source, index) {
  if (source[index] !== 'r') {
    return null
  }

  let cursor = index + 1
  while (source[cursor] === '#') {
    cursor += 1
  }

  if (source[cursor] !== '"') {
    return null
  }

  return cursor - index - 1
}
