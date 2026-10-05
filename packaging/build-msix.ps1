<#
  Builds the installed (Microsoft Store) edition: an MSIX package.

    powershell -ExecutionPolicy Bypass -File packaging\build-msix.ps1            # build the .msix
    powershell -ExecutionPolicy Bypass -File packaging\build-msix.ps1 -Register  # and install it here

  The package is left unsigned. The Store signs what is uploaded to it; -Register installs the
  unpacked layout directly, which Windows allows only with Developer Mode on.

  espansod.exe, its DLLs and the starter .espanso/.espanso-runtime come from the newest portable
  ZIP in paquetes\, the same pristine copy every portable user gets. Never from the development
  folder espanso-portable\: that one holds Alex's own expansions.

  Identity and publisher are placeholders until the app's final name is reserved in Partner Center;
  pass the values from its Product identity page then.
#>
param(
    [string]$Name = "EspansoManager.Dev",
    [string]$Publisher = "CN=Alex Palacios",
    [string]$PublisherDisplayName = "Alex Palacios",
    [string]$DisplayName = "Espanso Manager",
    [string]$Description = "Text expansions without touching a config file.",
    [string]$RuntimeZip = "",
    [switch]$Register
)
$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$paquetes = Join-Path $repo "..\..\paquetes" | Resolve-Path
$exeName = "$DisplayName.exe"

# --- Version: Cargo's 0.0.5-dev becomes the four numbers MSIX wants, 0.0.5.0.
$cargoVersion = (Select-String -Path (Join-Path $repo "Cargo.toml") -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$numeric = ($cargoVersion -split '-')[0]
$msixVersion = "$numeric.0"

# --- Build.
Push-Location $repo
try { cargo build --release; if ($LASTEXITCODE -ne 0) { throw "cargo build failed" } } finally { Pop-Location }

# --- Stage the layout. Kept under target\ so -Register has a folder that stays put.
$stage = Join-Path $repo "target\msix\layout"
# A copy registered with -Register runs straight out of this folder, espansod included, and a
# running .exe cannot be deleted. Only processes started from here are closed.
Get-Process | Where-Object { $_.Path -like "$stage\*" } | Stop-Process -Force
Start-Sleep -Milliseconds 500
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
$res = Join-Path $stage "Program Files"
$seed = Join-Path $res "seed"
New-Item -ItemType Directory -Force $res, $seed, (Join-Path $stage "Assets") | Out-Null
Copy-Item (Join-Path $repo "target\release\EspansoManager.exe") (Join-Path $stage $exeName)

# --- espanso, from the portable ZIP.
if ($RuntimeZip -eq "") {
    $RuntimeZip = (Get-ChildItem $paquetes -Filter "Espanso Manager Portable *.zip" | Sort-Object LastWriteTime | Select-Object -Last 1).FullName
}
Write-Host "espanso from: $RuntimeZip"
$unzip = Join-Path $repo "target\msix\portable"
if (Test-Path $unzip) { Remove-Item -Recurse -Force $unzip }
Expand-Archive $RuntimeZip $unzip
$pf = Get-ChildItem $unzip -Directory | ForEach-Object { Join-Path $_.FullName "Program Files" } | Select-Object -First 1
Copy-Item (Join-Path $pf "espansod.exe"), (Join-Path $pf "LICENSE.txt") $res
Copy-Item (Join-Path $pf "*.dll") $res
# The seed: copied into the user's LocalState on first start (install.rs). Not the portable's
# README or .bat files: they describe a folder this edition does not have.
Copy-Item -Recurse (Join-Path $pf ".espanso"), (Join-Path $pf ".espanso-runtime") $seed

# --- Logos, from the app's own icon.
# Every image in the .ico is stored as PNG. They are read straight out of the file: going through
# System.Drawing.Icon.ToBitmap() decodes a PNG-compressed entry as noise, which is what Start
# showed until 0.0.5 — no icon at all.
Add-Type -AssemblyName System.Drawing
$icoBytes = [System.IO.File]::ReadAllBytes((Join-Path $repo "assets\Espanso Manager.ico"))
$icons = @{}
for ($i = 0; $i -lt [BitConverter]::ToUInt16($icoBytes, 4); $i++) {
    $entry = 6 + 16 * $i
    $size = if ($icoBytes[$entry] -eq 0) { 256 } else { [int]$icoBytes[$entry] }
    $length = [BitConverter]::ToUInt32($icoBytes, $entry + 8)
    $offset = [BitConverter]::ToUInt32($icoBytes, $entry + 12)
    $png = New-Object byte[] $length
    [Array]::Copy($icoBytes, $offset, $png, 0, $length)
    $icons[$size] = [System.Drawing.Image]::FromStream((New-Object System.IO.MemoryStream(, $png)))
}
# The image drawn for an icon of $px pixels: the .ico's own one when it has that size (drawn by
# hand for it, so sharper than any scaling), otherwise the smallest larger one, scaled down.
function Get-Icon([int]$px) {
    $fit = $icons.Keys | Where-Object { $_ -ge $px } | Sort-Object | Select-Object -First 1
    if (-not $fit) { $fit = ($icons.Keys | Sort-Object | Select-Object -Last 1) }
    $icons[$fit]
}
function Save-Logo([string]$file, [int]$w, [int]$h, [int]$icon) {
    $bmp = New-Object System.Drawing.Bitmap($w, $h)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)
    $g.DrawImage((Get-Icon $icon), [int][Math]::Floor(($w - $icon) / 2), [int][Math]::Floor(($h - $icon) / 2), $icon, $icon)
    $g.Dispose()
    $bmp.Save((Join-Path $stage "Assets\$file"), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}
# Only qualified files: resources.pri (below) tells Windows which one to take for each screen
# scale and each size it draws. Scales stop where the 256 px source would have to be enlarged.
$scales = @{ 100 = 1.0; 125 = 1.25; 150 = 1.5; 200 = 2.0; 400 = 4.0 }
foreach ($s in $scales.Keys) {
    $k = $scales[$s]
    Save-Logo "Square44x44Logo.scale-$s.png" ([int](44 * $k)) ([int](44 * $k)) ([int](44 * $k))
    Save-Logo "StoreLogo.scale-$s.png" ([int](50 * $k)) ([int](50 * $k)) ([int](50 * $k))
    if ($s -le 200) {
        Save-Logo "Square150x150Logo.scale-$s.png" ([int](150 * $k)) ([int](150 * $k)) ([int](100 * $k))
        Save-Logo "Wide310x150Logo.scale-$s.png" ([int](310 * $k)) ([int](150 * $k)) ([int](100 * $k))
    }
}
# The fixed sizes the taskbar, Start's search and the list of apps ask for. "unplated" is the
# icon on its own, with no square of accent colour behind it.
foreach ($t in 16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 256) {
    Save-Logo "Square44x44Logo.targetsize-$t.png" $t $t $t
    Save-Logo "Square44x44Logo.targetsize-${t}_altform-unplated.png" $t $t $t
}

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
