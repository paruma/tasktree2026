param([switch]$Run)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$buildRoot = Join-Path $env:TEMP 'tasktree2026-windows-build'
$outputRoot = Join-Path $projectRoot 'windows-app'

# WSL 内のソースを正本とし、Windows ローカルディスクでコンパイルする。
New-Item -ItemType Directory -Force -Path "$buildRoot\src-tauri", "$buildRoot\dist" | Out-Null
$html = [IO.File]::ReadAllText((Join-Path $projectRoot 'index.html'))
$html = [regex]::Replace($html, '<meta http-equiv="Content-Security-Policy"[^>]*>\r?\n?', '')
[IO.File]::WriteAllText("$buildRoot\dist\index.html", $html, [Text.UTF8Encoding]::new($false))
foreach ($name in @('Cargo.toml', 'Cargo.lock', 'build.rs', 'tauri.conf.json')) {
    Copy-Item "$projectRoot\src-tauri\$name" "$buildRoot\src-tauri\$name" -Force
}
foreach ($name in @('src', 'icons', 'capabilities', 'permissions')) {
    if (Test-Path "$buildRoot\src-tauri\$name") { Remove-Item "$buildRoot\src-tauri\$name" -Recurse -Force }
    Copy-Item "$projectRoot\src-tauri\$name" "$buildRoot\src-tauri\$name" -Recurse -Force
}
Push-Location $buildRoot
try {
    & cargo test --manifest-path src-tauri/Cargo.toml --locked --features custom-protocol
    if ($LASTEXITCODE -ne 0) { throw 'Windows tests failed.' }
    & cargo build --manifest-path src-tauri/Cargo.toml --release --locked --features custom-protocol
    if ($LASTEXITCODE -ne 0) { throw 'Windows build failed.' }
} finally { Pop-Location }
New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null
Copy-Item "$buildRoot\src-tauri\target\release\tasktree.exe" "$outputRoot\tasktree.exe" -Force
Write-Output "Built: $outputRoot\tasktree.exe"
if ($Run) {
    Start-Process -FilePath "$buildRoot\src-tauri\target\release\tasktree.exe" -WorkingDirectory $buildRoot
}
