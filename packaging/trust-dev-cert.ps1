<#
  Makes this PC trust the Khipu Dev certificate, so Windows installs the Dev package. Once per PC.

  Windows installs a signed MSIX only when its certificate is in the machine's Trusted People store,
  and writing there needs administrator: this script asks for it (one UAC prompt). It trusts that
  one certificate, made on this PC by build-msix.ps1, and nothing else: no Developer Mode, no
  change to how Windows treats any other package.
#>
$ErrorActionPreference = "Stop"
$cer = Join-Path (Split-Path -Parent $PSScriptRoot) "target\msix\khipu-dev.cer"
if (-not (Test-Path $cer)) {
    throw "No certificate yet: run packaging\build-msix.ps1 -Channel Dev first; it makes one."
}

$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).
    IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) {
    Start-Process powershell -Verb RunAs -Wait -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
    return
}
Import-Certificate -FilePath $cer -CertStoreLocation Cert:\LocalMachine\TrustedPeople | Out-Null
Write-Host "Khipu Dev certificate trusted on this PC."
