const { test } = require('node:test');
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { createHash, webcrypto } = require('node:crypto');
const vm = require('node:vm');

const html = readFileSync(require('node:path').join(__dirname, '../index.html'), 'utf8');
const script = html.match(/<script>([\s\S]*?)<\/script>/)[1].replace(/\r\n?/g, '\n');

// ブラウザ専用 API には触れず、公開されたタスク操作と描画内容を検証する。
function app(options = {}) {
  class Element {
    constructor(tag = 'div') {
      this.tag = tag;
      this.children = [];
      this.style = { setProperty() {} };
      this.dataset = {};
      this.classList = { add() {}, remove() {} };
      this.events = {};
      this.attributes = {};
      this.value = '';
      this.scrollHeight = 32;
    }
    append(...items) { this.children.push(...items); }
    replaceChildren() { this.children = []; }
    setAttribute(name, value) { this.attributes[name] = value; }
    querySelector(selector) {
      return this.children.find(child => child.className?.split(' ').includes(selector.slice(1))) ?? null;
    }
    addEventListener(name, handler) { this.events[name] = handler; }
    focus() {}
    blur() {}
    getBoundingClientRect() { return { left: 0, top: 0, height: 40 }; }
  }
  const elements = {};
  let saved;
  let failSave = false;
  let channel;
  const replies = [];
  const document = {
    getElementById: id => elements[id] ??= new Element(),
    createElement: tag => new Element(tag),
    querySelector: () => null,
    querySelectorAll: () => [],
    addEventListener() {},
    documentElement: new Element(),
  };
  const context = {
    document,
    window: {
      addEventListener() {},
      ...(options.desktop ? { __TAURI__: { core: {
        Channel: class {},
        invoke(command, args) {
          if (command === 'connect_pipe') channel = args.channel;
          if (command === 'complete_pipe_request') replies.push(args.reply);
          return Promise.resolve();
        },
      } } } : {}),
    },
    crypto: webcrypto,
    localStorage: {
      getItem(key) { return key === 'tasktree2026.v1' ? options.saved ?? null : null; },
      setItem(key, value) { if (failSave) throw Error('QuotaExceededError'); saved = value; },
    },
  };
  vm.createContext(context);
  vm.runInContext(script, context);
  if (options.saved === undefined) context.taskTree.replace([]);
  return { api: context.taskTree, elements, context, saved: () => saved, failSave: value => { failSave = value; }, channel: () => channel, replies };
}

test('CSP は現在のコードだけを許可し、外部通信と任意スクリプトを許可しない', () => {
  const hash = createHash('sha256').update(script).digest('base64');
  const policy = html.match(/http-equiv="Content-Security-Policy" content="([^"]+)"/)[1];
  assert.ok(policy.includes(`script-src 'sha256-${hash}'`), 'node scripts/update-csp.mjs を実行してください');
  assert.ok(policy.includes("default-src 'none'"));
  assert.ok(policy.includes("connect-src 'none'"));
  assert.ok(policy.includes("script-src-attr 'none'"));
  assert.ok(!policy.includes('unsafe-eval'));
});

test('初期値・部分木の移動・集計・取り消し・保存', () => {
  const { api, elements, saved } = app();
  const parent = api.add({ title: '親' });
  const child = api.add({ title: '子', minutes: 15 });
  assert.equal(api.get()[0].minutes, 30);
  api.move(child, { parentId: parent });
  assert.equal(elements.total.textContent, '15分');
  api.update(child, { state: 2 });
  assert.equal(elements.tree.children[0].children[1].className, 'status done');
  api.move(child, {});
  assert.equal(api.get().length, 2);
  api.undo();
  assert.equal(api.get()[0].children[0].id, child);
  api.redo();
  assert.equal(api.get().length, 2);
  assert.equal(JSON.parse(saved()).length, 2);
});

test('通常版は既存タスクの子を開閉し、保存データと集計を保つ', () => {
  const saved = JSON.stringify([{
    id: 'parent', title: '親', minutes: null, state: 0, children: [{
      id: 'child', title: '子', minutes: 15, state: 0, children: [],
    }],
  }]);
  for (const desktop of [false, true]) {
    const { api, elements, saved: savedAfter } = app({ desktop, saved });
    const [parent, child] = elements.tree.children;
    const handle = parent.querySelector('.handle');
    assert.equal(handle.tag, 'button');
    assert.equal(handle.attributes['aria-expanded'], 'true');
    handle.onclick();
    assert.equal(handle.attributes['aria-expanded'], 'false');
    assert.equal(child.hidden, true);
    assert.equal(elements.total.textContent, '15分');
    assert.equal(JSON.stringify(api.get()), saved);
    assert.equal(savedAfter(), undefined);
    handle.onclick();
    assert.equal(handle.attributes['aria-expanded'], 'true');
    assert.equal(child.hidden, false);
  }
});

