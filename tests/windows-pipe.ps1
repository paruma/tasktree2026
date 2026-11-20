$ErrorActionPreference = 'Stop'
function Invoke-TaskPipe($request) {
    $sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    $pipe = [IO.Pipes.NamedPipeClientStream]::new('.', "paruma.tasktree2026.$sid", [IO.Pipes.PipeDirection]::InOut)
    try {
        $pipe.Connect(5000)
        $encoding = [Text.UTF8Encoding]::new($false)
        $writer = [IO.StreamWriter]::new($pipe, $encoding, 1024, $true)
        $reader = [IO.StreamReader]::new($pipe, $encoding, $false, 1024, $true)
        $writer.WriteLine(($request | ConvertTo-Json -Depth 20 -Compress))
        $writer.Flush()
        $pending = $reader.ReadLineAsync()
        if (-not $pending.Wait(15000)) { throw 'Pipe response timed out' }
        $text = $pending.Result
        if (-not $text) { throw 'Empty pipe response' }
        return ($text | ConvertFrom-Json)
    } finally {
        if ($writer) { $writer.Dispose() }
        if ($reader) { $reader.Dispose() }
        $pipe.Dispose()
    }
}
function Assert-True($value, $label) { if (-not $value) { throw "Failed: $label" }; Write-Output "PASS: $label" }
$original = Invoke-TaskPipe @{op='get'}
Assert-True $original.ok 'get'
$group = Invoke-TaskPipe @{op='add';title='Desktop pipe test'}
Assert-True $group.ok 'add'
$id = $group.result
try {
    $child = Invoke-TaskPipe @{op='add';title='Child from named pipe';minutes=15;parentId=$id}
    Assert-True $child.ok 'nested add'
    $changed = Invoke-TaskPipe @{op='update';id=$child.result;patch=@{state=2}}
    Assert-True $changed.ok 'state update'
    $moved = Invoke-TaskPipe @{op='move';id=$child.result;parentId=$null}
    Assert-True $moved.ok 'outdent'
    $undone = Invoke-TaskPipe @{op='undo'}
    Assert-True $undone.ok 'undo'
    $tree = Invoke-TaskPipe @{op='get'}
    $parent = $tree.result | Where-Object { $_.id -eq $id }
    Assert-True ($parent.children[0].state -eq 2 -and $parent.children[0].id -eq $child.result) 'tree and undo result'
    $bad = Invoke-TaskPipe @{op='eval';code='alert(1)'}
    Assert-True (-not $bad.ok) 'reject arbitrary operation'
    $bad = Invoke-TaskPipe @{op='get';unexpected='x'}
    Assert-True (-not $bad.ok) 'reject unexpected fields'
    $bad = Invoke-TaskPipe @{op='move';id=$id;parentId=$child.result}
    Assert-True (-not $bad.ok) 'reject cycle'
} finally {
    $removed = Invoke-TaskPipe @{op='remove';id=$id}
    Assert-True $removed.ok 'cleanup'
}
$final = Invoke-TaskPipe @{op='get'}
Assert-True (($original.result | ConvertTo-Json -Depth 80 -Compress) -eq ($final.result | ConvertTo-Json -Depth 80 -Compress)) 'original tasks preserved'
