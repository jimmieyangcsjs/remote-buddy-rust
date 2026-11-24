[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",

    [switch]$SkipFrontend,

    [switch]$SkipBundle
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

function Enter-VsDevShell {
    if (Get-Command cl.exe -ErrorAction SilentlyContinue) {
        return
    }

    $vswhere = "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path -LiteralPath $vswhere)) {
        throw "Visual Studio Build Tools not found. Install the Desktop development with C++ workload."
    }

    $installation = & $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath
    if (-not $installation) {
        throw "MSVC x64 tools not found. Install the Desktop development with C++ workload."
    }

    $vcvars = Join-Path $installation "VC\Auxiliary\Build\vcvars64.bat"
    if (-not (Test-Path -LiteralPath $vcvars)) {
        throw "vcvars64.bat not found under $installation"
    }
    $toolPath = $env:PATH
    foreach ($line in (& cmd.exe /d /s /c "`"$vcvars`" >nul && set")) {
        if ($line -match "^([^=]+)=(.*)$") {
            Set-Item -Path "Env:$($matches[1])" -Value $matches[2]
        }
    }
    $env:PATH = "$toolPath;$env:PATH"
}

function Invoke-Checked {
    param(
        [Parameter(Mandatory)]
        [scriptblock]$Command,

        [Parameter(Mandatory)]
        [string]$Description
    )

    Write-Host "==> $Description" -ForegroundColor Cyan
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE"
    }
}

function Invoke-TauriBuild {
    $tauriVersion = & cargo tauri --version 2>$null
    if ($LASTEXITCODE -eq 0) {
        Invoke-Checked { cargo tauri build } "Build NSIS installer with cargo-tauri"
        return
    }

    if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
        throw "Tauri CLI is missing. Install it with: cargo install tauri-cli --version '^2'"
    }
    Invoke-Checked { pnpm dlx '@tauri-apps/cli@^2' build } "Build NSIS installer with pnpm dlx"
}

Push-Location $root
try {
    Enter-VsDevShell

    if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
        throw "pnpm is required. Install Node.js 22+ and pnpm, then retry."
    }

    if (-not (Test-Path -LiteralPath "$root\ui\node_modules")) {
        Invoke-Checked { pnpm --dir ui install --frozen-lockfile } "Install UI dependencies"
    }

    if (-not $SkipBundle) {
        if ($Configuration -ne "Release") {
            throw "Tauri bundles are release-only. Use -Configuration Release or pass -SkipBundle."
        }
        Push-Location "$root\apps\desktop"
        try {
            Invoke-TauriBuild
        }
        finally {
            Pop-Location
        }
    }
    else {
        if (-not $SkipFrontend) {
            Invoke-Checked { pnpm --dir ui build } "Build UI"
        }

        $cargoArguments = @("build", "--workspace")
        if ($Configuration -eq "Release") {
            $cargoArguments += "--release"
            $cargoArguments += @("--features", "remote-buddy-desktop/custom-protocol")
        }
        Invoke-Checked { cargo @cargoArguments } "Build Rust workspace"
    }

    $binary = "$root\target\$($Configuration.ToLowerInvariant())\remote-buddy-desktop.exe"
    Write-Host ""
    Write-Host "Binary: $binary"
    $installerDirectory = "$root\target\release\bundle\nsis"
    if (Test-Path -LiteralPath $installerDirectory) {
        Get-ChildItem -LiteralPath $installerDirectory -Filter *.exe |
            ForEach-Object { Write-Host "Installer: $($_.FullName) ($([math]::Round($_.Length / 1MB, 1)) MB)" }
    }
}
finally {
    Pop-Location
}
