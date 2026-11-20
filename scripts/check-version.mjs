import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';

const root = new URL('../', import.meta.url);
const version = JSON.parse(readFileSync(new URL('package.json', root))).version;
const tauri = JSON.parse(readFileSync(new URL('src-tauri/tauri.conf.json', root))).version;
const cargo = readFileSync(new URL('src-tauri/Cargo.toml', root), 'utf8').match(/^version = "([^"]+)"/m)?.[1];
assert.equal(tauri, version, 'Tauri のバージョンが一致していません');
assert.equal(cargo, version, 'Cargo のバージョンが一致していません');
if (process.env.GITHUB_REF_TYPE === 'tag') {
  assert.equal(process.env.GITHUB_REF_NAME, version, 'タグとバージョンが一致していません');
}
console.log(`Version: ${version}`);
