import { mkdir, readFile, writeFile } from 'node:fs/promises';

// 同じ画面を同梱し、デスクトップ版の CSP は Tauri に生成させる。
const root = new URL('../', import.meta.url);
const html = await readFile(new URL('index.html', root), 'utf8');
await mkdir(new URL('dist/', root), { recursive: true });
await writeFile(new URL('dist/index.html', root),
  html.replace(/<meta http-equiv="Content-Security-Policy"[^>]*>\n?/, ''));
