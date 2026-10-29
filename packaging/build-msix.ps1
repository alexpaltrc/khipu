<#
  Builds Khipu as an MSIX package, for one of two channels.

    powershell -ExecutionPolicy Bypass -File packaging\build-msix.ps1               # Store package
    powershell -ExecutionPolicy Bypass -File packaging\build-msix.ps1 -Channel Dev  # Khipu Dev, installed here

  Store: the identity Partner Center assigned when "Khipu" was reserved (2026-10-05, Product identity
  page); the Store accepts no package that carries other values. Left unsigned: the Store signs what
  is uploaded to it. Never install it on a PC that will get Khipu from the Store.

  Dev: where every change lands and Alex tries it. Its own identity (AlexPalacios.Khipu.Dev, shown
  as "Khipu Dev"), so it never meets a Store install in Windows' eyes and keeps its own LocalState.
  Signed with a
  development certificate made on this PC, trusted only here (trust-dev-cert.ps1, once, as
  administrator). Every build gets a higher fourth number than the installed one, so it installs
  over it and LocalState, Alex's expansions, stays. Developer Mode is never needed.

  espansod.exe, its DLLs and the starter .espanso/.espanso-runtime come from a portable ZIP: by
  default the one on the latest GitHub release, the same pristine copy every portable user gets,
  downloaded once into target\msix\. -RuntimeZip names another.

  The package is written to target\msix\out\. Release packages are uploaded to the Store and to
  GitHub Releases; nothing keeps them here.
#>
param(
    [ValidateSet("Store", "Dev")]
    [string]$Channel = "Store",
    [string]$RuntimeZip = "",
    # Store only: the fourth number. Dev picks its own.
    [int]$Revision = 0
)
$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$work = Join-Path $repo "target\msix"
$exeName = "Khipu.exe"
$Description = "Text expansions without touching a config file."
$PublisherDisplayName = "Alex Palacios"

if ($Channel -eq "Dev") {
    $Name = "AlexPalacios.Khipu.Dev"
    $Publisher = "CN=Alex Palacios Dev"
    $DisplayName = "Khipu Dev"
} else {
    $Name = "AlexPalacios.Khipu"
    $Publisher = "CN=A3B4D4CD-AECB-4B6D-9268-AEAB19F8F17B"
    $DisplayName = "Khipu"
}

