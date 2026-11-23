# Tests `allot update` end to end on Windows: an older real release is installed with install.ps1
# into a throwaway USERPROFILE and runs allotd with a lease, this tree's build is packed as a
# setup and served as a release on a loopback port, and the allot.exe built from this tree
# updates the install to it. A wrong checksum comes first and must change nothing. The setup
# also writes the real user's PATH, Apps & features entry and logon task, so this is for CI's
# throwaway runners, not a PC with an Allot install. Run from the workspace root after
# `cargo build --release --locked`, with Inno Setup installed (install-inno-setup.ps1);
# OLD_VERSION names the release to start from.
#   test-update.ps1
$ErrorActionPreference = 'Stop'

$old = if ($env:OLD_VERSION) { $env:OLD_VERSION } else { '0.6.0' }
$root = (Get-Location).Path
$tree = Join-Path $root 'target\release'
$allot = Join-Path $tree 'allot.exe'
$work = Join-Path ([IO.Path]::GetTempPath()) "allot-update-$([guid]::NewGuid())"
$userHome = Join-Path $work 'home'
$serverPort = 47933
$daemonPort = 47934
$api = "http://127.0.0.1:$daemonPort"
$installed = Join-Path $userHome '.allot\bin\allot.exe'
$failures = 0
$started = @()

function Check([string]$Message) {
    Write-Host "ok: $Message"
}

function Problem([string]$Message) {
    Write-Host "FAIL: $Message"
    $script:failures++
}

function Stop-Started {
    foreach ($process in $script:started) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    }
}

# Runs $Exe with $Arguments and waits for it, not for what it starts: allotd and the tray app
# inherit a pipe and would hold it open. The output goes through files.
function Invoke-Program([string]$Exe, [string[]]$Arguments = @(), [int]$TimeoutSecs = 900) {
    $id = [guid]::NewGuid()
    $out = Join-Path $work "$id.out"
    $err = Join-Path $work "$id.err"
    $parameters = @{
        FilePath = $Exe; NoNewWindow = $true; PassThru = $true
        RedirectStandardOutput = $out; RedirectStandardError = $err
    }
    if ($Arguments) { $parameters.ArgumentList = $Arguments }
    $process = Start-Process @parameters
    $null = $process.Handle
    if (-not $process.WaitForExit($TimeoutSecs * 1000)) {
        $process.Kill()
        throw "$Exe was still running after $TimeoutSecs s"
    }
    $output = "$(Get-Content -Raw $out)$(Get-Content -Raw $err)"
    [pscustomobject]@{ Code = $process.ExitCode; Output = $output }
}

function Get-Health { Invoke-RestMethod -Uri "$api/health" }

function Get-OutcomeState {
    $file = Join-Path $userHome '.allot\update-install.json'
    if (Test-Path $file) { (Get-Content -Raw $file | ConvertFrom-Json).state }
}

