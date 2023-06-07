#!/usr/bin/env pwsh
# Modified to install Exograph, original license below.
# Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

$ErrorActionPreference = 'Stop'

if ($v) {
  $Version = "v${v}"
}
if ($Args.Length -eq 1) {
  $Version = $Args.Get(0)
}

$ExographInstall = $env:EXOGRAPH_INSTALL
$BinDir = if ($ExographInstall) {
  "${ExographInstall}\bin"
} else {
  "${Home}\.exograph\bin"
}

$ExographZip = "$BinDir\exograph.zip"
$ExoExe = "$BinDir\exo.exe"
$Target = 'x86_64-pc-windows-msvc'

$DownloadUrl = if (!$Version) {
  "https://github.com/exograph/exograph/releases/latest/download/exograph-${Target}.zip"
} else {
  "https://github.com/exograph/exograph/releases/download/${Version}/exograph-${Target}.zip"
}

if (!(Test-Path $BinDir)) {
  New-Item $BinDir -ItemType Directory | Out-Null
}

curl.exe -Lo $ExographZip $DownloadUrl

tar.exe xf $ExographZip -C $BinDir

Remove-Item $ExographZip

$User = [System.EnvironmentVariableTarget]::User
$Path = [System.Environment]::GetEnvironmentVariable('Path', $User)
if (!(";${Path};".ToLower() -like "*;${BinDir};*".ToLower())) {
  [System.Environment]::SetEnvironmentVariable('Path', "${Path};${BinDir}", $User)
  $Env:Path += ";${BinDir}"
}

Write-Output "Exograph was installed successfully to ${ExoExe}"
Write-Output "Run 'exo --help' to get started"
Write-Output "Stuck? File an issue at https://github.com/exograph/exograph"