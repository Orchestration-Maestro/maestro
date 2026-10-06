$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'tooling-bootstrap.ps1') rustdoc @args
exit $LASTEXITCODE
