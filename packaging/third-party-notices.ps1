<#
  Writes packaging\THIRD-PARTY-NOTICES.txt: the licence notices of everything Khipu ships that
  Alex did not write. Both editions carry it beside espanso (the portable ZIP in Program Files\,
  the MSIX at the root of its layout).

    powershell -ExecutionPolicy Bypass -File packaging\third-party-notices.ps1

  Run it again whenever Cargo.lock changes. What it lists, and why exactly that:

  - Every crate linked into Khipu.exe: `cargo tree` for the Windows target, normal edges only,
    without proc-macros. Build scripts and derive macros run on the build machine and leave nothing
    in the binary, so their licences are not conveyed with it. Most of these licences (MIT, BSD,
    Zlib, ISC, BSL, Apache) ask for exactly one thing in a binary: that their notice travels with it.
  - Each crate's licence files as published, verbatim, de-duplicated by content: the hundred Apache
    copies are one text with a list of crates above it. Crates published without any (egui's
    family, for one) get their authors from Cargo.toml and point at the standard text, which is
    then printed once at the end.
  - espanso, which is shipped unmodified next to Khipu: GPL-3.0, and where its source is.
#>
$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$registry = Get-ChildItem "$env:USERPROFILE\.cargo\registry\src" -Directory | Select-Object -First 1
$out = Join-Path $PSScriptRoot "THIRD-PARTY-NOTICES.txt"

# The espanso Khipu ships, read from the newest portable ZIP's own espansod (the same one both
# editions copy). Only its version is needed: it names the tag whose source is offered.
$espansoVersion = "2.4.1"

Push-Location $repo
try {
    $tree = cargo tree -e normal,no-proc-macro --target x86_64-pc-windows-msvc --prefix none -f "{p}|{l}"
    if ($LASTEXITCODE -ne 0) { throw "cargo tree failed" }
} finally { Pop-Location }

$crates = $tree | ForEach-Object { $_ -replace ' \(\*\)$', '' } | Sort-Object -Unique | ForEach-Object {
    $p, $license = $_ -split '\|', 2
    $parts = $p -split ' '
    $name = $parts[0]; $version = $parts[1].TrimStart('v')
    $dir = if ($p -match '\((.+)\)$') { $Matches[1] } else { Join-Path $registry.FullName "$name-$version" }
    [pscustomobject]@{ Name = $name; Version = $version; License = $license; Dir = $dir }
} | Where-Object { $_.Name -ne "khipu" }

function LicenseFiles($dir) {
    # The crate's root, plus one level down for crates that keep them in a folder (the default
    # fonts keep each font's licence beside the font).
    $candidates = @(Get-ChildItem $dir -File -ErrorAction SilentlyContinue)
    Get-ChildItem $dir -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -match '^(fonts|licen[sc]es?)$' } |
        ForEach-Object { $candidates += Get-ChildItem $_.FullName -File }
    $candidates | Where-Object {
        $_.Name -match '(?i)^(licen[sc]e|copying|notice|copyright|ofl|ufl)|licen[sc]e' -and
        $_.Extension -match '(?i)^(|\.txt|\.md|\.mit|\.apache|\.apache2|\.bsl|\.zlib)$' -or
        ($_.Name -match '(?i)^licen[sc]e-' )
    } | Sort-Object Name
}

function Authors($dir) {
    $toml = Join-Path $dir "Cargo.toml"
    if (-not (Test-Path $toml)) { return "" }
    $m = [regex]::Match((Get-Content -Raw -Encoding UTF8 $toml), '(?ms)^authors\s*=\s*\[(.*?)\]')
    if (-not $m.Success) { return "" }
    ([regex]::Matches($m.Groups[1].Value, '"([^"]*)"') | ForEach-Object { $_.Groups[1].Value }) -join ", "
}

