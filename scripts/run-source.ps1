$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
& (Join-Path $PSScriptRoot 'tooling-bootstrap.ps1') source powershell $root @args
exit $LASTEXITCODE
