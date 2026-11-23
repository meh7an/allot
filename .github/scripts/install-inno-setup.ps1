# Installs Inno Setup 7.1.0 for the current user, which the installer script builds with. The
# release's own setup pins the same version; the image only carries an older 6.x. The download is
# checked against the SHA-256 GitHub publishes for the release asset.
#   install-inno-setup.ps1
$ErrorActionPreference = 'Stop'
$temp = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [IO.Path]::GetTempPath() }
$installer = Join-Path $temp 'innosetup-7.1.0-x64.exe'
Invoke-WebRequest -OutFile $installer `
    https://github.com/jrsoftware/issrc/releases/download/is-7_1_0/innosetup-7.1.0-x64.exe
$hash = (Get-FileHash $installer -Algorithm SHA256).Hash.ToLower()
if ($hash -ne '0362a383ed217d4c4239b5933866dd96d3eb2102737da92f80f6057a4b40df2f') {
    throw "the Inno Setup installer has SHA-256 $hash"
}
$process = Start-Process $installer -Wait -PassThru -ArgumentList `
    '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/CURRENTUSER', '/SP-'
if ($process.ExitCode -ne 0) {
    throw "the Inno Setup install exited with $($process.ExitCode)"
}