# Group identical texts. Line endings and surrounding blank space are not part of a licence.
$texts = [ordered]@{}
$noFile = @()
foreach ($c in $crates) {
    $files = @(LicenseFiles $c.Dir)
    if ($files.Count -eq 0) { $noFile += $c; continue }
    foreach ($f in $files) {
        $body = ((Get-Content -Raw -Encoding UTF8 $f.FullName) -replace "`r`n", "`n").Trim()
        if (-not $texts.Contains($body)) { $texts[$body] = New-Object System.Collections.Generic.List[string] }
        $label = "$($c.Name) $($c.Version)"
        if ($f.DirectoryName -ne $c.Dir.TrimEnd('\')) { $label += " ($($f.Directory.Name)/$($f.Name))" }
        if (-not $texts[$body].Contains($label)) { $texts[$body].Add($label) }
    }
}

$rule = "=" * 78
$sb = New-Object System.Text.StringBuilder
function L([string]$s = "") { [void]$sb.Append($s).Append("`r`n") }

L "THIRD-PARTY NOTICES"
L "Khipu - https://github.com/alexpaltrc/khipu"
L
L "Khipu is free software by Alex Palacios, licensed under the GNU General Public License,"
L "version 3 or (at your option) any later version. Its source code is at the address above."
L "It is distributed WITHOUT ANY WARRANTY. The full licence text is in LICENSE.txt."
L
L "Khipu includes the software listed below. Each is the work of its own authors and is used"
L "under its own licence, reproduced here as published."
L
L $rule
L "espanso $espansoVersion"
L $rule
L "Files: espansod.exe, and the starter configuration in .espanso and .espanso-runtime."
L "Copyright (C) Federico Terzi and the espanso contributors. https://espanso.org"
L "Licence: GNU General Public License, version 3 (the same text as LICENSE.txt)."
L "Shipped unmodified, as published in espanso's portable edition for Windows. Its complete"
L "source code, for this exact version, is at:"
L "  https://github.com/espanso/espanso/tree/v$espansoVersion"
L "Khipu does not contain espanso's code: it starts espansod.exe as a separate program."
L
L $rule
L "Microsoft Visual C++ runtime"
L $rule
L "Files: concrt140.dll, msvcp140*.dll, vccorlib140.dll, vcruntime140*.dll"
L "Copyright (C) Microsoft Corporation. Redistributable files of Microsoft Visual Studio,"
L "shipped with espanso's portable edition and distributed under the Visual Studio licence"
L "terms for redistributable code."
L
L $rule
L "Rust libraries linked into Khipu.exe ($($crates.Count))"
L $rule
foreach ($c in $crates) { L ("  {0} {1}  -  {2}" -f $c.Name, $c.Version, $c.License) }
L
L "eframe and epaint are included with small changes made for Khipu; each change, and why it"
L "was made, is described in vendor/README.md in Khipu's source code."
L
foreach ($body in $texts.Keys) {
    L $rule
    L ("Used by: " + ($texts[$body] -join ", "))
    L $rule
    L
    L ($body -replace "`n", "`r`n")
    L
}

if ($noFile.Count -gt 0) {
    L $rule
    L "Published without a licence file"
    L $rule
    L "These crates declare their licence in Cargo.toml but publish no licence text. Their"
    L "copyright belongs to the authors named, and the standard text of each licence follows."
    L
    foreach ($c in $noFile) {
        $a = Authors $c.Dir
        if ($a -eq "") { $a = "the $($c.Name) authors" }
        L ("  {0} {1}  -  {2}" -f $c.Name, $c.Version, $c.License)
        L ("    Copyright (c) {0}" -f $a)
    }
    L
    L $rule
    L "MIT License (standard text, for the crates above that chose MIT)"
    L $rule
    L
    L "Permission is hereby granted, free of charge, to any person obtaining a copy of this"
    L "software and associated documentation files (the `"Software`"), to deal in the Software"
    L "without restriction, including without limitation the rights to use, copy, modify, merge,"
    L "publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons"
    L "to whom the Software is furnished to do so, subject to the following conditions:"
    L
    L "The above copyright notice and this permission notice shall be included in all copies or"
    L "substantial portions of the Software."
    L
    L "THE SOFTWARE IS PROVIDED `"AS IS`", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,"
    L "INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR"
    L "PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE"
    L "FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR"
    L "OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER"
    L "DEALINGS IN THE SOFTWARE."
    L
    L "The Boost Software License 1.0 and the Apache License 2.0 appear in full above, among the"
    L "texts other crates published."
}

[System.IO.File]::WriteAllText($out, $sb.ToString(), (New-Object System.Text.UTF8Encoding($false)))
Write-Host ("{0}: {1} crates, {2} distinct texts, {3} without a file" -f $out, $crates.Count, $texts.Count, $noFile.Count)
