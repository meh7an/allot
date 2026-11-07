# Tests install.ps1 in one PowerShell: refusals on an unsupported architecture and version, the
# checksum check against a local server with a fake setup, then real installs of the latest
# release into a throwaway USERPROFILE, first as `irm | iex` runs it, then again with switches
# as the scriptblock form runs it. -Offline leaves the real installs out (for a machine with a
# real install).
#   test-install-ps1.ps1 <path to install.ps1> <powershell or pwsh> [-Offline]
param([string]$Script, [string]$Shell, [switch]$Offline)

$ErrorActionPreference = 'Stop'
$Script = (Resolve-Path $Script).Path
$work = Join-Path ([IO.Path]::GetTempPath()) "allot-test-$([guid]::NewGuid())"
New-Item -ItemType Directory -Path $work | Out-Null
$failures = 0

function Check([string]$Message) {
    Write-Host "ok: $Message"
}

function Problem([string]$Message) {
    Write-Host "FAIL: $Message"
    $script:failures++
}

# Runs $Command in a new $Shell with the given environment variables set, and returns its exit
# code and output. The output goes through files, not a pipe: allotd and the tray app, which an
# install starts, inherit the shell's handles and would hold a pipe open. A shell still running
# after $TimeoutSecs is killed and fails the test.
function Invoke-Shell([hashtable]$Environment, [string]$Command, [int]$TimeoutSecs = 600) {
    $saved = @{}
    foreach ($name in $Environment.Keys) {
        $saved[$name] = [Environment]::GetEnvironmentVariable($name)
        [Environment]::SetEnvironmentVariable($name, $Environment[$name])
    }
    $id = [guid]::NewGuid()
    $out = Join-Path $work "$id.out"
    $err = Join-Path $work "$id.err"
    # A file, since Windows PowerShell writes redirected errors as CLIXML for -EncodedCommand.
    $file = Join-Path $work "$id.ps1"
    Set-Content -Path $file -Value $Command
    try {
        $arguments = '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
            '-File', "`"$file`""
        $process = Start-Process -FilePath $Shell -NoNewWindow -PassThru `
            -RedirectStandardOutput $out -RedirectStandardError $err -ArgumentList $arguments
        $null = $process.Handle
        if (-not $process.WaitForExit($TimeoutSecs * 1000)) {
            $process.Kill()
            throw "$Shell was still running after $TimeoutSecs s: $Command"
        }
        $output = "$(Get-Content -Raw $out)$(Get-Content -Raw $err)"
        [pscustomobject]@{ Code = $process.ExitCode; Output = $output }
    } finally {
        foreach ($name in $saved.Keys) {
            [Environment]::SetEnvironmentVariable($name, $saved[$name])
        }
    }
}

# Runs $Command (by default install.ps1 as a file) and checks that it fails with $Want.
# Downloads would go nowhere, so a refusal that came too late shows up as a download error.
function Expect-Refusal([string]$Want, [hashtable]$Environment, [string]$Command = '') {
    if (-not $Command) { $Command = "& '$Script'" }
    $Environment['ALLOT_DOWNLOAD_BASE'] = 'http://127.0.0.1:9'
    $result = Invoke-Shell $Environment $Command
    if ($result.Code -eq 0) {
        Problem "succeeded, expected '$Want'"
    } elseif (-not $result.Output.Contains($Want)) {
        Problem "expected '$Want', got: $($result.Output)"
    } else {
        Check $Want
    }
}

