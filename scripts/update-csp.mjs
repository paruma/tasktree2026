import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';

// HTML 内のスクリプトを変更した後に実行し、許可するコードのハッシュを更新する。
const path = new URL('../index.html', import.meta.url);
let html = readFileSync(path, 'utf8');
const script = html.match(/<script>([\s\S]*?)<\/script>/)?.[1];
if (!script) throw new Error('スクリプトが見つかりません');
const hash = createHash('sha256').update(script).digest('base64');
const policy = `default-src 'none'; script-src 'sha256-${hash}'; script-src-attr 'none'; style-src 'unsafe-inline'; connect-src 'none'; base-uri 'none'; form-action 'none'; object-src 'none'`;
const tag = `<meta http-equiv="Content-Security-Policy" content="${policy}">`;
html = html.includes('http-equiv="Content-Security-Policy"')
  ? html.replace(/<meta http-equiv="Content-Security-Policy"[^>]*>/, tag)
  : html.replace('<meta charset="utf-8">', '<meta charset="utf-8">\n' + tag);
writeFileSync(path, html);
