<#
  Builds the installed (Microsoft Store) edition: an MSIX package.

    powershell -ExecutionPolicy Bypass -File packaging\build-msix.ps1            # build the .msix
    powershell -ExecutionPolicy Bypass -File packaging\build-msix.ps1 -Register  # and install it here

  The package is left unsigned. The Store signs what is uploaded to it; -Register installs the
  unpacked layout directly, which Windows allows only with Developer Mode on.

  espansod.exe, its DLLs and the starter .espanso/.espanso-runtime come from the newest portable
  ZIP in paquetes\, the same pristine copy every portable user gets. Never from the development
  folder espanso-portable\: that one holds Alex's own expansions.

  Identity and publisher are the ones Partner Center assigned when "Khipu" was reserved
  (2026-10-05, Product identity page); the Store accepts no package that carries other values.
  It is a different package from the EspansoManager.Dev one registered on Alex's PC before that,
  so Windows keeps them apart, each with its own LocalState. To update that older copy instead:
    -Name EspansoManager.Dev -Publisher "CN=Alex Palacios" -Revision <higher> -Register
  Do not -Register the Store identity on a PC that will install Khipu from the Store: the
  developer copy would stand in the way of the signed one.
#>
param(
    [string]$Name = "AlexPalacios.Khipu",
    [string]$Publisher = "CN=A3B4D4CD-AECB-4B6D-9268-AEAB19F8F17B",
    [string]$PublisherDisplayName = "Alex Palacios",
    [string]$DisplayName = "Khipu",
    [string]$Description = "Text expansions without touching a config file.",
    [string]$RuntimeZip = "",
    # The fourth number of the package version. Windows refuses to register a changed manifest
    # under a version it already has installed, and uninstalling first would delete LocalState;
    # a higher revision is an update instead, which keeps it.
    [int]$Revision = 0,
    [switch]$Register
)
$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$paquetes = Join-Path $repo "..\..\paquetes" | Resolve-Path
$exeName = "$DisplayName.exe"

