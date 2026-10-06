$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$location = Get-Location
try {
    Set-Location -LiteralPath $root
    $rustc = & rustup which rustc
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally { Set-Location -LiteralPath $location.Path }
$suffix = if ($IsWindows) { '.exe' } else { '' }
$output = Join-Path $root "target/repository-tools$suffix"
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $output) | Out-Null
$sources = @(Get-Item -LiteralPath (Join-Path $root 'crates/maestro-test-conventions/src/bin/repository_tools.rs'))
$sources += Get-ChildItem -LiteralPath (Join-Path $root 'crates/maestro-test-conventions/src/repository_tools') -Filter '*.rs'
$rebuild = -not (Test-Path -LiteralPath $output -PathType Leaf)
if (-not $rebuild) {
    $stamp = (Get-Item -LiteralPath $output).LastWriteTimeUtc
    $rebuild = @($sources | Where-Object { $_.LastWriteTimeUtc -gt $stamp }).Count -gt 0
}
if ($rebuild) {
    $temporary = Join-Path (Split-Path -Parent $output) "tooling-bootstrap.$([guid]::NewGuid().ToString('N'))"
    New-Item -ItemType Directory -Path $temporary | Out-Null
    try {
        $compiled = Join-Path $temporary "repository-tools$suffix"
        & $rustc --edition=2024 (Join-Path $root 'crates/maestro-test-conventions/src/bin/repository_tools.rs') -o $compiled
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        [System.IO.File]::Move($compiled, $output, $true)
    } finally {
        Remove-Item -LiteralPath $temporary -Recurse -Force
    }
}
$start = [System.Diagnostics.ProcessStartInfo]::new()
$start.FileName = $output
$start.UseShellExecute = $false
foreach ($argument in $args) { $start.ArgumentList.Add([string]$argument) }
$child = [System.Diagnostics.Process]::Start($start)
$child.WaitForExit()
exit $child.ExitCode