try {
    New-Item -ItemType Directory -Path $userHome | Out-Null
    foreach ($exe in 'allot.exe', 'allotd.exe', 'allot-mcp.exe') {
        if (-not (Test-Path (Join-Path $tree $exe))) {
            throw "$tree\$exe is missing: run cargo build --release --locked first"
        }
    }
    $version = ((& $allot --version) -split ' ')[1]
    if ($version -eq $old) { throw "this tree is $version, the release to start from" }

    $iscc = Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 7\ISCC.exe'

    # Every program this script runs works in the throwaway profile. The setup finds its log
    # folder through the shell, which expands %USERPROFILE%, so LOCALAPPDATA moves with it for
    # allot.exe to find the same install.log. ALLOT_CLAUDE points nowhere, so nothing is
    # registered with a real Claude Code.
    $env:USERPROFILE = $userHome
    $env:LOCALAPPDATA = Join-Path $userHome 'AppData\Local'
    $env:ALLOT_CLAUDE = Join-Path $userHome 'no-claude.exe'
    $env:ALLOT_UPDATE_BASE = "http://127.0.0.1:$serverPort/releases"

    Write-Host "--- this tree as release $version"
    $download = Join-Path $work "release\download\v$version"
    $payload = Join-Path $work 'payload'
    New-Item -ItemType Directory -Path $download, $payload | Out-Null
    foreach ($exe in 'allot.exe', 'allotd.exe', 'allot-mcp.exe') {
        Copy-Item (Join-Path $tree $exe) $payload
    }
    & $iscc /Qp "/DAppVersion=$version" "/DPayload=$payload" "/O$download" `
        (Join-Path $root 'installer\allot.iss')
    if ($LASTEXITCODE -ne 0) { throw "ISCC exited with $LASTEXITCODE" }
    $setup = "allot-$version-setup.exe"
    $sumFile = Join-Path $download "$setup.sha256"
    $hash = (Get-FileHash (Join-Path $download $setup) -Algorithm SHA256).Hash.ToLower()
    $goodSum = "$hash  $setup"
    Set-Content -Path $sumFile -NoNewline -Value $goodSum

    $serverLog = Join-Path $work 'server.log'
    $server = Start-Process python -PassThru -NoNewWindow `
        -RedirectStandardError $serverLog -RedirectStandardOutput (Join-Path $work 'server.out') `
        -ArgumentList "`"$root\.github\scripts\serve-release.py`"", "`"$work\release`"",
            $serverPort, "v$version"
    $started += $server
    foreach ($attempt in 1..50) {
        try {
            $status = (Invoke-WebRequest -Uri "http://127.0.0.1:$serverPort/releases/latest" `
                -Method Head -UseBasicParsing -MaximumRedirection 0 `
                -SkipHttpErrorCheck).StatusCode
            if ($status -eq 302) { break }
            Start-Sleep -Milliseconds 100
        } catch {
            Start-Sleep -Milliseconds 100
        }
    }

    Write-Host "--- release $old, installed with install.ps1 into a throwaway USERPROFILE"
    $app = Join-Path $userHome '.allot'
    New-Item -ItemType Directory -Path $app | Out-Null
    # A port of its own, and a heartbeat timeout longer than this script, since nothing here
    # sends heartbeats.
    Set-Content -Path (Join-Path $app 'config.toml') -Value @"
[daemon]
port = $daemonPort

[leases]
heartbeat_ttl_secs = 900
"@
    $result = Invoke-Program (Join-Path $PSHOME 'pwsh.exe') @(
        '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File',
        (Join-Path $root 'install.ps1'), '-Version', $old, '-NoClaude', '-NoLaunchAtLogon')
    Write-Host $result.Output
    if ($result.Code -ne 0) { throw "install.ps1 exited with $($result.Code)" }
    $found = & $installed --version
    if ($found -ne "allot $old") { throw "expected allot $old, got $found" }
    $result = Invoke-Program $installed @('daemon', 'start')
    if ($result.Code -ne 0) { throw "allot daemon start failed: $($result.Output)" }
    $oldHealth = Get-Health
    if ($oldHealth.version -ne $old) { throw "allotd is $($oldHealth.version), not $old" }
    Check "allotd $old runs, pid $($oldHealth.pid)"

    Write-Host '--- a lease, held by a process that outlives the update'
    # The holder is the PowerShell that ran the probe and then sleeps: the probe's second
    # ancestry line is its pid and start time, the way the daemon identifies a session's process.
    $report = Join-Path $work 'holder.txt'
    $holder = Start-Process (Join-Path $PSHOME 'pwsh.exe') -PassThru -WindowStyle Hidden `
        -ArgumentList '-NoProfile', '-NonInteractive', '-Command',
            "& '$allot' probe --out '$report'; Start-Sleep 3600"
    $started += $holder
    foreach ($attempt in 1..100) {
        if ((Test-Path $report) -and (Get-Item $report).Length -gt 0) { break }
        Start-Sleep -Milliseconds 100
    }
    $lines = Get-Content $report
    $header = 0..($lines.Count - 1) | Where-Object { $lines[$_] -like '-- ancestry*' } |
        Select-Object -First 1
    if ($null -eq $header) { throw "the probe report has no ancestry: $lines" }
    $parent = $lines[$header + 2].Trim() -split '\s+'
    if ([int]$parent[0] -ne $holder.Id) {
        throw "the probe found holder $($parent[0]), not $($holder.Id): $lines"
    }
    $body = @{
        holder = @{ session = 'e2e-update'; pid = [int]$parent[0]; pid_start_time = [uint64]$parent[2] }
        profile = @{ class = 'light' }
        kind = 'normal'
        label = 'e2e update'
        wait_timeout_secs = 10
    } | ConvertTo-Json -Depth 5
    $token = (Get-Content -Raw (Join-Path $app 'token')).Trim()
    $response = Invoke-RestMethod -Uri "$api/leases" -Method Post -ContentType 'application/json' `
        -Headers @{ Authorization = "Bearer $token" } -Body $body
    $lease = $response.lease.id
    if ($response.lease.state -ne 'active') { throw "the lease is not active: $response" }
    Check "lease $lease is active"

    Write-Host '--- a wrong checksum changes nothing'
    Set-Content -Path $sumFile -NoNewline -Value "$('0' * 64)  $setup"
    $result = Invoke-Program $allot @('update')
    if ($result.Code -eq 0) {
        Problem "an update with a wrong checksum succeeded: $($result.Output)"
    } elseif (-not $result.Output.Contains('does not match its checksum')) {
        Problem "a wrong checksum failed for another reason: $($result.Output)"
    } else {
        Check 'a wrong checksum is refused'
    }
    if ((& $installed --version) -eq "allot $old" -and (Get-Health).pid -eq $oldHealth.pid) {
        Check "allot and allotd are still $old"
    } else {
        Problem 'the failed update changed the install'
    }
    if ((Get-OutcomeState) -ne 'failed') { Problem 'the failed update left no failed record' }
    Set-Content -Path $sumFile -NoNewline -Value $goodSum

    Write-Host "--- allot update to $version"
    $result = Invoke-Program $allot @('update')
    Write-Host $result.Output
    if ($result.Code -ne 0) { throw "allot update exited with $($result.Code)" }
    $now = & $installed --version
    if ($now -eq "allot $version") { Check "the install is $now" } else {
        Problem "expected allot $version, got $now"
    }
    $health = Get-Health
    if ($health.version -eq $version -and $health.pid -ne $oldHealth.pid) {
        Check "allotd restarted: $version, pid $($health.pid) after pid $($oldHealth.pid)"
    } else {
        Problem "allotd did not restart on ${version}: $($health | ConvertTo-Json -Compress)"
    }
    $leases = (Invoke-Program $installed @('leases', '--json')).Output | ConvertFrom-Json
    if (@($leases | Where-Object { $_.id -eq $lease -and $_.state -eq 'active' }).Count -eq 1) {
        Check "lease $lease is still active after the restart"
    } else {
        Problem "lease $lease was not restored: $($leases | ConvertTo-Json -Compress)"
    }
    if ((Get-OutcomeState) -ne 'succeeded') { Problem 'the update left no succeeded record' }
    if (-not $result.Output.Contains('restarted allotd on the new build')) {
        Problem 'the update did not say it restarted allotd'
    }
    $uninstall = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\' +
        '{7C1E4B2A-3D5F-4E8B-9A61-2F0C5D8E4B17}_is1'
    $displayed = (Get-ItemProperty $uninstall).DisplayVersion
    if ($displayed -eq $version) { Check "Apps & features says $displayed" } else {
        Problem "Apps & features says $displayed, not $version"
    }
    if (Test-Path (Join-Path $userHome ".allot\updates\$version")) {
        Problem 'the download folder was left behind'
    }
} catch {
    Write-Host "error: $_"
    $failures++
} finally {
    Stop-Started
    if ($failures -and (Test-Path (Join-Path $work 'server.log'))) {
        Write-Host '--- the release server log'
        Get-Content (Join-Path $work 'server.log')
    }
    Remove-Item -Recurse -Force -Path $work -ErrorAction SilentlyContinue
}

if ($failures) {
    Write-Host "$failures check(s) failed"
    exit 1
}
Write-Host 'all checks passed'
