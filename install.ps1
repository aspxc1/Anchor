<#
.SYNOPSIS
    Installs Anchor on Windows 10/11 (x64) by checking prerequisites, building
    the Tauri installer, and launching it.

.PARAMETER InstallerType
    Installer format to build and run: 'nsis' (default, .exe) or 'msi'.

.PARAMETER BuildOnly
    Build the installer without launching it.

.PARAMETER SkipPrereqInstall
    Fail instead of automatically installing missing prerequisites via winget.

.EXAMPLE
    .\install.ps1

.EXAMPLE
    irm https://raw.githubusercontent.com/aspxc1/Anchor/main/install.ps1 | iex
#>
[CmdletBinding()]
param(
    [ValidateSet("nsis", "msi")]
    [string]$InstallerType = "nsis",
    [switch]$BuildOnly,
    [switch]$SkipPrereqInstall
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoUrl = "https://github.com/aspxc1/Anchor.git"

function Write-Step {
    param([string]$Message)
    Write-Host "`n==> $Message" -ForegroundColor Cyan
}

function Write-Ok {
    param([string]$Message)
    Write-Host "    [OK] $Message" -ForegroundColor Green
}

function Write-Warn {
    param([string]$Message)
    Write-Host "    [!] $Message" -ForegroundColor Yellow
}

function Refresh-Path {
    $machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $parts = @()
    if ($machinePath) { $parts += $machinePath }
    if ($userPath) { $parts += $userPath }

    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    if (Test-Path $cargoBin) {
        $parts += $cargoBin
    }

    $env:Path = ($parts -join ";")
}

function Test-CommandExists {
    param([string]$Name)
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Invoke-Native {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [string[]]$Arguments = @()
    )
    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code ${LASTEXITCODE}: $FilePath $($Arguments -join ' ')"
    }
}

function Install-WithWinget {
    param(
        [Parameter(Mandatory = $true)][string]$Id,
        [Parameter(Mandatory = $true)][string]$DisplayName,
        [string]$Override = ""
    )

    if ($SkipPrereqInstall) {
        throw "$DisplayName is required. Install it manually or re-run without -SkipPrereqInstall."
    }

    if (-not (Test-CommandExists "winget")) {
        throw "$DisplayName is missing and winget is not available. Please install $DisplayName manually and re-run this script."
    }

    Write-Warn "$DisplayName not found. Installing via winget..."
    $wingetArgs = @(
        "install",
        "--id", $Id,
        "--exact",
        "--accept-package-agreements",
        "--accept-source-agreements"
    )
    if ($Override) {
        $wingetArgs += @("--override", $Override)
    }

    & winget @wingetArgs
    # 0 = success, -1978335189 (0x8A15002B) = already installed / no applicable update
    if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne -1978335189) {
        throw "Failed to install $DisplayName via winget (exit code $LASTEXITCODE)."
    }

    Refresh-Path
}

function Test-NodeReady {
    if (-not (Test-CommandExists "node") -or -not (Test-CommandExists "npm.cmd")) {
        return $false
    }
    $versionText = (& node -v 2>$null)
    if ($versionText -match "^v(\d+)\.(\d+)\.") {
        $major = [int]$Matches[1]
        $minor = [int]$Matches[2]
        return ($major -ge 22) -or ($major -eq 20 -and $minor -ge 19)
    }
    return $false
}

