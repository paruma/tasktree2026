# デスクトップ版

Windows 用の Tauri 2 アプリです。ブラウザ版と同じ `index.html` を同梱し、保存先はアプリ専用 WebView の localStorage です。Web 版との同期・自動移行は行いません。

## 起動・ビルド

Windows の Rust / MSVC ビルドツール / WebView2 が必要です。

WSL にあるこのリポジトリから Windows 版を作る場合:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File \\wsl.localhost\Ubuntu\home\paruma\dev\tasktree2026\scripts\windows.ps1 -Run
```

Windows の一時ディレクトリでデバッグビルドを行い、`windows-app/tasktree.exe` にコピーします。ビルド済みアプリやタスクデータは Git 管理しません。配布用ビルドは `-Release`、Rust テストも実行する場合は `-Test` を付けます。通常の UI 確認にはデバッグ版を使います。

通常の Windows チェックアウトで開発する場合は、`pnpm install --frozen-lockfile` の後に `pnpm desktop`、配布形式を生成する場合は `pnpm desktop:build` を使います。

Ctrl + ＋（＝）で拡大、Ctrl + − で縮小、Ctrl + 0 で元の倍率に戻せます。Ctrl + マウスホイールでも倍率を変更できます。

背景部分を右クリックすると「常に最前面に表示」を切り替えられます。「操作方法」でショートカット一覧を開けます。文字入力欄では通常の編集メニューを保ちます。最前面の設定は起動ごとに解除されます。

## 名前付きパイプ

アプリ起動中だけ、同じ Windows ユーザーからタスク操作を受け付けます。HTTP サーバー、CLI、MCP は含みません。

- 名前: `\\.\pipe\paruma.tasktree2026.<Windows ユーザー SID>`
- 通信: 1接続につき1要求・1応答。UTF-8 の JSON を1行送り、改行で終端します。BOM は付けません。
- 要求の上限: 改行を含め64 KiB。受信タイムアウト5秒、画面からの応答待ち10秒。
- 応答: `{"ok":true,"result":...}` または `{"ok":false,"error":"..."}`。
- 複数要求は画面の JavaScript 上で順に処理され、書き込みの成功応答は localStorage への保存成功後に返ります。
- タイムアウトや切断時は操作済みの可能性もあります。書き込みを自動再送せず、`get` で結果を確認してください。

要求例:

```json
{"op":"get"}
{"op":"add","title":"買い物","minutes":30}
{"op":"add","title":"野菜","parentId":"親のID"}
{"op":"update","id":"タスクのID","patch":{"state":2}}
{"op":"update","id":"タスクのID","patch":{"title":"変更後の名前","minutes":15}}
{"op":"move","id":"タスクのID","parentId":null,"beforeId":"移動先のタスクのID"}
{"op":"remove","id":"タスクのID"}
{"op":"undo"}
{"op":"redo"}
```

`get` はツリーの配列、`add` は新規 ID、その他の成功結果は `null` を返します。状態は `0: TODO / 1: DOING / 2: DONE` です。親の時間・状態は直接更新できません。`move` の `parentId` を省略または `null` にすると最上位へ、`beforeId` を省略すると移動先の末尾へ移動します。`remove` は子孫ごと削除します。

PowerShell からの接続例:

```powershell
$sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$pipe = [IO.Pipes.NamedPipeClientStream]::new('.', "paruma.tasktree2026.$sid", [IO.Pipes.PipeDirection]::InOut)
try {
    $pipe.Connect(3000)
    $encoding = [Text.UTF8Encoding]::new($false)
    $writer = [IO.StreamWriter]::new($pipe, $encoding, 1024, $true)
    $reader = [IO.StreamReader]::new($pipe, $encoding, $false, 1024, $true)
    $writer.WriteLine('{"op":"get"}')
    $writer.Flush()
    $reader.ReadLine()
} finally {
    if ($writer) { $writer.Dispose() }
    if ($reader) { $reader.Dispose() }
    $pipe.Dispose()
}
```

DACL を明示して同じユーザーだけを許可し、リモート接続も拒否します。同じユーザー権限で動くプログラムは操作できます。ユーザー権限内の悪意あるプロセスを隔離する機能ではありません。コード実行・任意ファイル操作は受け付けません。

## 接続テスト

アプリを起動した状態で `powershell.exe -NoProfile -ExecutionPolicy Bypass -File tests/windows-pipe.ps1` を実行できます。テスト用の親子タスクを追加して操作し、最後に削除して元のタスクが残っていることを確認します。

## CI と配布

- main への push・PR・手動実行: テストと Windows ビルドを行い、実行ファイル・第三者ライセンス・対応ソースを含む ZIP と SHA-256 を Actions の成果物に保存。
- `0.1.0` のようなタグの push: 上記に加えて GitHub Release を作成。タグは package.json / Cargo.toml / tauri.conf.json のバージョンと一致させます。

普段の動作確認にタグは不要です。利用者向けに版を確定するときだけタグを付けます。
