$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'tooling-bootstrap.ps1') @args
exit $LASTEXITCODE
