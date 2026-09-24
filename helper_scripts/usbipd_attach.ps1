param([switch]$Setup, [switch]$Elevated, [switch]$List, [string]$InstanceId)

$ErrorActionPreference = 'Stop'
$espUsbIds = '^USB\\VID_(303A&PID_[0-9A-F]{4}|10C4&PID_(EA60|EA70)|1A86&PID_(7523|55D4)|0403&PID_(6001|6010|6011))\\'

function Get-UsbipdPath {
    $command = Get-Command usbipd.exe -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }
    $installed = Join-Path $env:ProgramW6432 'usbipd-win\usbipd.exe'
    if (Test-Path $installed) { return $installed }
    return $null
}

function Get-Esp32Candidates {
    $module = Join-Path $env:ProgramW6432 'usbipd-win\Usbipd.Powershell.dll'
    if (Test-Path $module) {
        Import-Module $module -ErrorAction Stop
        return @(Get-UsbipdDevice | Where-Object { $_.IsConnected -and $_.InstanceId -match $espUsbIds })
    }
    return @(Get-CimInstance Win32_PnPEntity | Where-Object {
        $_.Status -eq 'OK' -and $_.PNPDeviceID -match $espUsbIds
    } | ForEach-Object {
        [pscustomobject]@{ InstanceId = $_.PNPDeviceID; Description = $_.Name; IsBound = $false }
    })
}

function Select-Esp32Candidate {
    $devices = @(Get-Esp32Candidates)
    if ($InstanceId) {
        $devices = @($devices | Where-Object { $_.InstanceId -ieq $InstanceId })
    }
    if ($devices.Count -ne 1) {
        $available = (Get-Esp32Candidates | ForEach-Object { "  $($_.Description): $($_.InstanceId)" }) -join "`n"
        throw "Expected one matching connected USB device; found $($devices.Count). Use --list and --instance-id to choose one. Candidates:`n$available"
    }
    return $devices[0]
}

$usbipd = Get-UsbipdPath
if ($List) {
    @(Get-Esp32Candidates) | Format-List Description, InstanceId, BusId, IsBound, IsAttached
    exit 0
}
$selected = Select-Esp32Candidate
$InstanceId = $selected.InstanceId
if ($Elevated) {
    $admin = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not $admin.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Setup requires an administrator PowerShell.' }
    if (-not $usbipd) {
        $winget = Get-Command winget.exe -ErrorAction Stop
        & $winget.Source install --id dorssel.usbipd-win --exact --source winget --silent --accept-source-agreements --accept-package-agreements
        if ($LASTEXITCODE -ne 0) { throw "usbipd-win installation failed (exit $LASTEXITCODE)." }
        $usbipd = Get-UsbipdPath
        if (-not $usbipd) { throw 'usbipd-win installed but executable was not found; restart Windows and retry.' }
    }
    $device = Select-Esp32Candidate
    if (-not $device.IsBound) {
        & $usbipd bind --busid $device.BusId
        if ($LASTEXITCODE -ne 0) { throw "usbipd bind failed for $($device.BusId) (exit $LASTEXITCODE)." }
    }
    Write-Host "Shared ESP32 bridge at $($device.BusId)."
    exit 0
}

if (-not $usbipd -or -not (Test-Path (Join-Path $env:ProgramW6432 'usbipd-win\Usbipd.Powershell.dll'))) {
    $needsSetup = $true
} else {
    $device = $selected
    $needsSetup = -not $device.IsBound
}

if ($needsSetup) {
    Write-Host 'Windows administrator approval is needed to install/bind usbipd-win. Approve the UAC prompt...'
    $arguments = '-NoProfile -ExecutionPolicy Bypass -File "' + $PSCommandPath + '" -Elevated -InstanceId "' + $InstanceId + '"'
    $process = Start-Process -FilePath 'powershell.exe' -Verb RunAs -ArgumentList $arguments -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "Administrator setup failed or was cancelled (exit $($process.ExitCode))." }
    $usbipd = Get-UsbipdPath
    $device = Select-Esp32Candidate
    if (-not $device.IsBound) { throw 'Device still is not shared after setup.' }
}

if ($Setup) { Write-Host 'Windows setup complete; attaching to WSL...' }

if ($device.IsAttached) {
    Write-Host "ESP32 bridge already attached at $($device.BusId)."
} else {
    & $usbipd attach --wsl --busid $device.BusId
    if ($LASTEXITCODE -ne 0) { throw "usbipd attach failed for $($device.BusId) (exit $LASTEXITCODE)." }
    Write-Host "Attached ESP32 bridge at $($device.BusId) to WSL."
}