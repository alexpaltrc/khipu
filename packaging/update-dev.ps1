<#
  "Actualizar Khipu", the shortcut on Alex's desktop: brings in what was done on GitHub (from the
  cloud or elsewhere), builds Khipu Dev, installs it over the one in use and opens it again.
  Expansions and settings live in the package's LocalState, which an update never touches.

    powershell -ExecutionPolicy Bypass -File packaging\update-dev.ps1 [-NoPull] [-NoPause]
#>
param([switch]$NoPull, [switch]$NoPause)
$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$Host.UI.RawUI.WindowTitle = "Actualizar Khipu"

function Finish([bool]$ok, [string]$message) {
    Write-Host ""
    Write-Host $message -ForegroundColor $(if ($ok) { "Green" } else { "Red" })
    if (-not $NoPause) { Write-Host ""; Read-Host "Pulsa Enter para cerrar" | Out-Null }
    exit $(if ($ok) { 0 } else { 1 })
}

try {
    Push-Location $repo
    if (-not $NoPull) {
        Write-Host "Bajando los cambios de GitHub..."
        git pull --ff-only
        if ($LASTEXITCODE -ne 0) { throw "no se pudieron bajar los cambios de GitHub (git pull)" }
    }
    Write-Host "Fabricando e instalando Khipu Dev..."
    & (Join-Path $PSScriptRoot "build-msix.ps1") -Channel Dev

    $package = Get-AppxPackage -Name "AlexPalacios.Khipu.Dev"
    Start-Process "explorer.exe" "shell:AppsFolder\$($package.PackageFamilyName)!App"
    Finish $true "Listo: Khipu Dev $($package.Version) instalada y abierta."
} catch {
    Finish $false "No se pudo actualizar: $($_.Exception.Message)"
} finally {
    Pop-Location
}
