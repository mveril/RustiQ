[CmdletBinding()]
param(
    [switch]$Open,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$RemainingArguments
)

$ErrorActionPreference = 'Stop'

foreach ($argument in $RemainingArguments) {
    if ($argument -ne '--open') {
        throw "Usage: .\\tools\\generate-rustdoc.ps1 [-Open|--open]"
    }
    $Open = $true
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$headerPath = Join-Path $repoRoot 'crates/rustiq-core/docs/rustdoc-mathjax.html'
$cargoArguments = @('rustdoc', '--package', 'rustiq-core')

if ($Open) {
    $cargoArguments += '--open'
}

$cargoArguments += '--', '--html-in-header', $headerPath

Push-Location $repoRoot
try {
    & cargo @cargoArguments
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}
finally {
    Pop-Location
}
