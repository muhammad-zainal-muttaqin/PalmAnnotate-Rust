[CmdletBinding()]
param(
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Root = Split-Path -Parent $PSScriptRoot
$ToolsRoot = Join-Path $Root '.toolchains'
$JdkRoot = Join-Path $ToolsRoot 'jdk-17'
$SdkRoot = Join-Path $ToolsRoot 'android-sdk'
$CmdlineRoot = Join-Path $SdkRoot 'cmdline-tools\latest'
$NdkVersion = '28.2.13676358'

New-Item -ItemType Directory -Force -Path $ToolsRoot, $SdkRoot | Out-Null

function Test-Command([string]$Name) {
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Install-Jdk {
    $java = Join-Path $JdkRoot 'bin\java.exe'
    if ((Test-Path $java) -and -not $Force) {
        $version = (& $java -version 2>&1 | Select-Object -First 1) -join ''
        if ($version -match '"17\.') {
            return
        }
    }
    Write-Host 'Installing portable Temurin JDK 17...'
    $archive = Join-Path $ToolsRoot 'jdk17.zip'
    Invoke-WebRequest `
        -UseBasicParsing `
        -Uri 'https://api.adoptium.net/v3/binary/latest/17/ga/windows/x64/jdk/hotspot/normal/eclipse' `
        -OutFile $archive
    $extract = Join-Path $ToolsRoot 'jdk-extract'
    Remove-Item -Recurse -Force $extract -ErrorAction SilentlyContinue
    Expand-Archive -LiteralPath $archive -DestinationPath $extract -Force
    $source = Get-ChildItem -Path $extract -Directory | Select-Object -First 1
    if (-not $source) { throw 'JDK archive did not contain a root directory.' }
    Remove-Item -Recurse -Force $JdkRoot -ErrorAction SilentlyContinue
    Move-Item -LiteralPath $source.FullName -Destination $JdkRoot
    Remove-Item -Recurse -Force $extract
    Remove-Item -Force $archive
}

function Install-AndroidCommandLineTools {
    if ((Test-Path (Join-Path $CmdlineRoot 'bin\sdkmanager.bat')) -and -not $Force) {
        return
    }
    Write-Host 'Installing Android command-line tools...'
    [xml]$repository = (Invoke-WebRequest -UseBasicParsing `
        -Uri 'https://dl.google.com/android/repository/repository2-1.xml').Content
    $package = $repository.SelectSingleNode(
        "//*[local-name()='remotePackage' and @path='cmdline-tools;latest']"
    )
    $archiveNode = $package.SelectSingleNode(
        ".//*[local-name()='archive'][*[local-name()='host-os' and text()='windows']]"
    )
    $relativeUrl = $archiveNode.SelectSingleNode(
        ".//*[local-name()='complete']/*[local-name()='url']"
    ).InnerText
    if (-not $relativeUrl) { throw 'Could not resolve Android command-line tools URL.' }
    $archive = Join-Path $ToolsRoot 'commandlinetools-win.zip'
    Invoke-WebRequest -UseBasicParsing -Uri ("https://dl.google.com/android/repository/$relativeUrl") -OutFile $archive
    $extract = Join-Path $ToolsRoot 'android-cli-extract'
    Remove-Item -Recurse -Force $extract -ErrorAction SilentlyContinue
    Expand-Archive -LiteralPath $archive -DestinationPath $extract -Force
    Remove-Item -Recurse -Force $CmdlineRoot -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $CmdlineRoot) | Out-Null
    Move-Item -LiteralPath (Join-Path $extract 'cmdline-tools') -Destination $CmdlineRoot
    Remove-Item -Recurse -Force $extract
    Remove-Item -Force $archive
}

function Ensure-RustTools {
    $toolchains = rustup toolchain list
    if ($toolchains -notmatch '^1\.93\.1-') {
        rustup toolchain install 1.93.1 --profile minimal --component rustfmt,clippy
    }
    $targets = rustup target list --installed --toolchain 1.93.1
    if ($targets -notcontains 'wasm32-unknown-unknown') {
        rustup target add wasm32-unknown-unknown --toolchain 1.93.1
    }
    if ($targets -notcontains 'aarch64-linux-android') {
        rustup target add aarch64-linux-android --toolchain 1.93.1
    }
    $tauriVersion = if (Test-Command 'cargo-tauri') {
        (cargo tauri --version 2>$null) -join ''
    } else {
        ''
    }
    if ($tauriVersion -notmatch '^tauri-cli 2\.11\.') {
        cargo install tauri-cli --version '~2.11' --locked --force
    }
    $dxVersion = if (Test-Command 'dx') {
        (dx --version 2>$null) -join ''
    } else {
        ''
    }
    if ($dxVersion -notmatch '^dioxus 0\.6\.3\b') {
        $archive = Join-Path $ToolsRoot 'dx-0.6.3.zip'
        Invoke-WebRequest `
            -UseBasicParsing `
            -Uri 'https://github.com/DioxusLabs/dioxus/releases/download/v0.6.3/dx-x86_64-pc-windows-msvc-v0.6.3.zip' `
            -OutFile $archive
        $extract = Join-Path $ToolsRoot 'dx-extract'
        Remove-Item -Recurse -Force $extract -ErrorAction SilentlyContinue
        Expand-Archive -LiteralPath $archive -DestinationPath $extract -Force
        Copy-Item -LiteralPath (Join-Path $extract 'dx.exe') `
            -Destination (Join-Path $env:USERPROFILE '.cargo\bin\dx.exe') -Force
        Remove-Item -Recurse -Force $extract
        Remove-Item -Force $archive
    }
}

Ensure-RustTools
Install-Jdk
Install-AndroidCommandLineTools

$env:JAVA_HOME = $JdkRoot
$env:ANDROID_HOME = $SdkRoot
$env:ANDROID_SDK_ROOT = $SdkRoot
$env:NDK_HOME = Join-Path $SdkRoot "ndk\$NdkVersion"
$env:PATH = "$JdkRoot\bin;$CmdlineRoot\bin;$SdkRoot\platform-tools;$env:PATH"

$SdkManager = Join-Path $CmdlineRoot 'bin\sdkmanager.bat'
1..40 | ForEach-Object { 'y' } | & $SdkManager --sdk_root=$SdkRoot --licenses | Out-Null
& $SdkManager --sdk_root=$SdkRoot `
    'platform-tools' `
    'platforms;android-34' `
    'build-tools;34.0.0' `
    "ndk;$NdkVersion"

Write-Host ''
Write-Host 'Android toolchain ready. Current-shell values:'
Write-Host "`$env:JAVA_HOME = '$JdkRoot'"
Write-Host "`$env:ANDROID_HOME = '$SdkRoot'"
Write-Host "`$env:ANDROID_SDK_ROOT = `$env:ANDROID_HOME"
Write-Host "`$env:NDK_HOME = '$env:NDK_HOME'"
Write-Host "`$env:PATH = '`$env:JAVA_HOME\bin;`$env:ANDROID_HOME\platform-tools;`$env:ANDROID_HOME\cmdline-tools\latest\bin;`$env:PATH'"