function Test-VsBuildToolsReady {
    $vswhereCandidates = @()
    if (${env:ProgramFiles(x86)}) {
        $vswhereCandidates += (Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe")
    }
    if ($env:ProgramFiles) {
        $vswhereCandidates += (Join-Path $env:ProgramFiles "Microsoft Visual Studio\Installer\vswhere.exe")
    }

    foreach ($vswhere in $vswhereCandidates) {
        if (Test-Path $vswhere) {
            $installPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
            if ($installPath) {
                return $true
            }
        }
    }
    return $false
}

function Test-WebView2Ready {
    $keys = @(
        "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
        "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
        "HKCU:\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
    )
    foreach ($key in $keys) {
        if (Test-Path $key) {
            $props = Get-ItemProperty -Path $key -ErrorAction SilentlyContinue
            if ($props -and ($props.PSObject.Properties.Name -contains "pv") -and $props.pv -and $props.pv -ne "0.0.0.0") {
                return $true
            }
        }
    }
    return $false
}

function Test-IsAnchorRepo {
    param([string]$Path)
    if (-not $Path) { return $false }
    return (Test-Path (Join-Path $Path "package.json")) -and
           (Test-Path (Join-Path $Path "src-tauri\tauri.conf.json"))
}

# 1. Verify Windows x64
if ($env:OS -ne "Windows_NT" -or -not [Environment]::Is64BitOperatingSystem) {
    throw "Anchor requires Windows 10 or Windows 11 (x64)."
}

Refresh-Path

# 2. Check and install prerequisites
Write-Step "Checking prerequisites"

if (Test-NodeReady) {
    Write-Ok "Node.js $(& node -v)"
} else {
    Install-WithWinget -Id "OpenJS.NodeJS.LTS" -DisplayName "Node.js LTS"
    if (-not (Test-NodeReady)) {
        throw "Node.js was installed, but 'node' is still unavailable in this shell. Open a new PowerShell window and re-run install.ps1."
    }
    Write-Ok "Node.js $(& node -v)"
}

if ((Test-CommandExists "cargo") -and (Test-CommandExists "rustc")) {
    Write-Ok "Rust $(& rustc --version)"
} else {
    Install-WithWinget -Id "Rustlang.Rustup" -DisplayName "Rust (rustup)"
    if ((Test-CommandExists "rustup") -and -not (Test-CommandExists "rustc")) {
        Invoke-Native -FilePath "rustup" -Arguments @("default", "stable-x86_64-pc-windows-msvc")
        Refresh-Path
    }
    if (-not (Test-CommandExists "cargo") -or -not (Test-CommandExists "rustc")) {
        throw "Rust was installed, but 'cargo'/'rustc' are not yet on PATH. Open a new PowerShell window and re-run install.ps1."
    }
    Write-Ok "Rust $(& rustc --version)"
}

if (Test-VsBuildToolsReady) {
    Write-Ok "Visual Studio C++ Build Tools"
} else {
    Install-WithWinget `
        -Id "Microsoft.VisualStudio.2022.BuildTools" `
        -DisplayName "Visual Studio C++ Build Tools" `
        -Override "--quiet --wait --norestart --nocache --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
    if (-not (Test-VsBuildToolsReady)) {
        throw "Visual Studio C++ Build Tools installation did not finish cleanly. Please install the 'Desktop development with C++' workload and re-run install.ps1."
    }
    Write-Ok "Visual Studio C++ Build Tools"
}

if (Test-WebView2Ready) {
    Write-Ok "Microsoft Edge WebView2 Runtime"
} else {
    Install-WithWinget -Id "Microsoft.EdgeWebView2Runtime" -DisplayName "Microsoft Edge WebView2 Runtime"
    Write-Ok "Microsoft Edge WebView2 Runtime"
}

# 3. Locate local repo or clone into a temporary directory
$repoDir = $null
$clonedTemp = $false

$scriptDir = Get-Variable -Name PSScriptRoot -ValueOnly -ErrorAction SilentlyContinue
if ($scriptDir -and (Test-IsAnchorRepo $scriptDir)) {
    $repoDir = $scriptDir
} elseif (Test-IsAnchorRepo (Get-Location).Path) {
    $repoDir = (Get-Location).Path
} else {
    if (-not (Test-CommandExists "git")) {
        Install-WithWinget -Id "Git.Git" -DisplayName "Git"
    }
    if (-not (Test-CommandExists "git")) {
        throw "Git was installed, but 'git' is not yet on PATH. Open a new PowerShell window and re-run install.ps1."
    }

    $repoDir = Join-Path $env:TEMP "Anchor-install"
    if (Test-Path $repoDir) {
        Remove-Item -Path $repoDir -Recurse -Force
    }

    Write-Step "Cloning Anchor into $repoDir"
    Invoke-Native -FilePath "git" -Arguments @("clone", "--depth", "1", $RepoUrl, $repoDir)
    $clonedTemp = $true
}

$originalLocation = Get-Location
try {
    Set-Location $repoDir

    # 4. Install frontend dependencies and build the installer
    Write-Step "Installing npm dependencies"
    Invoke-Native -FilePath "npm.cmd" -Arguments @("ci")

    Write-Step "Building Anchor ($InstallerType installer) - the first Rust build may take a few minutes"
    Invoke-Native -FilePath "npm.cmd" -Arguments @("run", "tauri", "--", "build", "--bundles", $InstallerType)

    # 5. Locate the generated installer
    if ($InstallerType -eq "nsis") {
        $bundleDir = Join-Path $repoDir "src-tauri\target\release\bundle\nsis"
        $pattern = "*-setup.exe"
    } else {
        $bundleDir = Join-Path $repoDir "src-tauri\target\release\bundle\msi"
        $pattern = "*.msi"
    }

    $installer = Get-ChildItem -Path $bundleDir -Filter $pattern -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1

    if (-not $installer) {
        throw "Build succeeded, but no installer matching '$pattern' was found in $bundleDir."
    }

    Write-Ok "Built installer: $($installer.FullName)"

    if ($BuildOnly) {
        Write-Step "Build complete (-BuildOnly specified)"
        Write-Host "Run the installer at: $($installer.FullName)"
        return
    }

    # 6. Close any running Anchor instance and launch the installer
    $running = Get-Process -Name "anchor" -ErrorAction SilentlyContinue
    if ($running) {
        Write-Step "Closing running Anchor instance before installing"
        $running | Stop-Process -Force -ErrorAction SilentlyContinue
        Start-Sleep -Milliseconds 600
    }

    Write-Step "Launching installer"
    if ($InstallerType -eq "nsis") {
        $proc = Start-Process -FilePath $installer.FullName -PassThru -Wait
    } else {
        $proc = Start-Process -FilePath "msiexec.exe" -ArgumentList @("/i", "`"$($installer.FullName)`"") -PassThru -Wait
    }

    if ($proc.ExitCode -ne 0) {
        Write-Warn "Installer exited with code $($proc.ExitCode)."
        return
    }

    Write-Step "Anchor installation finished"
    Write-Host "You can now launch Anchor from the Start Menu."
    Write-Host "Optional: open Settings -> Browser bridge -> Open extension folder to pair Chrome, Edge, or Brave."
}
finally {
    Set-Location $originalLocation
    if ($clonedTemp -and -not $BuildOnly -and (Test-Path $repoDir)) {
        Write-Step "Cleaning up temporary build directory"
        Remove-Item -Path $repoDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