# --- Version: Cargo's 0.0.8-dev becomes the four numbers MSIX wants, 0.0.8.<Revision>. Dev goes one
# past whatever is installed, so the new build always replaces it.
$cargoVersion = (Select-String -Path (Join-Path $repo "Cargo.toml") -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$numeric = ($cargoVersion -split '-')[0]
if ($Channel -eq "Dev") {
    $Revision = 1
    $installed = Get-AppxPackage -Name $Name
    if ($installed) {
        $v = [version]$installed.Version
        if ("$($v.Major).$($v.Minor).$($v.Build)" -eq $numeric) { $Revision = $v.Revision + 1 }
    }
}
$msixVersion = "$numeric.$Revision"

# --- Build.
Push-Location $repo
try { cargo build --release; if ($LASTEXITCODE -ne 0) { throw "cargo build failed" } } finally { Pop-Location }

$stage = Join-Path $work "layout"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
$res = Join-Path $stage "Program Files"
$seed = Join-Path $res "seed"
New-Item -ItemType Directory -Force $res, $seed, (Join-Path $stage "Assets") | Out-Null
Copy-Item (Join-Path $repo "target\release\Khipu.exe") (Join-Path $stage $exeName)

# --- espanso, from the portable ZIP.
if ($RuntimeZip -eq "") {
    $release = Invoke-RestMethod "https://api.github.com/repos/alexpaltrc/khipu/releases/latest"
    $asset = $release.assets | Where-Object { $_.name -like "Khipu-Portable-*.zip" } | Select-Object -First 1
    if (-not $asset) { throw "the latest release has no Khipu-Portable ZIP" }
    $RuntimeZip = Join-Path $work $asset.name
    if (-not (Test-Path $RuntimeZip)) {
        New-Item -ItemType Directory -Force $work | Out-Null
        Invoke-WebRequest $asset.browser_download_url -OutFile $RuntimeZip
    }
}
Write-Host "espanso from: $RuntimeZip"
$unzip = Join-Path $work "portable"
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
if ($Channel -eq "Dev") {
    # Khipu Dev wears «DEV» on every logo, the same label src\icons.rs draws on its window and tray
    # icons, from the same geometry on the 40-unit canvas: an amber pill ringed clear, and the
    # letters in ink. A wide logo takes it at the foot of its centred square.
    Add-Type -AssemblyName System.Drawing
    function Stadium([float]$s, [float]$r) {
        $p = New-Object System.Drawing.Drawing2D.GraphicsPath
        $p.AddArc((10 - $r) * $s, (32.5 - $r) * $s, 2 * $r * $s, 2 * $r * $s, 90, 180)
        $p.AddArc((30 - $r) * $s, (32.5 - $r) * $s, 2 * $r * $s, 2 * $r * $s, 270, 180)
        $p.CloseFigure()
        $p
    }
    $letters = @(
        @(11.0, 29.0, 11.0, 36.0), @(11.0, 29.0, 13.2, 29.0), @(13.2, 29.0, 15.4, 31.0),
        @(15.4, 31.0, 15.4, 34.0), @(15.4, 34.0, 13.2, 36.0), @(13.2, 36.0, 11.0, 36.0),
        @(18.0, 29.0, 18.0, 36.0), @(18.0, 29.0, 21.8, 29.0), @(18.0, 32.5, 21.2, 32.5),
        @(18.0, 36.0, 21.8, 36.0), @(24.2, 29.0, 26.7, 36.0), @(26.7, 36.0, 29.2, 29.0))
    foreach ($file in Get-ChildItem (Join-Path $stage "Assets\*.png")) {
        $bmp = [System.Drawing.Bitmap]::FromFile($file.FullName)
        $logo = New-Object System.Drawing.Bitmap $bmp
        $bmp.Dispose()
        $side = [Math]::Min($logo.Width, $logo.Height)
        $s = $side / 40
        $g = [System.Drawing.Graphics]::FromImage($logo)
        $g.SmoothingMode = "AntiAlias"
        $g.PixelOffsetMode = "HighQuality"
        $g.TranslateTransform(($logo.Width - $side) / 2, ($logo.Height - $side) / 2)
        $g.CompositingMode = "SourceCopy"
        $g.FillPath((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::Transparent)), (Stadium $s 7.0))
        $g.CompositingMode = "SourceOver"
        $g.FillPath((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(0xF5, 0xB0, 0x00))), (Stadium $s 5.5))
        $pen = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(0x21, 0x1B, 0x37)), (1.9 * $s)
        $pen.StartCap = "Round"; $pen.EndCap = "Round"
        foreach ($l in $letters) { $g.DrawLine($pen, $l[0] * $s, $l[1] * $s, $l[2] * $s, $l[3] * $s) }
        $g.Dispose()
        $logo.Save($file.FullName, [System.Drawing.Imaging.ImageFormat]::Png)
        $logo.Dispose()
    }
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
$priconfig = Join-Path $work "priconfig.xml"
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
$outDir = Join-Path $work "out"
New-Item -ItemType Directory -Force $outDir | Out-Null
$out = Join-Path $outDir "$DisplayName $msixVersion.msix"
& $sdk.FullName pack /d $stage /p $out /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }
Write-Host "package: $out"

if ($Channel -eq "Dev") {
    # --- Sign with the development certificate, made here the first time. Its private key never
    # leaves this user's certificate store; only the public half is exported, for trusting.
    $cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -eq $Publisher } | Select-Object -First 1
    if (-not $cert) {
        $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject $Publisher `
            -CertStoreLocation Cert:\CurrentUser\My -KeyUsage DigitalSignature `
            -FriendlyName "Khipu Dev" -NotAfter (Get-Date).AddYears(10) `
            -TextExtension @("2.5.29.19={text}")
    }
    Export-Certificate -Cert $cert -FilePath (Join-Path $work "khipu-dev.cer") | Out-Null
    $signtool = Join-Path $sdk.DirectoryName "signtool.exe"
    & $signtool sign /fd SHA256 /sha1 $cert.Thumbprint /s My $out | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "signtool failed" }

    $trusted = Get-ChildItem Cert:\LocalMachine\TrustedPeople | Where-Object { $_.Thumbprint -eq $cert.Thumbprint }
    if (-not $trusted) {
        # Once per PC: one UAC prompt, answered by whoever is at the keyboard.
        & (Join-Path $PSScriptRoot "trust-dev-cert.ps1")
        $trusted = Get-ChildItem Cert:\LocalMachine\TrustedPeople | Where-Object { $_.Thumbprint -eq $cert.Thumbprint }
        if (-not $trusted) { throw "this PC does not trust the Khipu Dev certificate, so Windows will not install it" }
    }

    # --- Install over the previous Khipu Dev. Windows closes it if it is open; LocalState stays.
    Add-AppxPackage -Path $out -ForceApplicationShutdown -ForceUpdateFromAnyVersion
    Write-Host "installed: $DisplayName $msixVersion"
}
