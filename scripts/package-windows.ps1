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
    $commit = (git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $commit.Length -ne 40) { throw "Full Git commit is required" }
    if (git status --porcelain) { throw "Commit all source changes before packaging" }
    cargo build --release -p pipeline-app -p pipeline-cli
    if ($LASTEXITCODE -ne 0) { throw "Release build failed" }
    New-Item -ItemType Directory -Path $bundle -ErrorAction Stop | Out-Null
    Copy-Item -LiteralPath (Join-Path $repository "target/release/pipeline-app.exe") -Destination $bundle
    Copy-Item -LiteralPath (Join-Path $repository "target/release/pipeline-cli.exe") -Destination $bundle
    Copy-Item -LiteralPath (Join-Path $repository "LICENSE") -Destination $bundle
    Copy-Item -LiteralPath (Join-Path $repository "README.md") -Destination $bundle
    @(
        "version=$version"
        "commit=$commit"
        "target=windows-x86_64"
        "built_at_utc=$((Get-Date).ToUniversalTime().ToString('o'))"
        "rustc=$((rustc --version).Trim())"
    ) | Set-Content -LiteralPath (Join-Path $bundle "BUILD-INFO.txt") -Encoding utf8
    Compress-Archive -LiteralPath $bundle -DestinationPath $archive -ErrorAction Stop
    $digest = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    "$digest  $([IO.Path]::GetFileName($archive))" | Set-Content -LiteralPath "$archive.sha256" -Encoding ascii
    Write-Output "Archive: $archive"
    Write-Output "SHA-256: $digest"
} finally {
    Pop-Location
}