# --- Version: Cargo's 0.0.5-dev becomes the four numbers MSIX wants, 0.0.5.0 (or 0.0.5.<Revision>).
$cargoVersion = (Select-String -Path (Join-Path $repo "Cargo.toml") -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$numeric = ($cargoVersion -split '-')[0]
$msixVersion = "$numeric.$Revision"

# --- Build.
Push-Location $repo
try { cargo build --release; if ($LASTEXITCODE -ne 0) { throw "cargo build failed" } } finally { Pop-Location }

# --- Stage the layout. Kept under target\ so -Register has a folder that stays put. A build that
# only packs gets a folder of its own: the registered copy runs out of layout\, and packing the
# Store identity there would close it and swap its files from under it.
$stage = Join-Path $repo $(if ($Register) { "target\msix\layout" } else { "target\msix\pack" })
# A copy registered with -Register runs straight out of this folder, espansod included, and a
# running .exe cannot be deleted. Only processes started from here are closed.
Get-Process | Where-Object { $_.Path -like "$stage\*" } | Stop-Process -Force
Start-Sleep -Milliseconds 500
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
$res = Join-Path $stage "Program Files"
$seed = Join-Path $res "seed"
New-Item -ItemType Directory -Force $res, $seed, (Join-Path $stage "Assets") | Out-Null
Copy-Item (Join-Path $repo "target\release\Khipu.exe") (Join-Path $stage $exeName)

# --- espanso, from the portable ZIP.
if ($RuntimeZip -eq "") {
    $RuntimeZip = (Get-ChildItem $paquetes -Filter "Khipu Portable *.zip" | Sort-Object LastWriteTime | Select-Object -Last 1).FullName
}
Write-Host "espanso from: $RuntimeZip"
$unzip = Join-Path $repo "target\msix\portable"
if (Test-Path $unzip) { Remove-Item -Recurse -Force $unzip }
Expand-Archive $RuntimeZip $unzip
$pf = Get-ChildItem $unzip -Directory | ForEach-Object { Join-Path $_.FullName "Program Files" } | Select-Object -First 1
Copy-Item (Join-Path $pf "espansod.exe"), (Join-Path $pf "LICENSE.txt") $res
Copy-Item (Join-Path $pf "*.dll") $res
# Whose work is in the package, under which licence, and where espanso's source is
# (third-party-notices.ps1). Beside LICENSE.txt, the GPL text Khipu and espanso share.
Copy-Item (Join-Path $PSScriptRoot "THIRD-PARTY-NOTICES.txt") $res
# The seed: copied into the user's LocalState on first start (install.rs). Not the portable's
# README or .bat files: they describe a folder this edition does not have.
Copy-Item -Recurse (Join-Path $pf ".espanso"), (Join-Path $pf ".espanso-runtime") $seed

# --- Logos: the identity pack's own, as delivered (assets\khipu\msix\Assets). Each one is drawn
# from the vector at its exact size, every scale and every size the taskbar, Start's search and
# the list of apps ask for. Only qualified files: resources.pri (below) tells Windows which one to
# take; without it Windows finds none of them, which is what Start showed until 0.0.5.
Copy-Item (Join-Path $repo "assets\khipu\msix\Assets\*.png") (Join-Path $stage "Assets")

# --- Manifest. XML-escaped, since names and descriptions are free text.
function X([string]$s) { [System.Security.SecurityElement]::Escape($s) }
$manifest = Get-Content -Raw -Encoding UTF8 (Join-Path $PSScriptRoot "AppxManifest.xml")
$manifest = $manifest.Replace('$NAME$', (X $Name)).Replace('$PUBLISHER$', (X $Publisher)).
    Replace('$VERSION$', $msixVersion).Replace('$DISPLAYNAME$', (X $DisplayName)).
    Replace('$PUBLISHERDISPLAYNAME$', (X $PublisherDisplayName)).Replace('$DESCRIPTION$', (X $Description)).
    Replace('$EXE$', (X $exeName))
[System.IO.File]::WriteAllText((Join-Path $stage "AppxManifest.xml"), $manifest, (New-Object System.Text.UTF8Encoding($false)))

# --- resources.pri: the index that maps Assets\Square44x44Logo.png in the manifest to the right
# file among the qualified ones above. Without it Windows finds none of them.
$sdk = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" | Sort-Object FullName | Select-Object -Last 1
if (-not $sdk) { throw "makeappx.exe not found: install the Windows SDK" }
$makepri = Join-Path $sdk.DirectoryName "makepri.exe"
$priconfig = Join-Path $repo "target\msix\priconfig.xml"
& $makepri createconfig /cf $priconfig /dq es-MX_en-US_fil-PH_hi-IN /pv 10.0.0 /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makepri createconfig failed" }
# The default config splits every scale into its own resources.scale-*.pri, which is meant for a
# bundle of resource packages. This is one package: everything stays in the one resources.pri.
$config = [xml](Get-Content -Raw $priconfig)
$packaging = $config.SelectSingleNode("//packaging")
if ($packaging) { $packaging.ParentNode.RemoveChild($packaging) | Out-Null }
$config.Save($priconfig)
& $makepri new /pr $stage /cf $priconfig /mn (Join-Path $stage "AppxManifest.xml") /of (Join-Path $stage "resources.pri") /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makepri new failed" }

# --- Pack.
$makeappx = $sdk
$out = Join-Path $paquetes "$DisplayName $cargoVersion.msix"
& $makeappx.FullName pack /d $stage /p $out /o | Out-Host
if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }
Write-Host "package: $out"

if ($Register) {
    Add-AppxPackage -Register (Join-Path $stage "AppxManifest.xml") -ForceApplicationShutdown
    Write-Host "registered from: $stage"
}
