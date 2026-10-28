<#
  Updates Khipu Dev, the copy where every change lands and Alex tries it: brings in what is on
  GitHub, builds Khipu Dev and installs it over the one installed. Expansions and settings live in
  the package's LocalState, which an update never touches. Never touches the stable Khipu from
  the Store. If Khipu Dev was open, Windows closes it to update it and this opens it again.

    powershell -ExecutionPolicy Bypass -File packaging\update-dev.ps1 [-NoPull]
#>
param([switch]$NoPull)
$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot

Push-Location $repo
try {
    if (-not $NoPull) {
        git pull --ff-only
        if ($LASTEXITCODE -ne 0) { throw "git pull failed" }
    }
    $wasOpen = Get-Process Khipu -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -like "*\AlexPalacios.Khipu.Dev_*" }
    & (Join-Path $PSScriptRoot "build-msix.ps1") -Channel Dev
    $package = Get-AppxPackage -Name "AlexPalacios.Khipu.Dev"
    if ($wasOpen) { Start-Process "explorer.exe" "shell:AppsFolder\$($package.PackageFamilyName)!App" }
    Write-Host "Khipu Dev $($package.Version)$(if ($wasOpen) { ', reopened' })"
} finally {
    Pop-Location
}
