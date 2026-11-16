/**
 * Writes a .zip the way Google Takeout sends one: deflated entries, UTF-8
 * names, in the order given. Reads one back, including the ZIP64 fields the
 * app's own export writes, so a spec can change a file the app wrote. Only what
 * the specs need: no folder entries, and nothing written over 4 GB.
 */
import { crc32, deflateRawSync, inflateRawSync } from 'node:zlib'

const DEFLATE = 8
/** Bit 11: the names are UTF-8 (Takeout's folder names follow the account's language). */
const UTF8_NAMES = 0x0800
/** 1980-01-01, the earliest date a zip can hold; the specs don't read it. */
const DOS_DATE = 0x21

/** The zip of `entries` (`[path, contents]`), listed in that order. */
export function zipBytes(entries: [string, string | Buffer][]): Buffer {
  const parts: Buffer[] = []
  const directory: Buffer[] = []
  let offset = 0
  for (const [name, contents] of entries) {
    const data =
      typeof contents === 'string' ? Buffer.from(contents, 'utf8') : contents
    const packed = deflateRawSync(data)
    const fileName = Buffer.from(name, 'utf8')
    const crc = crc32(data)

    const local = Buffer.alloc(30)
    local.writeUInt32LE(0x04034b50, 0)
    local.writeUInt16LE(20, 4)
    local.writeUInt16LE(UTF8_NAMES, 6)
    local.writeUInt16LE(DEFLATE, 8)
    local.writeUInt16LE(DOS_DATE, 12)
    local.writeUInt32LE(crc, 14)
    local.writeUInt32LE(packed.length, 18)
    local.writeUInt32LE(data.length, 22)
    local.writeUInt16LE(fileName.length, 26)

    const central = Buffer.alloc(46)
    central.writeUInt32LE(0x02014b50, 0)
    central.writeUInt16LE(20, 4)
    central.writeUInt16LE(20, 6)
    central.writeUInt16LE(UTF8_NAMES, 8)
    central.writeUInt16LE(DEFLATE, 10)
    central.writeUInt16LE(DOS_DATE, 14)
    central.writeUInt32LE(crc, 16)
    central.writeUInt32LE(packed.length, 20)
    central.writeUInt32LE(data.length, 24)
    central.writeUInt16LE(fileName.length, 28)
    central.writeUInt32LE(offset, 42)

    parts.push(local, fileName, packed)
    directory.push(central, fileName)
    offset += local.length + fileName.length + packed.length
  }
  const listing = Buffer.concat(directory)
  const end = Buffer.alloc(22)
  end.writeUInt32LE(0x06054b50, 0)
  end.writeUInt16LE(entries.length, 8)
  end.writeUInt16LE(entries.length, 10)
  end.writeUInt32LE(listing.length, 12)
  end.writeUInt32LE(offset, 16)
  return Buffer.concat([...parts, listing, end])
}

/** The value of a 32-bit field that is kept in the entry's ZIP64 field instead. */
const IN_ZIP64 = 0xffffffff

/** Where the directory starts and how many entries it has, from its end record. */
function directory(bytes: Buffer) {
  const end = bytes.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]))
  if (end < 0) throw new Error('not a zip: no end of its directory')
  const count = bytes.readUInt16LE(end + 10)
  const start = bytes.readUInt32LE(end + 16)
  if (count !== 0xffff && start !== IN_ZIP64) return { count, start }
  // A ZIP64 end record, found through the locator just before the end record.
  if (bytes.readUInt32LE(end - 20) !== 0x07064b50)
    throw new Error('not a zip: no ZIP64 end locator')
  const record = Number(bytes.readBigUInt64LE(end - 20 + 8))
  if (bytes.readUInt32LE(record) !== 0x06064b50)
    throw new Error('not a zip: a broken ZIP64 end record')
  return {
    count: Number(bytes.readBigUInt64LE(record + 32)),
    start: Number(bytes.readBigUInt64LE(record + 48)),
  }
}

/** The entries of a zip, in its directory's order: stored or deflated, UTF-8 names. */
export function zipEntries(bytes: Buffer): [string, Buffer][] {
  const { count, start } = directory(bytes)
  let at = start
  const entries: [string, Buffer][] = []
  for (let index = 0; index < count; index += 1) {
    if (bytes.readUInt32LE(at) !== 0x02014b50)
      throw new Error('not a zip: a broken directory entry')
    const method = bytes.readUInt16LE(at + 10)
    let packedSize = bytes.readUInt32LE(at + 20)
    let size = bytes.readUInt32LE(at + 24)
    const nameLength = bytes.readUInt16LE(at + 28)
    const extraLength = bytes.readUInt16LE(at + 30)
    const commentLength = bytes.readUInt16LE(at + 32)
    let localAt = bytes.readUInt32LE(at + 42)
    const name = bytes.toString('utf8', at + 46, at + 46 + nameLength)
    if (packedSize === IN_ZIP64 || size === IN_ZIP64 || localAt === IN_ZIP64) {
      // The ZIP64 field (id 1) holds, in this order, each of the three that
      // did not fit: the size, the packed size, where the entry starts.
      const extra = bytes.subarray(
        at + 46 + nameLength,
        at + 46 + nameLength + extraLength,
      )
      let field = 0
      while (field + 4 <= extra.length && extra.readUInt16LE(field) !== 1)
        field += 4 + extra.readUInt16LE(field + 2)
      if (field + 4 > extra.length)
        throw new Error(`${name}: a ZIP64 size without its ZIP64 field`)
      let value = field + 4
      const next = () => {
        const read = Number(extra.readBigUInt64LE(value))
        value += 8
        return read
      }
      if (size === IN_ZIP64) size = next()
      if (packedSize === IN_ZIP64) packedSize = next()
      if (localAt === IN_ZIP64) localAt = next()
    }
    if (method !== 0 && method !== DEFLATE)
      throw new Error(`${name}: compression ${method} is not read here`)
    const dataAt =
      localAt +
      30 +
      bytes.readUInt16LE(localAt + 26) +
      bytes.readUInt16LE(localAt + 28)
    const packed = bytes.subarray(dataAt, dataAt + packedSize)
    const data =
      method === DEFLATE ? inflateRawSync(packed) : Buffer.from(packed)
    if (data.length !== size)
      throw new Error(
        `${name}: ${data.length} bytes, its directory says ${size}`,
      )
    entries.push([name, data])
    at += 46 + nameLength + extraLength + commentLength
  }
  return entries
}