test('展開中の親の下半分と最初の子の上半分は同じ挿入先になる', () => {
  const node = (id, children = []) => ({ id, title: id, minutes: 1, state: 0, children });
  for (const [targetIndex, y] of [[0, 30], [1, 1]]) {
    const { api, elements } = app();
    api.replace([node('a', [node('b')]), node('c')]);
    const [parent, child, source] = elements.tree.children;
    source.querySelector('.handle').events.dragstart({ clientX: 0, dataTransfer: { setData() {} } });
    [parent, child][targetIndex].events.drop({
      clientX: 35, clientY: y, preventDefault() {}, stopPropagation() {},
    });
    assert.deepEqual(Array.from(api.get()[0].children, task => task.id), ['c', 'b']);
  }
});

test('末尾のタスクを末尾にドロップしても履歴は増えない', () => {
  const { api, elements } = app();
  api.add({ title: '先頭' });
  api.add({ title: '末尾' });
  const handle = elements.tree.children[1].querySelector('.handle');
  handle.events.dragstart({ clientX: 0, dataTransfer: { setData() {} } });
  elements['draft-row'].events.drop({ preventDefault() {} });
  api.undo();
  assert.deepEqual(Array.from(api.get(), task => task.title), ['先頭']);
});

test('Tab で階層の上限を超える移動は保存しない', () => {
  const { api, elements, saved } = app();
  const node = (id, children = []) => ({ id, title: id, minutes: 1, state: 0, children });
  let branch = node('level-50');
  for (let level = 49; level >= 1; level--) branch = node(`level-${level}`, [branch]);
  const tree = [node('target'), branch];
  api.replace(tree);
  elements.tree.children[1].querySelector('.name').events.keydown({
    key: 'Tab', preventDefault() {},
  });
  assert.equal(JSON.stringify(api.get()), JSON.stringify(tree));
  assert.equal(saved(), JSON.stringify(tree));
  assert.match(elements.notice.textContent, /階層が深すぎ/);
});

test('不正入力・循環・不正な挿入先は既存データを変更しない', () => {
  const { api } = app();
  const parent = api.add({ title: '親' });
  const child = api.add({ title: '子', parentId: parent });
  const before = JSON.stringify(api.get());
  for (const invalid of [
    () => api.move(parent, { parentId: child }),
    () => api.move(child, { beforeId: 'missing' }),
    () => api.update(child, { minutes: -1 }),
    () => api.update(child, { state: 9 }),
    () => api.update(parent, { state: 2 }),
    () => api.update(child, { children: [] }),
    () => api.add({ title: 123 }),
    () => api.replace([{ title: '不正', children: [] }]),
  ]) {
    assert.throws(invalid);
    assert.equal(JSON.stringify(api.get()), before);
  }
});

test('タスク名を HTML として描画せず、取得結果は内部データから分離する', () => {
  const { api, elements, context } = app();
  const payload = '<img src=x onerror=alert(1)><script>alert(1)</script>';
  api.add({ title: payload });
  const field = elements.tree.children[0].children[2];
  assert.equal(field.tag, 'textarea');
  assert.equal(field.value, payload);
  assert.equal(field.children.length, 0);
  const copy = api.get();
  copy[0].title = '外部からの変更';
  assert.equal(api.get()[0].title, payload);
  assert.equal(context.nodes, undefined);
  assert.equal(context.mutate, undefined);
  assert.ok(Object.isFrozen(api));
});


test('保存失敗時は外部操作を成功扱いにせず、タスクと履歴を戻す', () => {
  const { api, failSave, saved } = app();
  api.add({ title: '保存済み' });
  const before = JSON.stringify(api.get());
  const stored = saved();
  failSave(true);
  assert.throws(() => api.add({ title: '保存失敗' }), /保存/);
  assert.equal(JSON.stringify(api.get()), before);
  assert.equal(saved(), stored);
  assert.throws(() => api.undo(), /保存/);
  assert.equal(JSON.stringify(api.get()), before);
  failSave(false);
  api.undo();
  assert.equal(api.get().length, 0);
});

test('デスクトップ要求は固定操作だけを実行し、保存後に成功を返す', () => {
  const { api, channel, replies, failSave } = app({ desktop: true });
  const send = request => channel().onmessage({ id: 1, deadline: Date.now() + 10000, request });
  send({ op: 'add', title: 'パイプから追加' });
  assert.equal(replies.at(-1).ok, true);
  assert.equal(api.get()[0].title, 'パイプから追加');
  const before = JSON.stringify(api.get());
  send({ op: 'eval', code: 'alert(1)' });
  assert.equal(replies.at(-1).ok, false);
  channel().onmessage({ id: 2, deadline: 0, request: { op: 'remove', id: api.get()[0].id } });
  assert.equal(replies.at(-1).ok, false);
  failSave(true);
  send({ op: 'add', title: '保存できない' });
  assert.equal(replies.at(-1).ok, false);
  assert.equal(JSON.stringify(api.get()), before);
});
