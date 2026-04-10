import { readFileSync, writeFileSync } from 'fs'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const pkg = JSON.parse(readFileSync(resolve(__dirname, '../package.json'), 'utf-8'))

const tauriConfPath = resolve(__dirname, '../src-tauri/tauri.conf.json')
const tauriConf = JSON.parse(readFileSync(tauriConfPath, 'utf-8'))
tauriConf.version = pkg.version
writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n')

const cargoPath = resolve(__dirname, '../src-tauri/Cargo.toml')
let cargo = readFileSync(cargoPath, 'utf-8')
cargo = cargo.replace(/^version\s*=\s*".*"/m, `version = "${pkg.version}"`)
writeFileSync(cargoPath, cargo)

console.log(`Synced version to ${pkg.version}`)
