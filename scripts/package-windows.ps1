param(
    [string]$OutputRoot = "dist"
)

$ErrorActionPreference = "Stop"
$repository = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$version = "0.1.0"
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$output = Join-Path $repository $OutputRoot
$bundleName = "project-pipeline-$version-windows-x86_64-$stamp"
$bundle = Join-Path $output $bundleName
$archive = Join-Path $output "$bundleName.zip"

Push-Location $repository
try {
    cargo build --release -p pipeline-app -p pipeline-cli
    if ($LASTEXITCODE -ne 0) { throw "Release build failed" }
    New-Item -ItemType Directory -Path $bundle -ErrorAction Stop | Out-Null
    Copy-Item -LiteralPath (Join-Path $repository "target/release/pipeline-app.exe") -Destination $bundle
    Copy-Item -LiteralPath (Join-Path $repository "target/release/pipeline-cli.exe") -Destination $bundle
    Copy-Item -LiteralPath (Join-Path $repository "LICENSE") -Destination $bundle
    Copy-Item -LiteralPath (Join-Path $repository "README.md") -Destination $bundle
    Compress-Archive -LiteralPath $bundle -DestinationPath $archive -ErrorAction Stop
    $digest = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    "$digest  $([IO.Path]::GetFileName($archive))" | Set-Content -LiteralPath "$archive.sha256" -Encoding ascii
    Write-Output "Archive: $archive"
    Write-Output "SHA-256: $digest"
} finally {
    Pop-Location
}
