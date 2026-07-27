$ErrorActionPreference = "Stop"

$target = "x86_64-pc-windows-msvc"
$destination = if ($env:WTW_INSTALL_DIR) { $env:WTW_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\wtw\bin" }
$archive = Join-Path $env:TEMP "wtw-$target.zip"
$extract = Join-Path $env:TEMP "wtw-$target"

New-Item -ItemType Directory -Force -Path $destination | Out-Null
Invoke-WebRequest "https://github.com/lucasrgt/why-this-way/releases/latest/download/wtw-$target.zip" -OutFile $archive
Remove-Item -Recurse -Force $extract -ErrorAction SilentlyContinue
Expand-Archive $archive $extract -Force
$binary = Get-ChildItem $extract -Recurse -Filter wtw.exe | Select-Object -First 1
Copy-Item $binary.FullName (Join-Path $destination "wtw.exe") -Force

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (($userPath -split ";") -notcontains $destination) {
    [Environment]::SetEnvironmentVariable("Path", (($userPath.TrimEnd(";") + ";" + $destination).TrimStart(";")), "User")
}

Remove-Item -Recurse -Force $extract
Remove-Item -Force $archive
Write-Output "Installed wtw to $destination\wtw.exe"
