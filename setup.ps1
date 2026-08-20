[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Install-WingetPackage {
    param(
        [Parameter(Mandatory)]
        [string] $Id,

        [Parameter(Mandatory)]
        [string] $WingetPath
    )

    Write-Host "Installing $Id with Winget..."
    & $WingetPath install --id $Id --exact --source winget --accept-package-agreements --accept-source-agreements --silent --no-upgrade
    if ($LASTEXITCODE -ne 0) {
        throw "Winget failed to install $Id (exit code $LASTEXITCODE)."
    }
}

$winget = Get-Command winget.exe -ErrorAction SilentlyContinue
if ($null -eq $winget) {
    throw "Winget is required. Install App Installer from the Microsoft Store, then run this script again."
}

Install-WingetPackage -Id "Python.Python.3.13" -WingetPath $winget.Source
Install-WingetPackage -Id "Gyan.FFmpeg" -WingetPath $winget.Source

$env:Path = @(
    [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::Machine)
    [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::User)
    $env:Path
) -join ";"

$pythonLauncher = Get-Command py.exe -ErrorAction SilentlyContinue
if ($null -eq $pythonLauncher) {
    throw "Python was installed, but py.exe is not available. Open a new terminal and run this script again."
}

$virtualEnvironment = Join-Path $PSScriptRoot ".venv"
& $pythonLauncher.Source -3.13 -m venv $virtualEnvironment
if ($LASTEXITCODE -ne 0) {
    throw "Failed to create the Python virtual environment (exit code $LASTEXITCODE)."
}

$virtualEnvironmentPython = Join-Path $virtualEnvironment "Scripts\python.exe"
$requirements = Join-Path $PSScriptRoot "bridge\requirements.txt"
& $virtualEnvironmentPython -m pip install -r $requirements
if ($LASTEXITCODE -ne 0) {
    throw "Failed to install Python dependencies (exit code $LASTEXITCODE)."
}

$dotnet = Get-Command dotnet.exe -ErrorAction SilentlyContinue
if ($null -eq $dotnet) {
    throw ".NET 10 SDK is required. Install it, then run this script again."
}

& $dotnet.Source build (Join-Path $PSScriptRoot "FlingPoc.slnx")
if ($LASTEXITCODE -ne 0) {
    throw "Failed to build FlingPoc (exit code $LASTEXITCODE)."
}

Write-Host "Setup complete. Activate .venv before running FlingPoc."
