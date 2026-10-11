/**
 * Writes a .zip the way Google Takeout sends one: deflated entries, UTF-8
 * names, in the order given. Only what the specs need: no folder entries, no
 * ZIP64, nothing over 4 GB.
 */
import { crc32, deflateRawSync } from 'node:zlib'

const DEFLATE = 8
/** Bit 11: the names are UTF-8 (Takeout's folder names follow the account's language). */
const UTF8_NAMES = 0x0800
/** 1980-01-01, the earliest date a zip can hold; the specs don't read it. */
const DOS_DATE = 0x21

/** The zip of `entries` (`[path, contents]`), listed in that order. */
export function zipBytes(entries: [string, string][]): Buffer {
  const parts: Buffer[] = []
  const directory: Buffer[] = []
  let offset = 0
  for (const [name, contents] of entries) {
    const data = Buffer.from(contents, 'utf8')
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