# Installs with `irm | iex`, then again with the scriptblock form and its switches.
function Test-RealInstall {
    $userProfile = Join-Path $work 'home'
    New-Item -ItemType Directory -Path $userProfile | Out-Null
    $environment = @{
        USERPROFILE = $userProfile
        ALLOT_CLAUDE = (Join-Path $userProfile 'no-claude.exe')
    }
    $result = Invoke-Shell $environment "Get-Content -Raw '$Script' | Invoke-Expression"
    Write-Host $result.Output
    if ($result.Code -ne 0 -or $result.Output -notmatch 'Allot (\d+\.\d+\.\d+) is installed') {
        throw "irm | iex did not install Allot (exit $($result.Code))"
    }
    $version = $Matches[1]
    $allot = Join-Path $userProfile '.allot\bin\allot.exe'
    $installed = & $allot --version
    if ($installed -eq "allot $version") { Check "installed $installed" } else {
        Problem "expected allot $version, got $installed"
    }
    $settings = Join-Path $userProfile '.claude\settings.json'
    if ((Test-Path $settings) -and (Get-Content -Raw $settings).Contains('allot') -and
        (Test-Path (Join-Path $userProfile '.claude\allot.md'))) {
        Check 'set up Claude Code in the throwaway USERPROFILE'
    } else {
        Problem 'no hooks or rule in the throwaway USERPROFILE'
    }
    $uninstall = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\' +
        '{7C1E4B2A-3D5F-4E8B-9A61-2F0C5D8E4B17}_is1'
    if (Test-Path $uninstall) { Check 'added the Apps & features entry' } else {
        Problem 'no Apps & features entry'
    }

    $command = "& ([scriptblock]::Create((Get-Content -Raw '$Script'))) " +
        "-Version $version -NoClaude -NoLaunchAtLogon"
    $result = Invoke-Shell $environment $command
    Write-Host $result.Output
    if ($result.Code -eq 0 -and
        $result.Output.Contains("turned off the tray app's Launch at logon")) {
        Check 'the scriptblock form installs again with its switches passed to the setup'
    } else {
        Problem "the second install failed or ignored -NoLaunchAtLogon (exit $($result.Code))"
    }
}

try {
    Write-Host '--- unsupported platforms and versions'
    Expect-Refusal 'Windows on ARM64 is not supported' @{ PROCESSOR_ARCHITECTURE = 'ARM64' }
    Expect-Refusal 'Allot 0.2.0 has no setup this script can run' @{} "& '$Script' -Version 0.2.0"
    Expect-Refusal "'latest' is not a release version" @{ ALLOT_VERSION = 'latest' }
    Expect-Refusal 'Allot 0.1.0 has no setup this script can run' @{ ALLOT_VERSION = '0.1.0' } `
        "Get-Content -Raw '$Script' | Invoke-Expression"

    Write-Host '--- checksums, against a local server'
    $release = Join-Path $work 'release'
    $download = Join-Path $release 'download\v0.3.0'
    New-Item -ItemType Directory -Path $download | Out-Null
    $setup = 'allot-0.3.0-setup.exe'
    Set-Content -Path (Join-Path $download $setup) -Value 'not a setup'
    $port = 47932
    $server = Start-Process python -PassThru -WindowStyle Hidden -ArgumentList `
        "-m http.server $port --bind 127.0.0.1 --directory `"$release`""
    $localBase = "http://127.0.0.1:$port"
    foreach ($attempt in 1..50) {
        try {
            Invoke-WebRequest -Uri "$localBase/" -UseBasicParsing | Out-Null
            break
        } catch {
            Start-Sleep -Milliseconds 100
        }
    }

    $sum = Join-Path $download "$setup.sha256"
    Set-Content -Path $sum -NoNewline -Value "$('0' * 64)  $setup"
    $result = Invoke-Shell @{ ALLOT_DOWNLOAD_BASE = $localBase } "& '$Script' -Version 0.3.0"
    if ($result.Code -eq 0) {
        Problem 'a checksum mismatch was installed'
    } elseif (-not $result.Output.Contains("checksum mismatch for $setup")) {
        Problem "a mismatch failed for another reason: $($result.Output)"
    } elseif ($result.Output.Contains('running the setup')) {
        Problem 'the setup ran after a checksum mismatch'
    } else {
        Check 'a checksum mismatch stops before the setup runs'
    }

    Set-Content -Path $sum -NoNewline -Value "$('0' * 64)  other.exe"
    $result = Invoke-Shell @{ ALLOT_DOWNLOAD_BASE = $localBase } "& '$Script' -Version 0.3.0"
    if ($result.Code -eq 0 -or -not $result.Output.Contains('does not hold a checksum')) {
        Problem "a .sha256 for another file was not refused: $($result.Output)"
    } else {
        Check 'a .sha256 naming another file is refused'
    }
    Stop-Process -Id $server.Id -ErrorAction SilentlyContinue

    if ($Offline) {
        Write-Host '--- skipped the real installs (-Offline)'
    } else {
        Write-Host '--- the latest release, into a throwaway USERPROFILE'
        Test-RealInstall
    }
} finally {
    Remove-Item -Recurse -Force -Path $work -ErrorAction SilentlyContinue
}

if ($failures) {
    Write-Host "$failures check(s) failed"
    exit 1
}
Write-Host 'all checks passed'
