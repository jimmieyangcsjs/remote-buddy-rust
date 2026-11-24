[CmdletBinding()]
param(
    [string]$Report = "docs\validation.md",

    [switch]$SkipFrontend
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$results = [System.Collections.Generic.List[object]]::new()

function Enter-VsDevShell {
    if (Get-Command cl.exe -ErrorAction SilentlyContinue) {
        return
    }

    $vswhere = "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path -LiteralPath $vswhere)) {
        throw "Visual Studio Build Tools not found."
    }
    $installation = & $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath
    if (-not $installation) {
        throw "MSVC x64 tools not found."
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

function Invoke-Validation {
    param(
        [string]$Name,
        [scriptblock]$Command
    )

    Write-Host "==> $Name" -ForegroundColor Cyan
    $started = Get-Date
    try {
        & $Command
        if ($LASTEXITCODE -ne 0) {
            throw "exit code $LASTEXITCODE"
        }
        $results.Add([pscustomobject]@{
            Name = $Name
            Result = "PASS"
            Detail = ""
            Seconds = [math]::Round(((Get-Date) - $started).TotalSeconds, 1)
        })
    }
    catch {
        $results.Add([pscustomobject]@{
            Name = $Name
            Result = "FAIL"
            Detail = $_.Exception.Message
            Seconds = [math]::Round(((Get-Date) - $started).TotalSeconds, 1)
        })
    }
}

function Get-WebView2Version {
    $paths = @(
        "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
        "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
        "HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
    )
    foreach ($path in $paths) {
        if (Test-Path -LiteralPath $path) {
            return (Get-ItemProperty -LiteralPath $path).pv
        }
    }
    return "not detected"
}

Push-Location $root
try {
    Enter-VsDevShell

    Invoke-Validation "cargo fmt" { cargo fmt --all --check }
    Invoke-Validation "cargo test" { cargo test --workspace }
    Invoke-Validation "cargo clippy" {
        cargo clippy --workspace --all-targets -- -D warnings
    }

    if (-not $SkipFrontend) {
        if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
            $results.Add([pscustomobject]@{
                Name = "UI build"
                Result = "FAIL"
                Detail = "pnpm is not available"
                Seconds = 0
            })
        }
        else {
            Invoke-Validation "UI tests" { pnpm --dir ui test }
            Invoke-Validation "UI build" { pnpm --dir ui build }
        }
    }

    $failed = @($results | Where-Object Result -eq "FAIL")
    $reportPath = Join-Path $root $Report
    $reportDirectory = Split-Path -Parent $reportPath
    New-Item -ItemType Directory -Path $reportDirectory -Force | Out-Null

    $versionLines = @(
        "- Rust: $(& rustc --version)",
        "- Cargo: $(& cargo --version)",
        "- Node: $(& node --version)",
        "- pnpm: $(& pnpm --version)",
        "- OS: $([System.Environment]::OSVersion.VersionString)",
        "- WebView2: $(Get-WebView2Version)",
        "- Generated: $((Get-Date).ToString('yyyy-MM-dd HH:mm:ss zzz'))"
    )
    $table = @(
        "| Check | Result | Seconds | Detail |",
        "| --- | --- | ---: | --- |"
    )
    foreach ($result in $results) {
        $detail = $result.Detail.Replace("|", "\|")
        $table += "| $($result.Name) | $($result.Result) | $($result.Seconds) | $detail |"
    }

    $artifactSection = @(
        "## Release artifacts",
        ""
    )
    $artifactFiles = @(
        @{
            Name = "Application EXE"
            Relative = "target\release\remote-buddy-desktop.exe"
            Path = "$root\target\release\remote-buddy-desktop.exe"
        },
        @{
            Name = "NSIS installer"
            Relative = "target\release\bundle\nsis\Remote Buddy_0.2.0_x64-setup.exe"
            Path = "$root\target\release\bundle\nsis\Remote Buddy_0.2.0_x64-setup.exe"
        }
    )
    $existingArtifacts = @($artifactFiles | Where-Object {
        Test-Path -LiteralPath $_.Path
    })
    if ($existingArtifacts.Count -eq 0) {
        $artifactSection += 'No release artifacts found. Run `.\scripts\build.ps1 -Configuration Release`.'
    }
    else {
        $artifactSection += "| Artifact | Path | MiB | SHA-256 | Signature |"
        $artifactSection += "| --- | --- | ---: | --- | --- |"
        foreach ($artifact in $existingArtifacts) {
            $item = Get-Item -LiteralPath $artifact.Path
            $hash = Get-FileHash -LiteralPath $artifact.Path -Algorithm SHA256
            $signature = (Get-AuthenticodeSignature -LiteralPath $artifact.Path).Status
            $artifactSection += "| $($artifact.Name) | ``$($artifact.Relative)`` | $([math]::Round($item.Length / 1MB, 2)) | $($hash.Hash) | $signature |"
        }
        $artifactSection += ""
        $artifactSection += "Artifacts are produced by ``build.ps1``; validation does not rebuild them."
        $artifactSection += ""
    }

    $markdown = @(
        "# Remote Buddy validation",
        "",
        "## Environment",
        "",
        $versionLines,
        "",
        "## Automated checks",
        "",
        $table,
        ""
    ) + $artifactSection + @(
        "## Hardware acceptance",
        "",
        "Hardware checks are recorded separately after running on a paired G9N9N.",
        "",
        "- [ ] Pair the G9N9N through Windows Settings",
        "- [ ] ETW starts without elevation",
        "- [ ] GATT probe binds the ACL handle",
        "- [ ] Up / Down / Left / Right / Ok / Back / Home",
        "- [ ] Menu / volume / mute / play-pause / power / app keys",
        "- [ ] No duplicate Raw Input + ETW action",
        "- [ ] SmartToggle short press and HoldToTalk long press",
        "- [ ] Audio reaches CABLE Input and live gain changes are audible",
        "- [ ] Autostart launches the application after sign-in",
        "- [ ] Tray reconnect after 10+ minutes idle",
        "- [ ] Bluetooth radio off/on and sleep/resume recovery"
    )
    Set-Content -LiteralPath $reportPath -Value $markdown -Encoding utf8
    Write-Host ""
    Write-Host "Report: $reportPath"

    if ($failed.Count -gt 0) {
        $failed | Format-Table -AutoSize
        exit 1
    }
}
finally {
    Pop-Location
}
