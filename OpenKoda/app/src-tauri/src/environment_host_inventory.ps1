# Read-only, launched as the desktop user. stdin holds only the proposed disk location.
$ErrorActionPreference = 'Stop'
[Console]::InputEncoding = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$destination = [Console]::In.ReadToEnd().TrimEnd([char[]]"`r`n")
$facts = [ordered]@{
    windowsBuild = $null; architecture = $null; hypervisor = $null
    virtualizationFirmware = $null; virtualMachinePlatform = $null
    wslClient = $null; freeBytes = $null; boot = $null
}
try {
    $os = Get-CimInstance -ClassName Win32_OperatingSystem
    $facts.windowsBuild = [int]$os.BuildNumber
    $facts.boot = $os.LastBootUpTime.ToUniversalTime().ToString('o')
} catch {}
try {
    $cpu = Get-CimInstance -ClassName Win32_Processor | Select-Object -First 1
    $facts.architecture = switch ($cpu.Architecture) { 9 { 'x64' }; 12 { 'arm64' }; 0 { 'x86' }; default { $null } }
    $facts.virtualizationFirmware = $cpu.VirtualizationFirmwareEnabled
} catch {}
try { $facts.hypervisor = (Get-CimInstance -ClassName Win32_ComputerSystem).HypervisorPresent } catch {}
try {
    $feature = Get-CimInstance -ClassName Win32_OptionalFeature -Filter "Name='VirtualMachinePlatform'"
    if ($feature.InstallState -in 1,2,3) { $facts.virtualMachinePlatform = $feature.InstallState -eq 1 }
} catch {}
try { $facts.wslClient = [IO.File]::Exists([IO.Path]::Combine([Environment]::SystemDirectory,'wsl.exe')) } catch {}
try { $facts.freeBytes = [IO.DriveInfo]::new([IO.Path]::GetPathRoot($destination)).AvailableFreeSpace } catch {}
$facts | ConvertTo-Json -Compress
