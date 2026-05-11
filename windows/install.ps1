<#
.SYNOPSIS
A simple installer for Soromantic on Windows.

.DESCRIPTION
This script will:
1. Install Chocolatey (if missing).
2. Install mpv and ffmpeg dependencies.
3. Install Soromantic to %LOCALAPPDATA%\Soromantic.
4. Create a Start Menu shortcut.
#>

$ErrorActionPreference = "Stop"

function Check-Admin {
    $currentPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    if (-not $currentPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        Write-Host "Please run this script as Administrator (Right-click > Run with PowerShell)" -ForegroundColor Red
        exit 1
    }
}

Check-Admin

Write-Host "=== Soromantic Installer ===" -ForegroundColor Cyan

# 1. Install Chocolatey
if (-not (Get-Command choco -ErrorAction SilentlyContinue)) {
    Write-Host "Installing Chocolatey..." -ForegroundColor Yellow
    Set-ExecutionPolicy Bypass -Scope Process -Force
    [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
    iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
    
    # Reload env vars
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
} else {
    Write-Host "Chocolatey already installed." -ForegroundColor Green
}

# 2. Install Dependencies
Write-Host "Checking dependencies..." -ForegroundColor Yellow
choco install mpv ffmpeg -y --no-progress

# 3. Locate or Download Exe
$exeName = "soromantic.exe"
$localExe = Join-Path $PSScriptRoot $exeName
$devExe = Join-Path $PSScriptRoot "..\target\release\soromantic.exe"
$sourceExe = $null

if (Test-Path $localExe) {
    Write-Host "Found local executable." -ForegroundColor Cyan
    $sourceExe = $localExe
} elseif (Test-Path $devExe) {
    Write-Host "Found dev build." -ForegroundColor Cyan
    $sourceExe = $devExe
} else {
    Write-Host "Downloading latest release from GitHub..." -ForegroundColor Cyan
    $repo = "giannifer7/soromantic"
    $url = "https://github.com/giannifer7/soromantic/releases/latest/download/soromantic-windows-x86_64.zip"
    $zipPath = "$env:TEMP\soromantic_install.zip"
    
    try {
        Invoke-WebRequest -Uri $url -OutFile $zipPath
        Write-Host "Extracting..." -ForegroundColor Cyan
        Expand-Archive -Path $zipPath -DestinationPath "$env:TEMP\soromantic_install" -Force
        $sourceExe = "$env:TEMP\soromantic_install\$exeName"
        
        if (-not (Test-Path $sourceExe)) {
            throw "Extracted zip did not contain $exeName"
        }
    } catch {
        Write-Host "Failed to download/extract release: $_" -ForegroundColor Red
        exit 1
    }
}

# 4. Install App
$installDir = "$env:LOCALAPPDATA\Soromantic"
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir | Out-Null
}

Write-Host "Installing to $installDir..." -ForegroundColor Cyan
Copy-Item $sourceExe -Destination $installDir -Force

# 5. Create Shortcut
$wshShell = New-Object -ComObject WScript.Shell
$shortcutPath = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Soromantic.lnk"
$shortcut = $wshShell.CreateShortcut($shortcutPath)
$shortcut.TargetPath = "$installDir\$exeName"
$shortcut.WorkingDirectory = $installDir
$shortcut.Save()

Write-Host "Success! Soromantic has been installed." -ForegroundColor Green
Write-Host "You can find it in your Start Menu."
Start-Sleep -Seconds 3
