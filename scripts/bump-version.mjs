import { promises as fs } from 'node:fs'
import path from 'node:path'

const cwd = process.cwd()
const nextVersion = process.argv[2]?.trim()
const semverPattern = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/

if (!nextVersion || !semverPattern.test(nextVersion)) {
  console.error('Usage: bun run release:bump -- <semver>')
  process.exit(1)
}

const packageJsonPath = path.join(cwd, 'package.json')
const cargoTomlPath = path.join(cwd, 'src-tauri', 'Cargo.toml')
const tauriConfigPath = path.join(cwd, 'src-tauri', 'tauri.conf.json')

const packageJson = JSON.parse(await fs.readFile(packageJsonPath, 'utf8'))
if (packageJson.version !== nextVersion) {
  packageJson.version = nextVersion
  await fs.writeFile(
    packageJsonPath,
    `${JSON.stringify(packageJson, null, 2)}\n`,
  )
}

const cargoToml = await fs.readFile(cargoTomlPath, 'utf8')
const packageSection = cargoToml.match(/\[package\][\s\S]*?(?=^\[|\Z)/m)?.[0]

if (!packageSection) {
  console.error('Could not find the [package] section in src-tauri/Cargo.toml.')
  process.exit(1)
}

const nextPackageSection = packageSection.replace(
  /^version = ".*"$/m,
  `version = "${nextVersion}"`,
)

if (
  nextPackageSection === packageSection &&
  !packageSection.includes(`version = "${nextVersion}"`)
) {
  console.error('Could not update the package version in src-tauri/Cargo.toml.')
  process.exit(1)
}

if (nextPackageSection !== packageSection) {
  await fs.writeFile(
    cargoTomlPath,
    cargoToml.replace(packageSection, nextPackageSection),
  )
}

const tauriConfig = JSON.parse(await fs.readFile(tauriConfigPath, 'utf8'))
if (tauriConfig.version !== nextVersion) {
  tauriConfig.version = nextVersion
  await fs.writeFile(
    tauriConfigPath,
    `${JSON.stringify(tauriConfig, null, 2)}\n`,
  )
}

console.log(
  `Bumped package.json, src-tauri/Cargo.toml, and src-tauri/tauri.conf.json to ${nextVersion}.`,
)
