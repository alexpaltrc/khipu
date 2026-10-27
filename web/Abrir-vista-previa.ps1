$ErrorActionPreference = 'Stop'
$previewUrl = 'http://127.0.0.1:4319/'
$previewNode = 'C:/Users/aleks/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node.exe'
if (-not (Test-Path -LiteralPath $previewNode)) {
    $previewNode = (Get-Command node -ErrorAction Stop).Source
}
$previewRunning = $false
try { $previewResponse = Invoke-WebRequest $previewUrl -UseBasicParsing -TimeoutSec 2; $previewRunning = $previewResponse.StatusCode -eq 200 -and $previewResponse.Content.Contains('Khipu') } catch {}
if (-not $previewRunning) {
    Start-Process -FilePath $previewNode -ArgumentList ('"' + (Join-Path $PSScriptRoot 'server.mjs') + '"') -WorkingDirectory $PSScriptRoot -WindowStyle Hidden
    for ($previewAttempt = 0; $previewAttempt -lt 20; $previewAttempt++) {
        try { $previewResponse = Invoke-WebRequest $previewUrl -UseBasicParsing -TimeoutSec 1; if ($previewResponse.StatusCode -eq 200) { $previewRunning = $true; break } } catch {}
        Start-Sleep -Milliseconds 200
    }
}
if (-not $previewRunning) { throw 'No se pudo iniciar la vista previa de Khipu.' }
Start-Process $previewUrl